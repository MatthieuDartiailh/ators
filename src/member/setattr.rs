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
    Bound, Py, PyResult, Python, pyclass,
    types::{PyAny, PyAnyMethods, PyString},
};

create_behavior_callable_checker!(pres_callmov, PreSetattrBehavior, CallMemberObject, 3);

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
    CallMemberObjectValue { callable: pres_callmov::Callable },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
}

impl PreSetattrBehavior {
    ///
    pub(crate) fn pre_set<'py>(
        &self,
        member: &Bound<'py, super::Member>,
        object: &Bound<'py, crate::core::AtorsBase>,
        current: &Option<Py<PyAny>>,
    ) -> PyResult<()> {
        match self {
            Self::NoOp {} => Ok(()),
            Self::Constant {} => Err(pyo3::exceptions::PyTypeError::new_err(
                "Cannot set the value of a constant member",
            )),
            Self::ReadOnly {} => {
                if current.is_some() {
                    Err(pyo3::exceptions::PyTypeError::new_err(
                        "Cannot change the value of an already set read only member",
                    ))
                } else {
                    Ok(())
                }
            }
            Self::CallMemberObjectValue { callable } => {
                let py = member.py();
                println!("Calling");
                callable
                    .0
                    .bind(py)
                    // XXX should use sentinel value
                    .call1((
                        member,
                        object,
                        current.as_ref().unwrap_or(&py.None()).bind(py),
                    ))
                    .map(|_| ())
            }
            Self::ObjectMethod { meth_name } => object
                // XXX should use sentinel value
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
            Self::CallMemberObjectValue { callable } => Self::CallMemberObjectValue {
                callable: pres_callmov::Callable(callable.0.clone_ref(py)),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}

create_behavior_callable_checker!(
    posts_callmoon,
    PostSetattrBehavior,
    CallMemberObjectOldNew,
    4
);

#[pyclass(frozen)]
#[derive(Debug)]
pub enum PostSetattrBehavior {
    #[pyo3(constructor = ())]
    NoOp {},
    #[pyo3(constructor = (callable))]
    CallMemberObjectOldNew { callable: posts_callmoon::Callable },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
}

impl PostSetattrBehavior {
    ///
    pub(crate) fn post_set<'py>(
        &self,
        member: &Bound<'py, super::Member>,
        object: &Bound<'py, crate::core::AtorsBase>,
        old: &Option<Py<PyAny>>,
        new: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        match self {
            Self::NoOp {} => Ok(()),
            Self::CallMemberObjectOldNew { callable } => callable
                .0
                .bind(member.py())
                // XXX should use sentinel value
                .call1((member, object, old, new))
                .map(|_| ()),
            Self::ObjectMethod { meth_name } => object
                // XXX should use sentinel value
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
                callable: posts_callmoon::Callable(callable.0.clone_ref(py)),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}
