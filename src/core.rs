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
    types::{PyAnyMethods, PyDict, PyDictMethods, PyString, PyType, PyTypeMethods},
};
use std::cell::UnsafeCell;

use crate::get_type_mutability_map;
use crate::member::{Member, MemberCustomizationTool, member_coerce_init};
use crate::observers::{AtorsChange, ObserverPool};
use crate::utils::Mutability;

pub static ATORS_MEMBERS: &str = "__ators_members__";
pub static ATORS_MEMBER_CUSTOMIZER: &str = "__ators_member_customizer__";
pub static ATORS_MEMBERS_MUTABILITY: &str = "__ators_members_mutability__";
pub static ATORS_OBSERVABLE: &str = "__ators_observable__";

/// Inner mutable state of an AtorsBase instance, stored in an UnsafeCell to allow
/// interior mutability while keeping AtorsBase frozen.
struct InnerAtors {
    frozen: bool,
    notification_enabled: bool,
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

#[pymethods]
impl AtorsBase {
    #[new]
    #[pyo3(signature = (**_kwargs))]
    #[classmethod]
    fn py_new(cls: &Bound<'_, PyType>, _kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let py = cls.py();
        let slots_count = cls.getattr(intern!(py, ATORS_MEMBERS))?.len()?;
        let is_observable = class_is_observable(cls)?;
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

fn class_is_observable(cls: &Bound<'_, PyType>) -> PyResult<bool> {
    Ok(cls
        .getattr(ATORS_OBSERVABLE)
        .ok()
        .and_then(|v| v.extract::<bool>().ok())
        .unwrap_or(false))
}

pub(crate) fn notifications_enabled(obj: &Bound<'_, AtorsBase>) -> bool {
    with_critical_section(obj.as_any(), || {
        // Safety: we hold the critical section lock on this object.
        unsafe { (*obj.get().inner.get()).notification_enabled }
    })
}

pub(crate) fn get_observer_pool<'py>(
    obj: &Bound<'py, AtorsBase>,
) -> PyResult<Option<Bound<'py, ObserverPool>>> {
    let py = obj.py();
    let pool_obj = with_critical_section(obj.as_any(), || {
        // Safety: we hold the critical section lock on this object.
        unsafe {
            (*obj.get().inner.get())
                .slots
                .first()
                .and_then(|item| item.as_ref().map(|item| item.clone_ref(py)))
        }
    });
    if let Some(pool) = pool_obj {
        Ok(Some(pool.bind(py).cast::<ObserverPool>()?.clone()))
    } else {
        Ok(None)
    }
}

pub(crate) fn notify_member_change<'py>(
    obj: &Bound<'py, AtorsBase>,
    member_name: &str,
    oldvalue: Py<PyAny>,
    newvalue: Py<PyAny>,
) -> PyResult<()> {
    if !notifications_enabled(obj) {
        return Ok(());
    }

    if let Some(pool) = get_observer_pool(obj)? {
        let py = obj.py();
        let change = Bound::new(
            py,
            AtorsChange::new(
                obj.clone().into_any().unbind(),
                member_name.to_string(),
                oldvalue,
                newvalue,
            ),
        )?;
        let errors = ObserverPool::fire(&pool, member_name, &change)?;
        if !errors.is_empty() {
            let exception_group = py
                .import(intern!(py, "builtins"))?
                .getattr(intern!(py, "ExceptionGroup"))?
                .call1(("errors in observers", errors))?;
            return Err(pyo3::PyErr::from_value(exception_group));
        }
    }
    Ok(())
}

// FIXME move once #[init] has landed
#[pyfunction]
pub fn init_ators<'py>(self_: &Bound<'py, AtorsBase>, kwargs: &Bound<'py, PyDict>) -> PyResult<()> {
    let members = self_.getattr(ATORS_MEMBERS)?;
    for (k, v) in kwargs.cast::<PyDict>()?.iter() {
        let key = k.cast::<PyString>()?;
        {
            match self_.setattr(key, v.clone()) {
                Ok(_) => Ok(()),
                Err(err) => {
                    let m = members.as_any().get_item(key)?.cast_into::<Member>()?;
                    if let Some(r) = member_coerce_init(&m, self_, &v) {
                        let coerced_v = r?;
                        self_.setattr(key, coerced_v).map(|_| ())
                    } else {
                        Err(err)
                    }
                }
            }
        }?
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
    Ok(obj
        .getattr(ATORS_MEMBERS)?
        .get_item(member_name)?
        .cast_into()?)
}

/// Retrieve all members from an Ators objetc.
#[pyfunction]
pub fn get_members<'py>(obj: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    obj.getattr(ATORS_MEMBERS)?.cast::<PyDict>()?.copy()
}

/// Retrieve all members with a specific metadata key and the value associated with it.
#[pyfunction]
pub fn get_members_by_tag<'py>(
    obj: &Bound<'py, PyAny>,
    tag: String,
) -> PyResult<Bound<'py, PyDict>> {
    let py = obj.py();
    let members = PyDict::new(obj.py());
    for (k, v) in obj.getattr(ATORS_MEMBERS)?.cast::<PyDict>()?.iter() {
        if let Some(m) = v.cast::<Member>()?.get().metadata()
            && m.contains_key(&tag)
        {
            members.set_item(&k, (v.clone(), m[&tag].clone_ref(py)))?;
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
    for (k, member) in obj.getattr(ATORS_MEMBERS)?.cast::<PyDict>()?.iter() {
        if let Some(m) = member.cast::<Member>()?.get().metadata()
            && m.contains_key(&tag)
            // If comparison fails the member should not be included
            && value.as_any().eq(&m[&tag]).unwrap_or(false)
        {
            members.set_item(&k, member.clone())?;
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
    let obj_type = obj.get_type();
    if !class_is_observable(&obj_type)? {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "Cannot register observers on a non-observable class",
        ));
    }

    let members_obj = obj_type.getattr(ATORS_MEMBERS)?;
    let members = members_obj.cast::<PyDict>()?;
    if !members.contains(&member_name)? {
        return Err(pyo3::exceptions::PyAttributeError::new_err(format!(
            "Unknown member '{member_name}'"
        )));
    }

    if let Some(pool) = get_observer_pool(obj)? {
        ObserverPool::add(&pool, &member_name, callback)
    } else {
        Err(pyo3::exceptions::PyRuntimeError::new_err(
            "Observable instance is missing its observer pool",
        ))
    }
}

#[pyfunction]
pub fn unobserve<'py>(
    obj: &Bound<'py, AtorsBase>,
    member_name: String,
    callback: &Bound<'py, PyAny>,
) -> PyResult<()> {
    let obj_type = obj.get_type();
    if !class_is_observable(&obj_type)? {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "Cannot unregister observers on a non-observable class",
        ));
    }

    if let Some(pool) = get_observer_pool(obj)? {
        ObserverPool::remove(&pool, &member_name, callback)
    } else {
        Err(pyo3::exceptions::PyRuntimeError::new_err(
            "Observable instance is missing its observer pool",
        ))
    }
}

#[pyfunction]
pub fn enable_notifications<'py>(obj: &Bound<'py, AtorsBase>) -> PyResult<()> {
    let obj_type = obj.get_type();
    if !class_is_observable(&obj_type)? {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "Cannot enable notifications on a non-observable class",
        ));
    }

    with_critical_section(obj.as_any(), || {
        // Safety: we hold the critical section lock on this object.
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
