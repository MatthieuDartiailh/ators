/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use crate::utils::create_behavior_callable_checker;
use pyo3::{
    Bound, Py, PyRef, PyResult, Python, pyclass,
    types::PyString,
    types::{PyAny, PyAnyMethods},
};

create_behavior_callable_checker!(preg_callmo, PreGetattrBehavior, CallNameObject, 2);

///
#[pyclass(module = "ators._ators", frozen)]
#[derive(Debug)]
pub enum PreGetattrBehavior {
    #[pyo3(constructor = ())]
    NoOp {},
    #[pyo3(constructor = (callable))]
    CallNameObject { callable: preg_callmo::Callable },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
}

impl PreGetattrBehavior {
    ///
    // new is unvalidated at this stage
    pub(crate) fn pre_get<'py>(
        &self,
        member: &PyRef<'py, super::Member>,
        object: &Bound<'py, crate::core::AtorsBase>,
    ) -> PyResult<()> {
        match self {
            Self::NoOp {} => Ok(()),
            Self::CallNameObject { callable } => callable
                .0
                .bind(member.py())
                .call1((&member.name, object))
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
            Self::CallNameObject { callable } => Self::CallNameObject {
                callable: preg_callmo::Callable(callable.0.clone_ref(py)),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}

create_behavior_callable_checker!(postg_callmov, PreGetattrBehavior, CallNameObjectValue, 3);

#[pyclass(module = "ators._ators", frozen)]
#[derive(Debug)]
pub enum PostGetattrBehavior {
    #[pyo3(constructor = ())]
    NoOp {},
    #[pyo3(constructor = (callable))]
    CallNameObjectValue { callable: postg_callmov::Callable },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
}

impl PostGetattrBehavior {
    ///
    // Value cannot be modified this is a design choice
    pub(crate) fn post_get<'py>(
        &self,
        member: &PyRef<'py, super::Member>,
        object: &Bound<'py, crate::core::AtorsBase>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        match self {
            Self::NoOp {} => Ok(()),
            Self::CallNameObjectValue { callable } => callable
                .0
                .bind(member.py())
                .call1((&member.name, object, value))
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
            Self::CallNameObjectValue { callable } => Self::CallNameObjectValue {
                callable: postg_callmov::Callable(callable.0.clone_ref(py)),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}
