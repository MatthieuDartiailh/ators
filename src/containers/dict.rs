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
    types::{PyAnyMethods, PyDict, PyDictMethods},
};
use std::cell::UnsafeCell;

use crate::{
    class::AtorsBase,
    containers::{AtorsList, AtorsSet, common::matches_assignment_context},
    validators::Validator,
};

pub(crate) fn restore_nested_container_value<'py>(
    value: &Bound<'py, PyAny>,
    value_validator: &Validator,
    member_name: Option<&str>,
    object: Option<&Bound<'py, AtorsBase>>,
) {
    use crate::validators::types::TypeValidator;

    match &value_validator.type_validator {
        TypeValidator::List {
            item: Some(item_validator),
        } => {
            if let Ok(nested) = value.cast::<AtorsList>() {
                AtorsList::restore(nested, (*item_validator.0).clone(), member_name, object);
            }
        }
        TypeValidator::Set {
            item: Some(item_validator),
        } => {
            if let Ok(nested) = value.cast::<AtorsSet>() {
                AtorsSet::restore(nested, (*item_validator.0).clone(), member_name, object);
            }
        }
        TypeValidator::Dict {
            items: Some((key_validator, value_validator)),
        } => {
            if let Ok(nested) = value.cast::<AtorsDict>() {
                AtorsDict::restore(
                    nested,
                    (*key_validator.0).clone(),
                    (*value_validator.0).clone(),
                    member_name,
                    object,
                );
            }
        }
        TypeValidator::DefaultDict {
            items: (key_validator, value_validator),
        } => {
            if let Ok(nested) = value.cast::<AtorsDefaultDict>() {
                AtorsDefaultDict::restore(
                    nested,
                    (*key_validator.0).clone(),
                    (*value_validator.0).clone(),
                    member_name,
                    object,
                );
            }
        }
        _ => {}
    }
}

fn update_dict_with_validation<'py, F>(
    py: Python<'py>,
    ndict: &Bound<'py, PyDict>,
    other: Option<&Bound<'py, PyAny>>,
    kwargs: Option<&Bound<'py, PyDict>>,
    validate_item: F,
) -> PyResult<()>
where
    F: Fn(
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)>,
{
    let valid = PyDict::new(py);
    if let Some(o) = other {
        if let Ok(od) = o.cast::<PyDict>() {
            for (k, v) in od.iter() {
                let (valid_key, valid_value) = validate_item(&k, &v)?;
                valid.set_item(valid_key, valid_value)?;
            }
        } else if o.hasattr(intern!(py, "keys"))? {
            let keys = o.call_method0(intern!(py, "keys"))?;
            for key in keys.try_iter()? {
                let k = key?;
                let v = o.getattr(intern!(py, "__getitem__"))?.call1((&k,))?;
                let (valid_key, valid_value) = validate_item(&k, &v)?;
                valid.set_item(valid_key, valid_value)?;
            }
        } else {
            for t in o.try_iter()? {
                let (k, v) = t?.extract::<(Bound<'py, PyAny>, Bound<'py, PyAny>)>()?;
                let (valid_key, valid_value) = validate_item(&k, &v)?;
                valid.set_item(valid_key, valid_value)?;
            }
        }
    }

    if let Some(kw) = kwargs {
        for (k, v) in kw.iter() {
            let (valid_key, valid_value) = validate_item(&k, &v)?;
            valid.set_item(valid_key, valid_value)?;
        }
    }

    ndict.update(valid.as_mapping())
}

