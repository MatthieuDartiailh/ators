/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{
    Bound, FromPyObject, IntoPyObject, Py, PyAny, PyResult, Python,
    ffi::{PyBool_Check, PyBytes_Check, PyFloat_Check, PyLong_Check, PyUnicode_Check},
    pyclass, pymethods,
    sync::OnceLockExt,
    types::{
        IntoPyDict, PyAnyMethods, PyDict, PyString, PyTuple, PyTupleMethods, PyType, PyTypeMethods,
    },
};
use std::{convert::Infallible, sync::OnceLock};

use super::Validator;
use crate::annotations::{build_validator_from_annotation, get_type_tools};

#[derive(Debug)]
pub(crate) struct TypesTuple(Py<PyTuple>);

impl TypesTuple {
    ///
    pub fn coerce<'py>(&self, value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let py = value.py();
        let type_ = self.0.bind(py).get_item(0)?;
        type_.call1((value,))
    }
}

impl FromPyObject<'_> for TypesTuple {
    fn extract_bound<'py>(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let py = ob.py();
        if let Ok(ty) = ob.cast::<PyType>() {
            Ok(TypesTuple(PyTuple::new(py, [ty])?.into()))
        } else if let Ok(s) = ob.cast::<PyTuple>()
            && s.len() > 0
            && s.iter().all(|item| item.is_instance_of::<PyType>())
        {
            Ok(TypesTuple(s.clone().unbind()))
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

///
#[pyclass(frozen)]
#[derive(Debug)]

pub(crate) struct LateResolvedValidator {
    validator_cell: OnceLock<PyResult<Py<TypeValidator>>>,
    forward_ref: Py<PyAny>,
    ctx_provider: Option<Py<PyAny>>,
    type_containers: i64,
    name: Py<PyString>,
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
        }
    }

    ///
    pub fn get_validator<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, TypeValidator>> {
        let validator = self.validator_cell.get_or_init_py_attached(py, || {
            let typing = py.import("typing")?;
            let evaluate_forward_ref = typing.getattr("evaluate_forward_ref")?;
            let forward_ref = self.forward_ref.bind(py);
            let resolved;
            if let Some(cp) = &self.ctx_provider {
                let ctx_provider = cp.bind(py);
                let kwargs = [("locals", ctx_provider.call0()?)].into_py_dict(py)?;
                resolved = evaluate_forward_ref.call((forward_ref,), Some(&kwargs))?;
            } else {
                resolved = evaluate_forward_ref.call1((forward_ref,))?;
            }
            Ok(Py::new(
                py,
                build_validator_from_annotation(
                    self.name.bind(py),
                    &resolved,
                    self.type_containers,
                    &get_type_tools(py)?,
                    None,
                )?
                .type_validator,
            )?)
        });
        match validator {
            Ok(tv) => Ok(tv.bind(py).clone()),
            Err(e) => Err(e.clone_ref(py)),
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
        })
    }
}

///
#[pyclass(frozen)]
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
    Str {},
    #[pyo3(constructor = ())]
    Bytes {},
    #[pyo3(constructor = (items))]
    Tuple { items: Vec<Validator> },
    #[pyo3(constructor = (item))]
    VarTuple { item: Option<Py<Validator>> },
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
    // XXX need a custom type to perform init validation
    // XXX need a mode for union to cleanly validate list[int] | dict[int, int]
    // Sequence,
    // List,
    // FrozenSet,
    // Set,
    // Mapping,
    // Dict,
    // DefaultDict,
    // NumpyArray,
    // Callable,
}

