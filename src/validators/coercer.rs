///
use pyo3::{
    pyclass,
    types::{PyAnyMethods, PyBool, PyBytes, PyFloat, PyInt, PyString, PyTuple},
    Bound, Py, PyAny, PyResult, PyTypeInfo, Python,
};

use super::TypeValidator;

///
#[pyclass(frozen)]
pub enum Coercer {
    #[pyo3(constructor = ())]
    TypeInferred {},
    #[pyo3(constructor = (callable))]
    CallObject { callable: Py<PyAny> },
    #[pyo3(constructor = (meth_name))]
    MemberMethod { meth_name: Py<PyString> },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
}

impl Coercer {
    ///
    pub(crate) fn coerce_value<'py>(
        &self,
        type_validator: &TypeValidator,
        member: &Bound<'py, crate::member::Member>,
        object: &Bound<'py, crate::core::BaseAtors>,
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
                TypeValidator::Tuple { items: _ } => PyTuple::type_object(py).call1((value,)),
                TypeValidator::Typed { type_ } => type_.bind(py).call1((value,)),
            },
            Self::CallObject { callable } => callable.bind(member.py()).call1((value,)),
            Self::MemberMethod { meth_name } => member.call_method1(meth_name, (object, value)),
            Self::ObjectMethod { meth_name } => object.call_method1(meth_name, (member, value)),
        }
    }
}

impl Clone for Coercer {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            Self::TypeInferred {} => Self::TypeInferred {},
            Self::CallObject { callable } => Self::CallObject {
                callable: callable.clone_ref(py),
            },
            Self::MemberMethod { meth_name } => Self::MemberMethod {
                meth_name: meth_name.clone_ref(py),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}