fn setdefault_with_validation<'py, FK, FV>(
    py: Python<'py>,
    ndict: &Bound<'py, PyDict>,
    key: &Bound<'py, PyAny>,
    default: Option<&Bound<'py, PyAny>>,
    validate_key: FK,
    validate_value: FV,
) -> PyResult<Bound<'py, PyAny>>
where
    FK: Fn(&Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>>,
    FV: Fn(&Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>>,
{
    let valid_key = validate_key(key)?;
    if let Some(existing) = ndict.get_item(&valid_key)? {
        return Ok(existing);
    }

    let value = if let Some(def) = default {
        def
    } else {
        &py.None().into_bound(py)
    };
    let valid_value = validate_value(value)?;
    ndict.set_item(&valid_key, &valid_value)?;
    Ok(valid_value)
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
// to this object have been dropped - ensuring no concurrent access (holds for both GIL and
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
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Safety: same as AtorsList::validate_item.
        let key_validator = unsafe { &*self.key_validator.get() };
        let m = unsafe { &*self.member_name.get() }.as_deref();
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        key_validator.validate(m, o, key)
    }

    /// Validate a value for insertion into the dict
    fn validate_value<'py>(
        &self,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Safety: same as AtorsList::validate_item.
        let value_validator = unsafe { &*self.value_validator.get() };
        let m = unsafe { &*self.member_name.get() }.as_deref();
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        value_validator.validate(m, o, value)
    }

    /// Validate both key and value for insertion into the dict
    fn validate_item<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyAny>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        // Safety: same as AtorsList::validate_item.
        let key_validator = unsafe { &*self.key_validator.get() };
        let value_validator = unsafe { &*self.value_validator.get() };
        let m = unsafe { &*self.member_name.get() }.as_deref();
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        let valid_key = key_validator.validate(m, o, key)?;
        let valid_value = value_validator.validate(m, o, value)?;
        Ok((valid_key, valid_value))
    }

    pub(crate) fn matches_assignment_context<'py>(
        &self,
        member_name: Option<&str>,
        object: Option<&Bound<'py, AtorsBase>>,
    ) -> bool {
        matches_assignment_context(&self.member_name, &self.object, member_name, object)
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
        // Capture the value validator type before the critical section for nested restore.
        let value_v = value_validator.clone();

        with_critical_section(adict.as_any(), || {
            // SAFETY: AtorsDict is declared as `extends=PyDict`, so this cast is always valid.
            let pydict = unsafe { adict.cast_unchecked::<PyDict>() };
            for (_, v) in pydict.iter() {
                restore_nested_container_value(&v, &value_validator, member_name, object);
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
        for (_, v) in py_dict.iter() {
            restore_nested_container_value(&v, &value_v, member_name, object);
        }
    }
}

#[pymethods]
impl AtorsDict {
    pub fn __setitem__<'py>(
        self_: &Bound<'py, AtorsDict>,
        key: &Bound<'py, PyAny>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = key.py();
        let (valid_key, valid_value) = self_.get().validate_item(py, key, value)?;
        let ndict = unsafe { self_.cast_unchecked::<PyDict>() };
        ndict.set_item(valid_key, valid_value)
    }

    // Required because the Python C API defines a single slot used for both
    // __delitem__ and __setitem__
    pub fn __delitem__<'py>(
        self_: &Bound<'py, AtorsDict>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let ndict = unsafe { self_.cast_unchecked::<PyDict>() };
        ndict.del_item(key)
    }

    #[pyo3(signature = (other=None, **kwargs))]
    /// Update the dict from mappings/iterables/kwargs after validating all entries.
    ///
    /// Validation is performed eagerly into a temporary dict to avoid partial
    /// updates when one item fails validation.
    pub fn update<'py>(
        self_: &Bound<'py, AtorsDict>,
        other: Option<&Bound<'py, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let py = self_.py();
        let ndict = unsafe { self_.cast_unchecked::<PyDict>() };
        update_dict_with_validation(py, ndict, other, kwargs, |k, v| {
            self_.get().validate_item(py, k, v)
        })
    }

    /// Return the value for `key`, inserting a validated default if absent.
    pub fn setdefault<'py>(
        self_: &Bound<'py, AtorsDict>,
        key: &Bound<'py, PyAny>,
        default: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = key.py();
        let ndict = unsafe { self_.cast_unchecked::<PyDict>() };
        setdefault_with_validation(
            py,
            ndict,
            key,
            default,
            |k| self_.get().validate_key(py, k),
            |v| self_.get().validate_value(py, v),
        )
    }

    pub fn __ior__<'py>(self_: &Bound<'py, AtorsDict>, other: &Bound<'py, PyAny>) -> PyResult<()> {
        AtorsDict::update(self_, Some(other), None)
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

