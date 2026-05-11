/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
/// Core descriptor class defining Ators events and related utilities.
use crate::{
    class::base::{
        AtorsBase, instance_is_observable, is_frozen, notifications_enabled, notify_member_change,
    },
    validators::{TypeValidator, Validator, ValueValidator},
};
use pyo3::{
    Bound, IntoPyObjectExt, Py, PyAny, PyRef, PyRefMut, PyResult, Python, pyclass, pymethods,
    types::{
        PyAnyMethods, PyDict, PyDictMethods, PyGenericAlias, PyString, PyTuple,
        PyType,
    },
};
use std::{clone::Clone, collections::HashMap};

use crate::utils::err_with_cause;

/// Helper function to clone event metadata (same pattern as for members).
fn clone_metadata(
    metadata: &Option<HashMap<String, Py<PyAny>>>,
) -> Option<HashMap<String, Py<PyAny>>> {
    Python::attach(|py| {
        metadata.as_ref().map(|hm| {
            hm.iter()
                .map(|(k, v)| (k.clone(), v.clone_ref(py)))
                .collect()
        })
    })
}

/// A write-only Python descriptor that defines an event on an Ators class.
///
/// Events validate assigned values, emit observer notifications on successful
/// set, and never store the assigned value.  Events require the owning object
/// to be observable and have notifications enabled; they cannot be set on
/// frozen objects.
#[pyclass(module = "ators._ators", frozen, get_all)]
#[derive(Debug)]
pub struct Event {
    pub name: String,
    validator: Validator,
    // Optional metadata dictionary.
    metadata: Option<HashMap<String, Py<PyAny>>>,
}

impl Event {
    /// Access the validator for this event.
    pub fn validator(&self) -> &Validator {
        &self.validator
    }

    /// Access the metadata for this event.
    pub fn metadata(&self) -> &Option<HashMap<String, Py<PyAny>>> {
        &self.metadata
    }
}

/// Cold path: validation failure for events.
#[cold]
fn validate_event_set_failed<'py>(
    py: Python<'py>,
    event: &PyRef<'py, Event>,
    object: &Bound<'py, AtorsBase>,
    err: pyo3::PyErr,
) -> PyResult<pyo3::PyErr> {
    Ok(err_with_cause(
        py,
        pyo3::PyErr::from_type(
            err.get_type(py),
            format!(
                "Validation failed for event '{}' of {}",
                event.name,
                object.repr()?,
            ),
        ),
        err,
    ))
}

#[pymethods]
impl Event {
    /// Descriptor get — always raises AttributeError on instance access (write-only).
    /// Returns the descriptor itself on class access (object is None).
    pub fn __get__<'py>(
        self_: PyRef<'py, Self>,
        object: &Bound<'py, PyAny>,
        _objtype: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if object.is_none() {
            // Class access: return the descriptor.
            return self_.into_bound_py_any(object.py());
        }
        Err(pyo3::exceptions::PyAttributeError::new_err(format!(
            "Event '{}' is write-only and cannot be read.",
            self_.name
        )))
    }

    /// Descriptor set — validates, then notifies observers (value is not stored).
    pub fn __set__<'py>(
        self_: PyRef<'py, Self>,
        object: &Bound<'py, PyAny>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = self_.py();
        let object = object.cast::<AtorsBase>()?;

        // Reject writes to frozen objects before any validation.
        if is_frozen(object) {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Cannot set event '{}' on frozen object {}.",
                self_.name,
                object.repr()?
            )));
        }

        // Events are only meaningful when notifications can fire.
        if !instance_is_observable(object) || !notifications_enabled(object) {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Cannot set event '{}': notifications are not enabled on {}.",
                self_.name,
                object.repr()?
            )));
        }

        // Validate the value.
        let new = match self_
            .validator
            .validate(Some(&self_.name), Some(object), value)
        {
            Ok(v) => v,
            Err(err) => return Err(validate_event_set_failed(py, &self_, object, err)?),
        };

        // Notify observers — no value is stored.
        notify_member_change(object, &self_.name, py.None(), new.unbind())?;

        Ok(())
    }

    /// Descriptor delete — always raises AttributeError (events cannot be deleted).
    pub fn __delete__(&self, _object: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(pyo3::exceptions::PyAttributeError::new_err(
            "Event descriptors do not support deletion.",
        ))
    }

    /// GC traversal for metadata values.
    pub fn __traverse__(&self, visit: pyo3::PyVisit) -> Result<(), pyo3::PyTraverseError> {
        if let Some(m) = &self.metadata {
            for (_k, v) in m.iter() {
                visit.call(v)?
            }
        }
        Ok(())
    }

    /// Enable `Event[T]` subscription syntax.
    #[classmethod]
    pub fn __class_getitem__<'py>(
        cls: &Bound<'py, PyType>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = cls.py();
        // Wrap the subscription item in a tuple for GenericAlias. Arity
        // validation (exactly 1 arg) is deferred to the metaclass so that
        // wrong-arity annotations inside a class body produce a clear error.
        let alias_args = match item.cast::<PyTuple>() {
            Ok(t) => t.to_owned(),
            Err(_) => PyTuple::new(py, [item])?,
        };
        Ok(PyGenericAlias::new(py, cls.as_any(), alias_args.as_any())?.into_any())
    }
}

