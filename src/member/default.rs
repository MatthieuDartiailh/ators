///
use pyo3::{
    pyclass,
    types::{PyAnyMethods, PyDict, PyString, PyTuple},
    Bound, Py, PyAny, PyResult, Python,
};

///
#[pyclass(frozen)]
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
    CallObject { callable: Py<PyAny> },
    #[pyo3(constructor = (meth_name))]
    MemberMethod { meth_name: Py<PyString> },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
}

impl DefaultBehavior {
    ///
    pub(crate) fn default<'py>(
        &self,
        member: &Bound<'py, super::Member>,
        object: &Bound<'py, crate::core::BaseAtors>,
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
            Self::CallObject { callable } => callable.bind(member.py()).call0(),
            Self::MemberMethod { meth_name } => member.call_method1(meth_name, (object,)),
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