#[pyclass(module = "ators._ators", extends=PyDict, frozen)]
pub struct AtorsDefaultDict {
    key_validator: UnsafeCell<Validator>,
    value_validator: UnsafeCell<Validator>,
    member_name: UnsafeCell<Option<String>>,
    // Wrapped in UnsafeCell to allow clearing during GC while keeping the class frozen.
    object: UnsafeCell<Option<Py<AtorsBase>>>,
}

// Safety: key_validator, value_validator and member_name are written at
// construction and may be updated during restore while holding a critical section on the same
// object, guaranteeing exclusive access during those writes; outside restore they are effectively
// immutable. object is only modified during __clear__, which Python's GC calls only once all
// references to this object have been dropped - ensuring no concurrent access (holds for both GIL
// and free-threaded builds).
unsafe impl Sync for AtorsDefaultDict {}

impl AtorsDefaultDict {
    pub(crate) fn new_empty<'py>(
        py: Python<'py>,
        key_validator: Validator,
        value_validator: Validator,
        member_name: Option<&str>,
        object: Option<Py<AtorsBase>>,
    ) -> PyResult<Bound<'py, AtorsDefaultDict>> {
        Bound::new(
            py,
            AtorsDefaultDict {
                key_validator: UnsafeCell::new(key_validator),
                value_validator: UnsafeCell::new(value_validator),
                member_name: UnsafeCell::new(member_name.map(|m| m.to_string())),
                object: UnsafeCell::new(object),
            },
        )
    }

    fn validate_key<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let key_validator = unsafe { &*self.key_validator.get() };
        let m = unsafe { &*self.member_name.get() }.as_deref();
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        key_validator.validate(m, o, key)
    }

    fn validate_value<'py>(
        &self,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let value_validator = unsafe { &*self.value_validator.get() };
        let m = unsafe { &*self.member_name.get() }.as_deref();
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        value_validator.validate(m, o, value)
    }

    fn validate_item<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyAny>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        let key_validator = unsafe { &*self.key_validator.get() };
        let value_validator = unsafe { &*self.value_validator.get() };
        let m = unsafe { &*self.member_name.get() }.as_deref();
        let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        let valid_key = key_validator.validate(m, o, key)?;
        let valid_value = value_validator.validate(m, o, value)?;
        Ok((valid_key, valid_value))
    }

    /// Build a value for a missing key from the inferred value annotation.
    ///
    /// The returned value is validated by `__missing__`; insertion is handled by callers
    /// such as `__getitem__`.
    fn build_missing_value<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let value_validator = unsafe { &*self.value_validator.get() };
        let m = unsafe { &*self.member_name.get() }.as_deref();
        let o = unsafe { &*self.object.get() }
            .as_ref()
            .map(|ob| ob.bind(py));
        value_validator.create_inferred_default(m, o, py)
    }

    pub(crate) fn matches_assignment_context<'py>(
        &self,
        member_name: Option<&str>,
        object: Option<&Bound<'py, AtorsBase>>,
    ) -> bool {
        matches_assignment_context(&self.member_name, &self.object, member_name, object)
    }

    pub(crate) fn clone_for_assignment<'py>(
        source: &Bound<'py, AtorsDefaultDict>,
    ) -> PyResult<Bound<'py, AtorsDefaultDict>> {
        let dict = source.get();
        let key_validator = unsafe { &*dict.key_validator.get() }.clone();
        let value_validator = unsafe { &*dict.value_validator.get() }.clone();
        let member_name = unsafe { &*dict.member_name.get() }
            .as_deref()
            .map(|s| s.to_string());
        let object = unsafe { &*dict.object.get() }
            .as_ref()
            .map(|object| object.clone_ref(source.py()));
        let adict = AtorsDefaultDict::new_empty(
            source.py(),
            key_validator,
            value_validator,
            member_name.as_deref(),
            object,
        )?;
        let py_dict = unsafe { source.cast_unchecked::<PyDict>() };
        let adict_as_dict = adict.cast::<PyDict>()?;
        for (k, v) in py_dict.iter() {
            adict_as_dict.set_item(&k, &v)?;
        }
        Ok(adict)
    }

    /// Restore Ators-specific metadata after unpickling.
    pub(crate) fn restore<'py>(
        adict: &Bound<'py, AtorsDefaultDict>,
        key_validator: Validator,
        value_validator: Validator,
        member_name: Option<&str>,
        object: Option<&Bound<'py, AtorsBase>>,
    ) {
        let value_v = value_validator.clone();

        with_critical_section(adict.as_any(), || {
            let pydict = unsafe { adict.cast_unchecked::<PyDict>() };
            for (_, v) in pydict.iter() {
                restore_nested_container_value(&v, &value_validator, member_name, object);
            }
            let inner = adict.get();
            unsafe {
                (*inner.key_validator.get()) = key_validator;
                (*inner.value_validator.get()) = value_validator;
                (*inner.member_name.get()) = member_name.map(|s| s.to_string());
                (*inner.object.get()) = object.map(|o| o.clone().unbind());
            }
        });

        let py_dict = unsafe { adict.cast_unchecked::<PyDict>() };
        for (_, v) in py_dict.iter() {
            restore_nested_container_value(&v, &value_v, member_name, object);
        }
    }
}

