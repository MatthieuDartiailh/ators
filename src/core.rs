/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
/// Core Ators object and related utilities.
use pyo3::{
    Bound, IntoPyObjectExt, Py, PyAny, PyResult, intern, pyclass, pyfunction, pymethods,
    sync::critical_section::with_critical_section,
    types::{
        PyAnyMethods, PyDict, PyDictMethods, PyMapping, PyMappingMethods, PyString, PyType,
        PyTypeMethods,
    },
};
use std::cell::UnsafeCell;

use crate::get_type_mutability_map;
use crate::member::{Member, MemberCustomizationTool, member_coerce_init};
use crate::meta::get_class_info;
use crate::observers::{AtorsChange, ObserverPool};
use crate::utils::Mutability;

pub static ATORS_MEMBERS: &str = "__ators_members__";
pub static ATORS_MEMBER_CUSTOMIZER: &str = "__ators_member_customizer__";
pub static ATORS_MEMBERS_MUTABILITY: &str = "__ators_members_mutability__";
pub static ATORS_OBSERVABLE: &str = "__ators_observable__";
pub static ATORS_PICKLE_POLICY: &str = "__ators_pickle_policy__";

/// Inner mutable state of an AtorsBase instance, stored in an UnsafeCell to allow
/// interior mutability while keeping AtorsBase frozen.
struct InnerAtors {
    frozen: bool,
    notification_enabled: bool,
    /// Whether the class this instance belongs to is observable.
    /// Set once at construction time and never mutated thereafter;
    /// it may therefore be read without holding the critical section.
    is_observable: bool,
    slots: Box<[Option<Py<PyAny>>]>,
}

#[pyclass(module = "ators._ators", subclass, frozen)]
pub struct AtorsBase {
    inner: UnsafeCell<InnerAtors>,
}

// Safety: All concurrent accesses to the UnsafeCell are protected by Python critical
// sections, which guarantee mutual exclusion. GC methods (__traverse__, __clear__) are
// called with Python GC guarantees that ensure exclusive access regardless of whether
// the GIL is enabled (holds for both GIL and free-threaded builds).
unsafe impl Sync for AtorsBase {}

#[pyclass(module = "ators._ators", frozen, from_py_object)]
#[derive(Debug, Clone)]
pub enum ClassMutability {
    #[pyo3(constructor = ())]
    Immutable {},
    #[pyo3(constructor = ())]
    Mutable {},
    #[pyo3(constructor = (values))]
    InspectValues { values: Vec<String> },
}

#[pyclass(module = "ators._ators", frozen, from_py_object, eq, eq_int)]
#[derive(Debug, Clone, PartialEq)]
pub enum PicklePolicy {
    /// Include all members in pickle state (default).
    #[pyo3(name = "ALL")]
    All,
    /// Exclude all members from pickle state.
    #[pyo3(name = "NONE")]
    None,
    /// Include only public members (those not starting with `_`) in pickle state.
    #[pyo3(name = "PUBLIC")]
    Public,
}

