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
    types::{PyAnyMethods, PyBool, PyBytes, PyFloat, PyInt, PyString, PyTuple},
};

use super::TypeValidator;

///
#[pyclass(frozen)]
#[derive(Debug)]
pub enum Coercer {
    #[pyo3(constructor = ())]
    TypeInferred {},
    #[pyo3(constructor = (callable))]
    CallValueInit { callable: Py<PyAny> }, // Use a custom object to encapsulate a callable
    #[pyo3(constructor = (callable))]
    CallMemberObjectValueInit { callable: Py<PyAny> },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
}

impl Coercer {
    ///
    pub(crate) fn coerce_value<'py>(
        &self,
        is_init_coercion: bool,
        type_validator: &TypeValidator,
        member: Option<&Bound<'py, crate::member::Member>>,
        object: Option<&Bound<'py, crate::core::AtorsBase>>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = value.py();
        match self {
            Self::TypeInferred {} => match type_validator {
                TypeValidator::Any {} => Ok(value),
                TypeValidator::Bool {} => PyBool::type_object(py).call1((value,)),
                TypeValidator::Int {} => PyInt::type_object(py).call1((value,)),
                TypeValidator::Float {} => PyFloat::type_object(py).call1((value,)),
                TypeValidator::Str {} => PyString::type_object(py).call1((value,)),
                TypeValidator::Bytes {} => PyBytes::type_object(py).call1((value,)),
                // XXX we should attempt to coercer tuple element
                // forward is init coercion value to further validators
                TypeValidator::Tuple { items: _ } => PyTuple::type_object(py).call1((value,)),
                TypeValidator::Typed { type_ } => type_.bind(py).call1((value,)),
            },
            Self::CallValueInit { callable } => callable.bind(value.py()).call1((value, is_init_coercion)),
            Self::CallMemberObjectValueInit { callable } => callable
                .bind(value.py())
                .call1(
                (
                        member.ok_or(pyo3::exceptions::PyRuntimeError::new_err(
                    "Cannot use CallMemberObjectValue coercion when validator is not linked to a member."
                        ))?,
                        object.ok_or(
                            pyo3::exceptions::PyTypeError::new_err(
                                "Cannot use CallMemberObjectValue coercion when validator is not linked to a member."
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
                        member.ok_or(
                            pyo3::exceptions::PyTypeError::new_err(
                                "Cannot use ObjectMethod coercion when validator is not linked to a member."
                            )
                        )?,
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
            Self::CallValueInit { callable } => Self::CallValueInit {
                callable: callable.clone_ref(py),
            },
            Self::CallMemberObjectValueInit { callable } => Self::CallMemberObjectValueInit {
                callable: callable.clone_ref(py),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}
