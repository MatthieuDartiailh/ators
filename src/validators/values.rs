///
use pyo3::{
    Bound, Py, PyAny, PyResult, Python, pyclass,
    types::{PyFrozenSet, PyString},
};

#[pyclass(frozen)]
pub enum ValueValidator {
    #[pyo3(constructor = (values))]
    Enum { values: Py<PyFrozenSet> },
    #[pyo3(constructor = (items))]
    TupleItems { items: Vec<ValueValidator> },
    #[pyo3(constructor = (item))]
    SequenceItems { item: Py<ValueValidator> },
    #[pyo3(constructor = (callable))]
    CallMemberObjectValue { callable: Py<PyAny> },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: Py<PyString> },
    // #[pyo3(constructor = (min, max))]
    // Range { min: f64, max: f64 },
    // #[pyo3(constructor = (options))]
    // Options { options: Vec<Py<PyAny>> },
}

impl ValueValidator {
    pub fn validate_value<'py>(
        &self,
        member: Option<&Bound<'py, crate::member::Member>>,
        object: Option<&Bound<'py, crate::core::BaseAtors>>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        Ok(()) // XXX
    }
}

impl Clone for ValueValidator {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            Self::Enum { values } => Self::Enum {
                values: values.clone_ref(py),
            },
            Self::TupleItems { items } => Self::TupleItems {
                items: items.to_vec(),
            },
            Self::SequenceItems { item } => Self::SequenceItems {
                item: item.clone_ref(py),
            },
            Self::CallMemberObjectValue { callable } => Self::CallMemberObjectValue {
                callable: callable.clone_ref(py),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}
