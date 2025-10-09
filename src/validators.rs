// Handle type validation, coercion, default value building

///
use pyo3::{
    pyclass,
    types::{PyDict, PyTuple},
    Bound, Py, PyAny, PyResult,
};

mod coercer;
use coercer::Coercer;
mod types;
use types::TypeValidator;
mod values;
use values::ValueValidator;

// NOTE There is no sanity check that value validators make sense in combination
// with the type validator since arbitrary code (member method, object method)
// prevent any truly meaningful validation
#[pyclass]
pub struct Validator {
    type_validator: TypeValidator,
    coercer: Option<Coercer>,
    value_validators: Vec<ValueValidator>,
}

// XXX all validation function should take an option for member and object
impl Validator {
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
            Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "No validator related value exist."
            )))
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
