/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Matthieu C. Dartiailh
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{
    Bound, Py, PyAny, PyResult, Python, pyclass,
    types::{PyAnyMethods, PyDict, PyString, PyTuple},
};

///
#[pyclass(frozen)]
#[derive(Debug)]
pub enum DefaultBehavior {
    #[pyo3(constructor = ())]
    NoDefault {},
    #[pyo3(constructor = (value))]
    Static { value: Py<PyAny> },
    #[pyo3(constructor = (args, kwargs))]
    ValidatorDelegate {
        args: Py<PyTuple>,
        kwargs: Option<Py<PyDict>>,
    },
    #[pyo3(constructor = (callable))]
    Call { callable: Py<PyAny> },
    #[pyo3(constructor = (callable))]
    CallMemberObject { callable: Py<PyAny> },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
}

impl DefaultBehavior {
    ///
    pub(crate) fn default<'py>(
        &self,
        member: &Bound<'py, super::Member>,
        object: &Bound<'py, crate::core::AtorsBase>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::NoDefault {} => Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "The member {} from {} value is unset and has no default",
                member.borrow().name,
                object.repr()?
            ))),
            Self::Static { value } => Ok(value.clone_ref(member.py()).into_bound(member.py())),
            Self::ValidatorDelegate { args, kwargs } => member
                .borrow()
                .validator
                .create_default(args.bind(member.py()), kwargs),
            Self::Call { callable } => callable.bind(member.py()).call0(),
            Self::CallMemberObject { callable } => {
                callable.bind(member.py()).call1((member, object))
            }
            Self::ObjectMethod { meth_name } => object.call_method1(meth_name, (member,)),
        }
    }
}

impl Clone for DefaultBehavior {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            Self::NoDefault {} => Self::NoDefault {},
            Self::Static { value } => Self::Static {
                value: value.clone_ref(py),
            },
            Self::ValidatorDelegate { args, kwargs } => Self::ValidatorDelegate {
                args: args.clone_ref(py),
                kwargs: kwargs.as_ref().map(|v| v.clone_ref(py)),
            },
            Self::Call { callable } => Self::Call {
                callable: callable.clone_ref(py),
            },
            Self::CallMemberObject { callable } => Self::CallMemberObject {
                callable: callable.clone_ref(py),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}
