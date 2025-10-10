///
use pyo3::{
    Bound, Py, PyResult, Python, pyclass,
    types::PyString,
    types::{PyAny, PyAnyMethods},
};

///
#[pyclass(frozen)]
pub enum PreGetattrBehavior {
    #[pyo3(constructor = ())]
    NoOp {},
    #[pyo3(constructor = (callable))]
    CallMemberObject { callable: Py<PyAny> },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
}

impl PreGetattrBehavior {
    ///
    // new is unvalidated at this stage
    pub(crate) fn pre_get<'py>(
        &self,
        member: &Bound<'py, super::Member>,
        object: &Bound<'py, crate::core::BaseAtors>,
    ) -> PyResult<()> {
        match self {
            Self::NoOp {} => Ok(()),
            Self::CallMemberObject { callable } => callable
                .bind(member.py())
                .call1((member, object))
                .map(|_| ()),
            Self::ObjectMethod { meth_name } => {
                object.call_method1(meth_name, (member,)).map(|_| ())
            }
        }
    }
}

impl Clone for PreGetattrBehavior {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            Self::NoOp {} => Self::NoOp {},
            Self::CallMemberObject { callable } => Self::CallMemberObject {
                callable: callable.clone_ref(py),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}

#[pyclass(frozen)]
pub enum PostGetattrBehavior {
    #[pyo3(constructor = ())]
    NoOp {},
    #[pyo3(constructor = (callable))]
    CallMemberObjectValue { callable: Py<PyAny> },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
}

impl PostGetattrBehavior {
    ///
    // Value cannot be modified this is a design choice
    pub(crate) fn post_get<'py>(
        &self,
        member: &Bound<'py, super::Member>,
        object: &Bound<'py, crate::core::BaseAtors>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        match self {
            Self::NoOp {} => Ok(()),
            Self::CallMemberObjectValue { callable } => callable
                .bind(member.py())
                .call1((member, object, value))
                .map(|_| ()),
            Self::ObjectMethod { meth_name } => {
                object.call_method1(meth_name, (member, value)).map(|_| ())
            }
        }
    }
}

impl Clone for PostGetattrBehavior {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            Self::NoOp {} => Self::NoOp {},
            Self::CallMemberObjectValue { callable } => Self::CallMemberObjectValue {
                callable: callable.clone_ref(py),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}