macro_rules! validation_error {
    ($type:expr, $member:expr, $object:expr, $value:expr) => {
        if let Some(m) = $member
            && let Some(o) = $object
        {
            Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "The member {} from {} expects a {}, got {} ({})",
                m.borrow().name(),
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
    ///
    pub fn validate_type<'py>(
        &self,
        member: Option<&Bound<'py, crate::member::Member>>,
        object: Option<&Bound<'py, crate::core::AtorsBase>>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Any {} => Ok(value),
            Self::None {} => {
                if value.is_none() {
                    Ok(value)
                } else {
                    validation_error!("None", member, object, value)
                }
            }
            Self::Bool {} => {
                if unsafe { PyBool_Check(value.as_ptr()) } != 0 {
                    Ok(value)
                } else {
                    validation_error!("bool", member, object, value)
                }
            }
            Self::Int {} => {
                if unsafe { PyLong_Check(value.as_ptr()) } != 0 {
                    Ok(value)
                } else {
                    validation_error!("int", member, object, value)
                }
            }
            Self::Float {} => {
                if unsafe { PyFloat_Check(value.as_ptr()) } != 0 {
                    Ok(value)
                } else {
                    validation_error!("float", member, object, value)
                }
            }
            Self::Str {} => {
                if unsafe { PyUnicode_Check(value.as_ptr()) } != 0 {
                    Ok(value)
                } else {
                    validation_error!("str", member, object, value)
                }
            }
            Self::Bytes {} => {
                if unsafe { PyBytes_Check(value.as_ptr()) } != 0 {
                    Ok(value)
                } else {
                    validation_error!("bytes", member, object, value)
                }
            }
            Self::Tuple { items } => {
                if let Ok(tuple) = value.cast_exact::<pyo3::types::PyTuple>() {
                    let t_length = tuple.len();
                    if t_length != items.len() {
                        return {
                            if let Some(m) = member
                                && let Some(o) = object
                            {
                                Err(pyo3::exceptions::PyTypeError::new_err(format!(
                                    "The member {} from {} expects a tuple of length {}, got a tuple of length {}",
                                    m.borrow().name(),
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
                        match validator.validate(member, object, item.clone()) {
                            Ok(v) => {
                                if !v.is(item) {
                                    match &mut validated_items {
                                        Some(vec) => vec.push(v),
                                        None => {
                                            let mut vec = Vec::with_capacity(t_length);
                                            for i in 0..index {
                                                vec.push(tuple.get_item(i).unwrap());
                                            }
                                            vec.push(v);
                                            validated_items = Some(vec);
                                        }
                                    }
                                }
                            }
                            Err(cause) => {
                                if let Some(m) = member
                                    && let Some(o) = object
                                {
                                    let exc = pyo3::exceptions::PyTypeError::new_err(format!(
                                        "Failed to validate item {} for the member {} of {}.",
                                        index,
                                        m.borrow().name(),
                                        o.repr()?
                                    ));
                                    exc.set_cause(value.py(), Some(cause));
                                    return Err(exc);
                                } else {
                                    let exc = pyo3::exceptions::PyTypeError::new_err(format!(
                                        "Failed to validate item {index}.",
                                    ));
                                    exc.set_cause(value.py(), Some(cause));
                                    return Err(exc);
                                }
                            }
                        }
                    }
                    Ok(if let Some(vi) = validated_items {
                        pyo3::types::PyTuple::new(value.py(), vi)?.into_any()
                    } else {
                        value
                    })
                } else {
                    validation_error!("tuple", member, object, value)
                }
            }
            Self::VarTuple { item: Some(item) } => {
                if let Ok(tuple) = value.cast_exact::<pyo3::types::PyTuple>() {
                    let mut validated_items: Option<Vec<Bound<'_, PyAny>>> = None;
                    for (index, titem) in tuple.iter().enumerate() {
                        match item
                            .borrow(value.py())
                            .validate(member, object, titem.clone())
                        {
                            Ok(v) => {
                                if !v.is(item) {
                                    match &mut validated_items {
                                        Some(vec) => vec.push(v),
                                        None => {
                                            let mut vec = Vec::with_capacity(tuple.len());
                                            for i in 0..index {
                                                vec.push(tuple.get_item(i).unwrap());
                                            }
                                            vec.push(v);
                                            validated_items = Some(vec);
                                        }
                                    }
                                }
                            }
                            Err(cause) => {
                                if let Some(m) = member
                                    && let Some(o) = object
                                {
                                    let exc = pyo3::exceptions::PyTypeError::new_err(format!(
                                        "Failed to validate item {} for the member {} of {}.",
                                        index,
                                        m.borrow().name(),
                                        o.repr()?
                                    ));
                                    exc.set_cause(value.py(), Some(cause));
                                    return Err(exc);
                                } else {
                                    let exc = pyo3::exceptions::PyTypeError::new_err(format!(
                                        "Failed to validate item {index}.",
                                    ));
                                    exc.set_cause(value.py(), Some(cause));
                                    return Err(exc);
                                }
                            }
                        }
                    }
                    Ok(if let Some(vi) = validated_items {
                        pyo3::types::PyTuple::new(value.py(), vi)?.into_any()
                    } else {
                        value
                    })
                } else {
                    validation_error!("tuple", member, object, value)
                }
            }
            Self::VarTuple { item: None } => {
                if value.cast_exact::<pyo3::types::PyTuple>().is_ok() {
                    Ok(value)
                } else {
                    validation_error!("tuple", member, object, value)
                }
            }
            Self::Typed { type_ } => {
                let t = type_.bind(value.py());
                if value.is_instance(t)? {
                    Ok(value)
                } else {
                    validation_error!(t.repr()?, member, object, value)
                }
            }
            Self::Instance { types } => {
                let t = types.0.bind(value.py());
                if value.is_instance(t)? {
                    Ok(value)
                } else {
                    validation_error!(t.repr()?, member, object, value)
                }
            }
            Self::Union { members } => {
                let mut err = Vec::with_capacity(members.len());
                for v in members.iter() {
                    match v.validate(member, object, Bound::clone(&value)) {
                        Ok(validated) => return Ok(validated),
                        Err(e) => err.push(e),
                    }
                }
                let eg = pyo3::exceptions::PyTypeError::new_err(format!(
                    "Value {} is not valid for any member of the union for {:?}",
                    value.repr()?,
                    members
                ));
                eg.set_cause(
                    value.py(),
                    Some(pyo3::exceptions::PyBaseExceptionGroup::new_err(err)),
                );
                Err(eg)
            }
            Self::GenericAttributes { type_, attributes } => {
                let t = type_.bind(value.py());
                if !value.is_instance(t)? {
                    return validation_error!(t.repr()?, member, object, value);
                }
                for (attr_name, validator) in attributes {
                    let attr_value = value.getattr(attr_name.as_str())?;
                    // Coercing the attribute of generic type to the expected form
                    // does not make sense in general, so we use strict_validate here
                    match validator.strict_validate(member, object, attr_value) {
                        Ok(_) => {}
                        Err(cause) => {
                            if let Some(m) = member
                                && let Some(o) = object
                            {
                                let exc = pyo3::exceptions::PyTypeError::new_err(format!(
                                    "Failed to validate attribute '{}' of {} for the member {} of {}.",
                                    attr_name,
                                    value.repr()?,
                                    m.borrow().name(),
                                    o.repr()?
                                ));
                                exc.set_cause(value.py(), Some(cause));
                                return Err(exc);
                            } else {
                                let exc = pyo3::exceptions::PyTypeError::new_err(format!(
                                    "Failed to validate attribute '{}' of {}.",
                                    attr_name,
                                    value.repr()?
                                ));
                                exc.set_cause(value.py(), Some(cause));
                                return Err(exc);
                            }
                        }
                    }
                }
                Ok(value)
            }
            Self::ForwardValidator { late_validator } => {
                let py = value.py();
                let resolved_validator = late_validator.get_validator(py)?;
                resolved_validator
                    .get()
                    .validate_type(member, object, value)
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
}

impl Clone for TypeValidator {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            Self::Any {} => Self::Any {},
            Self::None {} => Self::None {},
            Self::Bool {} => Self::Bool {},
            Self::Int {} => Self::Int {},
            Self::Float {} => Self::Float {},
            Self::Str {} => Self::Str {},
            Self::Bytes {} => Self::Bytes {},
            Self::Tuple { items } => Self::Tuple {
                items: items.to_vec(),
            },
            Self::VarTuple { item } => Self::VarTuple {
                item: item.as_ref().map(|inner| inner.clone_ref(py)),
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
