/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Matthieu C. Dartiailh
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{
    Bound, Py, PyAny, PyResult, Python, pyclass,
    types::{PyAnyMethods, PyFrozenSet, PyFrozenSetMethods, PyString},
};

#[pyclass(frozen)]
#[derive(Debug)]
pub enum ValueValidator {
    #[pyo3(constructor = (values))]
    Enum { values: Py<PyFrozenSet> },
    #[pyo3(constructor = (items))]
    TupleItems { items: Vec<Vec<ValueValidator>> },
    #[pyo3(constructor = (item))]
    SequenceItems { item: Vec<ValueValidator> },
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
        match self {
            Self::Enum { values } => {
                if values
                    .bind(value.py())
                    .contains(value)
                    .unwrap_or(false)
                {
                    Ok(())
                } else {
                    Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "Value {} not in enum {}",
                        value.repr()?,
                        values.bind(value.py()).repr()?
                    )))
                }
            }
            Self::TupleItems { items } => {
                // The number of items is checked by the type validator and
                // the validator ensure this value validator is only ever used
                // with the appropriate type validator
                for (item_res, item_validators) in value.try_iter()?.zip(items.iter()) {
                    let item = item_res?;
                    for item_validator in item_validators.iter() {
                        item_validator.validate_value(member, object, &item)?
                    };
                }
                Ok(())
            }
            Self::SequenceItems { item } => {
                for el_res in value.try_iter()? {
                    let el = el_res?;
                    for ival in item.iter() {
                        ival.validate_value(member, object, &el)?;
                    }
                }
                Ok(())
            }
            Self::CallMemberObjectValue { callable } => callable
                .bind(value.py())
                .call1(
                    (
                        member.ok_or(pyo3::exceptions::PyRuntimeError::new_err(
                            "Cannot use CallMemberObjectValue validation when validator is not linked to a member."
                        ))?,
                        object.ok_or(
                            pyo3::exceptions::PyTypeError::new_err(
                                "Cannot use CallMemberObjectValue validation when validator is not linked to a member."
                            )
                        )?,
                        value,
                    ),
                )
                .map(|_| ()),
            Self::ObjectMethod { meth_name } => object
                .ok_or(pyo3::exceptions::PyTypeError::new_err(
                    "Cannot use ObjectMethod validation when validator is not linked to a member.",
                ))?
                .call_method1(meth_name, (member.ok_or(pyo3::exceptions::PyRuntimeError::new_err(
                    "Cannot use ObjectMethod validation when validator is not linked to a member."
                ))?, value))
                .map(|_| ()),
        }
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
                item: item.to_vec(),
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
