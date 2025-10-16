/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{
    Bound, Py, PyAny, PyResult, Python,
    ffi::{PyBool_Check, PyBytes_Check, PyFloat_Check, PyLong_Check, PyUnicode_Check},
    pyclass,
    types::{PyAnyMethods, PyDict, PyTuple, PyTupleMethods, PyType, PyTypeMethods},
};

///
#[pyclass(frozen)]
#[derive(Debug)]
pub enum TypeValidator {
    #[pyo3(constructor = ())]
    Any {},
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
    Tuple { items: Vec<TypeValidator> },
    // VarTuple {
    //     item: Py<TypeValidator>,
    // },
    #[pyo3(constructor = (type_))]
    Typed { type_: Py<PyType> },
    // XXX need also a custom constructor
    // ForwardTyped {
    //     type_: Option<Py<PyType>>,
    //     resolver: Py<PyAny>,
    // },
    // XXX need a custom constructor for validation
    // Instance {
    //     types: Py<PyTuple>,
    // },
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
                m.borrow().name,
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
    pub fn validate_type<'py>(
        &self,
        member: Option<&Bound<'py, crate::member::Member>>,
        object: Option<&Bound<'py, crate::core::AtorsBase>>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Any {} => Ok(value),
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
                                    m.borrow().name,
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
                    let mut validated_items = Vec::with_capacity(items.len());
                    for (item, validator) in tuple.iter().zip(items) {
                        let v = validator.validate_type(member, object, item)?;
                        validated_items.push(v);
                    }
                    Ok(pyo3::types::PyTuple::new(value.py(), validated_items)?.into_any())
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
            Self::Bool {} => Self::Bool {},
            Self::Int {} => Self::Int {},
            Self::Float {} => Self::Float {},
            Self::Str {} => Self::Str {},
            Self::Bytes {} => Self::Bytes {},
            Self::Tuple { items } => Self::Tuple {
                items: items.iter().cloned().collect(),
            },
            // Self::VarTuple { item } => Self::VarTuple {
            //     item: item.clone_ref(py),
            // },
            Self::Typed { type_ } => Self::Typed {
                type_: type_.clone_ref(py),
            },
        })
    }
}
