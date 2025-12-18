/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{
    Bound, Py, PyAny, PyResult, PyTypeInfo, Python, pyclass,
    types::{
        PyAnyMethods, PyBool, PyBytes, PyDict, PyDictMethods, PyFloat, PyFrozenSet, PyInt,
        PyListMethods, PyMapping, PyMappingMethods, PySequence, PySequenceMethods, PySet, PyString,
        PyTuple,
    },
};

use super::TypeValidator;
use crate::utils::{create_behavior_callable_checker, err_with_cause};

create_behavior_callable_checker!(co_callv, Coercer, CallValue, 1);
create_behavior_callable_checker!(co_callmovi, Coercer, CallNameObjectValueInit, 4);

///
#[pyclass(module = "ators._ators", frozen)]
#[derive(Debug)]
pub enum Coercer {
    #[pyo3(constructor = ())]
    TypeInferred {},
    // FIXME handle nested coercing for container by providing custom modes
    #[pyo3(constructor = (callable))]
    CallValue { callable: co_callv::Callable },
    #[pyo3(constructor = (callable))]
    CallNameObjectValueInit { callable: co_callmovi::Callable },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
}

impl Coercer {
    ///
    pub(crate) fn coerce_value<'py>(
        &self,
        is_init_coercion: bool,
        type_validator: &TypeValidator,
        member_name: Option<&str>,
        object: Option<&Bound<'py, crate::core::AtorsBase>>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = value.py();
        match self {
            Self::TypeInferred {} => match type_validator {
                TypeValidator::Any {} => Ok(value.clone()),  // Dead code but for completeness
                TypeValidator::None {} => Err(
                    pyo3::exceptions::PyTypeError::new_err(
                        "Cannot coerce a value to NoneType",
                    ),
                ),
                TypeValidator::Bool {} => PyBool::type_object(py).call1((value,)),
                TypeValidator::Int {} => PyInt::type_object(py).call1((value,)),
                TypeValidator::Float {} => PyFloat::type_object(py).call1((value,)),
                TypeValidator::Str {} => PyString::type_object(py).call1((value,)),
                TypeValidator::Bytes {} => PyBytes::type_object(py).call1((value,)),
                TypeValidator::Tuple { items } => {
                    let temp = value.cast::<PySequence>()?;
                    if temp.len()? != items.len() {
                        return Err(
                            pyo3::exceptions::PyTypeError::new_err(
                                format!(
                                    "Cannot coerce a {}-tuple into a {}-tuple",
                                    temp.len()?,
                                    items.len())
                            )
                        );
                    }
                    PyTuple::new(
                        py,
                        temp
                        .try_iter()?
                        .zip(items)
                        .map(|(v, t)| -> PyResult<Bound<'py, PyAny>> {
                            self.coerce_value(is_init_coercion, &t.type_validator, member_name, object, &v?)
                            }
                        )
                        .collect::<PyResult<Vec<_>>>()?
                    ).map(|ob| ob.as_any().clone())
                },
                TypeValidator::VarTuple { item } => {
                    let temp = value.cast::<PySequence>()?;
                    PyTuple::new(
                        py,
                        temp
                        .try_iter()?
                        .map(|v| -> PyResult<Bound<'py, PyAny>> {
                                if let Some(item_validator) = item {
                                    self.coerce_value(is_init_coercion, &item_validator.get().type_validator, member_name, object, &v?)
                                }
                                else {
                                    v
                                }
                            }
                        )
                        .collect::<PyResult<Vec<_>>>()?
                    ).map(|ob| ob.as_any().clone())
                },
                TypeValidator::FrozenSet { item } => {
                    let temp = value.cast::<PySequence>()?;
                    PyFrozenSet::new(
                        py,
                        temp
                        .try_iter()?
                        .map(|v| -> PyResult<Bound<'py, PyAny>> {
                                if let Some(item_validator) = item {
                                    self.coerce_value(is_init_coercion, &item_validator.get().type_validator, member_name, object, &v?)
                                }
                                else {
                                    v
                                }
                            }
                        )
                        .collect::<PyResult<Vec<_>>>()?
                    ).map(|ob| ob.as_any().clone())
                },
                TypeValidator::Set { item } => {
                    let temp = value.cast::<PySequence>()?;
                    // FIXME create the right container upfront so that we can use
                    // a fast validation path
                    PySet::new(
                        py,
                        temp
                        .try_iter()?
                        .map(|v| -> PyResult<Bound<'py, PyAny>> {
                                if let Some(item_validator) = item {
                                    self.coerce_value(is_init_coercion, &item_validator.get().type_validator, member_name, object, &v?)
                                }
                                else {
                                    v
                                }
                            }
                        )
                        .collect::<PyResult<Vec<_>>>()?
                    ).map(|ob| ob.as_any().clone())
                },
                TypeValidator::Dict { items } => {
                    let coerced = PyDict::new(py);
                    if let Ok(t) = value.cast::<PyDict>() {
                        for (k, v) in t.iter(){
                            if let Some((key_validator, val_validator)) = items {
                                let ck = self.coerce_value(is_init_coercion, &key_validator.get().type_validator, member_name, object, &k);
                                let cv = self.coerce_value(is_init_coercion, &val_validator.get().type_validator, member_name, object, &v);
                                coerced.set_item(ck?, cv?)?;
                            } else {
                                coerced.set_item(k, v)?;
                            }
                        }
                    } else if let Ok(tm) = value.cast::<PyMapping>() {
                        for i in tm.items()?.iter(){
                            let (k, v) = i.extract()?;
                            if let Some((key_validator, val_validator)) = items {
                                let ck = self.coerce_value(is_init_coercion, &key_validator.get().type_validator, member_name, object, &k);
                                let cv = self.coerce_value(is_init_coercion, &val_validator.get().type_validator, member_name, object, &v);
                                coerced.set_item(ck?, cv?)?;
                            } else {
                                coerced.set_item(k, v)?;
                            }
                        }
                    } else {
                        for p in value.try_iter()? {
                            let (k, v) = p?.extract()?;
                            if let Some((key_validator, val_validator)) = items {

                                let ck = self.coerce_value(is_init_coercion, &key_validator.get().type_validator, member_name, object, &k);
                                let cv = self.coerce_value(is_init_coercion, &val_validator.get().type_validator, member_name, object, &v);
                                coerced.set_item(ck?, cv?)?;
                            } else {
                                coerced.set_item(k, v)?;
                            }
                        }
                    };

                    // FIXME create the right container upfront so that we can use
                    // a fast validation path
                    Ok(coerced.as_any().clone())
                },
                TypeValidator::Typed { type_ } => type_.bind(py).call1((value,)),
                TypeValidator::Instance { types } => types.coerce(value),
                TypeValidator::ForwardValidator { late_validator } => self.coerce_value(
                    is_init_coercion,
                    late_validator.get_validator(py)?.get(),
                    member_name,
                    object,
                    value,
                ),
                TypeValidator::Union { members } => {
                    let mut err = Vec::with_capacity(members.len());
                    for m in members {
                        match m.coerce_value(is_init_coercion, member_name, object, value) {
                            Ok(validated) => return Ok(validated),
                            Err(e) => err.push(e),
                        }
                    }
                    Err(
                        err_with_cause(
                            value.py(),
                            pyo3::exceptions::PyTypeError::new_err(format!(
                                "Could not coerce value {} to any member in union {:?}",
                                value.repr()?,
                                members
                            )),
                            pyo3::exceptions::PyBaseExceptionGroup::new_err(err)
                        )
                    )
                },
                TypeValidator::GenericAttributes { type_, .. } => {
                    type_.bind(py).call1((value,))
                }
            },
            Self::CallValue { callable } => callable.0.bind(value.py()).call1((value,)),
            Self::CallNameObjectValueInit { callable } => callable
                .0.bind(value.py())
                .call1(
                (
                        member_name.ok_or(pyo3::exceptions::PyRuntimeError::new_err(
                    "Cannot use CallNameObjectValueInit coercion when validator is not linked to a member."
                        ))?,
                        object.ok_or(
                            pyo3::exceptions::PyTypeError::new_err(
                                "Cannot use CallNameObjectValueInit coercion when validator is not linked to a member."
                            )
                        )?,
                        value,
                        is_init_coercion,
                    ),
                ),
            Self::ObjectMethod { meth_name } => object
                .ok_or(pyo3::exceptions::PyTypeError::new_err(
                    "Cannot use ObjectMethod coercion when validator is not linked to a member."
                ))?
                .call_method1(
                    meth_name,
                    (
                        member_name.ok_or(
                            pyo3::exceptions::PyTypeError::new_err(
                                "Cannot use ObjectMethod coercion when validator is not linked to a member."
                            )
                        )?,
                        value,
                        is_init_coercion
                    ),
                ),
        }
    }
}

impl Clone for Coercer {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            Self::TypeInferred {} => Self::TypeInferred {},
            Self::CallValue { callable } => Self::CallValue {
                callable: co_callv::Callable(callable.0.clone_ref(py)),
            },
            Self::CallNameObjectValueInit { callable } => Self::CallNameObjectValueInit {
                callable: co_callmovi::Callable(callable.0.clone_ref(py)),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}