// ─── EventBuilder ────────────────────────────────────────────────────────────

/// Builder class for Event that allows ergonomic declaration of events
/// in the class body.
#[pyclass(module = "ators._ators", name = "event", from_py_object)]
#[derive(Debug, Default)]
pub struct EventBuilder {
    /// Name resolved by the metaclass.
    pub name: Option<String>,
    type_validator: Option<TypeValidator>,
    value_validators: Option<Vec<ValueValidator>>,
    metadata: Option<HashMap<String, Py<PyAny>>>,
    pub inherit: bool,
}

#[pymethods]
impl EventBuilder {
    /// Create a new event builder.
    ///
    /// Does not accept `default`, `default_factory`, pre/post get/set hooks,
    /// or any other member-only capability.
    #[new]
    pub fn py_new() -> Self {
        EventBuilder::default()
    }

    /// Mark this event as inheriting from the declaration on a parent class.
    pub fn inherit<'py>(mut self_: PyRefMut<'py, Self>) -> PyResult<PyRefMut<'py, Self>> {
        self_.inherit = true;
        Ok(self_)
    }

    /// Attach arbitrary metadata to this event.
    #[pyo3(signature = (**tags))]
    pub fn tag<'py>(
        mut self_: PyRefMut<'py, Self>,
        tags: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        if self_.metadata.is_none() {
            self_.metadata = Some(HashMap::with_capacity(tags.map(|d| d.len()).unwrap_or(0)));
        }
        if let Some(tags) = tags
            && let Some(d) = &mut self_.metadata
        {
            d.extend(tags.iter().map(|(k, v)| {
                (
                    k.extract()
                        .expect("Tag keys are strings by construction, unwrap is safe"),
                    v.unbind(),
                )
            }));
        }
        Ok(self_)
    }

    /// Append a value validator to this event.
    pub fn append_value_validator<'py>(
        mut self_: PyRefMut<'py, Self>,
        value_validator: Bound<'py, crate::validators::ValueValidator>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let vv: ValueValidator = value_validator.as_any().extract()?;
        if let Some(vvs) = &mut self_.value_validators {
            vvs.push(vv);
        } else {
            self_.value_validators = Some(vec![vv]);
        }
        Ok(self_)
    }
}

impl EventBuilder {
    /// Whether this builder requests inheritance from a base-class event.
    #[inline]
    pub fn should_inherit(&self) -> bool {
        self.inherit
    }

    #[inline]
    pub fn type_validator(&self) -> Option<&TypeValidator> {
        self.type_validator.as_ref()
    }

    #[inline]
    pub fn value_validators(&self) -> Option<&Vec<ValueValidator>> {
        self.value_validators.as_ref()
    }