#[pymethods]
impl AtorsBase {
    #[new]
    #[pyo3(signature = (**_kwargs))]
    #[classmethod]
    fn py_new(cls: &Bound<'_, PyType>, _kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let py = cls.py();
        let class_info = get_class_info(cls)?;
        let slots_count = class_info.members_by_name().len();
        // Determine observability at instantiation time by checking the class attribute.
        // The result is cached on the instance (is_observable field) and never mutated,
        // so later accesses can skip the critical section.
        let is_observable = class_info.observable();
        if slots_count > (u8::MAX as usize) {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "The class {} has more than 255 members which is not supported.",
                cls.name().unwrap_or(PyString::new(py, "<unknown>"))
            )));
        }
        // NOTE using a boxed slice is suboptimal size wise since we do not need a usize
        // when limiting ourselves to 255 members but it is the easiest way to have
        // a fixed size array without using unsafe code.
        // We can revisit this later if needed.
        let total_slots = slots_count + usize::from(is_observable);
        let mut slots: Box<[Option<Py<PyAny>>]> = (0..total_slots).map(|_| None).collect();
        if is_observable {
            let pool = Bound::new(py, ObserverPool::new())?.into_any().unbind();
            slots[0] = Some(pool);
        }
        Ok(Self {
            inner: UnsafeCell::new(InnerAtors {
                frozen: false,
                notification_enabled: is_observable,
                is_observable,
                slots,
            }),
        })
    }

    pub fn __traverse__(&self, visit: pyo3::PyVisit) -> Result<(), pyo3::PyTraverseError> {
        // Safety: Python guarantees exclusive access when calling GC methods, ensuring
        // no concurrent mutation of the inner state (holds for both GIL and free-threaded builds).
        let inner = unsafe { &*self.inner.get() };
        for slot in inner.slots.iter().flatten() {
            visit.call(slot)?;
        }
        Ok(())
    }

    pub fn __clear__(&self) {
        // Safety: Python guarantees exclusive access when calling GC methods, ensuring
        // no concurrent mutation of the inner state (holds for both GIL and free-threaded builds).
        let inner = unsafe { &mut *self.inner.get() };
        for o in inner.slots.iter_mut() {
            *o = None;
        }
    }

    #[pyo3(signature = (**kwargs))]
    pub fn __init__(
        slf: &Bound<'_, AtorsBase>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let Some(kwargs) = kwargs else {
            return Ok(());
        };
        let py = slf.py();
        let class_info = get_class_info(&slf.get_type())?;
        let init_members = class_info.init_member_names();
        let required_init_members = class_info.required_init_member_names();
        if kwargs.len() > init_members.len() {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Too many init values passed: got {}, expected at most {}",
                kwargs.len(),
                init_members.len()
            )));
        }
        if kwargs.len() < required_init_members.len() {
            for name in required_init_members {
                if !kwargs.contains(name.as_str())? {
                    return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                        "Missing required init value for member '{name}'"
                    )));
                }
            }
        }
        for (k, v) in kwargs.iter() {
            let key = k.cast::<PyString>()?;
            let key_name: String = key.extract()?;
            if !init_members.iter().any(|n| n == &key_name) {
                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "Cannot pass an init value for member '{key}' because it is not marked as init."
                )));
            }
            match slf.setattr(key, v.clone()) {
                Ok(_) => Ok(()),
                Err(err) => {
                    // FIXME use cold_branch once Rust 1.95 is out
                    let m = class_info
                        .members_by_name()
                        .get(&key_name)
                        .ok_or_else(|| {
                            pyo3::exceptions::PyAttributeError::new_err(format!(
                                "Unknown member '{key_name}'"
                            ))
                        })?
                        .bind(py)
                        .clone();
                    if let Some(r) = member_coerce_init(&m, slf, &v) {
                        let coerced_v = r?;
                        slf.setattr(key, coerced_v).map(|_| ())
                    } else {
                        Err(err)
                    }
                }
            }?
        }
        Ok(())
    }

    pub fn __getstate__<'py>(slf: &Bound<'py, AtorsBase>) -> PyResult<Bound<'py, PyDict>> {
        let py = slf.py();
        let cls = slf.get_type();

        let members = cls
            .getattr(intern!(py, ATORS_MEMBERS))?
            .cast_into::<PyMapping>()?;

        let state = PyDict::new(py);
        for item in members.items()?.try_iter()? {
            let item = item?;
            let (name, member_obj) = item.extract::<(Bound<'py, PyAny>, Bound<'py, PyAny>)>()?;
            let name_str: String = name.extract()?;
            let member = member_obj.cast_into::<Member>()?;
            let mb = member.get();

            if mb.pickle
                && let Some(value) = get_slot_owned(slf, mb.index())
            {
                state.set_item(&name_str, value.into_bound(py))?;
            }
        }

        Ok(state)
    }

    pub fn __setstate__<'py>(
        slf: &Bound<'py, AtorsBase>,
        state: &Bound<'py, PyDict>,
    ) -> PyResult<()> {
        use crate::containers::{AtorsDict, AtorsList, AtorsSet};
        use crate::validators::types::TypeValidator;

        let py = slf.py();
        let cls = slf.get_type();

        let members = cls
            .getattr(intern!(py, ATORS_MEMBERS))?
            .cast_into::<PyMapping>()?;

        // Validate all keys are known members
        for (key, _) in state.iter() {
            let key_str: String = key.extract()?;
            if !members.contains(&key)? {
                return Err(pyo3::exceptions::PyKeyError::new_err(format!(
                    "Unknown member '{}' for {}",
                    key_str,
                    cls.name()?
                )));
            }
        }

        // Restore values
        for (key, value) in state.iter() {
            let member = members.get_item(&key)?.cast_into::<Member>()?;
            let mb = member.get();

            // For container members: restore metadata before slot assignment.
            // `item_bv` is a `BoxedValidator(Box<Validator>)`; `item_bv.0` is the
            // inner `Box<Validator>`, and `*item_bv.0` dereferences it to `Validator`.
            match &mb.validator().type_validator {
                TypeValidator::List {
                    item: Some(item_bv),
                } => {
                    if let Ok(alist) = value.cast::<AtorsList>() {
                        AtorsList::restore(alist, (*item_bv.0).clone(), Some(mb.name()), Some(slf));
                    }
                }
                TypeValidator::Set {
                    item: Some(item_bv),
                } => {
                    if let Ok(aset) = value.cast::<AtorsSet>() {
                        AtorsSet::restore(aset, (*item_bv.0).clone(), Some(mb.name()), Some(slf));
                    }
                }
                TypeValidator::Dict {
                    items: Some((key_bv, val_bv)),
                } => {
                    if let Ok(adict) = value.cast::<AtorsDict>() {
                        AtorsDict::restore(
                            adict,
                            (*key_bv.0).clone(),
                            (*val_bv.0).clone(),
                            Some(mb.name()),
                            Some(slf),
                        );
                    }
                }
                _ => {}
            }

            // Write directly to slot, bypassing validation
            set_slot(slf, mb.index(), &value);
        }

        Ok(())
    }
}

