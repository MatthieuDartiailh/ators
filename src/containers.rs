/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
/// Container types with validation and related utilities.
use pyo3::{
    Bound, IntoPyObjectExt, Py, PyAny, PyErr, PyRef, PyResult, Python, ffi, intern, pyclass,
    pymethods,
    sync::critical_section::with_critical_section,
    types::{
        PyAnyMethods, PyDict, PyDictMethods, PyList, PyListMethods, PySet, PySetMethods, PySlice,
        PySliceMethods, PyTuple,
    },
};
use std::cell::UnsafeCell;

use crate::{core::AtorsBase, utils::error_on_minusone, validators::Validator};

#[pyclass(module = "ators._ators", extends=PyList, frozen)]
pub struct AtorsList {
    validator: UnsafeCell<Validator>,
    member_name: UnsafeCell<Option<String>>,
    // Wrapped in UnsafeCell to allow clearing during GC while keeping the class frozen.
    object: UnsafeCell<Option<Py<AtorsBase>>>,
}

// Safety: validator and member_name are written only once (at construction or during restore
// before any other references exist), and after that are effectively immutable; object is only
// modified during __clear__, which Python's GC calls only once all references to this object
// have been dropped — ensuring no concurrent access (holds for both GIL and free-threaded builds).
unsafe impl Sync for AtorsList {}

impl AtorsList {
    pub(crate) fn new_empty<'py>(
        py: Python<'py>,
        validator: Validator,
        member_name: Option<&str>,
        object: Option<Py<AtorsBase>>,
    ) -> PyResult<Bound<'py, AtorsList>> {
        Bound::new(
            py,
            AtorsList {
                validator: UnsafeCell::new(validator),
                member_name: UnsafeCell::new(member_name.map(|m| m.to_string())),
                object: UnsafeCell::new(object),
            },
        )
    }

    fn validate_item<'py>(
        &self,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Safety: validator and member_name are written only once (at construction or restore)
        // and are effectively immutable during normal use. A live reference is required to call
        // this method, ensuring no concurrent restoration is occurring.
        let validator = unsafe { &*self.validator.get() };
        let m = unsafe { &*self.member_name.get() }.as_deref();
        // Safety: object is only written during __clear__, which can only run after all
        // live references to this object are gone. A live reference is required to call
        // this method, so __clear__ cannot run concurrently (holds for both GIL and
        // free-threaded builds).
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        validator.validate(m, o, value)
    }

    fn validate_iterable<'py>(
        &self,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        // Safety: same as validate_item.
        let validator = unsafe { &*self.validator.get() };
        let m = unsafe { &*self.member_name.get() }.as_deref();
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        let mut validated_items = Vec::with_capacity(value.len().unwrap_or(0));
        for item in value.try_iter()? {
            let valid = validator.validate(m, o, &item?)?;
            validated_items.push(valid);
        }
        PyList::new(py, validated_items)
    }

    pub(crate) fn matches_assignment_context<'py>(
        &self,
        member_name: Option<&str>,
        object: Option<&Bound<'py, AtorsBase>>,
    ) -> bool {
        // Safety: same as validate_item.
        unsafe { &*self.member_name.get() }.as_deref() == member_name
            && match (unsafe { &*self.object.get() }.as_ref(), object) {
                (None, None) => true,
                (Some(stored), Some(current)) => {
                    stored.bind(current.py()).as_ptr() == current.as_ptr()
                }
                _ => false,
            }
    }

    pub(crate) fn clone_for_assignment<'py>(
        source: &Bound<'py, AtorsList>,
    ) -> PyResult<Bound<'py, AtorsList>> {
        let list = source.get();
        // Safety: same as validate_item.
        let validator = unsafe { &*list.validator.get() }.clone();
        let member_name = unsafe { &*list.member_name.get() }
            .as_deref()
            .map(|s| s.to_string());
        let object = unsafe { &*list.object.get() }
            .as_ref()
            .map(|object| object.clone_ref(source.py()));
        let alist = AtorsList::new_empty(source.py(), validator, member_name.as_deref(), object)?;
        // Safety: AtorsList is declared as `extends=PyList`, so this cast is always valid.
        let py_list = unsafe { source.cast_unchecked::<PyList>() };
        let alist_as_list = alist.cast::<PyList>()?;
        for item in py_list.iter() {
            alist_as_list.append(&item)?;
        }
        Ok(alist)
    }

    /// Restore Ators-specific metadata after unpickling.
    /// Called by `AtorsBase.__setstate__` before writing the container to a slot.
    pub(crate) fn restore<'py>(
        alist: &Bound<'py, AtorsList>,
        validator: Validator,
        member_name: Option<&str>,
        object: Option<&Bound<'py, AtorsBase>>,
    ) {
        use crate::validators::types::TypeValidator;

        // Capture the item validator before the critical section so we can use it
        // afterwards to restore any nested containers within the list.
        let item_v = match &validator.type_validator {
            TypeValidator::List {
                item: Some(item_bv),
            } => Some((*item_bv.0).clone()),
            _ => None,
        };

        with_critical_section(alist.as_any(), || {
            let inner = alist.get();
            // Safety: we hold the critical section lock. These fields are only written
            // here (during restore) and during construction; after restore they are
            // effectively immutable, matching the normal post-construction invariant.
            unsafe {
                (*inner.validator.get()) = validator;
                (*inner.member_name.get()) = member_name.map(|s| s.to_string());
                (*inner.object.get()) = object.map(|o| o.clone().unbind());
            }
        });

        // Restore any nested containers within the list items.
        if let Some(item_validator) = item_v {
            // Safety: AtorsList is declared as `extends=PyList`, so this cast is always valid.
            let py_list = unsafe { alist.cast_unchecked::<PyList>() };
            for list_item in py_list.iter() {
                match &item_validator.type_validator {
                    TypeValidator::List {
                        item: Some(nested_bv),
                    } => {
                        if let Ok(nested) = list_item.cast::<AtorsList>() {
                            AtorsList::restore(nested, (*nested_bv.0).clone(), member_name, object);
                        }
                    }
                    TypeValidator::Dict {
                        items: Some((key_bv, val_bv)),
                    } => {
                        if let Ok(nested) = list_item.cast::<AtorsDict>() {
                            AtorsDict::restore(
                                nested,
                                (*key_bv.0).clone(),
                                (*val_bv.0).clone(),
                                member_name,
                                object,
                            );
                        }
                    }
                    // Set cannot contain list/set/dict (unhashable), no nested restore needed.
                    _ => {}
                }
            }
        }
    }
}