    #[inline]
    pub fn metadata(&self) -> &Option<HashMap<String, Py<PyAny>>> {
        &self.metadata
    }

    #[inline]
    pub fn set_type_validator(&mut self, tv: TypeValidator) {
        self.type_validator = Some(tv);
    }

    #[inline]
    pub fn take_value_validators(&mut self) -> Option<Vec<ValueValidator>> {
        self.value_validators.take()
    }

    #[inline]
    pub fn set_value_validators(&mut self, v: Vec<ValueValidator>) {
        self.value_validators = Some(v);
    }

    /// Populate unset fields from an existing `Event` instance (inherit mode).
    pub fn get_inherited_behavior_from_event(&mut self, event: &Event) {
        if self.type_validator.is_none() {
            self.type_validator = Some(event.validator.type_validator.clone());
        }
        if self.value_validators.is_none() {
            self.value_validators = Some(event.validator.value_validators.to_vec());
        }
        if self.metadata.is_none() {
            self.metadata = clone_metadata(&event.metadata);
        }
    }

    /// Finalize the builder and construct an `Event` descriptor.
    pub fn build(self, type_name: &Bound<'_, PyString>) -> PyResult<Event> {
        let Some(name) = self.name else {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Cannot build event belonging to {type_name} without an assigned name."
            )));
        };
        Ok(Event {
            name,
            validator: Validator::new(
                self.type_validator.unwrap_or(TypeValidator::Any {}),
                self.value_validators,
                None,
                None,
            ),
            metadata: self.metadata,
        })
    }
}

impl Clone for EventBuilder {
    fn clone(&self) -> Self {
        Python::attach(|py| EventBuilder {
            name: self.name.clone(),
            type_validator: self.type_validator.clone(),
            value_validators: self.value_validators.clone(),
            metadata: self.metadata.as_ref().map(|hm| {
                hm.iter()
                    .map(|(k, v)| (k.clone(), v.clone_ref(py)))
                    .collect()
            }),
            inherit: self.inherit,
        })
    }
}

// ─── EventCustomizationTool ───────────────────────────────────────────────────

/// Tool for customizing event builders during `__init_subclass__`.
///
/// Mirrors `MemberCustomizationTool` but for events.  Obtained via
/// `get_event_customization_tool(cls)` inside a `__init_subclass__` body.
#[pyclass(module = "ators._ators")]
pub struct EventCustomizationTool {
    events: HashMap<String, Option<Py<EventBuilder>>>,
}

impl EventCustomizationTool {
    pub fn new(event_names: impl IntoIterator<Item = String>) -> Self {
        EventCustomizationTool {
            events: event_names.into_iter().map(|n| (n, None)).collect(),
        }
    }

    /// Drain all builders that have been configured and return them.
    pub fn get_builders<'py>(
        &mut self,
        py: Python<'py>,
    ) -> impl Iterator<Item = (String, EventBuilder)> {
        self.events.iter_mut().filter_map(move |(k, v)| {
            v.take().map(|v| {
                (
                    k.clone(),
                    v.extract(py).expect(
                        "EventBuilder was constructed internally so extraction cannot fail",
                    ),
                )
            })
        })
    }
}

#[pymethods]
impl EventCustomizationTool {
    /// Access an event builder for the given event name.
    pub fn __getitem__<'py>(
        mut self_: PyRefMut<'py, Self>,
        name: &str,
    ) -> PyResult<Bound<'py, EventBuilder>> {
        if !self_.events.contains_key(name) {
            return Err(pyo3::exceptions::PyKeyError::new_err(format!(
                "No event named '{name}' to customize."
            )));
        }
        let py = self_.py();
        let entry = self_
            .events
            .get_mut(name)
            .expect("Key is known to be in map")
            .get_or_insert_with(|| Py::new(py, EventBuilder::default()).unwrap())
            .bind(py);
        Ok(Bound::clone(entry))
    }
}