#[inline]
/// Get a reference to the value stored in the slot at index if any.
/// A critical section is always used to guarantee safe concurrent access.
pub(crate) fn get_slot<'a, 'py>(
    object: &'a Bound<'py, AtorsBase>,
    index: u8,
) -> Option<&'a Py<PyAny>> {
    with_critical_section(object.as_any(), || {
        // Safety: we hold the critical section lock on this object.
        let inner = unsafe { &*object.get().inner.get() };
        inner.slots[index as usize].as_ref()
    })
}

#[inline]
/// Get an owned clone of the value stored in the slot at index if any.
///
/// This helper is intended for return-oriented paths such as Member.__get__
/// where the caller needs an owned Python reference.
pub(crate) fn get_slot_owned<'py>(object: &Bound<'py, AtorsBase>, index: u8) -> Option<Py<PyAny>> {
    let py = object.py();
    with_critical_section(object.as_any(), || {
        // Safety: we hold the critical section lock on this object.
        let inner = unsafe { &*object.get().inner.get() };
        inner.slots[index as usize]
            .as_ref()
            .map(|value| value.clone_ref(py))
    })
}

#[inline]
/// Set the slot at index to the specified value
pub(crate) fn set_slot<'py>(object: &Bound<'py, AtorsBase>, index: u8, value: &Bound<'py, PyAny>) {
    let py = object.py();
    with_critical_section(object.as_any(), || {
        // Safety: we hold the critical section lock on this object. We write through the
        // raw pointer instead of creating a &mut T to avoid relying on Rust aliasing rules.
        unsafe {
            (*object.get().inner.get()).slots[index as usize].replace(
                value
                    .into_py_any(py)
                    .expect("Unfaillible conversion to Py<PyAny>"),
            );
        }
    })
}

pub(crate) enum ReplaceSlotOutcome {
    Replaced(Option<Py<PyAny>>),
    Unchanged,
}

