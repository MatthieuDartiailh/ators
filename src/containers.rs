/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
/// Container types with validation and related utilities.
use pyo3::{
    Bound, Py, PyAny, PyErr, PyRef, PyResult, Python, ffi, intern, pyclass, pymethods,
    sync::critical_section::with_critical_section,
    types::{
        PyAnyMethods, PyDict, PyDictMethods, PyList, PyListMethods, PySet, PySetMethods, PySlice,
    },
};
use std::cell::UnsafeCell;

use crate::{core::AtorsBase, validators::Validator};

// XXX not pickable ...
#[pyclass(module = "ators._ators", extends=PyList, frozen)]
pub struct AtorsList {
    validator: Validator,
    member_name: Option<String>,
    // Wrapped in UnsafeCell to allow clearing during GC while keeping the class frozen.
    object: UnsafeCell<Option<Py<AtorsBase>>>,
}

// Safety: validator and member_name are immutable after construction; object is only
// modified during __clear__, which Python's GC calls only once all references to this
// object have been dropped — ensuring no concurrent access (holds for both GIL and
// free-threaded builds).
unsafe impl Sync for AtorsList {}

impl AtorsList {
    /// Create an empty, owner-bound `AtorsList` with the given validator.
    ///
    /// Note on pre-allocation: `PyList_New(n)` creates a plain `PyList`; a
    /// subtype cannot be seeded with it.  `PyList_Resize` is CPython-private.
    /// `ffi::PyList_Extend` (public since Python 3.13, available via PyO3 ffi)
    /// would give exact-capacity extension from a pre-built source list, but
    /// that still requires an intermediate container.  For typical small sizes
    /// `PyList_Append`'s amortised growth is sufficient.
    pub(crate) fn new_empty<'py>(
        py: Python<'py>,
        validator: Validator,
        member_name: Option<&str>,
        object: Option<Py<AtorsBase>>,
    ) -> PyResult<Bound<'py, AtorsList>> {
        Bound::new(
            py,
            AtorsList {
                validator,
                member_name: member_name.map(|m| m.to_string()),
                object: UnsafeCell::new(object),
            },
        )
    }

    fn validate_item<'py>(
        &self,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let m = self.member_name.as_deref();
        // Safety: object is only written during __clear__, which can only run after all
        // live references to this object are gone. A live reference is required to call
        // this method, so __clear__ cannot run concurrently (holds for both GIL and
        // free-threaded builds).
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        self.validator.validate(m, o, value)
    }

    fn validate_iterable<'py>(
        &self,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyList>> {
        let m = self.member_name.as_deref();
        // Safety: same as validate_item.
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        // Pre-allocate if the source supports __len__ (e.g. list, tuple, set)
        // so we only do at most one Rust Vec realloc regardless of input type.
        let mut validated_items = Vec::with_capacity(value.len().unwrap_or(0));
        for item in value.try_iter()? {
            let valid = self.validator.validate(m, o, &item?)?;
            validated_items.push(valid);
        }
        PyList::new(py, validated_items)
    }

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

    pub(crate) fn clone_for_assignment<'py>(
        source: &Bound<'py, AtorsList>,
    ) -> PyResult<Bound<'py, AtorsList>> {
        let list = source.get();
        let alist = AtorsList::new_empty(
            source.py(),
            list.validator.clone(),
            list.member_name.as_deref(),
            unsafe { &*list.object.get() }
                .as_ref()
                .map(|object| object.clone_ref(source.py())),
        )?;
        // Iterate the source list and append items directly without an intermediate Vec.
        // Cast is unchecked on the source since AtorsList extends PyList and cast is guaranteed safe.
        // Cast is checked when building to ensure the result is properly typed.
        // Safety: AtorsList is declared as `extends=PyList`, so this cast is always valid.
        let py_list = unsafe { source.cast_unchecked::<PyList>() };
        let alist_as_list = alist.cast::<PyList>()?;
        for item in py_list.iter() {
            alist_as_list.append(&item)?;
        }
        Ok(alist)
    }
}

