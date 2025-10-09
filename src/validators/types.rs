use pyo3::{
    ffi::{PyBool_Check, PyBytes_Check, PyFloat_Check, PyLong_Check, PyUnicode_Check},
    pyclass,
    types::{
        PyAnyMethods, PyBool, PyBytes, PyDict, PyFloat, PyInt, PyString, PyTuple, PyTupleMethods,
        PyType, PyTypeMethods,
    },
    Bound, Py, PyAny, PyResult, PyTypeInfo,
};

#[pyclass(frozen)]
pub enum TypeValidator {
    #[pyo3(constructor = ())]
    Any {},
    Bool {},
    Int {},
    Float {},
    Str {},
    Bytes {},
    Tuple {
        items: Vec<Py<TypeValidator>>,
    },
    // VarTuple {
    //     item: Py<TypeValidator>,
    // },
    Typed {
        type_: Py<PyType>,
    },
    // XXX need aslo a custom constructor
    // ForwardTyped {
    //     type_: Option<Py<PyType>>,
    //     resolver: Py<PyAny>,
    // },
    // XXX need a custom constructor for validation
    // Instance {
    //     types: Py<PyTuple>,
    // },
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
        Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "The member {} from {} expects an {}, got {} ({})",
            $member.borrow().name,
            $object.repr()?,
            $type,
            $value.repr()?,
            $value.get_type().name()?
        )))
    };
}

impl TypeValidator {
    pub fn validate_type<'py>(
        &self,
        member: &Bound<'py, crate::member::Member>,
        object: &Bound<'py, crate::core::BaseAtors>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Any {} => Ok(value),
            Self::Bool {} => {
                if unsafe { PyBool_Check(value.as_ptr()) } == 0 {
                    Ok(value)
                } else {
                    validation_error!("bool", member, object, value)
                }
            }
            Self::Int {} => {
                if unsafe { PyLong_Check(value.as_ptr()) } == 0 {
                    Ok(value)
                } else {
                    validation_error!("int", member, object, value)
                }
            }
            Self::Float {} => {
                if unsafe { PyFloat_Check(value.as_ptr()) } == 0 {
                    Ok(value)
                } else {
                    validation_error!("float", member, object, value)
                }
            }
            Self::Str {} => {
                if unsafe { PyUnicode_Check(value.as_ptr()) } == 0 {
                    Ok(value)
                } else {
                    validation_error!("str", member, object, value)
                }
            }
            Self::Bytes {} => {
                if unsafe { PyBytes_Check(value.as_ptr()) } == 0 {
                    Ok(value)
                } else {
                    validation_error!("bytes", member, object, value)
                }
            }
            Self::Tuple { items } => {
                if let Ok(tuple) = value.cast_exact::<pyo3::types::PyTuple>() {
                    let t_length = tuple.len();
                    if t_length != items.len() {
                        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                            "The member {} from {} expects a tuple of length {}, got a tuple of length {}",
                            member.borrow().name,
                            object.repr()?,
                            items.len(),
                            t_length,
                        )));
                    }
                    let py = value.py();
                    let mut validated_items = Vec::with_capacity(items.len());
                    for (item, validator) in tuple.iter().zip(items) {
                        let v = validator.borrow(py).validate_type(member, object, item)?;
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
            _ => Err(pyo3::exceptions::PyTypeError::new_err(format!(""))),
        }
    }
}
