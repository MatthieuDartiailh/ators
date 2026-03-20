/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
/// Structures used to manage type validation.
use super::Validator;
use crate::annotations::{build_validator_from_annotation, get_type_tools};
use crate::get_type_mutability_map;
use crate::utils::{Mutability, err_with_cause};
use pyo3::Borrowed;
use pyo3::sync::critical_section::with_critical_section;
use pyo3::types::PyStringMethods;
use pyo3::{
    Bound, FromPyObject, IntoPyObject, Py, PyAny, PyErr, PyResult, Python,
    ffi::{
        PyBool_Check, PyBytes_Check, PyComplex_Check, PyFloat_Check, PyLong_Check, PyUnicode_Check,
    },
    pyclass, pymethods,
    sync::OnceLockExt,
    types::{
        PyAnyMethods, PyDict, PyDictMethods, PyFrozenSetMethods, PyList, PyListMethods, PySet,
        PySetMethods, PyString, PyTuple, PyTupleMethods, PyType, PyTypeMethods,
    },
};
use std::{
    convert::Infallible,
    ops::{Deref, DerefMut},
    sync::OnceLock,
};

/// A newtype wrapper around `Box<Validator>` that implements PyO3 conversion traits.
/// This allows using heap-allocated validators in TypeValidator variants without
/// requiring GIL-bound storage (Py<Validator>).
/// The risk of creating reference cycles exist but is low and since validators
///  exists only on types that are expected to be long-lived, it is unlikely to
/// create any real world issues.
#[derive(Debug, Clone)]
pub struct BoxedValidator(pub Box<Validator>);

impl Deref for BoxedValidator {
    type Target = Validator;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BoxedValidator {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Validator> for BoxedValidator {
    fn from(v: Validator) -> Self {
        BoxedValidator(Box::new(v))
    }
}

impl FromPyObject<'_, '_> for BoxedValidator {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        let validator: Validator = ob.extract()?;
        Ok(BoxedValidator(Box::new(validator)))
    }
}

impl<'py> IntoPyObject<'py> for BoxedValidator {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = pyo3::PyErr;
    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        // Convert the inner Validator to a Python object
        Py::new(py, *self.0).map(|p| p.into_bound(py).into_any())
    }
}

impl<'py> IntoPyObject<'py> for &BoxedValidator {
    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = pyo3::PyErr;
    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        // Clone and convert the inner Validator to a Python object
        Py::new(py, (*self.0).clone()).map(|p| p.into_bound(py).into_any())
    }
}

#[derive(Debug)]
/// Struct storing a tuple of types for the TypeValidator::Instance variant
pub(crate) struct TypesTuple(Py<PyTuple>);

impl TypesTuple {
    /// Coerce the value to the first type in the tuple
    pub fn coerce<'py>(&self, value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let py = value.py();
        let type_ = self.0.bind(py).get_item(0)?;
        type_.call1((value,))
    }

    /// Iterate over the types in the tuple
    pub fn iter<'py>(&self, py: Python<'py>) -> impl Iterator<Item = Bound<'py, PyType>> {
        self.0
            .bind(py)
            .iter()
            .map(|o| o.cast_into::<PyType>().expect("Known tuple of types"))
    }
}

impl FromPyObject<'_, '_> for TypesTuple {
    type Error = PyErr;

    fn extract(ob: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        let py = ob.py();
        if let Ok(ty) = ob.cast::<PyType>() {
            Ok(TypesTuple(PyTuple::new(py, [ty])?.into()))
        } else if let Ok(s) = ob.cast::<PyTuple>()
            && s.len() > 0
            && s.iter().all(|item| item.is_instance_of::<PyType>())
        {
            Ok(TypesTuple(s.to_owned().unbind()))
        } else {
            Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Expected a 'type' or 'tuple[type, ...]' for a TypeValidator.Instance, got {}",
                ob.get_type().name()?
            )))
        }
    }
}

impl<'py> IntoPyObject<'py> for &TypesTuple {
    type Target = PyTuple;
    type Output = Bound<'py, PyTuple>;
    type Error = Infallible;
    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        Ok(self.0.clone_ref(py).into_bound(py))
    }
}

/// Validator struct used to resolve forward references in TypeValidator::ForwardValidator
#[pyclass(module = "ators._ators", frozen, from_py_object)]
#[derive(Debug)]

pub struct LateResolvedValidator {
    validator_cell: OnceLock<PyResult<Py<TypeValidator>>>,
    forward_ref: Py<PyAny>,
    ctx_provider: Option<Py<PyAny>>,
    type_containers: i64,
    name: Py<PyString>,
    owner: Option<Py<PyAny>>,
}

