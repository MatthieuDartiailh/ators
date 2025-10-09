///
use pyo3::{Bound, Py, PyAny, PyResult, pyclass, types::PyFrozenSet};

#[pyclass]
pub enum ValueValidator {
    #[pyo3(constructor = (values))]
    Enum { values: Py<PyFrozenSet> },
    #[pyo3(constructor = (items))]
    TupleItems { items: Vec<Py<ValueValidator>> },
    #[pyo3(constructor = (item))]
    SequenceItems { item: Py<ValueValidator> },
    #[pyo3(constructor = (meth_name))]
    MemberMethod { meth_name: String },
    #[pyo3(constructor = (meth_name))]
    ObjectMethod { meth_name: String },
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
