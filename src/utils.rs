/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{
    Bound, FromPyObject, Py, PyAny, PyErr, PyRefMut, PyResult, PyTypeInfo, Python, pyclass,
    pymethods,
    types::{PyAnyMethods, PyBool, PyBytes, PyFloat, PyInt, PyString, PyType},
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

    pub fn get_type_mutability(&self, type_: &Bound<'_, PyType>) -> Option<bool> {
        self.map.get(&type_.into()).and_then(|spec| match spec {
            MutabilitySpec::Mutable => Some(true),
            MutabilitySpec::Immutable => Some(false),
            MutabilitySpec::Inspect(_) => None,
        })
    }

    pub fn get_object_mutability<'py>(&self, obj: &Bound<'py, PyAny>) -> PyResult<Option<bool>> {
        let py = obj.py();
        let obj_type = obj.get_type();

        match self.map.get(&(&obj_type).into()) {
            None => Ok(None),
            Some(spec) => match spec {
                MutabilitySpec::Mutable => Ok(Some(true)),
                MutabilitySpec::Immutable => Ok(Some(false)),
                MutabilitySpec::Inspect(callable) => {
                    let call_result = callable.call1(py, (obj,))?;
                    let call_result_bound = call_result.bind(py);
                    call_result_bound.extract::<bool>().map_err(|_| {
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
                    }).map(Some)
                }
            },
        }
    }
}