#[inline]
/// Atomically check frozen state, write the slot, and return the previous value.
///
/// Returns `Ok(ReplaceSlotOutcome::Replaced(old))` on success where `old` is the
/// previous slot value.
/// Returns `Ok(ReplaceSlotOutcome::Unchanged)` if the slot already contains the
/// exact same Python object.
/// Returns `Err(())` if the object was frozen at write-time (write was skipped).
pub(crate) fn replace_slot<'py>(
    object: &Bound<'py, AtorsBase>,
    index: u8,
    value: &Bound<'py, PyAny>,
) -> Result<ReplaceSlotOutcome, ()> {
    let py = object.py();
    with_critical_section(object.as_any(), || {
        // Safety: we hold the critical section lock on this object.
        let inner = unsafe { &mut *object.get().inner.get() };
        if inner.frozen {
            return Err(());
        }
        let old = inner.slots[index as usize].replace(
            value
                .into_py_any(py)
                .expect("Unfaillible conversion to Py<PyAny>"),
        );
        if old
            .as_ref()
            .is_some_and(|old| old.as_ptr() == value.as_ptr())
        {
            Ok(ReplaceSlotOutcome::Unchanged)
        } else {
            Ok(ReplaceSlotOutcome::Replaced(old))
        }
    })
}

#[inline]
/// Del the slot value at index
pub(crate) fn del_slot<'py>(object: &Bound<'py, AtorsBase>, index: u8) {
    with_critical_section(object.as_any(), || {
        // Safety: we hold the critical section lock on this object. We write through the
        // raw pointer instead of creating a &mut T to avoid relying on Rust aliasing rules.
        unsafe {
            (*object.get().inner.get()).slots[index as usize] = None;
        }
    })
}

/// Check whether an instance belongs to an observable class.
///
/// This reads `is_observable` directly from `InnerAtors` **without** acquiring the critical
/// section. This is safe because `is_observable` is set once at construction time and is
/// never mutated afterwards.
#[inline]
pub(crate) fn instance_is_observable(obj: &Bound<'_, AtorsBase>) -> bool {
    // Safety: is_observable is written exactly once (in py_new) and never modified
    // afterwards, so a relaxed read without the critical section is safe here.
    unsafe { (*obj.get().inner.get()).is_observable }
}

pub(crate) fn notifications_enabled(obj: &Bound<'_, AtorsBase>) -> bool {
    with_critical_section(obj.as_any(), || {
        // Safety: we hold the critical section lock on this object.
        unsafe { (*obj.get().inner.get()).notification_enabled }
    })
}

/// Return a borrowed reference to the observer pool for an observable instance.
///
/// Callers must have verified that `instance_is_observable(obj)` is `true` before calling
/// this function. When the instance is observable, `slots[0]` is guaranteed to hold the
/// `ObserverPool` for the entire lifetime of the object (set at construction, never replaced).
///
/// No critical section is needed here because `slots[0]` is written exactly once (in
/// `py_new`) and never mutated afterwards; it may therefore be dereferenced without locking.
pub(crate) fn get_observer_pool<'a, 'py>(
    obj: &'a Bound<'py, AtorsBase>,
) -> &'a Bound<'py, ObserverPool> {
    let py = obj.py();
    // Safety:
    // - is_observable is true (caller-verified), so slots[0] is always Some(ObserverPool).
    // - slots[0] is set once at construction and never mutated; reading without CS is safe.
    // - The returned reference is bounded by 'a (the borrow of obj), which keeps AtorsBase alive,
    //   ensuring the Py<PyAny> in slots[0] remains valid for the reference's lifetime.
    unsafe {
        let inner: &'a InnerAtors = &*obj.get().inner.get();
        let pool: &'a Py<PyAny> = inner.slots[0].as_ref().unwrap_unchecked();
        pool.bind(py).cast_unchecked::<ObserverPool>()
    }
}

pub(crate) fn notify_member_change<'py>(
    obj: &Bound<'py, AtorsBase>,
    member_name: &str,
    oldvalue: Py<PyAny>,
    newvalue: Py<PyAny>,
) -> PyResult<()> {
    // Fast path: check is_observable without a critical section.
    // is_observable is set once at construction and never mutated.
    if !instance_is_observable(obj) {
        return Ok(());
    }

    // Check notification_enabled under a single critical section.
    if !notifications_enabled(obj) {
        return Ok(());
    }

    // slots[0] is immutable after construction: no CS needed to get a reference to the pool.
    let pool = get_observer_pool(obj);
    let py = obj.py();
    let change = Bound::new(
        py,
        AtorsChange::new(
            obj.clone().unbind(),
            member_name.to_string(),
            oldvalue,
            newvalue,
        ),
    )?;
    let errors = ObserverPool::fire(pool, member_name, &change)?;
    if !errors.is_empty() {
        let exception_group = py
            .import(intern!(py, "builtins"))?
            .getattr(intern!(py, "ExceptionGroup"))?
            .call1(("errors in observers", errors))?;
        return Err(pyo3::PyErr::from_value(exception_group));
    }
    Ok(())
}

