/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
/// Validator structs managing type and value validation and performing
/// coercion if necessary
use pyo3::{
    Bound, Py, PyAny, PyResult, Python, pyclass, pymethods,
    types::{PyDict, PyTuple},
};

mod coercer;
pub use coercer::Coercer;
pub(crate) mod types;
pub use types::TypeValidator;
mod values;
pub(crate) use values::ValidValues;
pub use values::ValueValidator;

// FIXME pub visibility is required to alter coercion behaviors (for Union),
// may want a specific API later
// NOTE There is no sanity check that value validators make sense in combination
// with the type validator since arbitrary code (member method, object method)
// prevent any truly meaningful validation
#[pyclass(module = "ators._ators", frozen)]
#[derive(Debug)]
pub struct Validator {
    pub type_validator: TypeValidator,
    pub value_validators: Box<[ValueValidator]>,
    pub coercer: Option<Coercer>,
    pub init_coercer: Option<Coercer>,
}

#[pymethods]
impl Validator {
    #[new]
    pub fn new(
        type_validator: TypeValidator,
        value_validators: Option<Vec<ValueValidator>>,
        coercer: Option<Coercer>,
        init_coercer: Option<Coercer>,
    ) -> Self {
        Self {
            type_validator,
            value_validators: value_validators
                .map(|v| v.into_boxed_slice())
                .unwrap_or_else(|| Box::new([])),
            coercer,
            init_coercer,
        }
    }

    fn new_with_extra_value_validators(&self, extra: Vec<ValueValidator>) -> PyResult<Validator> {
        Ok(Validator {
            type_validator: self.type_validator.clone(),
            value_validators: [&self.value_validators, extra.as_slice()]
                .concat()
                .into_boxed_slice(),
            coercer: self.coercer.clone(),
            init_coercer: self.init_coercer.clone(),
        })
    }

    #[getter]
    fn get_type_validator(&self) -> TypeValidator {
        self.type_validator.clone()
    }

    #[getter]
    fn get_value_validators(&self) -> Vec<ValueValidator> {
        self.value_validators.to_vec()
    }

    #[getter]
    fn get_coercer(&self) -> Option<Coercer> {
        self.coercer.clone()
    }

    #[getter]
    fn get_init_coercer(&self) -> Option<Coercer> {
        self.coercer.clone()
    }
}

impl Validator {
    /// Validate the value against the type and value validators, with coercion
    /// if validation fails and a coercer is defined
    pub fn validate<'py>(
        &self,
        name: Option<&str>,
        object: Option<&Bound<'py, crate::core::AtorsBase>>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self.strict_validate(name, object, value) {
            Ok(v) => Ok(v),
            Err(err) => {
                if let Some(c) = &self.coercer {
                    c.coerce_value(false, &self.type_validator, name, object, value)
                } else {
                    Err(err)
                }
            }
        }
    }

    /// Create a default value using the type validator
    pub fn create_default<'py>(
        &self,
        args: &Bound<'py, PyTuple>,
        kwargs: &Option<Py<PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.type_validator.create_default(args, kwargs)
    }

    /// Coerce the value if a coercer is defined, otherwise return an error
    pub fn coerce_value<'py>(
        &self,
        is_init: bool,
        member_name: Option<&str>,
        object: Option<&Bound<'py, crate::core::AtorsBase>>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if is_init && let Some(c) = &self.init_coercer {
            c.coerce_value(is_init, &self.type_validator, member_name, object, value)
        } else if !is_init && let Some(c) = &self.coercer {
            c.coerce_value(is_init, &self.type_validator, member_name, object, value)
        } else {
            Err(pyo3::exceptions::PyTypeError::new_err(
                "No coercer defined for {:?}",
            ))
        }
    }

    /// Validate the value against the type and value validators, without coercion
    fn strict_validate<'py>(
        &self,
        member_name: Option<&str>,
        object: Option<&Bound<'py, crate::core::AtorsBase>>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let v = self
            .type_validator
            .validate_type(member_name, object, value)?;
        for vv in &self.value_validators {
            vv.validate_value(member_name, object, &v)?;
        }
        Ok(v)
    }
}

impl Validator {
    /// Clone and set the owner of the type validator which is used for ForwardRef resolution
    pub(crate) fn with_owner(&self, py: Python<'_>, owner: &Bound<'_, PyAny>) -> Self {
        Self {
            type_validator: self.type_validator.with_owner(py, owner),
            value_validators: self.value_validators.clone(),
            coercer: self.coercer.clone(),
            init_coercer: self.init_coercer.clone(),
        }
    }
}

impl Clone for Validator {
    fn clone(&self) -> Self {
        Self {
            type_validator: self.type_validator.clone(),
            value_validators: self.value_validators.iter().cloned().collect(),
            coercer: self.coercer.clone(),
            init_coercer: self.init_coercer.clone(),
        }
    }
}

impl Default for Validator {
    fn default() -> Self {
        Validator {
            type_validator: TypeValidator::Any {},
            value_validators: Box::new([]),
            coercer: None,
            init_coercer: None,
        }
    }
}
