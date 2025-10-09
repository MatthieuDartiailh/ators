///
use pyo3::prelude::PyAnyMethods;
use pyo3::{pyclass, types::PyAny, Bound, PyResult};

///
#[pyclass(frozen)]
#[derive(Clone)]
pub enum PreSetattrBehavior {
    #[pyo3(constructor = ())]
    NoOp {},
    #[pyo3(constructor = ())]
    Constant {},
    #[pyo3(constructor = ())]
    ReadOnly {},
    #[pyo3(constructor = (meth_name))]
    MemberMethod { meth_name: String },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: String },
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
            Self::MemberMethod { meth_name } => member
                .call_method1(meth_name, (object, current))
                .map(|_| ()),
            Self::ObjectMethod { meth_name } => object
                .call_method1(meth_name, (member, current))
                .map(|_| ()),
        };
        Ok(())
    }
}

#[pyclass(frozen)]
#[derive(Clone)]
pub enum PostSetattrBehavior {
    #[pyo3(constructor = ())]
    NoOp {},
    #[pyo3(constructor = (meth_name))]
    MemberMethod { meth_name: String },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: String },
}

impl PostSetattrBehavior {
    ///
    // new is unvalidated at this stage
    pub(crate) fn post_set<'py>(
        &self,
        member: &Bound<'py, super::Member>,
        object: &Bound<'py, crate::core::BaseAtors>,
        old: &Bound<'py, PyAny>,
        new: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        match self {
            Self::NoOp {} => Ok(()),
            Self::MemberMethod { meth_name } => member
                .call_method1(meth_name, (object, old, new))
                .map(|_| ()),
            Self::ObjectMethod { meth_name } => object
                .call_method1(meth_name, (member, old, new))
                .map(|_| ()),
        };
        Ok(())
    }
}
