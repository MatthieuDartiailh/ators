/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Matthieu C. Dartiailh
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/

///
use pyo3::{
    Bound, Py, PyAny, PyResult, pyclass, pymethods,
    types::{PyDict, PyTuple},
};

mod coercer;
pub use coercer::Coercer;
mod types;
pub use types::TypeValidator;
mod values;
pub use values::ValueValidator;

// NOTE There is no sanity check that value validators make sense in combination
// with the type validator since arbitrary code (member method, object method)
// prevent any truly meaningful validation
#[pyclass(frozen)]
#[derive(Debug)]
pub struct Validator {
    type_validator: TypeValidator,
    value_validators: Box<[ValueValidator]>,
    coercer: Option<Coercer>,
}

#[pymethods]
impl Validator {
    #[new]
    fn new(
        type_validator: TypeValidator,
        value_validators: Option<Vec<ValueValidator>>,
        coercer: Option<Coercer>,
    ) -> Self {
        Self {
            type_validator,
            value_validators: value_validators
                .map(|v| v.into_boxed_slice())
                .unwrap_or_else(|| Box::new([])),
            coercer,
        }
    }

    fn new_with_extra_value_validators(&self, extra: Vec<ValueValidator>) -> PyResult<Validator> {
        Ok(Validator {
            type_validator: self.type_validator.clone(),
            value_validators: [&self.value_validators, extra.as_slice()]
                .concat()
                .into_boxed_slice(),
            coercer: self.coercer.clone(),
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
}

impl Validator {
    ///
    pub fn with_type_validator(self, type_validator: TypeValidator) -> Self {
        Validator {
            type_validator,
            value_validators: self.value_validators,
            coercer: self.coercer,
        }
    }

    ///
    pub fn with_value_validators(self, value_validators: Box<[ValueValidator]>) -> Self {
        Validator {
            type_validator: self.type_validator,
            value_validators,
            coercer: self.coercer,
        }
    }

    ///
    pub fn with_appended_value_validator(self, value_validator: ValueValidator) -> Self {
        Validator {
            type_validator: self.type_validator,
            value_validators: {
                [self.value_validators.iter().as_slice(), &[value_validator]]
                    .concat()
                    .into_boxed_slice()
            },
            coercer: self.coercer,
        }
    }

    ///
    pub fn with_coercer(self, coercer: Option<Coercer>) -> Self {
        Validator {
            type_validator: self.type_validator,
            value_validators: self.value_validators,
            coercer,
        }
    }

    ///
    pub fn value_validators(&self) -> &Box<[ValueValidator]> {
        &self.value_validators
    }

    ///
    pub fn validate<'py>(
        &self,
        member: Option<&Bound<'py, crate::member::Member>>,
        object: Option<&Bound<'py, crate::core::BaseAtors>>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self.strict_validate(member, object, value.clone()) {
            Ok(v) => Ok(v),
            Err(err) => {
                if let Some(c) = &self.coercer {
                    c.coerce_value(&self.type_validator, member, object, value)
                } else {
                    Err(err)
                }
            }
        }
    }

    ///
    pub fn create_default<'py>(
        &self,
        args: &Bound<'py, PyTuple>,
        kwargs: &Option<Py<PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.type_validator.create_default(args, kwargs)
    }

    ///
    pub fn coerce_value<'py>(
        &self,
        member: Option<&Bound<'py, crate::member::Member>>,
        object: Option<&Bound<'py, crate::core::BaseAtors>>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if let Some(c) = &self.coercer {
            let current = c.coerce_value(&self.type_validator, member, object, value)?;
            Ok(current)
        } else {
            Err(pyo3::exceptions::PyTypeError::new_err(
                "No coercer defined for {:?}",
            ))
        }
    }

    ///
    fn strict_validate<'py>(
        &self,
        member: Option<&Bound<'py, crate::member::Member>>,
        object: Option<&Bound<'py, crate::core::BaseAtors>>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let v = self.type_validator.validate_type(member, object, value)?;
        for vv in &self.value_validators {
            vv.validate_value(member, object, &v)?;
        }
        Ok(v)
    }
}

impl Clone for Validator {
    fn clone(&self) -> Self {
        Self {
            type_validator: self.type_validator.clone(),
            value_validators: self.value_validators.iter().cloned().collect(),
            coercer: self.coercer.clone(),
        }
    }
}

impl Default for Validator {
    fn default() -> Self {
        Validator {
            type_validator: TypeValidator::Any {},
            value_validators: Box::new([]),
            coercer: None,
        }
    }
}
