/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
use pyo3::{
    Bound, IntoPyObjectExt, Py, PyAny, PyResult, Python, intern, pyclass, pymethods,
    sync::critical_section::with_critical_section,
    types::{PyAnyMethods, PySet, PySetMethods},
};
use std::cell::UnsafeCell;

use crate::{
    class::AtorsBase, containers::common::matches_assignment_context, validators::Validator,
};

#[pyclass(module = "ators._ators", extends=PySet, frozen)]
pub struct AtorsSet {
    validator: UnsafeCell<Validator>,
    member_name: UnsafeCell<Option<String>>,
    // Wrapped in UnsafeCell to allow clearing during GC while keeping the class frozen.
    object: UnsafeCell<Option<Py<AtorsBase>>>,
}

// Safety: validator and member_name are written only once (at construction or during restore
// before any other references exist), and after that are effectively immutable; object is only
// modified during __clear__, which Python's GC calls only once all references to this object
// have been dropped - ensuring no concurrent access (holds for both GIL and free-threaded builds).
unsafe impl Sync for AtorsSet {}

impl AtorsSet {
    pub(crate) fn new_empty<'py>(
        py: Python<'py>,
        validator: Validator,
        member_name: Option<&str>,
        object: Option<Py<AtorsBase>>,
    ) -> PyResult<Bound<'py, AtorsSet>> {
        Bound::new(
            py,
            AtorsSet {
                validator: UnsafeCell::new(validator),
                member_name: UnsafeCell::new(member_name.map(|m| m.to_string())),
                object: UnsafeCell::new(object),
            },
        )
    }

    fn validate_set<'py>(
        &self,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PySet>> {
        if unsafe { pyo3::ffi::PyAnySet_Check(value.as_ptr()) } == 0 {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "Expected a set for validation",
            ));
        }
        // Safety: same as validate_item for AtorsList.
        let validator = unsafe { &*self.validator.get() };
        let m = unsafe { &*self.member_name.get() }.as_deref();
        let mut validated_items = Vec::with_capacity(value.len()?);
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        for item in value.try_iter()? {
            let valid = validator.validate(m, o, &item?)?;
            validated_items.push(valid);
        }
        PySet::new(py, validated_items)
    }

    pub(crate) fn matches_assignment_context<'py>(
        &self,
        member_name: Option<&str>,
        object: Option<&Bound<'py, AtorsBase>>,
    ) -> bool {
        matches_assignment_context(&self.member_name, &self.object, member_name, object)
    }

    pub(crate) fn clone_for_assignment<'py>(
        source: &Bound<'py, AtorsSet>,
    ) -> PyResult<Bound<'py, AtorsSet>> {
        let set = source.get();
        // Safety: same as AtorsList::clone_for_assignment.
        let validator = unsafe { &*set.validator.get() }.clone();
        let member_name = unsafe { &*set.member_name.get() }
            .as_deref()
            .map(|s| s.to_string());
        let object = unsafe { &*set.object.get() }
            .as_ref()
            .map(|object| object.clone_ref(source.py()));
        let aset = AtorsSet::new_empty(source.py(), validator, member_name.as_deref(), object)?;
        // Safety: AtorsSet is declared as `extends=PySet`, so this cast is always valid.
        let py_set = unsafe { source.cast_unchecked::<PySet>() };
        let aset_as_set = aset.cast::<PySet>()?;
        for item in py_set.iter() {
            aset_as_set.add(&item)?;
        }
        Ok(aset)
    }

    /// Restore Ators-specific metadata after unpickling.
    /// Called by `AtorsBase.__setstate__` before writing the container to a slot.
    pub(crate) fn restore<'py>(
        aset: &Bound<'py, AtorsSet>,
        validator: Validator,
        member_name: Option<&str>,
        object: Option<&Bound<'py, AtorsBase>>,
    ) {
        with_critical_section(aset.as_any(), || {
            let inner = aset.get();
            // Safety: we hold the critical section lock. These fields are only written
            // here (during restore) and during construction; after restore they are
            // effectively immutable, matching the normal post-construction invariant.
            unsafe {
                (*inner.validator.get()) = validator;
                (*inner.member_name.get()) = member_name.map(|s| s.to_string());
                (*inner.object.get()) = object.map(|o| o.clone().unbind());
            }
            // Set cannot contain list/set/dict so we do not need to check the value validator to restore the value after unpickling.
        });
        // Set cannot contain list/set/dict (unhashable), so we do not need to
        // check the item validator to restore nested containers.
    }
}

