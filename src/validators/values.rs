/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
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
create_behavior_callable_checker!(vv_callmov, ValueValidator, CallNameObjectValue, 3);

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

#[pyclass(module = "ators._ators", frozen)]
#[derive(Debug)]
pub enum ValueValidator {
    #[pyo3(constructor = (values))]
    // ValidValues is build from a Python object and we do not need to expose
    // it directly since it is not needed to build an Enum variant from the
    // Python side.
    #[allow(private_interfaces)]
    Values { values: ValidValues },
    #[pyo3(constructor = (callable))]
    CallValue { callable: vv_callv::Callable },
    #[pyo3(constructor = (callable))]
    CallNameObjectValue { callable: vv_callmov::Callable },
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
        name: Option<&str>,
        object: Option<&Bound<'py, crate::core::AtorsBase>>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        match self {
            Self::Values { values } => {
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
            Self::CallValue { callable } => callable
                .0.bind(value.py())
                .call1(
                    (
                        value,
                    ),
                )
                .map(|_| ()),
            Self::CallNameObjectValue { callable } => callable
                .0.bind(value.py())
                .call1(
                    (
                        name.ok_or(pyo3::exceptions::PyRuntimeError::new_err(
                            "Cannot use CallNameObjectValue validation when validator is not linked to a member."
                        ))?,
                        object.ok_or(
                            pyo3::exceptions::PyTypeError::new_err(
                                "Cannot use CallNameObjectValue validation when validator is not linked to a member."
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
                .call_method1(meth_name, (name.ok_or(pyo3::exceptions::PyRuntimeError::new_err(
                    "Cannot use ObjectMethod validation when validator is not linked to a member."
                ))?, value))
                .map(|_| ()),
        }
    }
}

impl Clone for ValueValidator {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            Self::Values { values } => Self::Values {
                values: ValidValues(values.0.clone_ref(py)),
            },
            Self::CallValue { callable } => Self::CallValue {
                callable: vv_callv::Callable(callable.0.clone_ref(py)),
            },
            Self::CallNameObjectValue { callable } => Self::CallNameObjectValue {
                callable: vv_callmov::Callable(callable.0.clone_ref(py)),
            },
            Self::ObjectMethod { meth_name } => Self::ObjectMethod {
                meth_name: meth_name.clone_ref(py),
            },
        })
    }
}
