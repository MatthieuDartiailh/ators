/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use crate::utils::create_behavior_callable_checker;
use pyo3::{
    Bound, Py, PyAny, PyRef, PyResult, Python, pyclass,
    types::{PyAnyMethods, PyDict, PyString, PyTuple},
};

create_behavior_callable_checker!(db_call, DefaultBehavior, Call, 0);

create_behavior_callable_checker!(db_callmo, DefaultBehavior, CallMemberObject, 2);

///
#[pyclass(module = "ators._ators", frozen)]
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
    Call { callable: db_call::Callable },
    #[pyo3(constructor = (callable))]
    CallMemberObject { callable: db_callmo::Callable },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
}

impl DefaultBehavior {
    ///
    pub(crate) fn default<'py>(
        &self,
        member: &PyRef<'py, super::Member>,
        object: &Bound<'py, crate::core::AtorsBase>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::NoDefault {} => Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "The member {} from {} value is unset and has no default",
                member.name,
                object.repr()?
            ))),
            Self::Static { value } => Ok(value.clone_ref(member.py()).into_bound(member.py())),
            Self::ValidatorDelegate { args, kwargs } => member
                .validator
                .create_default(args.bind(member.py()), kwargs),
            Self::Call { callable } => callable.0.bind(member.py()).call0(),
            Self::CallMemberObject { callable } => {
                callable.0.bind(member.py()).call1((member, object))
            }
            // XXX improve error message since people writing the method may not
            // realize the required signature and we cannot check it at
            // behavior definition time
            // Do it if the call fails only and do it for all relevant behavior
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
                callable: db_call::Callable(callable.0.clone_ref(py)),
            },
            Self::CallMemberObject { callable } => Self::CallMemberObject {
                callable: db_callmo::Callable(callable.0.clone_ref(py)),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}
