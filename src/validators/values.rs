/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{
    Bound, FromPyObject, IntoPyObject, Py, PyAny, PyResult, Python, pyclass,
    types::{
        PyAnyMethods, PyFrozenSet, PyFrozenSetMethods, PySet, PySetMethods, PyString, PyTypeMethods,
    },
};

use crate::utils::create_behavior_callable_checker;
use std::convert::Infallible;

create_behavior_callable_checker!(vv_callv, ValueValidator, CallValue, 1);
create_behavior_callable_checker!(vv_callmov, ValueValidator, CallMemberObjectValue, 3);

#[derive(Debug)]
pub(crate) struct ValidValues(pub Py<PyFrozenSet>);

impl FromPyObject<'_> for ValidValues {
    fn extract_bound<'py>(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        let py = ob.py();
        if let Ok(fs) = ob.cast::<PyFrozenSet>() {
            Ok(ValidValues(fs.clone().unbind()))
        } else if let Ok(s) = ob.cast::<PySet>() {
            Ok(ValidValues(PyFrozenSet::new(py, s.iter())?.unbind()))
        } else {
            Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Expected a 'set' or 'frozenset' for a ValueValidator.Enum, got {}",
                ob.get_type().name()?
            )))
        }
    }
}

impl<'py> IntoPyObject<'py> for &ValidValues {
    type Target = PyFrozenSet;
    type Output = Bound<'py, PyFrozenSet>;
    type Error = Infallible;
    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        Ok(self.0.clone_ref(py).into_bound(py))
    }
}

#[pyclass(frozen)]
#[derive(Debug)]
pub enum ValueValidator {
    #[pyo3(constructor = (values))]
    // ValidValues is build from a Python object and we do not need to expose
    // it directly since it is not needed to build an Enum variant from the
    // Python side.
    #[allow(private_interfaces)]
    Enum { values: ValidValues },
    #[pyo3(constructor = (items))]
    TupleItems { items: Vec<Vec<ValueValidator>> },
    #[pyo3(constructor = (item))]
    SequenceItems { item: Vec<ValueValidator> },
    #[pyo3(constructor = (callable))]
    CallValue { callable: vv_callv::Callable },
    #[pyo3(constructor = (callable))]
    CallMemberObjectValue { callable: vv_callmov::Callable },
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
        object: Option<&Bound<'py, crate::core::AtorsBase>>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        match self {
            Self::Enum { values } => {
                if values
                    .0.bind(value.py())
                    .contains(value)
                    .unwrap_or(false)
                {
                    Ok(())
                } else {
                    Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "Value {} not in permitted list {}",
                        value.repr()?,
                        values.0.bind(value.py()).repr()?
                    )))
                }
            }
            Self::TupleItems { items } => {
                // The number of items is checked by the type validator and
                // the validator ensure this value validator is only ever used
                // with the appropriate type validator
                let py = value.py();
                for (index, (item_res, item_validators)) in value.try_iter()?.zip(items.iter()).enumerate() {
                    let item = item_res?;
                    for item_validator in item_validators.iter() {
                        item_validator.validate_value(member, object, &item)
                            .map_err(|err| {
                            let new = pyo3::exceptions::PyValueError::new_err(
                                format!("Failed to validate item {index} of {value}.")
                            );
                            new.set_cause(py, Some(err));
                            new
                        })?;
                    };
                }
                Ok(())
            }
            Self::SequenceItems { item } => {
                let py = value.py();
                for (index,el_res) in value.try_iter()?.enumerate() {
                    let el = el_res?;
                    for  ival in item.iter() {
                        ival.validate_value(member, object, &el)
                            .map_err(|err| {
                            let new = pyo3::exceptions::PyValueError::new_err(
                                format!("Failed to validate item {index} of {value}.")
                            );
                            new.set_cause(py, Some(err));
                            new
                        })?;
                    }
                }
                Ok(())
            }
            Self::CallValue { callable } => callable
                .0.bind(value.py())
                .call1(
                    (
                        value,
                    ),
                )
                .map(|_| ()),
            Self::CallMemberObjectValue { callable } => callable
                .0.bind(value.py())
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
                values: ValidValues(values.0.clone_ref(py)),
            },
            Self::TupleItems { items } => Self::TupleItems {
                items: items.to_vec(),
            },
            Self::SequenceItems { item } => Self::SequenceItems {
                item: item.to_vec(),
            },
            Self::CallValue { callable } => Self::CallValue {
                callable: vv_callv::Callable(callable.0.clone_ref(py)),
            },
            Self::CallMemberObjectValue { callable } => Self::CallMemberObjectValue {
                callable: vv_callmov::Callable(callable.0.clone_ref(py)),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}