// __isub__, difference_update, __iand__ and intersection_update do not need
// item validation since they remove items
// __ior__, update, __ixor__, symmetric_difference_update and add, need
// item validation since they can add items
#[pymethods]
impl AtorsSet {
    /// Add one value to the set after validating it with the item validator.
    pub fn add<'py>(self_: &Bound<'py, AtorsSet>, value: Bound<'py, PyAny>) -> PyResult<()> {
        let py = value.py();
        // Safety: validator and member_name are effectively immutable; object is not modified
        // while live references exist (see struct-level safety comment).
        let valid = unsafe { &*self_.get().validator.get() }.validate(
            unsafe { &*self_.get().member_name.get() }.as_deref(),
            unsafe { &*self_.get().object.get() }
                .as_ref()
                .map(|o| o.bind(py)),
            &value,
        )?;
        // SAFETY: AtorsSet is declared as `extends=PySet`, so this cast is
        // always valid, and the resulting PySet is valid for calling add.
        let set = unsafe { self_.cast_unchecked::<PySet>() };
        set.add(&valid)
    }

    pub fn __ior__<'py>(self_: &Bound<'py, Self>, value: Bound<'py, PyAny>) -> PyResult<()> {
        let py = value.py();
        let valid = self_.get().validate_set(py, &value)?;
        // SAFETY: AtorsSet is declared as `extends=PySet`, so this cast is
        // always valid, and the resulting PySet is valid for calling add.
        let set = unsafe { self_.cast_unchecked::<PySet>() };
        for item in valid.iter() {
            set.add(&item)?;
        }
        Ok(())
    }

    /// Update the set from `other` after validating each candidate item.
    pub fn update<'py>(self_: &Bound<'py, AtorsSet>, other: Bound<'py, PyAny>) -> PyResult<()> {
        AtorsSet::__ior__(self_, other)
    }

    pub fn __ixor__<'py>(self_: &Bound<'py, Self>, value: &Bound<'py, PyAny>) -> PyResult<()> {
        let py = value.py();
        let valid = self_.get().validate_set(py, value)?;
        let this = self_.cast::<PySet>()?;
        for item in valid.iter() {
            if this.contains(&item)? {
                this.discard(item)?;
            } else {
                this.add(item)?;
            }
        }
        Ok(())
    }

    pub fn symmetric_difference_update<'py>(
        self_: &Bound<'py, AtorsSet>,
        other: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        AtorsSet::__ixor__(self_, other)
    }

    // The traverse method of the parent class (PySet) is called automatically and
    // the type is also traversed so we only need to visit our own references.
    pub fn __traverse__(&self, visit: pyo3::PyVisit) -> Result<(), pyo3::PyTraverseError> {
        // Safety: Python guarantees exclusive access when calling GC methods, ensuring
        // no concurrent mutation (holds for both GIL and free-threaded builds).
        if let Some(o) = unsafe { &*self.object.get() } {
            visit.call(o)?;
        }
        Ok(())
    }

    // The clear method of the parent class (PySet) is called automatically and
    // so we only need to visit our own references.
    pub fn __clear__(&self) {
        // Safety: Python guarantees exclusive access when calling GC methods, ensuring
        // no concurrent mutation (holds for both GIL and free-threaded builds).
        unsafe { *self.object.get() = None };
    }

    #[staticmethod]
    pub fn _construct<'py>(
        py: Python<'py>,
        args: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, AtorsSet>> {
        // This is a dummy constructor used solely for unpickling. It creates an empty AtorsSet
        // without any meaningful metadata; the actual validator and related metadata will be
        // populated by the restore method called from AtorsBase.__setstate__ after construction. Values are restored from the provided iterator.
        use crate::validators::types::TypeValidator;
        let new = Bound::new(
            py,
            AtorsSet {
                validator: UnsafeCell::new(Validator {
                    type_validator: TypeValidator::Any {},
                    value_validators: Box::new([]),
                    coercer: None,
                    init_coercer: None,
                }),
                member_name: UnsafeCell::new(None),
                object: UnsafeCell::new(None),
            },
        )?;
        let temp = unsafe { new.cast_unchecked::<PySet>() };
        for o in args.try_iter()? {
            temp.add(o?)?;
        }
        Ok(new)
    }

    pub fn __reduce_ex__<'py>(
        self_: &Bound<'py, Self>,
        py: Python<'py>,
        _protocol: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        (
            self_.getattr(intern!(py, "_construct"))?,
            (unsafe { self_.cast_unchecked::<PySet>() }.try_iter()?,),
        )
            .into_bound_py_any(py)
    }
}
