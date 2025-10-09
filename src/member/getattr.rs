///
use pyo3::{
    pyclass,
    types::{PyAny, PyAnyMethods},
    Bound, IntoPyObject, PyResult,
};

///
#[pyclass(frozen)]
#[derive(Clone)]
pub enum PreGetattrBehavior {
    #[pyo3(constructor = ())]
    NoOp {},
    #[pyo3(constructor = (meth_name))]
    MemberMethod { meth_name: String },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: String },
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
            Self::MemberMethod { meth_name } => member
                .into_pyobject(object.py())?
                .call_method1(meth_name, (object,))
                .map(|_| ()),
            Self::ObjectMethod { meth_name } => {
                object.call_method1(meth_name, (member,)).map(|_| ())
            }
        }
    }
}

#[pyclass(frozen)]
#[derive(Clone)] // using Py<PyString> would be possible by using a custom Clone impl
pub enum PostGetattrBehavior {
    #[pyo3(constructor = ())]
    NoOp {},
    #[pyo3(constructor = (meth_name))]
    MemberMethod { meth_name: String },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: String },
}

impl PostGetattrBehavior {
    ///
    // new is unvalidated at this stage
    pub(crate) fn post_get<'py>(
        &self,
        member: &Bound<'py, super::Member>,
        object: &Bound<'py, crate::core::BaseAtors>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        match self {
            Self::NoOp {} => Ok(()),
            // XXX Those cannot modify the value is this desirable
            // My gut feeling is that it is indeed better (you can record stuff, you cannot lie)
            Self::MemberMethod { meth_name } => {
                member.call_method1(meth_name, (object, value)).map(|_| ())
            }
            Self::ObjectMethod { meth_name } => {
                object.call_method1(meth_name, (member, value)).map(|_| ())
            }
        }
    }
}