#[pymethods]
impl LateResolvedValidator {
    #[new]
    pub fn new<'py>(
        forward_ref: &Bound<'py, PyAny>,
        ctx_provider: Option<&Bound<'py, PyAny>>,
        type_containers: i64,
        name: &Bound<'py, PyString>,
    ) -> Self {
        Self {
            validator_cell: OnceLock::new(),
            forward_ref: forward_ref.clone().unbind(),
            ctx_provider: ctx_provider.map(|cp| cp.clone().unbind()),
            type_containers,
            name: name.clone().unbind(),
            owner: None,
        }
    }

    /// Get the validator by resolving the forward reference
    pub fn get_validator<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, TypeValidator>> {
        let validator = self.validator_cell.get_or_init_py_attached(py, || {
            let typing = py.import("typing")?;
            let evaluate_forward_ref = typing.getattr("evaluate_forward_ref")?;
            let forward_ref = self.forward_ref.bind(py);
            let resolved;

            let kwargs = PyDict::new(py);
            if let Some(cp) = &self.ctx_provider {
                let ctx_provider = cp.bind(py);
                kwargs.set_item("locals", ctx_provider.call0()?)?;
            }
            if let Some(owner) = &self.owner {
                let owner_bound = owner.bind(py);
                kwargs.set_item("owner", owner_bound)?;
            }

            if !kwargs.is_empty() {
                resolved = evaluate_forward_ref.call((forward_ref,), Some(&kwargs));
            } else {
                resolved = evaluate_forward_ref.call1((forward_ref,));
            }

            Py::new(
                py,
                build_validator_from_annotation(
                    self.name.bind(py),
                    &resolved.map_err(|err| {
                        err_with_cause(
                            py,
                            pyo3::PyErr::from_type(
                                err.get_type(py),
                                format!(
                                    "Failed to resolve forward reference for {}: {}\n{}",
                                    self.name.bind(py).to_str().unwrap_or("<invalid name>"),
                                    forward_ref,
                                    err
                                ),
                            ),
                            err,
                        )
                    })?,
                    self.type_containers,
                    &get_type_tools(py)?,
                    None,
                )?
                .0
                .type_validator,
            )
        });
        match validator {
            Ok(tv) => Ok(tv.bind(py).clone()),
            Err(e) => Err(e.clone_ref(py)),
        }
    }

    // NOTE this cannot be done right after class creation since the class has
    // not yet been stored in the module dict and thus cannot be resolved by
    // the forward reference, but
    /// Determine if the type is mutable by resolving the forward reference and
    /// checking the mutability of the resolved type
    pub fn is_type_mutable<'py>(&self, py: Python<'py>) -> Mutability {
        match self.get_validator(py) {
            Ok(validator) => validator.get().is_type_mutable(py),
            Err(_) => Mutability::Undecidable,
        }
    }
}

impl LateResolvedValidator {
    pub(crate) fn with_owner(&self, py: Python<'_>, owner: &Bound<'_, PyAny>) -> Self {
        Self {
            validator_cell: OnceLock::new(),
            forward_ref: self.forward_ref.clone_ref(py),
            ctx_provider: self.ctx_provider.as_ref().map(|cp| cp.clone_ref(py)),
            type_containers: self.type_containers,
            name: self.name.clone_ref(py),
            owner: Some(owner.clone().unbind()),
        }
    }
}

impl Clone for LateResolvedValidator {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            validator_cell: OnceLock::new(),
            forward_ref: self.forward_ref.clone_ref(py),
            ctx_provider: self.ctx_provider.as_ref().map(|cp| cp.clone_ref(py)),
            type_containers: self.type_containers,
            name: self.name.clone_ref(py),
            owner: self.owner.as_ref().map(|o| o.clone_ref(py)),
        })
    }
}

