/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
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
    ///
    pub fn validate<'py>(
        &self,
        member: Option<&Bound<'py, crate::member::Member>>,
        object: Option<&Bound<'py, crate::core::AtorsBase>>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // NOTE not sure how to avoid cloning somewhere in the call chain if not here
        // We are only cloning a reference so the cost should be minimal
        match self.strict_validate(member, object, Bound::clone(&value)) {
            Ok(v) => Ok(v),
            Err(err) => {
                if let Some(c) = &self.coercer {
                    c.coerce_value(false, &self.type_validator, member, object, &value)
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
        is_init: bool,
        member: Option<&Bound<'py, crate::member::Member>>,
        object: Option<&Bound<'py, crate::core::AtorsBase>>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if is_init && let Some(c) = &self.init_coercer {
            c.coerce_value(is_init, &self.type_validator, member, object, value)
        } else if !is_init && let Some(c) = &self.coercer {
            c.coerce_value(is_init, &self.type_validator, member, object, value)
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
        object: Option<&Bound<'py, crate::core::AtorsBase>>,
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