// remove, pop, clear, sort, reverse and __imul__ do not need
// item validation since they only remove or rearrange existing items.
// append, insert, __setitem__, extend and __iadd__ need item validation
// since they can add new items.
#[pymethods]
impl AtorsList {
    // Required since CPython uses a single slot for setitem/delitem which prevents
    // inheriting the delitem behavior from PyList when __setitem__ is overridden.
    pub fn __delitem__<'py>(
        self_: &Bound<'py, AtorsList>,
        index: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = index.py();
        // SAFETY: AtorsList is declared as `extends=PyList`, so this unchecked cast
        // is valid and gives us access to high-level PyList helpers.
        let list = unsafe { self_.cast_unchecked::<PyList>() };

        if index.is_instance_of::<PySlice>() {
            // SAFETY: instance check guarantees this is a PySlice.
            let slice = unsafe { index.cast_unchecked::<PySlice>() };
            let slice_indices = slice.indices(list.len() as isize)?;

            if slice_indices.step == 1 {
                // Contiguous slice: replace the slice range with an empty list.
                list.del_slice(slice_indices.start as usize, slice_indices.stop as usize)?;
            } else {
                // Extended slice: delete in reverse order so earlier indices stay valid.
                // XXX this could be done without allocation
                let start = slice_indices.start;
                let step = slice_indices.step;
                let slicelength = slice_indices.slicelength;
                // Collect indices to delete, then remove from largest to smallest.
                let mut indices: Vec<isize> = (0..slicelength)
                    .map(|i| start + i as isize * step)
                    .collect();
                // Sort descending so deleting one index doesn't shift the others.
                indices.sort_unstable_by(|a, b| b.cmp(a));
                for idx in indices {
                    list.del_item(idx);
                }
            }
        } else {
            // Single integer index.
            let idx = index.extract::<isize>()?;
            let len = list.len() as isize;
            let normalized = if idx < 0 { idx + len } else { idx };
            if normalized < 0 || normalized >= len {
                return Err(pyo3::exceptions::PyIndexError::new_err(
                    "list assignment index out of range",
                ));
            }
            // Above check make casting safe
            list.del_item(normalized as usize)?;
        }
        Ok(())
    }

    pub fn append<'py>(self_: &Bound<'py, AtorsList>, value: &Bound<'py, PyAny>) -> PyResult<()> {
        let py = value.py();
        let valid = self_.get().validate_item(py, value)?;
        // SAFETY: AtorsList is declared as `extends=PyList`, so this cast is
        // always valid, and the resulting PyList is valid for calling append.
        unsafe { self_.cast_unchecked::<PyList>() }.append(&valid)
    }

    pub fn insert<'py>(
        self_: &Bound<'py, AtorsList>,
        index: usize,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = value.py();
        let valid = self_.get().validate_item(py, value)?;
        // SAFETY: AtorsList is declared as `extends=PyList`, so this cast is
        // always valid, and the resulting PyList is valid for calling append.
        unsafe { self_.cast_unchecked::<PyList>() }.insert(index, &valid)
    }

    pub fn __setitem__<'py>(
        self_: &Bound<'py, AtorsList>,
        index: &Bound<'py, PyAny>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = index.py();

        // Cast once to PyList (AtorsList extends PyList). Use unchecked cast to avoid
        // an extra runtime check and to get access to PyList helper methods.
        let list = unsafe { self_.cast_unchecked::<PyList>() };

        // Slice assignment path
        if index.is_instance_of::<PySlice>() {
            // SAFETY cast validity guaranteed by instance check
            let slice = unsafe { index.cast_unchecked::<PySlice>() };
            // Use high-level indices normalization (handles negative indices)
            let slice_indices = slice.indices(list.len() as isize)?;

            let validated_list = self_.get().validate_iterable(py, value)?;

            // contiguous slice replacement fast-path
            if slice_indices.step == 1 {
                // When step == 1, PySlice::indices guarantees start >= 0 and stop >= 0,
                // so the casts to usize are safe.
                list.set_slice(
                    slice_indices.start as usize,
                    slice_indices.stop as usize,
                    validated_list.as_any(),
                )?;
                return Ok(());
            }

            // Extended-slice assignment: lengths must match
            if validated_list.len() != slice_indices.slicelength {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "attempt to assign sequence of size {} to extended slice of size {}",
                    validated_list.len(),
                    slice_indices.slicelength
                )));
            }

            // Assign element-by-element using high-level set_item.
            // Works for both positive and negative step values.
            // PySlice::indices guarantees that for each i in 0..slicelength the value
            // `start + i * step` is a valid list index (0 <= idx < len), so the cast
            // to usize is safe.
            let start = slice_indices.start;
            let step = slice_indices.step;
            for (i, item) in validated_list.iter().enumerate() {
                let target_idx = (start + (i as isize) * step) as usize;
                list.set_item(target_idx, item.as_any())?;
            }

            return Ok(());
        }

        // Non-slice: single-index assignment
        // Validate the new value under critical section
        let valid = self_.get().validate_item(py, value)?;

        // Convert index to integer using PyO3's extract (honours __index__/index-like subclasses)
        let idx = index.as_any().extract::<isize>()?;

        // Normalize negative indices relative to list length
        let len = list.len() as isize;
        let normalized = if idx < 0 { idx + len } else { idx };
        if normalized < 0 || normalized >= len {
            return Err(pyo3::exceptions::PyIndexError::new_err(
                "list assignment index out of range",
            ));
        }

        // Use high-level set_item (no ffi), conversion is safe since normalized is > 0
        list.set_item(normalized as usize, valid.as_any())?;
        Ok(())
    }

    pub fn extend<'py>(self_: &Bound<'py, AtorsList>, other: &Bound<'py, PyAny>) -> PyResult<()> {
        let valid = with_critical_section(self_.as_any(), || {
            self_.get().validate_iterable(other.py(), other)
        })?;
        let list = unsafe { self_.cast_unchecked::<PyList>() };
        unsafe {
            error_on_minusone(
                self_.py(),
                ffi::compat::PyList_Extend(list.as_ptr(), valid.as_ptr()),
            )
        }
    }

    pub fn __iadd__<'py>(self_: &Bound<'py, Self>, value: &Bound<'py, PyAny>) -> PyResult<()> {
        AtorsList::extend(self_, value)
    }

    // The traverse method of the parent class (PyList) is called automatically and
    // the type is also traversed so we only need to visit our own references.
    pub fn __traverse__(&self, visit: pyo3::PyVisit) -> Result<(), pyo3::PyTraverseError> {
        // Safety: Python guarantees exclusive access when calling GC methods, ensuring
        // no concurrent mutation (holds for both GIL and free-threaded builds).
        if let Some(o) = unsafe { &*self.object.get() } {
            visit.call(o)?;
        }
        Ok(())
    }

    // The clear method of the parent class (PyList) is called automatically and
    // so we only need to visit our own references.
    pub fn __clear__(&self) {
        // Safety: Python guarantees exclusive access when calling GC methods, ensuring
        // no concurrent mutation (holds for both GIL and free-threaded builds).
        unsafe { *self.object.get() = None };
    }

    #[staticmethod]
    pub fn _construct<'py>(
        py: Python<'py>,
        _args: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, AtorsList>> {
        // This is a dummy constructor used solely for unpickling. It creates an empty AtorsList
        // without any meaningful metadata; the actual validator and related metadata will be
        // populated by the restore method called from AtorsBase.__setstate__ after construction.
        use crate::validators::types::TypeValidator;
        Bound::new(
            py,
            AtorsList {
                validator: UnsafeCell::new(Validator {
                    type_validator: TypeValidator::Any {},
                    value_validators: Box::new([]),
                    coercer: None,
                    init_coercer: None,
                }),
                member_name: UnsafeCell::new(None),
                object: UnsafeCell::new(None),
            },
        )
    }

    pub fn __reduce_ex__<'py>(
        self_: &Bound<'py, Self>,
        py: Python<'py>,
        _protocol: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        (
            self_.getattr(intern!(py, "_construct"))?,
            (py.None(),),
            py.None(),
            unsafe { self_.cast_unchecked::<PyList>() }.try_iter()?,
        )
            .into_bound_py_any(py)
    }
}

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
// have been dropped — ensuring no concurrent access (holds for both GIL and free-threaded builds).
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
        // Safety: same as AtorsList::validate_item.
        unsafe { &*self.member_name.get() }.as_deref() == member_name
            && match (unsafe { &*self.object.get() }.as_ref(), object) {
                (None, None) => true,
                (Some(stored), Some(current)) => {
                    stored.bind(current.py()).as_ptr() == current.as_ptr()
                }
                _ => false,
            }
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

    //
    #[staticmethod]
    pub fn _construct<'py>(
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
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
        for o in args.get_item(0)?.try_iter()? {
            temp.add(o?)?;
        }
        Ok(new)
    }

    //
    pub fn __reduce_ex__<'py>(
        self_: &Bound<'py, Self>,
        py: Python<'py>,
        _protocol: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        (
            self_.getattr(intern!(py, "_construct"))?,
            (),
            unsafe { self_.cast_unchecked::<PySet>() }.try_iter()?,
        )
            .into_bound_py_any(py)
    }
}