/// Private helper: set the frozen bit on an AtorsBase object inside a critical section.
fn do_freeze(obj: &Bound<'_, AtorsBase>) {
    with_critical_section(obj.as_any(), || {
        // Safety: we hold the critical section lock on this object. We write through the
        // raw pointer instead of creating a &mut T to avoid relying on Rust aliasing rules.
        unsafe { (*obj.get().inner.get()).frozen = true };
    });
}

#[pyfunction]
pub fn freeze<'py>(obj: &Bound<'py, AtorsBase>) -> PyResult<()> {
    let py = obj.py();

    // Check class mutability to determine if freezing is allowed
    let class_type = obj.get_type();
    match class_type.getattr(ATORS_MEMBERS_MUTABILITY) {
        Ok(mutability_obj) => {
            let mutability_enum = mutability_obj.extract::<ClassMutability>()?;
            match mutability_enum {
                ClassMutability::Immutable {} => {
                    // All members are immutable, allow freezing
                    do_freeze(obj);
                    Ok(())
                }
                ClassMutability::Mutable {} => {
                    // Some member type is mutable, cannot freeze
                    Err(pyo3::exceptions::PyTypeError::new_err(
                        "Cannot freeze an object with mutable member types",
                    ))
                }
                ClassMutability::InspectValues { values } => {
                    // Inspect each attribute and check if it's mutable
                    let members_dict = class_type.getattr(ATORS_MEMBERS)?;
                    let ty_mutability_map = get_type_mutability_map(py);

                    for attr_name in &values {
                        let member_obj = PyAnyMethods::get_item(&members_dict, attr_name)?
                            .cast_into::<Member>()?;

                        // Get the slot index and retrieve the value
                        if let Some(slot_value) = get_slot(obj, member_obj.get().index()) {
                            let attr_bound = slot_value.bind(py);
                            let attr_mutability =
                                with_critical_section(ty_mutability_map.as_any(), || {
                                    ty_mutability_map.borrow().get_object_mutability(attr_bound)
                                })?;
                            match attr_mutability {
                                Mutability::Mutable | Mutability::Undecidable => {
                                    return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                                        "Cannot freeze object: member '{}' contains potentially mutable value",
                                        attr_name
                                    )));
                                }
                                Mutability::Immutable => {}
                            }
                        }
                    }

                    // All inspected attributes are immutable, allow freezing
                    do_freeze(obj);
                    Ok(())
                }
            }
        }
        Err(_) => Err(pyo3::exceptions::PyAttributeError::new_err(format!(
            "Class {} is missing the required attribute '{}' to determine mutability",
            class_type.name().expect("Type object always has a name"),
            ATORS_MEMBERS_MUTABILITY
        ))),
    }
}

#[pyfunction]
pub fn is_frozen<'py>(obj: &Bound<'py, AtorsBase>) -> bool {
    with_critical_section(obj.as_any(), || {
        // Safety: we hold the critical section lock on this object.
        unsafe { (*obj.get().inner.get()).frozen }
    })
}

/// Retrieve a single Member from an Ators object by name.
#[pyfunction]
pub fn get_member<'py>(
    obj: Bound<'py, PyAny>,
    member_name: Bound<'py, PyString>,
) -> PyResult<Bound<'py, Member>> {
    let info = get_class_info(&obj.get_type())?;
    let name: String = member_name.extract()?;
    info.members_by_name()
        .get(&name)
        .map(|m| m.bind(obj.py()).clone())
        .ok_or_else(|| {
            pyo3::exceptions::PyAttributeError::new_err(format!("Unknown member '{name}'"))
        })
}