/// Type validation struct managing type validation
#[pyclass(module = "ators._ators", frozen, from_py_object)]
#[derive(Debug)]
pub enum TypeValidator {
    #[pyo3(constructor = ())]
    Any {},
    #[pyo3(constructor = ())]
    None {},
    #[pyo3(constructor = ())]
    Bool {},
    #[pyo3(constructor = ())]
    Int {},
    #[pyo3(constructor = ())]
    Float {},
    #[pyo3(constructor = ())]
    Complex {},
    #[pyo3(constructor = ())]
    Str {},
    #[pyo3(constructor = ())]
    Bytes {},
    #[pyo3(constructor = (items))]
    Tuple { items: Vec<Validator> },
    #[pyo3(constructor = (item))]
    VarTuple { item: Option<BoxedValidator> },
    #[pyo3(constructor = (type_))]
    Typed { type_: Py<PyType> },
    #[pyo3(constructor = (types))]
    // TypesTuple is build from a Python object and we do not need to expose
    // it directly since it is not needed to build an Instance variant from the
    // Python side.
    #[allow(private_interfaces)]
    Instance { types: TypesTuple },
    #[pyo3(constructor = (members))]
    Union { members: Vec<Validator> },
    #[pyo3(constructor = (type_, attributes))]
    GenericAttributes {
        type_: Py<PyType>,
        attributes: Vec<(String, Validator)>,
    },
    ForwardValidator {
        late_validator: LateResolvedValidator,
    },
    #[pyo3(constructor = (item))]
    FrozenSet { item: Option<BoxedValidator> },
    #[pyo3(constructor = (item))]
    Set { item: Option<BoxedValidator> },
    #[pyo3(constructor = (item))]
    List { item: Option<BoxedValidator> },
    #[pyo3(constructor = (items))]
    Dict {
        items: Option<(BoxedValidator, BoxedValidator)>,
    },
    // Sequence,
    // List,
    // Mapping,
    // DefaultDict,
    // OrderedDict,
    // Callable,
}

macro_rules! validation_error {
    ($type:expr, $member:expr, $object:expr, $value:expr) => {
        if let Some(m) = $member
            && let Some(o) = $object
        {
            Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "The member {} from {} expects a {}, got {} ({})",
                m,
                o.repr()?,
                $type,
                $value.repr()?,
                $value.get_type().name()?
            )))
        } else {
            Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Expected a {}, got {} ({})",
                $type,
                $value.repr()?,
                $value.get_type().name()?
            )))
        }
    };
}

impl TypeValidator {
    pub(crate) fn with_owner(&self, py: Python<'_>, owner: &Bound<'_, PyAny>) -> Self {
        match self {
            Self::Tuple { items } => Self::Tuple {
                items: items.iter().map(|v| v.with_owner(py, owner)).collect(),
            },
            Self::VarTuple { item } => Self::VarTuple {
                item: item
                    .as_ref()
                    .map(|v| BoxedValidator::from(v.with_owner(py, owner))),
            },
            Self::Union { members } => Self::Union {
                members: members.iter().map(|v| v.with_owner(py, owner)).collect(),
            },
            Self::GenericAttributes { type_, attributes } => Self::GenericAttributes {
                type_: type_.clone_ref(py),
                attributes: attributes
                    .iter()
                    .map(|(n, v)| (n.clone(), v.with_owner(py, owner)))
                    .collect(),
            },
            Self::ForwardValidator { late_validator } => Self::ForwardValidator {
                late_validator: late_validator.with_owner(py, owner),
            },
            Self::FrozenSet { item } => Self::FrozenSet {
                item: item
                    .as_ref()
                    .map(|v| BoxedValidator::from(v.with_owner(py, owner))),
            },
            Self::Set { item } => Self::Set {
                item: item
                    .as_ref()
                    .map(|v| BoxedValidator::from(v.with_owner(py, owner))),
            },
            Self::List { item } => Self::List {
                item: item
                    .as_ref()
                    .map(|v| BoxedValidator::from(v.with_owner(py, owner))),
            },
            Self::Dict { items } => Self::Dict {
                items: items.as_ref().map(|(k, v)| {
                    (
                        BoxedValidator::from(k.with_owner(py, owner)),
                        BoxedValidator::from(v.with_owner(py, owner)),
                    )
                }),
            },
            _ => self.clone(),
        }
    }

