/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{
    Bound, FromPyObject, Py, PyAny, PyErr, PyRefMut, PyResult, PyTypeInfo, Python, intern, pyclass,
    pymethods,
    types::{PyAnyMethods, PyBool, PyBytes, PyFloat, PyInt, PyString, PyType, PyTypeMethods},
};
use std::collections::HashMap;

///
pub(crate) fn err_with_cause<'py>(py: Python<'py>, err: PyErr, cause: PyErr) -> PyErr {
    err.set_cause(py, Some(cause));
    err
}

// Copied from pyo3 internals

/// Returns Ok if the error code is not -1.
#[inline]
pub(crate) fn error_on_minusone<T: SignedInteger>(py: Python<'_>, result: T) -> PyResult<()> {
    if result != T::MINUS_ONE {
        Ok(())
    } else {
        Err(PyErr::fetch(py))
    }
}

pub(crate) trait SignedInteger: Eq {
    const MINUS_ONE: Self;
}

macro_rules! impl_signed_integer {
    ($t:ty) => {
        impl SignedInteger for $t {
            const MINUS_ONE: Self = -1;
        }
    };
}

impl_signed_integer!(i8);
impl_signed_integer!(i16);
impl_signed_integer!(i32);
impl_signed_integer!(i64);
impl_signed_integer!(i128);
impl_signed_integer!(isize);

///
macro_rules! create_behavior_callable_checker {
    ($mod: ident, $behavior:ident, $variant:ident, $n:literal) => {
        mod $mod {
            use pyo3::{
                Bound, FromPyObject, IntoPyObject, Py, PyAny, PyResult, Python, intern,
                types::PyAnyMethods,
            };
            use std::convert::Infallible;

            #[derive(Debug)]
            pub struct Callable(pub Py<PyAny>);

            impl FromPyObject<'_> for Callable {
                fn extract_bound<'py>(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
                    let py = ob.py();
                    let sig = py
                        .import(intern!(py, "inspect"))?
                        .getattr(intern!(py, "signature"))?;
                    let ob_sig_len = sig
                        .call1((ob,))?
                        .getattr(intern!(py, "parameters"))?
                        .len()?;
                    if !ob.is_callable() || ob_sig_len != $n {
                        Err(pyo3::exceptions::PyValueError::new_err(format!(
                            "{}.{} expect a callable taking {} got {} which takes {}.",
                            stringify!($behavior),
                            stringify!($variant),
                            $n,
                            ob,
                            ob_sig_len
                        )))
                    } else {
                        Ok(Callable(ob.clone().unbind()))
                    }
                }
            }

            impl<'py> IntoPyObject<'py> for &Callable {
                type Target = PyAny;
                type Output = Bound<'py, PyAny>;
                type Error = Infallible;
                fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
                    Ok(self.0.clone_ref(py).into_bound(py))
                }
            }
        }
    };
}

pub(crate) use create_behavior_callable_checker;
// This approach allows to implement an equivalent of custom constructor
// for enums

#[allow(dead_code)]
/// Wrapper allowing to hash and compare for eq Py<PyType> for use in HashMap
/// while guaranteeing that the underlying Python type remain valid.
struct PyTypeWrap {
    type_: Py<PyType>,
    id: isize,
}

impl From<&Bound<'_, PyType>> for PyTypeWrap {
    fn from(value: &Bound<'_, PyType>) -> Self {
        let id = value.as_ptr() as isize;
        PyTypeWrap {
            type_: Bound::clone(value).unbind(),
            id,
        }
    }
}