/// Retrieve all members from an Ators objetc.
#[pyfunction]
pub fn get_members<'py>(obj: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py = obj.py();
    let copy = PyDict::new(py);
    let info = get_class_info(&obj.get_type())?;
    for (name, member) in info.members_by_name() {
        copy.set_item(name, member.bind(py))?;
    }
    Ok(copy)
}

/// Retrieve all members with a specific metadata key and the value associated with it.
#[pyfunction]
pub fn get_members_by_tag<'py>(
    obj: &Bound<'py, PyAny>,
    tag: String,
) -> PyResult<Bound<'py, PyDict>> {
    let py = obj.py();
    let members = PyDict::new(obj.py());
    let info = get_class_info(&obj.get_type())?;
    for (name, v) in info.members_by_name() {
        let member = v.bind(py);
        if let Some(m) = member.get().metadata()
            && m.contains_key(&tag)
        {
            members.set_item(name, (member, m[&tag].clone_ref(py)))?;
        }
    }
    Ok(members)
}

/// Retrieve all members with a specific metadata key and value.
#[pyfunction]
pub fn get_members_by_tag_and_value<'py>(
    obj: &Bound<'py, PyAny>,
    tag: String,
    value: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let members = PyDict::new(obj.py());
    let py = obj.py();
    let info = get_class_info(&obj.get_type())?;
    for (name, member) in info.members_by_name() {
        let member = member.bind(py);
        if let Some(m) = member.get().metadata()
            && m.contains_key(&tag)
            // If comparison fails the member should not be included
            && value.as_any().eq(&m[&tag]).unwrap_or(false)
        {
            members.set_item(name, member)?;
        }
    }
    Ok(members)
}

/// Retrieve the member customization tool from a class.
#[pyfunction]
pub fn get_member_customization_tool<'py>(
    cls: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, MemberCustomizationTool>> {
    let attr = cls.getattr(ATORS_MEMBER_CUSTOMIZER)?;
    if attr.is_none() {
        Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
            "Member customization is only possible during __init_subclass__ for class {}",
            cls.get_type().name().unwrap()
        )))
    } else {
        Ok(attr.cast_into::<MemberCustomizationTool>()?)
    }
}

#[pyfunction]
pub fn observe<'py>(
    obj: &Bound<'py, AtorsBase>,
    member_name: String,
    callback: &Bound<'py, PyAny>,
) -> PyResult<()> {
    if !instance_is_observable(obj) {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "Cannot register observers on a non-observable class",
        ));
    }

    let class_info = get_class_info(&obj.get_type())?;
    if !class_info.members_by_name().contains_key(&member_name) {
        return Err(pyo3::exceptions::PyAttributeError::new_err(format!(
            "Unknown member '{member_name}'"
        )));
    }

    let pool = get_observer_pool(obj);
    ObserverPool::add(pool, &member_name, callback)
}

#[pyfunction]
pub fn unobserve<'py>(
    obj: &Bound<'py, AtorsBase>,
    member_name: String,
    callback: &Bound<'py, PyAny>,
) -> PyResult<()> {
    if !instance_is_observable(obj) {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "Cannot unregister observers on a non-observable class",
        ));
    }

    let pool = get_observer_pool(obj);
    ObserverPool::remove(pool, &member_name, callback)
}

#[pyfunction]
pub fn enable_notifications<'py>(obj: &Bound<'py, AtorsBase>) -> PyResult<()> {
    if !instance_is_observable(obj) {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "Cannot enable notifications on a non-observable class",
        ));
    }

    with_critical_section(obj.as_any(), || {
        // Safety: we hold the critical section lock on this object. We write through
        // the raw pointer instead of creating a &mut T to avoid relying on Rust aliasing rules.
        unsafe {
            (*obj.get().inner.get()).notification_enabled = true;
        }
    });
    Ok(())
}

#[pyfunction]
pub fn disable_notifications<'py>(obj: &Bound<'py, AtorsBase>) {
    with_critical_section(obj.as_any(), || {
        // Safety: we hold the critical section lock on this object.
        unsafe {
            (*obj.get().inner.get()).notification_enabled = false;
        }
    });
}

#[pyfunction]
pub fn is_notifications_enabled<'py>(obj: &Bound<'py, AtorsBase>) -> bool {
    notifications_enabled(obj)
}