#[pyclass(module = "ators._ators", extends=PyDict, frozen)]
pub struct AtorsDict {
    key_validator: UnsafeCell<Validator>,
    value_validator: UnsafeCell<Validator>,
    member_name: UnsafeCell<Option<String>>,
    // Wrapped in UnsafeCell to allow clearing during GC while keeping the class frozen.
    object: UnsafeCell<Option<Py<AtorsBase>>>,
}

// Safety: key_validator, value_validator, and member_name are written only once (at construction
// or during restore before any other references exist), and after that are effectively immutable;
// object is only modified during __clear__, which Python's GC calls only once all references
// to this object have been dropped — ensuring no concurrent access (holds for both GIL and
// free-threaded builds).
unsafe impl Sync for AtorsDict {}

impl AtorsDict {
    pub(crate) fn new_empty<'py>(
        py: Python<'py>,
        key_validator: Validator,
        value_validator: Validator,
        member_name: Option<&str>,
        object: Option<Py<AtorsBase>>,
    ) -> PyResult<Bound<'py, AtorsDict>> {
        Bound::new(
            py,
            AtorsDict {
                key_validator: UnsafeCell::new(key_validator),
                value_validator: UnsafeCell::new(value_validator),
                member_name: UnsafeCell::new(member_name.map(|m| m.to_string())),
                object: UnsafeCell::new(object),
            },
        )
    }

    /// Validate a key using the key_validator
    fn validate_key<'py>(
        &self,
        py: Python<'py>,
        key: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Safety: same as AtorsList::validate_item.
        let key_validator = unsafe { &*self.key_validator.get() };
        let m = unsafe { &*self.member_name.get() }.as_deref();
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        key_validator.validate(m, o, &key)
    }

    /// Validate a value for insertion into the dict
    fn validate_value<'py>(
        &self,
        py: Python<'py>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Safety: same as AtorsList::validate_item.
        let value_validator = unsafe { &*self.value_validator.get() };
        let m = unsafe { &*self.member_name.get() }.as_deref();
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        value_validator.validate(m, o, &value)
    }

    /// Validate both key and value for insertion into the dict
    fn validate_item<'py>(
        &self,
        py: Python<'py>,
        key: Bound<'py, PyAny>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        // Safety: same as AtorsList::validate_item.
        let key_validator = unsafe { &*self.key_validator.get() };
        let value_validator = unsafe { &*self.value_validator.get() };
        let m = unsafe { &*self.member_name.get() }.as_deref();
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        let valid_key = key_validator.validate(m, o, &key)?;
        let valid_value = value_validator.validate(m, o, &value)?;
        Ok((valid_key, valid_value))
    }

    pub(crate) fn matches_assignment_context<'py>(
        &self,
        member_name: Option<&str>,
        object: Option<&Bound<'py, AtorsBase>>,
    ) -> bool {
        // Safety: same as AtorsList::validate_item.
        unsafe { &*self.member_name.get() }.as_deref() == member_name
            && match (unsafe { &*self.object.get() }.as_ref(), object) {
                (None, None) => true,
                (Some(stored), Some(current)) => {
                    stored.bind(current.py()).as_ptr() == current.as_ptr()
                }
                _ => false,
            }
    }

    pub(crate) fn clone_for_assignment<'py>(
        source: &Bound<'py, AtorsDict>,
    ) -> PyResult<Bound<'py, AtorsDict>> {
        let dict = source.get();
        // Safety: same as AtorsList::clone_for_assignment.
        let key_validator = unsafe { &*dict.key_validator.get() }.clone();
        let value_validator = unsafe { &*dict.value_validator.get() }.clone();
        let member_name = unsafe { &*dict.member_name.get() }
            .as_deref()
            .map(|s| s.to_string());
        let object = unsafe { &*dict.object.get() }
            .as_ref()
            .map(|object| object.clone_ref(source.py()));
        let adict = AtorsDict::new_empty(
            source.py(),
            key_validator,
            value_validator,
            member_name.as_deref(),
            object,
        )?;
        // Safety: AtorsDict is declared as `extends=PyDict`, so this cast is always valid.
        let py_dict = unsafe { source.cast_unchecked::<PyDict>() };
        let adict_as_dict = adict.cast::<PyDict>()?;
        for (k, v) in py_dict.iter() {
            adict_as_dict.set_item(&k, &v)?;
        }
        Ok(adict)
    }

    /// Restore Ators-specific metadata after unpickling.
    /// Called by `AtorsBase.__setstate__` before writing the container to a slot.
    pub(crate) fn restore<'py>(
        adict: &Bound<'py, AtorsDict>,
        key_validator: Validator,
        value_validator: Validator,
        member_name: Option<&str>,
        object: Option<&Bound<'py, AtorsBase>>,
    ) {
        use crate::validators::types::TypeValidator;

        // Capture the value validator type before the critical section for nested restore.
        let value_v = value_validator.clone();

        with_critical_section(adict.as_any(), || {
            // SAFETY: AtorsDict is declared as `extends=PyDict`, so this cast is always valid.
            let pydict = unsafe { adict.cast_unchecked::<PyDict>() };
            match &value_validator.type_validator {
                TypeValidator::List { item: Some(item_v) } => {
                    for (_, v) in pydict.iter() {
                        AtorsList::restore(
                            unsafe { v.cast_unchecked::<AtorsList>() },
                            (*item_v.0).clone(),
                            member_name,
                            object,
                        )
                    }
                }
                TypeValidator::Set { item: Some(item_v) } => {
                    for (_, v) in pydict.iter() {
                        AtorsSet::restore(
                            unsafe { v.cast_unchecked::<AtorsSet>() },
                            (*item_v.0).clone(),
                            member_name,
                            object,
                        )
                    }
                }
                TypeValidator::Dict {
                    items: Some((key_v, val_v)),
                } => {
                    for (_, v) in pydict.iter() {
                        AtorsDict::restore(
                            unsafe { v.cast_unchecked::<AtorsDict>() },
                            (*key_v.0).clone(),
                            (*val_v.0).clone(),
                            member_name,
                            object,
                        )
                    }
                }
                _ => {}
            }
            let inner = adict.get();
            // Safety: we hold the critical section lock. These fields are only written
            // here (during restore) and during construction; after restore they are
            // effectively immutable, matching the normal post-construction invariant.
            unsafe {
                (*inner.key_validator.get()) = key_validator;
                (*inner.value_validator.get()) = value_validator;
                (*inner.member_name.get()) = member_name.map(|s| s.to_string());
                (*inner.object.get()) = object.map(|o| o.clone().unbind());
            }
        });

        // Restore any nested containers in the dict values.
        // Safety: AtorsDict is declared as `extends=PyDict`, so this cast is always valid.
        let py_dict = unsafe { adict.cast_unchecked::<PyDict>() };
        match &value_v.type_validator {
            TypeValidator::List {
                item: Some(item_bv),
            } => {
                for (_, v) in py_dict.iter() {
                    if let Ok(nested) = v.cast::<AtorsList>() {
                        AtorsList::restore(nested, (*item_bv.0).clone(), member_name, object);
                    }
                }
            }
            TypeValidator::Set {
                item: Some(item_bv),
            } => {
                for (_, v) in py_dict.iter() {
                    if let Ok(nested) = v.cast::<AtorsSet>() {
                        AtorsSet::restore(nested, (*item_bv.0).clone(), member_name, object);
                    }
                }
            }
            TypeValidator::Dict {
                items: Some((key_bv, val_bv)),
            } => {
                for (_, v) in py_dict.iter() {
                    if let Ok(nested) = v.cast::<AtorsDict>() {
                        AtorsDict::restore(
                            nested,
                            (*key_bv.0).clone(),
                            (*val_bv.0).clone(),
                            member_name,
                            object,
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

fn dict_set_item<'py>(
    py: Python<'py>,
    dict: *mut ffi::PyObject,
    key: Bound<'py, PyAny>,
    value: Bound<'py, PyAny>,
) -> PyResult<()> {
    // Use direct PyDict C API to avoid converting into bound to cast to PyDict
    crate::utils::error_on_minusone(py, unsafe {
        ffi::PyDict_SetItem(dict, key.as_ptr(), value.as_ptr())
    })
}

#[pymethods]
impl AtorsDict {
    pub fn __setitem__<'py>(
        self_: PyRef<'py, AtorsDict>,
        key: Bound<'py, PyAny>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = key.py();
        let (valid_key, valid_value) = self_.validate_item(py, key, value)?;
        // Use direct PyDict C API to avoid converting into bound to cast to PyDict
        dict_set_item(py, self_.as_ptr(), valid_key, valid_value)
    }

    #[pyo3(signature = (other=None, **kwargs))]
    pub fn update<'py>(
        self_: PyRef<'py, AtorsDict>,
        other: Option<&Bound<'py, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let py = self_.py();
        if let Some(o) = other {
            // Shortcut for dicts for which we can safely iterate over
            if let Ok(od) = o.cast::<PyDict>() {
                for (k, v) in od.iter() {
                    let (valid_key, valid_value) = self_.validate_item(self_.py(), k, v)?;
                    dict_set_item(py, self_.as_ptr(), valid_key, valid_value)?;
                }
            }
            // Handle object providing keys() method
            else if o.hasattr(intern!(self_.py(), "keys"))? {
                let keys = o.call_method0(intern!(self_.py(), "keys"))?;
                for key in keys.try_iter()? {
                    let k = key?;
                    let v = o
                        .getattr(intern!(self_.py(), "__getitem__"))?
                        .call1((&k,))?;
                    let (valid_key, valid_value) = self_.validate_item(self_.py(), k, v)?;
                    dict_set_item(py, self_.as_ptr(), valid_key, valid_value)?;
                }
            }
            // Handle iterable of key-value pairs
            else {
                for t in o.try_iter()? {
                    let (k, v) = t?.extract::<(Bound<'py, PyAny>, Bound<'py, PyAny>)>()?;
                    let (valid_key, valid_value) = self_.validate_item(self_.py(), k, v)?;
                    dict_set_item(py, self_.as_ptr(), valid_key, valid_value)?;
                }
            }
        }

        // Handle keyword arguments
        if let Some(kw) = kwargs {
            for (k, v) in kw.iter() {
                let (valid_key, valid_value) = self_.validate_item(self_.py(), k, v)?;
                dict_set_item(py, self_.as_ptr(), valid_key, valid_value)?;
            }
        }

        Ok(())
    }

    pub fn setdefault<'py>(
        self_: PyRef<'py, AtorsDict>,
        key: Bound<'py, PyAny>,
        default: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = key.py();
        let valid_key = self_.validate_key(py, key)?;
        // Use direct PyDict C API to avoid converting into bound to cast to PyDict
        // Such a casting would consume the ref and we cannot clone it.
        if let Some(existing) = {
            let mut result: *mut ffi::PyObject = std::ptr::null_mut();
            // Safety: All pointers are valid Python objects
            match unsafe {
                ffi::compat::PyDict_GetItemRef(self_.as_ptr(), valid_key.as_ptr(), &mut result)
            } {
                std::ffi::c_int::MIN..=-1 => Err(PyErr::fetch(py)),
                0 => Ok(None),
                1..=std::ffi::c_int::MAX => {
                    // Safety: PyDict_GetItemRef positive return value means the result is a valid
                    // owned reference
                    Ok(Some(unsafe { Bound::from_owned_ptr(py, result) }))
                }
            }?
        } {
            return Ok(existing);
        }

        // The key does not exist, insert the default value
        let value = if let Some(def) = default {
            def
        } else {
            py.None().into_bound(py)
        };
        let valid_value = self_.validate_value(py, value)?;
        dict_set_item(py, self_.as_ptr(), valid_key, Bound::clone(&valid_value))?;

        Ok(valid_value)
    }

    pub fn __ior__<'py>(self_: PyRef<'py, AtorsDict>, other: Bound<'py, PyAny>) -> PyResult<()> {
        AtorsDict::update(self_, Some(&other), None)
    }

    // The traverse method of the parent class (PyDict) is called automatically and
    // the type is also traversed so we only need to visit our own references.
    pub fn __traverse__(&self, visit: pyo3::PyVisit) -> Result<(), pyo3::PyTraverseError> {
        // Safety: Python guarantees exclusive access when calling GC methods, ensuring
        // no concurrent mutation (holds for both GIL and free-threaded builds).
        if let Some(o) = unsafe { &*self.object.get() } {
            visit.call(o)?;
        }
        Ok(())
    }

    // The clear method of the parent class (PyDict) is called automatically and
    // so we only need to clear our own references.
    pub fn __clear__(&self) {
        // Safety: Python guarantees exclusive access when calling GC methods, ensuring
        // no concurrent mutation (holds for both GIL and free-threaded builds).
        unsafe { *self.object.get() = None };
    }

    /// Dummy constructor used solely for unpickling.
    ///
    /// Creates an empty `AtorsDict` without any Ators metadata. The validators and
    /// related metadata will be populated by the `restore` method called from
    /// `AtorsBase.__setstate__` after construction. Dict items are restored via the
    /// 5th slot of the pickle reduce tuple (dict items iterator).
    #[staticmethod]
    pub fn _construct<'py>(py: Python<'py>) -> PyResult<Bound<'py, AtorsDict>> {
        use crate::validators::types::TypeValidator;
        Bound::new(
            py,
            AtorsDict {
                key_validator: UnsafeCell::new(Validator {
                    type_validator: TypeValidator::Any {},
                    value_validators: Box::new([]),
                    coercer: None,
                    init_coercer: None,
                }),
                value_validator: UnsafeCell::new(Validator {
                    type_validator: TypeValidator::Any {},
                    value_validators: Box::new([]),
                    coercer: None,
                    init_coercer: None,
                }),
                member_name: UnsafeCell::new(None),
                object: UnsafeCell::new(None),
            },
        )
    }

    pub fn __reduce_ex__<'py>(
        self_: &Bound<'py, Self>,
        py: Python<'py>,
        _protocol: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Returns a 5-tuple (callable, args, state, list_items, dict_items).
        // The 5th element must be an iterator yielding (key, value) 2-tuples.
        let py_dict = unsafe { self_.cast_unchecked::<PyDict>() };
        let items: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)> = py_dict.iter().collect();
        let items_iter = items.into_bound_py_any(py)?.try_iter()?;
        (
            self_.getattr(intern!(py, "_construct"))?,
            (),
            py.None(),
            py.None(),
            items_iter,
        )
            .into_bound_py_any(py)
    }
}