// __delitem__, remove, pop, clear, sort, reverse and __imul__ do not need
// item validation since they only remove or rearrange existing items.
// append, insert, __setitem__, extend and __iadd__ need item validation
// since they can add new items.
#[pymethods]
impl AtorsList {
    pub fn append<'py>(self_: PyRef<'py, AtorsList>, value: &Bound<'py, PyAny>) -> PyResult<()> {
        let py = value.py();
        let valid = self_.validate_item(py, value)?;
        // Use direct PyList C API to avoid converting into bound to cast to PyList
        crate::utils::error_on_minusone(py, unsafe {
            ffi::PyList_Append(self_.as_ptr(), valid.as_ptr())
        })
    }

    pub fn insert<'py>(
        self_: PyRef<'py, AtorsList>,
        index: isize,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = value.py();
        let valid = self_.validate_item(py, value)?;
        // Use direct PyList C API to avoid converting into bound to cast to PyList
        crate::utils::error_on_minusone(py, unsafe {
            ffi::PyList_Insert(self_.as_ptr(), index, valid.as_ptr())
        })
    }

    pub fn __setitem__<'py>(
        self_: &Bound<'py, AtorsList>,
        index: &Bound<'py, PyAny>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = index.py();
        let valid = if index.is_instance_of::<PySlice>() {
            // For slice assignment, value is an iterable of items to validate
            with_critical_section(self_.as_any(), || self_.get().validate_iterable(py, value))?
                .into_any()
        } else {
            // For single-index assignment, validate the single value
            with_critical_section(self_.as_any(), || self_.get().validate_item(py, value))?
        };
        // Use py_super() to call the parent list's __setitem__ to avoid re-dispatching
        // through our own override (which would cause infinite recursion with PyObject_SetItem)
        self_
            .py_super()?
            .call_method1(intern!(py, "__setitem__"), (index, valid))
            .map(|_| ())
    }

    pub fn extend<'py>(self_: &Bound<'py, AtorsList>, other: &Bound<'py, PyAny>) -> PyResult<()> {
        let valid = with_critical_section(self_.as_any(), || {
            self_.get().validate_iterable(other.py(), other)
        })?;
        let list = self_.cast::<PyList>()?;
        for item in valid.iter() {
            list.append(&item)?;
        }
        Ok(())
    }

    pub fn __iadd__<'py>(self_: &Bound<'py, Self>, value: &Bound<'py, PyAny>) -> PyResult<()> {
        AtorsList::extend(self_, value)
    }

    pub fn __delitem__<'py>(
        self_: &Bound<'py, AtorsList>,
        index: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = index.py();
        self_
            .py_super()?
            .call_method1(intern!(py, "__delitem__"), (index,))
            .map(|_| ())
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
}

// XXX not pickable ...
#[pyclass(module = "ators._ators", extends=PySet, frozen)]
pub struct AtorsSet {
    validator: Validator,
    member_name: Option<String>,
    // Wrapped in UnsafeCell to allow clearing during GC while keeping the class frozen.
    object: UnsafeCell<Option<Py<AtorsBase>>>,
}

// Safety: validator and member_name are immutable after construction; object is only
// modified during __clear__, which Python's GC calls only once all references to this
// object have been dropped — ensuring no concurrent access (holds for both GIL and
// free-threaded builds).
unsafe impl Sync for AtorsSet {}

impl AtorsSet {
    /// Create an empty, owner-bound `AtorsSet` with the given validator.
    ///
    /// Note on pre-allocation: `_PySet_Presized` is CPython-private and not
    /// exposed in PyO3's ffi bindings.  `PySet_New(iterable)` creates a plain
    /// `PySet`, not a subtype.  For typical small container sizes iterative
    /// `PySet_Add` is sufficient.
    pub(crate) fn new_empty<'py>(
        py: Python<'py>,
        validator: Validator,
        member_name: Option<&str>,
        object: Option<Py<AtorsBase>>,
    ) -> PyResult<Bound<'py, AtorsSet>> {
        Bound::new(
            py,
            AtorsSet {
                validator,
                member_name: member_name.map(|m| m.to_string()),
                object: UnsafeCell::new(object),
            },
        )
    }

    fn validate_set<'py>(
        &self,
        py: Python<'py>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PySet>> {
        if unsafe { pyo3::ffi::PyAnySet_Check(value.as_ptr()) } == 0 {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "Expected a set for validation",
            ));
        }
        let mut validated_items = Vec::with_capacity(value.len()?);
        let m = self.member_name.as_deref();
        // Safety: object is only written during __clear__, which can only run after all
        // live references to this object are gone. A live reference is required to call
        // this method, so __clear__ cannot run concurrently (holds for both GIL and
        // free-threaded builds).
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        for item in value.try_iter()? {
            let valid = self.validator.validate(m, o, &item?)?;
            validated_items.push(valid);
        }
        PySet::new(py, validated_items)
    }

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

    pub(crate) fn clone_for_assignment<'py>(
        source: &Bound<'py, AtorsSet>,
    ) -> PyResult<Bound<'py, AtorsSet>> {
        let set = source.get();
        let aset = AtorsSet::new_empty(
            source.py(),
            set.validator.clone(),
            set.member_name.as_deref(),
            unsafe { &*set.object.get() }
                .as_ref()
                .map(|object| object.clone_ref(source.py())),
        )?;
        // Iterate the source set and add items directly without an intermediate Vec.
        // Cast is unchecked on the source since AtorsSet extends PySet and cast is guaranteed safe.
        // Cast is checked when building to ensure the result is properly typed.
        // Safety: AtorsSet is declared as `extends=PySet`, so this cast is always valid.
        let py_set = unsafe { source.cast_unchecked::<PySet>() };
        let aset_as_set = aset.cast::<PySet>()?;
        for item in py_set.iter() {
            aset_as_set.add(&item)?;
        }
        Ok(aset)
    }
}

