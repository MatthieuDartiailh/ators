/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
use pyo3::{Bound, Py, PyAny, PyResult, Python, pyclass, pymethods};
use std::cell::UnsafeCell;

use crate::{class::AtorsBase, validators::Validator};

/// Validation core for `AtorsOrderedDict` (the Python class defined in
/// `python/ators/_containers.py`).
///
/// `AtorsOrderedDict` is defined in Python as
/// `class AtorsOrderedDict(collections.OrderedDict)`, making it a proper
/// `OrderedDict` subclass.  This Rust struct holds the key/value validators
/// and the owner-assignment context so that every mutating operation on the
/// Python container can delegate validation here.
#[pyclass(module = "ators._ators", frozen)]
pub struct AtorsOrderedDictCore {
    pub(crate) key_validator: Validator,
    pub(crate) value_validator: Validator,
    pub(crate) member_name: Option<String>,
    // Wrapped in UnsafeCell to allow clearing during GC while keeping the class frozen.
    pub(crate) object: UnsafeCell<Option<Py<AtorsBase>>>,
}

// Safety: key_validator, value_validator, and member_name are immutable after construction;
// object is only modified during __clear__, which Python's GC calls only once all references
// to this object have been dropped — ensuring no concurrent access (holds for both GIL and
// free-threaded builds).
unsafe impl Sync for AtorsOrderedDictCore {}

impl AtorsOrderedDictCore {
    /// Create a new `AtorsOrderedDictCore` with the given validators and context.
    pub(crate) fn new_empty<'py>(
        py: Python<'py>,
        key_validator: Validator,
        value_validator: Validator,
        member_name: Option<&str>,
        object: Option<Py<AtorsBase>>,
    ) -> PyResult<Bound<'py, AtorsOrderedDictCore>> {
        Bound::new(
            py,
            AtorsOrderedDictCore {
                key_validator,
                value_validator,
                member_name: member_name.map(|m| m.to_string()),
                object: UnsafeCell::new(object),
            },
        )
    }

    /// Return `true` if this core matches the given assignment context.
    pub(crate) fn matches_assignment_context<'py>(
        &self,
        member_name: Option<&str>,
        object: Option<&Bound<'py, AtorsBase>>,
    ) -> bool {
        self.member_name.as_deref() == member_name
            && match (unsafe { &*self.object.get() }.as_ref(), object) {
                (None, None) => true,
                (Some(stored), Some(current)) => {
                    stored.bind(current.py()).as_ptr() == current.as_ptr()
                }
                _ => false,
            }
    }
}

#[pymethods]
impl AtorsOrderedDictCore {
    /// Validate and return a key.
    pub fn validate_key<'py>(
        &self,
        key: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = key.py();
        let m = self.member_name.as_deref();
        // Safety: object is only written during __clear__, which can only run after all
        // live references to this object are gone. A live reference is required to call
        // this method, so __clear__ cannot run concurrently (holds for both GIL and
        // free-threaded builds).
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        self.key_validator.validate(m, o, &key)
    }

    /// Validate and return a value.
    pub fn validate_value<'py>(
        &self,
        value: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = value.py();
        let m = self.member_name.as_deref();
        // Safety: same as validate_key.
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        self.value_validator.validate(m, o, &value)
    }

    /// Validate a key-value pair and return `(valid_key, valid_value)`.
    pub fn validate_item<'py>(
        &self,
        key: Bound<'py, PyAny>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        let py = key.py();
        let m = self.member_name.as_deref();
        // Safety: same as validate_key.
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        let valid_key = self.key_validator.validate(m, o, &key)?;
        let valid_value = self.value_validator.validate(m, o, &value)?;
        Ok((valid_key, valid_value))
    }

    // The type is also traversed by Python's GC so we only need to visit our own references.
    pub fn __traverse__(&self, visit: pyo3::PyVisit) -> Result<(), pyo3::PyTraverseError> {
        // Safety: Python guarantees exclusive access when calling GC methods, ensuring
        // no concurrent mutation (holds for both GIL and free-threaded builds).
        if let Some(o) = unsafe { &*self.object.get() } {
            visit.call(o)?;
        }
        Ok(())
    }

    pub fn __clear__(&self) {
        // Safety: Python guarantees exclusive access when calling GC methods, ensuring
        // no concurrent mutation (holds for both GIL and free-threaded builds).
        unsafe { *self.object.get() = None };
    }
}