#[pymethods]
impl AtorsDefaultDict {
    pub fn __getitem__<'py>(
        self_: &Bound<'py, AtorsDefaultDict>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = key.py();
        let valid_key = self_.get().validate_key(py, key)?;
        let ndict = unsafe { self_.cast_unchecked::<PyDict>() };
        if let Some(existing) = ndict.get_item(&valid_key)? {
            Ok(existing)
        } else {
            let missing = AtorsDefaultDict::__missing__(self_, &valid_key)?;
            ndict.set_item(&valid_key, &missing)?;
            Ok(missing)
        }
    }

    pub fn __missing__<'py>(
        self_: &Bound<'py, AtorsDefaultDict>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = key.py();
        let default = self_.get().build_missing_value(py)?;
        self_.get().validate_value(py, &default)
    }

    pub fn __setitem__<'py>(
        self_: &Bound<'py, AtorsDefaultDict>,
        key: &Bound<'py, PyAny>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = key.py();
        let (valid_key, valid_value) = self_.get().validate_item(py, key, value)?;
        let ndict = unsafe { self_.cast_unchecked::<PyDict>() };
        ndict.set_item(valid_key, valid_value)
    }

    pub fn __delitem__<'py>(
        self_: &Bound<'py, AtorsDefaultDict>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let ndict = unsafe { self_.cast_unchecked::<PyDict>() };
        ndict.del_item(key)
    }

    #[pyo3(signature = (other=None, **kwargs))]
    pub fn update<'py>(
        self_: &Bound<'py, AtorsDefaultDict>,
        other: Option<&Bound<'py, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let py = self_.py();
        let ndict = unsafe { self_.cast_unchecked::<PyDict>() };
        update_dict_with_validation(py, ndict, other, kwargs, |k, v| {
            self_.get().validate_item(py, k, v)
        })
    }

    pub fn setdefault<'py>(
        self_: &Bound<'py, AtorsDefaultDict>,
        key: &Bound<'py, PyAny>,
        default: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = key.py();
        let ndict = unsafe { self_.cast_unchecked::<PyDict>() };
        setdefault_with_validation(
            py,
            ndict,
            key,
            default,
            |k| self_.get().validate_key(py, k),
            |v| self_.get().validate_value(py, v),
        )
    }

    pub fn __ior__<'py>(
        self_: &Bound<'py, AtorsDefaultDict>,
        other: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        AtorsDefaultDict::update(self_, Some(other), None)
    }

    pub fn __traverse__(&self, visit: pyo3::PyVisit) -> Result<(), pyo3::PyTraverseError> {
        if let Some(o) = unsafe { &*self.object.get() } {
            visit.call(o)?;
        }
        Ok(())
    }

    pub fn __clear__(&self) {
        unsafe { *self.object.get() = None };
    }

    #[staticmethod]
    pub fn _construct<'py>(py: Python<'py>) -> PyResult<Bound<'py, AtorsDefaultDict>> {
        use crate::validators::types::TypeValidator;
        Bound::new(
            py,
            AtorsDefaultDict {
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
