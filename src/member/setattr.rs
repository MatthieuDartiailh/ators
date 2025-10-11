/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Matthieu C. Dartiailh
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{
    Bound, Py, PyResult, Python, pyclass,
    types::{PyAny, PyAnyMethods, PyString},
};

///
#[pyclass(frozen)]
#[derive(Debug)]
pub enum PreSetattrBehavior {
    #[pyo3(constructor = ())]
    NoOp {},
    #[pyo3(constructor = ())]
    Constant {},
    #[pyo3(constructor = ())]
    ReadOnly {},
    #[pyo3(constructor = (callable))]
    CallMemberObject { callable: Py<PyAny> },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
}

impl PreSetattrBehavior {
    ///
    pub(crate) fn pre_set<'py>(
        &self,
        member: &Bound<'py, super::Member>,
        object: &Bound<'py, crate::core::BaseAtors>,
        current: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        match self {
            Self::NoOp {} => Ok(()),
            Self::Constant {} => Err(pyo3::exceptions::PyTypeError::new_err(
                "Cannot set the value of a constant member",
            )),
            Self::ReadOnly {} => {
                if object
                    .borrow()
                    .is_slot_set(member.borrow().slot_index as usize)
                {
                    Err(pyo3::exceptions::PyTypeError::new_err(
                        "Cannot change the value of a read only member",
                    ))
                } else {
                    Ok(())
                }
            }
            Self::CallMemberObject { callable } => callable
                .bind(member.py())
                .call1((member, object, current))
                .map(|_| ()),
            Self::ObjectMethod { meth_name } => object
                .call_method1(meth_name, (member, current))
                .map(|_| ()),
        }
    }
}

impl Clone for PreSetattrBehavior {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            Self::NoOp {} => Self::NoOp {},
            Self::Constant {} => Self::Constant {},
            Self::ReadOnly {} => Self::ReadOnly {},
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
#[derive(Debug)]
pub enum PostSetattrBehavior {
    #[pyo3(constructor = ())]
    NoOp {},
    #[pyo3(constructor = (callable))]
    CallMemberObjectOldNew { callable: Py<PyAny> },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
}

impl PostSetattrBehavior {
    ///
    pub(crate) fn post_set<'py>(
        &self,
        member: &Bound<'py, super::Member>,
        object: &Bound<'py, crate::core::BaseAtors>,
        old: &Bound<'py, PyAny>,
        new: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        match self {
            Self::NoOp {} => Ok(()),
            Self::CallMemberObjectOldNew { callable } => callable
                .bind(member.py())
                .call1((member, object, old, new))
                .map(|_| ()),
            Self::ObjectMethod { meth_name } => object
                .call_method1(meth_name, (member, old, new))
                .map(|_| ()),
        }
    }
}

impl Clone for PostSetattrBehavior {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            Self::NoOp {} => Self::NoOp {},
            Self::CallMemberObjectOldNew { callable } => Self::CallMemberObjectOldNew {
                callable: callable.clone_ref(py),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}