    /// Validate the type of the value, for container a new container may be
    /// returned (e.g. a new tuple with validated items), but the value itself
    /// is not coerced (e.g. a str is not converted to int even if the type
    /// validator is Int)
    pub fn validate_type<'py>(
        &self,
        name: Option<&str>,
        object: Option<&Bound<'py, crate::core::AtorsBase>>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Any {} => Ok(value.clone()),
            Self::None {} => {
                if value.is_none() {
                    Ok(value.clone())
                } else {
                    validation_error!("None", name, object, value)
                }
            }
            Self::Bool {} => {
                if unsafe { PyBool_Check(value.as_ptr()) } != 0 {
                    Ok(value.clone())
                } else {
                    validation_error!("bool", name, object, value)
                }
            }
            Self::Int {} => {
                if unsafe { PyLong_Check(value.as_ptr()) } != 0 {
                    Ok(value.clone())
                } else {
                    validation_error!("int", name, object, value)
                }
            }
            Self::Float {} => {
                if unsafe { PyFloat_Check(value.as_ptr()) } != 0 {
                    Ok(value.clone())
                } else {
                    validation_error!("float", name, object, value)
                }
            }
            Self::Complex {} => {
                if unsafe { PyComplex_Check(value.as_ptr()) } != 0 {
                    Ok(value.clone())
                } else {
                    validation_error!("complex", name, object, value)
                }
            }
            Self::Str {} => {
                if unsafe { PyUnicode_Check(value.as_ptr()) } != 0 {
                    Ok(value.clone())
                } else {
                    validation_error!("str", name, object, value)
                }
            }
            Self::Bytes {} => {
                if unsafe { PyBytes_Check(value.as_ptr()) } != 0 {
                    Ok(value.clone())
                } else {
                    validation_error!("bytes", name, object, value)
                }
            }
            Self::Tuple { items } => {
                if let Ok(tuple) = value.cast_exact::<pyo3::types::PyTuple>() {
                    let t_length = tuple.len();
                    if t_length != items.len() {
                        return {
                            if let Some(m) = name
                                && let Some(o) = object
                            {
                                Err(pyo3::exceptions::PyTypeError::new_err(format!(
                                    "The member {} from {} expects a tuple of length {}, got a tuple of length {}",
                                    m,
                                    o.repr()?,
                                    items.len(),
                                    t_length,
                                )))
                            } else {
                                Err(pyo3::exceptions::PyTypeError::new_err(format!(
                                    "Expected a tuple of length {}, got a tuple of length {}",
                                    items.len(),
                                    t_length,
                                )))
                            }
                        };
                    }
                    let mut validated_items: Option<Vec<Bound<'_, PyAny>>> = None;
                    for (index, (item, validator)) in tuple.iter().zip(items).enumerate() {
                        // FIXME the loop body logic could be extracted into a helper function
                        match validator.validate(name, object, &item) {
                            Ok(v) => {
                                if !v.is(item) {
                                    match &mut validated_items {
                                        Some(vec) => vec.push(v),
                                        None => {
                                            let mut vec = Vec::with_capacity(t_length);
                                            for i in 0..index {
                                                vec.push(
                                                    tuple.get_item(i).expect(
                                                        "All indexes are known to be valid.",
                                                    ),
                                                );
                                            }
                                            vec.push(v);
                                            validated_items = Some(vec);
                                        }
                                    }
                                }
                            }
                            Err(cause) => {
                                if let Some(m) = name
                                    && let Some(o) = object
                                {
                                    return Err(crate::utils::err_with_cause(
                                        value.py(),
                                        pyo3::exceptions::PyTypeError::new_err(format!(
                                            "Failed to validate item {} for the member {} of {}.",
                                            index,
                                            m,
                                            o.repr()?
                                        )),
                                        cause,
                                    ));
                                } else {
                                    return Err(crate::utils::err_with_cause(
                                        value.py(),
                                        pyo3::exceptions::PyTypeError::new_err(format!(
                                            "Failed to validate item {index}.",
                                        )),
                                        cause,
                                    ));
                                }
                            }
                        }
                    }
                    Ok(if let Some(vi) = validated_items {
                        pyo3::types::PyTuple::new(value.py(), vi)?.into_any()
                    } else {
                        value.clone()
                    })
                } else {
                    validation_error!("tuple", name, object, value)
                }
            }
            Self::VarTuple { item: Some(item) } => {
                if let Ok(tuple) = value.cast_exact::<pyo3::types::PyTuple>() {
                    let mut validated_items: Option<Vec<Bound<'_, PyAny>>> = None;
                    for (index, titem) in tuple.iter().enumerate() {
                        match item.validate(name, object, &titem) {
                            Ok(v) => {
                                if !v.is(&titem) {
                                    match &mut validated_items {
                                        Some(vec) => vec.push(v),
                                        None => {
                                            let mut vec = Vec::with_capacity(tuple.len());
                                            for i in 0..index {
                                                vec.push(
                                                    tuple.get_item(i).expect(
                                                        "All indexes are known to be valid.",
                                                    ),
                                                );
                                            }
                                            vec.push(v);
                                            validated_items = Some(vec);
                                        }
                                    }
                                }
                            }
                            Err(cause) => {
                                if let Some(m) = name
                                    && let Some(o) = object
                                {
                                    return Err(crate::utils::err_with_cause(
                                        value.py(),
                                        pyo3::exceptions::PyTypeError::new_err(format!(
                                            "Failed to validate item {} for the member {} of {}.",
                                            index,
                                            m,
                                            o.repr()?
                                        )),
                                        cause,
                                    ));
                                } else {
                                    return Err(crate::utils::err_with_cause(
                                        value.py(),
                                        pyo3::exceptions::PyTypeError::new_err(format!(
                                            "Failed to validate item {index}.",
                                        )),
                                        cause,
                                    ));
                                }
                            }
                        }
                    }
                    Ok(if let Some(vi) = validated_items {
                        pyo3::types::PyTuple::new(value.py(), vi)?.into_any()
                    } else {
                        value.clone()
                    })
                } else {
                    validation_error!("tuple", name, object, value)
                }
            }
            Self::VarTuple { item: None } => {
                if value.cast_exact::<pyo3::types::PyTuple>().is_ok() {
                    Ok(value.clone())
                } else {
                    validation_error!("tuple", name, object, value)
                }
            }
            Self::FrozenSet { item: Some(item) } => {
                if let Ok(fset) = value.cast_exact::<pyo3::types::PyFrozenSet>() {
                    let mut validated_items: Option<Vec<Bound<'_, PyAny>>> = None;
                    for (index, titem) in fset.iter().enumerate() {
                        match item.validate(name, object, &titem) {
                            Ok(v) => {
                                if !v.is(&titem) {
                                    match &mut validated_items {
                                        Some(vec) => vec.push(v),
                                        None => {
                                            let mut vec = Vec::with_capacity(fset.len());
                                            for i in 0..index {
                                                vec.push(
                                                    fset.get_item(i).expect(
                                                        "All indexes are known to be valid.",
                                                    ),
                                                );
                                            }
                                            vec.push(v);
                                            validated_items = Some(vec);
                                        }
                                    }
                                }
                            }
                            Err(cause) => {
                                if let Some(m) = name
                                    && let Some(o) = object
                                {
                                    return Err(crate::utils::err_with_cause(
                                        value.py(),
                                        pyo3::exceptions::PyTypeError::new_err(format!(
                                            "Failed to validate item {} for the member {} of {}.",
                                            index,
                                            m,
                                            o.repr()?
                                        )),
                                        cause,
                                    ));
                                } else {
                                    return Err(crate::utils::err_with_cause(
                                        value.py(),
                                        pyo3::exceptions::PyTypeError::new_err(format!(
                                            "Failed to validate item {index}.",
                                        )),
                                        cause,
                                    ));
                                }
                            }
                        }
                    }
                    Ok(if let Some(vi) = validated_items {
                        pyo3::types::PyFrozenSet::new(value.py(), vi)?.into_any()
                    } else {
                        value.clone()
                    })
                } else {
                    validation_error!("frozenset", name, object, value)
                }
            }
            Self::FrozenSet { item: None } => {
                if value.cast_exact::<pyo3::types::PyFrozenSet>().is_ok() {
                    Ok(value.clone())
                } else {
                    validation_error!("frozenset", name, object, value)
                }
            }
            Self::Set { item: Some(item) } => {
                // FIXME add a fast path for ATorsSet with matching object and memeber
                if let Ok(set) = value.cast::<pyo3::types::PySet>() {
                    let py = value.py();
                    let mut validated_items: Vec<Bound<'_, PyAny>> = Vec::with_capacity(set.len());
                    for (index, titem) in set.iter().enumerate() {
                        match item.validate(name, object, &titem) {
                            Ok(v) => validated_items.push(v),
                            Err(cause) => {
                                if let Some(m) = name
                                    && let Some(o) = object
                                {
                                    return Err(crate::utils::err_with_cause(
                                        value.py(),
                                        pyo3::exceptions::PyTypeError::new_err(format!(
                                            "Failed to validate item {} for the member {} of {}.",
                                            index,
                                            m,
                                            o.repr()?
                                        )),
                                        cause,
                                    ));
                                } else {
                                    return Err(crate::utils::err_with_cause(
                                        value.py(),
                                        pyo3::exceptions::PyTypeError::new_err(format!(
                                            "Failed to validate item {index}.",
                                        )),
                                        cause,
                                    ));
                                }
                            }
                        }
                    }
                    Ok({
                        crate::containers::AtorsSet::new(
                            py,
                            (*item.0).clone(),
                            name,
                            object.map(|m| m.clone().unbind()),
                            validated_items,
                        )?
                        .into_any()
                    })
                } else {
                    validation_error!("set", name, object, value)
                }
            }
            Self::Set { item: None } => {
                if let Ok(v) = value.cast::<pyo3::types::PySet>() {
                    // Preserve the copy on assignment semantic
                    PySet::new(v.py(), v.iter()).map(|s| s.into_any())
                } else {
                    validation_error!("set", name, object, value)
                }
            }
            Self::List { item: Some(item) } => {
                // FIXME add a fast path for AtorsList with matching object and member
                if let Ok(list) = value.cast::<pyo3::types::PyList>() {
                    let py = value.py();
                    let mut validated_items: Vec<Bound<'_, PyAny>> =
                        Vec::with_capacity(list.len());
                    for (index, titem) in list.iter().enumerate() {
                        match item.validate(name, object, &titem) {
                            Ok(v) => validated_items.push(v),
                            Err(cause) => {
                                if let Some(m) = name
                                    && let Some(o) = object
                                {
                                    return Err(crate::utils::err_with_cause(
                                        value.py(),
                                        pyo3::exceptions::PyTypeError::new_err(format!(
                                            "Failed to validate item {} for the member {} of {}.",
                                            index,
                                            m,
                                            o.repr()?
                                        )),
                                        cause,
                                    ));
                                } else {
                                    return Err(crate::utils::err_with_cause(
                                        value.py(),
                                        pyo3::exceptions::PyTypeError::new_err(format!(
                                            "Failed to validate item {index}.",
                                        )),
                                        cause,
                                    ));
                                }
                            }
                        }
                    }
                    Ok({
                        crate::containers::AtorsList::new(
                            py,
                            (*item.0).clone(),
                            name,
                            object.map(|m| m.clone().unbind()),
                            validated_items,
                        )?
                        .into_any()
                    })
                } else {
                    validation_error!("list", name, object, value)
                }
            }
            Self::List { item: None } => {
                if let Ok(v) = value.cast::<pyo3::types::PyList>() {
                    // Preserve the copy on assignment semantic
                    PyList::new(v.py(), v.iter()).map(|l| l.into_any())
                } else {
                    validation_error!("list", name, object, value)
                }
            }
            Self::Dict {
                items: Some((key_v, val_v)),
            } => {
                // FIXME add a fast path for AtorsDict with matching object and memeber
                if let Ok(dict) = value.cast::<pyo3::types::PyDict>() {
                    let mut validated_items: Vec<(Bound<'_, PyAny>, Bound<'_, PyAny>)> =
                        Vec::with_capacity(dict.len());
                    for (tk, tv) in dict.iter() {
                        match (
                            key_v.validate(name, object, &tk),
                            val_v.validate(name, object, &tv),
                        ) {
                            (Ok(k), Ok(v)) => validated_items.push((k, v)),
                            (Err(err), __ior__) => {
                                if let Some(m) = name
                                    && let Some(o) = object
                                {
                                    return Err(crate::utils::err_with_cause(
                                        value.py(),
                                        pyo3::exceptions::PyTypeError::new_err(format!(
                                            "Failed to validate key '{}' for the member {} of {}.",
                                            tk.repr()?,
                                            m,
                                            o.repr()?
                                        )),
                                        err,
                                    ));
                                } else {
                                    return Err(crate::utils::err_with_cause(
                                        value.py(),
                                        pyo3::exceptions::PyTypeError::new_err(format!(
                                            "Failed to validate key '{}'.",
                                            tk.repr()?,
                                        )),
                                        err,
                                    ));
                                }
                            }
                            (Ok(_), Err(err)) => {
                                if let Some(m) = name
                                    && let Some(o) = object
                                {
                                    return Err(crate::utils::err_with_cause(
                                        value.py(),
                                        pyo3::exceptions::PyTypeError::new_err(format!(
                                            "Failed to validate value '{}' with key '{}' for the member {} of {}.",
                                            tv.repr()?,
                                            tk.repr()?,
                                            m,
                                            o.repr()?
                                        )),
                                        err,
                                    ));
                                } else {
                                    return Err(crate::utils::err_with_cause(
                                        value.py(),
                                        pyo3::exceptions::PyTypeError::new_err(format!(
                                            "Failed to validate value '{}' with key '{}'.",
                                            tk.repr()?,
                                            tv.repr()?
                                        )),
                                        err,
                                    ));
                                }
                            }
                        }
                    }
                    Ok({
                        let py = value.py();
                        crate::containers::AtorsDict::new(
                            py,
                            (*key_v.0).clone(),
                            (*val_v.0).clone(),
                            name,
                            object.map(|m| m.clone().unbind()),
                            validated_items,
                        )?
                        .into_any()
                    })
                } else {
                    validation_error!("dict", name, object, value)
                }
            }
            Self::Dict { items: None } => {
                if let Ok(v) = value.cast::<pyo3::types::PyDict>() {
                    // Preserve the copy on assignment semantic
                    PyDict::from_sequence(v).map(|d| d.into_any())
                } else {
                    validation_error!("dict", name, object, value)
                }
            }
            Self::Typed { type_ } => {
                let t = type_.bind(value.py());
                if value.is_instance(t)? {
                    Ok(value.clone())
                } else {
                    validation_error!(t.repr()?, name, object, value)
                }
            }
            Self::Instance { types } => {
                let t = types.0.bind(value.py());
                if value.is_instance(t)? {
                    Ok(value.clone())
                } else {
                    validation_error!(t.repr()?, name, object, value)
                }
            }
            Self::Union { members } => {
                let mut err = Vec::with_capacity(members.len());
                for v in members.iter() {
                    match v.validate(name, object, value) {
                        Ok(validated) => return Ok(validated),
                        Err(e) => err.push(e),
                    }
                }
                let eg = pyo3::exceptions::PyTypeError::new_err(format!(
                    "Value {} is not valid for any member of the union for {:?}",
                    value.repr()?,
                    members
                ));
                Err(crate::utils::err_with_cause(
                    value.py(),
                    eg,
                    pyo3::exceptions::PyBaseExceptionGroup::new_err(err),
                ))
            }
            Self::GenericAttributes { type_, attributes } => {
                let t = type_.bind(value.py());
                if !value.is_instance(t)? {
                    return validation_error!(t.repr()?, name, object, value);
                }
                for (attr_name, validator) in attributes {
                    let attr_value = value.getattr(attr_name.as_str())?;
                    // Coercing the attribute of generic type to the expected form
                    // does not make sense in general, so we use strict_validate here
                    match validator.strict_validate(name, object, &attr_value) {
                        Ok(_) => {}
                        Err(cause) => {
                            if let Some(m) = name
                                && let Some(o) = object
                            {
                                return Err(crate::utils::err_with_cause(
                                    value.py(),
                                    pyo3::exceptions::PyTypeError::new_err(format!(
                                        "Failed to validate attribute '{}' of {} for the member {} of {}.",
                                        attr_name,
                                        value.repr()?,
                                        m,
                                        o.repr()?
                                    )),
                                    cause,
                                ));
                            } else {
                                return Err(crate::utils::err_with_cause(
                                    value.py(),
                                    pyo3::exceptions::PyTypeError::new_err(format!(
                                        "Failed to validate attribute '{}' of {}.",
                                        attr_name,
                                        value.repr()?
                                    )),
                                    cause,
                                ));
                            }
                        }
                    }
                }
                Ok(value.clone())
            }
            Self::ForwardValidator { late_validator } => {
                let py = value.py();
                let resolved_validator = late_validator.get_validator(py)?;
                resolved_validator.get().validate_type(name, object, value)
            }
        }
    }

    pub fn create_default<'py>(
        &self,
        args: &Bound<'py, PyTuple>,
        kwargs: &Option<Py<PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = args.py();
        match self {
            Self::Typed { type_ } => type_.bind(py).call(
                args,
                match kwargs {
                    None => None,
                    Some(kw) => Some(kw.bind(py)),
                },
            ),
            _ => Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Cannot create a default value using args and kwargs for {self:?}"
            ))),
        }
    }

    pub fn is_type_mutable<'py>(&self, py: Python<'py>) -> Mutability {
        match self {
            Self::None {}
            | Self::Bool {}
            | Self::Int {}
            | Self::Float {}
            | Self::Complex {}
            | Self::Bytes {}
            | Self::Str {} => Mutability::Immutable,
            Self::Any {} => Mutability::Undecidable,
            Self::FrozenSet { item } | Self::VarTuple { item } => match item {
                None => Mutability::Immutable,
                Some(iv) => iv.type_validator.is_type_mutable(py),
            },
            // NOTE try_fold does not seem relevant here
            Self::Tuple { items } => {
                items
                    .iter()
                    .fold(Mutability::Immutable, |acc: Mutability, e| {
                        match (acc, e.type_validator.is_type_mutable(py)) {
                            // If one item is mutable the tuple is seen as mutable
                            (Mutability::Mutable, _) => Mutability::Mutable,
                            // If one item is undecidable, the tuple is mutable if the
                            // new item is otherwise it remains undecidable
                            (Mutability::Undecidable, Mutability::Mutable) => Mutability::Mutable,
                            (Mutability::Undecidable, Mutability::Undecidable) => {
                                Mutability::Undecidable
                            }
                            (Mutability::Undecidable, Mutability::Immutable) => {
                                Mutability::Undecidable
                            }
                            // If all previous items are immutable everything depend on
                            // the last visited one.
                            (Mutability::Immutable, im) => im,
                        }
                    })
            }
            Self::Set { item: _ } => Mutability::Mutable,
            Self::List { item: _ } => Mutability::Mutable,
            Self::Dict { items: _ } => Mutability::Mutable,
            Self::Typed { type_ } => {
                let mm = get_type_mutability_map(py);
                with_critical_section(mm.as_any(), || {
                    mm.borrow().get_type_mutability(type_.bind(py))
                })
            }
            Self::Instance { types } => {
                types
                    .iter(py)
                    .fold(Mutability::Immutable, |acc: Mutability, e| {
                        let mm = get_type_mutability_map(py);
                        match (
                            acc,
                            with_critical_section(mm.as_any(), || {
                                mm.borrow().get_type_mutability(&e)
                            }),
                        ) {
                            // If one item is mutable the tuple is seen as mutable
                            (Mutability::Mutable, _) => Mutability::Mutable,
                            // If one item is undecidable, the tuple is mutable if the
                            // new item is otherwise it remains undecidable
                            (Mutability::Undecidable, Mutability::Mutable) => Mutability::Mutable,
                            (Mutability::Undecidable, Mutability::Undecidable) => {
                                Mutability::Undecidable
                            }
                            (Mutability::Undecidable, Mutability::Immutable) => {
                                Mutability::Undecidable
                            }
                            // If all previous items are immutable everything depend on
                            // the last visited one.
                            (Mutability::Immutable, im) => im,
                        }
                    })
            }
            Self::ForwardValidator { late_validator } => late_validator.is_type_mutable(py),
            Self::GenericAttributes {
                type_,
                attributes: _,
            } => {
                let mm = get_type_mutability_map(py);
                with_critical_section(mm.as_any(), || {
                    mm.borrow().get_type_mutability(type_.bind(py))
                })
            }
            Self::Union { members } => {
                members
                    .iter()
                    .fold(Mutability::Immutable, |acc: Mutability, e| {
                        match (acc, e.type_validator.is_type_mutable(py)) {
                            // If one item is mutable the tuple is seen as mutable
                            (Mutability::Mutable, _) => Mutability::Mutable,
                            // If one item is undecidable, the tuple is mutable if the
                            // new item is otherwise it remains undecidable
                            (Mutability::Undecidable, Mutability::Mutable) => Mutability::Mutable,
                            (Mutability::Undecidable, Mutability::Undecidable) => {
                                Mutability::Undecidable
                            }
                            (Mutability::Undecidable, Mutability::Immutable) => {
                                Mutability::Undecidable
                            }
                            // If all previous items are immutable everything depend on
                            // the last visited one.
                            (Mutability::Immutable, im) => im,
                        }
                    })
            }
        }
    }
}