impl std::hash::Hash for PyTypeWrap {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for PyTypeWrap {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for PyTypeWrap {}

create_behavior_callable_checker!(mutability_callable_check, TypeMutabilityMap, __setitem__, 1);

// /// Dedicated class to store user specified information about third party
// /// generic types
// #[pyclass]
// pub struct GenericAttributesMap

use crate::core::AtorsBase;

use crate::meta::ATORS_FROZEN;

/// Enum representing whether a type is mutable, immutable, or mutability is undecidable
#[pyclass(module = "ators._ators", eq, frozen)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutability {
    /// The type is mutable
    Mutable,
    /// The type is immutable
    Immutable,
    /// The type's mutability cannot be determined
    Undecidable,
}

/// Enum representing the mutability specification for a type
enum MutabilitySpec {
    /// Type is always mutable
    Mutable,
    /// Type is always immutable
    Immutable,
    /// Type mutability should be inspected by calling the provided callable
    Inspect(Py<PyAny>),
}

/// Dedicated class to store user specified mutability
#[pyclass]
pub struct TypeMutabilityMap {
    map: HashMap<PyTypeWrap, MutabilitySpec>,
}

impl TypeMutabilityMap {
    pub fn new(py: Python<'_>) -> Py<TypeMutabilityMap> {
        let mut map = HashMap::default();

        // Add built-in immutable types with Immutable variant
        let int_type = PyInt::type_object(py);
        map.insert((&int_type).into(), MutabilitySpec::Immutable);
        let float_type = PyFloat::type_object(py);
        map.insert((&float_type).into(), MutabilitySpec::Immutable);
        let bool_type = PyBool::type_object(py);
        map.insert((&bool_type).into(), MutabilitySpec::Immutable);
        let str_type = PyString::type_object(py);
        map.insert((&str_type).into(), MutabilitySpec::Immutable);
        let bytes_type = PyBytes::type_object(py);
        map.insert((&bytes_type).into(), MutabilitySpec::Immutable);

        Py::new(py, TypeMutabilityMap { map }).expect("TypeMutabilityMap creation cannot fail.")
    }
}

#[pymethods]
impl TypeMutabilityMap {
    pub fn __setitem__<'py>(
        mut self_: PyRefMut<'py, Self>,
        type_: &Bound<'py, PyType>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let spec = if let Ok(bool_val) = value.extract::<bool>() {
            // Convert bool to appropriate variant
            if bool_val {
                MutabilitySpec::Mutable
            } else {
                MutabilitySpec::Immutable
            }
        } else {
            // Validate it's a callable with the appropriate signature
            let validated =
                <mutability_callable_check::Callable as FromPyObject>::extract_bound(value)?;
            MutabilitySpec::Inspect(validated.0)
        };

        self_.map.insert(type_.into(), spec);
        Ok(())
    }

    pub fn get_type_mutability<'py>(&self, type_: &Bound<'py, PyType>) -> Mutability {
        let py = type_.py();
        if let Ok(t) = type_.cast::<AtorsBase>() {
            if t.getattr(ATORS_FROZEN)
                .expect("Subclass of AtorsBase must have __ators_frozen__ set")
                .extract::<bool>()
                .expect("__ators_frozen__ should always be a bool")
            {
                Mutability::Immutable
            } else {
                Mutability::Mutable
            }
        } else if let Ok(params) = type_.getattr(intern!(py, "__dataclass_params__"))
            && params
                .getattr(intern!(py, "frozen"))
                .expect("DataclassParams have a frozen attr")
                .extract()
                .expect("Frozen is a bool")
        {
            Mutability::Immutable
        } else {
            self.map
                .get(&type_.into())
                .map_or(Mutability::Undecidable, |spec| match spec {
                    MutabilitySpec::Mutable => Mutability::Mutable,
                    MutabilitySpec::Immutable => Mutability::Immutable,
                    MutabilitySpec::Inspect(_) => Mutability::Undecidable,
                })
        }
    }

    pub fn get_object_mutability<'py>(&self, obj: &Bound<'py, PyAny>) -> PyResult<Mutability> {
        let obj_type = obj.get_type();
        let py = obj.py();
        let ators_base_type = py.get_type::<AtorsBase>();

        if obj_type.is_subclass(&ators_base_type)? {
            // For Ators objects, check if frozen
            let ators_obj = obj.cast::<AtorsBase>()?;
            if ators_obj.borrow().is_frozen() {
                Ok(Mutability::Immutable)
            } else {
                Ok(Mutability::Mutable)
            }
        } else {
            // For other objects, first check type mutability and then inspect object
            // if undecidable
            let type_mutability = self.get_type_mutability(&obj_type);
            if type_mutability == Mutability::Undecidable {
                // If type mutability is undecidable, inspect the object
                match self.map.get(&(&obj_type).into()) {
                    None => Ok(Mutability::Undecidable),
                    Some(spec) => match spec {
                        MutabilitySpec::Mutable => Ok(Mutability::Mutable),
                        MutabilitySpec::Immutable => Ok(Mutability::Immutable),
                        MutabilitySpec::Inspect(callable) => {
                            let call_result = callable.call1(py, (obj,))?;
                            let call_result_bound = call_result.bind(py);
                            call_result_bound.extract::<bool>().map(
                                |b| if b { Mutability::Mutable } else { Mutability::Immutable }
                            ).map_err(|_| {
                                let obj_type_name = obj_type
                                    .getattr("__name__")
                                    .and_then(|n| n.extract::<String>())
                                    .unwrap_or_else(|_| "<unknown>".to_string());
                                let result_type = call_result_bound.get_type();
                                let result_type_name = result_type
                                    .getattr("__name__")
                                    .and_then(|n| n.extract::<String>())
                                    .unwrap_or_else(|_| "<unknown>".to_string());
                                PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                                    format!(
                                        "Mutability callable for type {} did not return a bool, but returned {}",
                                        obj_type_name, result_type_name
                                    )
                                )
                            })
                        }
                    },
                }
            } else {
                Ok(type_mutability)
            }
        }
    }
}