// __isub__, difference_update, __iand__ and intersection_update do not need
// item validation since they remove items
// __ior__, update, __ixor__, symmetric_difference_update and add, need
// item validation since they can add items
#[pymethods]
impl AtorsSet {
    pub fn add<'py>(self_: PyRef<'py, AtorsSet>, value: Bound<'py, PyAny>) -> PyResult<()> {
        let py = value.py();
        // Safety: object is only written during __clear__, which can only run after all
        // live references to this object are gone. A live reference is required to call
        // this method, so __clear__ cannot run concurrently (holds for both GIL and
        // free-threaded builds).
        let valid = self_.validator.validate(
            self_.member_name.as_deref(),
            unsafe { &*self_.object.get() }.as_ref().map(|o| o.bind(py)),
            &value,
        )?;
        // Use direct PySet C API to avoid converting into bound to cast to PySet
        crate::utils::error_on_minusone(py, unsafe {
            ffi::PySet_Add(self_.as_ptr(), valid.as_ptr())
        })
    }

    pub fn __ior__<'py>(self_: &Bound<'py, Self>, value: Bound<'py, PyAny>) -> PyResult<()> {
        let py = value.py();
        let valid = with_critical_section(self_.as_any(), || self_.get().validate_set(py, value))?;
        let set = self_.cast::<PySet>()?;
        for item in valid.iter() {
            set.add(&item)?;
        }
        Ok(())
    }

    pub fn update<'py>(self_: &Bound<'py, AtorsSet>, other: Bound<'py, PyAny>) -> PyResult<()> {
        AtorsSet::__ior__(self_, other)
    }

    pub fn __ixor__<'py>(self_: &Bound<'py, Self>, value: Bound<'py, PyAny>) -> PyResult<()> {
        let py = value.py();
        let valid = with_critical_section(self_.as_any(), || self_.get().validate_set(py, value))?;
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
        other: Bound<'py, PyAny>,
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
}

#[pyclass(module = "ators._ators", extends=PyDict, frozen)]
pub struct AtorsDict {
    key_validator: Validator,
    value_validator: Validator,
    member_name: Option<String>,
    // Wrapped in UnsafeCell to allow clearing during GC while keeping the class frozen.
    object: UnsafeCell<Option<Py<AtorsBase>>>,
}

// Safety: key_validator, value_validator, and member_name are immutable after construction;
// object is only modified during __clear__, which Python's GC calls only once all references
// to this object have been dropped — ensuring no concurrent access (holds for both GIL and
// free-threaded builds).
unsafe impl Sync for AtorsDict {}

impl AtorsDict {
    /// Create an empty, owner-bound `AtorsDict` with the given validators.
    ///
    /// Note on pre-allocation: `_PyDict_NewPresized` is CPython-private and not
    /// exposed in PyO3's ffi bindings, so there is no public API to hint the
    /// initial hash-table capacity.  For typical small container sizes iterative
    /// `PyDict_SetItem` is sufficient.
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
                key_validator,
                value_validator,
                member_name: member_name.map(|m| m.to_string()),
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
        let m = self.member_name.as_deref();
        // Safety: object is only written during __clear__, which can only run after all
        // live references to this object are gone. A live reference is required to call
        // this method, so __clear__ cannot run concurrently (holds for both GIL and
        // free-threaded builds).
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        self.key_validator.validate(m, o, &key)
    }

    /// Validate a value for insertion into the dict
    fn validate_value<'py>(
        &self,
        py: Python<'py>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let m = self.member_name.as_deref();
        // Safety: same as validate_key.
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        self.value_validator.validate(m, o, &value)
    }

    /// Validate both key and value for insertion into the dict
    fn validate_item<'py>(
        &self,
        py: Python<'py>,
        key: Bound<'py, PyAny>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        let m = self.member_name.as_deref();
        // Safety: same as validate_key.
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        let valid_key = self.key_validator.validate(m, o, &key)?;
        let valid_value = self.value_validator.validate(m, o, &value)?;
        Ok((valid_key, valid_value))
    }

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

    pub(crate) fn clone_for_assignment<'py>(
        source: &Bound<'py, AtorsDict>,
    ) -> PyResult<Bound<'py, AtorsDict>> {
        let dict = source.get();
        let adict = AtorsDict::new_empty(
            source.py(),
            dict.key_validator.clone(),
            dict.value_validator.clone(),
            dict.member_name.as_deref(),
            unsafe { &*dict.object.get() }
                .as_ref()
                .map(|object| object.clone_ref(source.py())),
        )?;
        // Iterate the source dict and set items directly without an intermediate Vec.
        // Cast is unchecked on the source since AtorsDict extends PyDict and cast is guaranteed safe.
        // Cast is checked when building to ensure the result is properly typed.
        // Safety: AtorsDict is declared as `extends=PyDict`, so this cast is always valid.
        let py_dict = unsafe { source.cast_unchecked::<PyDict>() };
        let adict_as_dict = adict.cast::<PyDict>()?;
        for (k, v) in py_dict.iter() {
            adict_as_dict.set_item(&k, &v)?;
        }
        Ok(adict)
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

    // XXX can simply implement __getstate__ and __setstate__ without dealing with items
}

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