impl Clone for TypeValidator {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            Self::Any {} => Self::Any {},
            Self::None {} => Self::None {},
            Self::Bool {} => Self::Bool {},
            Self::Int {} => Self::Int {},
            Self::Float {} => Self::Float {},
            Self::Complex {} => Self::Complex {},
            Self::Str {} => Self::Str {},
            Self::Bytes {} => Self::Bytes {},
            Self::Tuple { items } => Self::Tuple {
                items: items.to_vec(),
            },
            Self::VarTuple { item } => Self::VarTuple { item: item.clone() },
            Self::FrozenSet { item } => Self::FrozenSet { item: item.clone() },
            Self::Set { item } => Self::Set { item: item.clone() },
            Self::List { item } => Self::List { item: item.clone() },
            Self::Dict { items } => Self::Dict {
                items: items.clone(),
            },
            Self::Typed { type_ } => Self::Typed {
                type_: type_.clone_ref(py),
            },
            Self::Instance { types } => Self::Instance {
                types: TypesTuple(types.0.clone_ref(py)),
            },
            Self::Union { members } => Self::Union {
                members: members.to_vec(),
            },
            Self::GenericAttributes { type_, attributes } => Self::GenericAttributes {
                type_: type_.clone_ref(py),
                attributes: attributes.clone(),
            },
            Self::ForwardValidator { late_validator } => Self::ForwardValidator {
                late_validator: late_validator.clone(),
            },
        })
    }
}
