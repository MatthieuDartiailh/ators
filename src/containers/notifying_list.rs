/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
use pyo3::{
    Bound, IntoPyObjectExt, Py, PyAny, PyErr, PyResult, Python, ffi, intern, pyclass, pymethods,
    sync::critical_section::with_critical_section,
    types::{PyAnyMethods, PyList, PyListMethods, PyType},
};
use std::cell::UnsafeCell;

use crate::{
    class::AtorsBase,
    containers::{
        AtorsDict, AtorsList, AtorsSet,
        common::{
            ContainerOperation, NotificationBuffer, NotificationState, matches_assignment_context,
            notification_context,
        },
    },
    utils::error_on_minusone,
    validators::Validator,
};

// ============================================================================
// NotifyingList - List with detailed change notifications
// ============================================================================
#[pyclass(module = "ators._ators", frozen)]
pub struct NotifyingListBatchNotificationsContext {
    notifying_list: Py<NotifyingList>,
}

#[pymethods]
impl NotifyingListBatchNotificationsContext {
    pub fn __enter__<'py>(
        self_: &Bound<'py, NotifyingListBatchNotificationsContext>,
    ) -> Bound<'py, NotifyingListBatchNotificationsContext> {
        let notifying_list = self_.get().notifying_list.bind(self_.py());
        notifying_list.get().begin_batch_inner(notifying_list);
        Bound::clone(self_)
    }

    pub fn __exit__(
        self_: &Bound<'_, NotifyingListBatchNotificationsContext>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let notifying_list = self_.get().notifying_list.bind(self_.py());
        notifying_list
            .get()
            .end_batch_inner(self_.py(), notifying_list)?;
        Ok(false)
    }
}

#[pyclass(module = "ators._ators", extends=PyList, frozen)]
pub struct NotifyingList {
    validator: UnsafeCell<Validator>,
    member_name: UnsafeCell<Option<String>>,
    // Wrapped in UnsafeCell to allow clearing during GC while keeping the class frozen.
    object: UnsafeCell<Option<Py<AtorsBase>>>,
    notification_buffer: UnsafeCell<NotificationBuffer>,
}

// Safety: validator and member_name are written only once (at construction or during restore
// before any other references exist), and after that are effectively immutable; object is only
// modified during __clear__, which Python's GC calls only once all references to this object
// have been dropped — ensuring no concurrent access (holds for both GIL and free-threaded builds).
// notification_state and pending_operations are protected by critical sections.
unsafe impl Sync for NotifyingList {}

impl NotifyingList {
    pub(crate) fn new_empty<'py>(
        py: Python<'py>,
        validator: Validator,
        member_name: Option<&str>,
        object: Option<Py<AtorsBase>>,
    ) -> PyResult<Bound<'py, NotifyingList>> {
        Bound::new(
            py,
            NotifyingList {
                validator: UnsafeCell::new(validator),
                member_name: UnsafeCell::new(member_name.map(|m| m.to_string())),
                object: UnsafeCell::new(object),
                notification_buffer: UnsafeCell::new(NotificationBuffer::new()),
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
        matches_assignment_context(&self.member_name, &self.object, member_name, object)
    }

    pub(crate) fn clone_for_assignment<'py>(
        source: &Bound<'py, NotifyingList>,
    ) -> PyResult<Bound<'py, NotifyingList>> {
        let list = source.get();
        // Safety: same as validate_item.
        let validator = unsafe { &*list.validator.get() }.clone();
        let member_name = unsafe { &*list.member_name.get() }
            .as_deref()
            .map(|s| s.to_string());
        let object = unsafe { &*list.object.get() }
            .as_ref()
            .map(|object| object.clone_ref(source.py()));
        let alist =
            NotifyingList::new_empty(source.py(), validator, member_name.as_deref(), object)?;
        // Safety: NotifyingList is declared as `extends=PyList`, so this cast is always valid.
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
        alist: &Bound<'py, NotifyingList>,
        validator: Validator,
        member_name: Option<&str>,
        object: Option<&Bound<'py, AtorsBase>>,
    ) {
        use crate::validators::types::TypeValidator;

        // Capture the validator so we can restore nested containers after
        // rebinding this list metadata.
        let item_v = validator.clone();

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
        // Safety: NotifyingList is declared as `extends=PyList`, so this cast is always valid.
        let py_list = unsafe { alist.cast_unchecked::<PyList>() };
        for list_item in py_list.iter() {
            match &item_v.type_validator {
                TypeValidator::List {
                    item: Some(nested_bv),
                } => {
                    if let Ok(nested) = list_item.cast::<AtorsList>() {
                        AtorsList::restore(nested, (*nested_bv.0).clone(), member_name, object);
                    }
                }
                TypeValidator::NotifyingList {
                    item: Some(nested_bv),
                } => {
                    if let Ok(nested) = list_item.cast::<NotifyingList>() {
                        NotifyingList::restore(nested, (*nested_bv.0).clone(), member_name, object);
                    }
                }
                TypeValidator::Set {
                    item: Some(nested_bv),
                } => {
                    if let Ok(nested) = list_item.cast::<AtorsSet>() {
                        AtorsSet::restore(nested, (*nested_bv.0).clone(), member_name, object);
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
                _ => {}
            }
        }
    }

    /// Record an operation and either emit immediately or accumulate for batch.
    fn record_operation<'py>(
        &self,
        py: Python<'py>,
        operation: ContainerOperation,
        self_bound: &Bound<'py, NotifyingList>,
    ) -> PyResult<()> {
        let should_emit = with_critical_section(self_bound.as_any(), || unsafe {
            let buffer = &mut *self.notification_buffer.get();
            buffer.record_operation(operation.clone()).is_some()
        });

        if should_emit {
            let Some((object, member_name)) = notification_context(&self.member_name, &self.object)
            else {
                return Ok(());
            };
            let py_list = unsafe { self_bound.cast_unchecked::<PyList>() };
            let newvalue: Py<PyAny> = py_list.clone().unbind().into();
            NotificationBuffer::emit_container_change(
                py,
                object.bind(py),
                &member_name,
                newvalue,
                vec![operation],
            )
        } else {
            Ok(())
        }
    }

    /// Emit a notification with the given operations.
    fn emit_notification<'py>(
        &self,
        py: Python<'py>,
        operations: Vec<ContainerOperation>,
        self_bound: &Bound<'py, NotifyingList>,
    ) -> PyResult<()> {
        let Some((object, member_name)) = notification_context(&self.member_name, &self.object)
        else {
            return Ok(());
        };
        let py_list = unsafe { self_bound.cast_unchecked::<PyList>() };
        let newvalue: Py<PyAny> = py_list.clone().unbind().into();
        NotificationBuffer::emit_container_change(
            py,
            object.bind(py),
            &member_name,
            newvalue,
            operations,
        )
    }

    fn del_item_base<'py>(py_list: &Bound<'py, PyList>, index: usize) -> PyResult<()> {
        error_on_minusone(py_list.py(), unsafe {
            ffi::PyList_SetSlice(
                py_list.as_ptr(),
                index as ffi::Py_ssize_t,
                (index + 1) as ffi::Py_ssize_t,
                std::ptr::null_mut(),
            )
        })
    }

    /// Enter batch mode: start accumulating operations.
    fn begin_batch_inner(&self, self_bound: &Bound<'_, NotifyingList>) {
        with_critical_section(self_bound.as_any(), || unsafe {
            (*self.notification_buffer.get()).begin_batch();
        });
    }

    /// Exit batch mode: emit accumulated operations and return to normal mode.
    fn end_batch_inner<'py>(
        &self,
        py: Python<'py>,
        self_bound: &Bound<'py, NotifyingList>,
    ) -> PyResult<()> {
        let operations = with_critical_section(self_bound.as_any(), || unsafe {
            (*self.notification_buffer.get()).end_batch()
        });

        if !operations.is_empty() {
            self.emit_notification(py, operations, self_bound)?;
        }

        Ok(())
    }
}

// Standard list mutation methods with notification recording
#[pymethods]
impl NotifyingList {
    pub fn append<'py>(
        self_: &Bound<'py, NotifyingList>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = value.py();
        let valid = self_.get().validate_item(py, value)?;
        let py_list = unsafe { self_.cast_unchecked::<PyList>() };
        let index = py_list.len();
        py_list.append(&valid)?;

        let operation = ContainerOperation::Added {
            index,
            payload: valid.unbind(),
        };
        self_.get().record_operation(py, operation, self_)
    }

    pub fn insert<'py>(
        self_: &Bound<'py, NotifyingList>,
        index: usize,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = value.py();
        let valid = self_.get().validate_item(py, value)?;
        let py_list = unsafe { self_.cast_unchecked::<PyList>() };
        py_list.insert(index, &valid)?;

        let operation = ContainerOperation::Added {
            index,
            payload: valid.unbind(),
        };
        self_.get().record_operation(py, operation, self_)
    }

    pub fn extend<'py>(
        self_: &Bound<'py, NotifyingList>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = value.py();
        let validated = self_.get().validate_iterable(py, value)?;
        let py_list = unsafe { self_.cast_unchecked::<PyList>() };
        let start_index = py_list.len();

        for item in validated.iter() {
            py_list.append(item)?;
        }

        for (offset, item) in validated.iter().enumerate() {
            let operation = ContainerOperation::Added {
                index: start_index + offset,
                payload: item.unbind(),
            };
            self_.get().record_operation(py, operation, self_)?;
        }

        Ok(())
    }

    pub fn remove<'py>(
        self_: &Bound<'py, NotifyingList>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = value.py();
        let py_list = unsafe { self_.cast_unchecked::<PyList>() };

        // Find the index
        let mut found_index = None;
        for (i, item) in py_list.iter().enumerate() {
            if item.eq(value)? {
                found_index = Some(i);
                break;
            }
        }

        if let Some(index) = found_index {
            let item = py_list.get_item(index)?;
            Self::del_item_base(py_list, index)?;

            let operation = ContainerOperation::Removed {
                old_index: index,
                payload: item.unbind(),
            };
            self_.get().record_operation(py, operation, self_)
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "list.remove(x): x not in list",
            ))
        }
    }

    pub fn pop<'py>(
        self_: &Bound<'py, NotifyingList>,
        index: Option<isize>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let py_list = unsafe { self_.cast_unchecked::<PyList>() };
        let len = py_list.len() as isize;

        let idx = match index {
            Some(i) => {
                let normalized = if i < 0 { len + i } else { i };
                if normalized < 0 || normalized >= len {
                    return Err(PyErr::new::<pyo3::exceptions::PyIndexError, _>(
                        "pop index out of range",
                    ));
                }
                normalized as usize
            }
            None => (len - 1) as usize,
        };

        let item = py_list.get_item(idx)?;
        Self::del_item_base(py_list, idx)?;

        let operation = ContainerOperation::Removed {
            old_index: idx,
            payload: item.clone().unbind(),
        };
        self_.get().record_operation(py, operation, self_)?;

        Ok(item)
    }

    pub fn clear<'py>(self_: &Bound<'py, NotifyingList>) -> PyResult<()> {
        let py = self_.py();
        let py_list = unsafe { self_.cast_unchecked::<PyList>() };

        // Record removal operations for each item (in reverse order)
        // Delete from the end backwards to maintain correct indices
        while py_list.len() > 0 {
            let index = py_list.len() - 1;
            let item = py_list.get_item(index)?;
            Self::del_item_base(py_list, index)?;

            let operation = ContainerOperation::Removed {
                old_index: index,
                payload: item.unbind(),
            };
            self_.get().record_operation(py, operation, self_)?;
        }

        Ok(())
    }

    /// New move method: move an item from one index to another.
    pub fn move_item<'py>(
        self_: &Bound<'py, NotifyingList>,
        from_index: usize,
        to_index: usize,
    ) -> PyResult<()> {
        let py = self_.py();
        let py_list = unsafe { self_.cast_unchecked::<PyList>() };
        let len = py_list.len();

        if from_index >= len || to_index >= len {
            return Err(PyErr::new::<pyo3::exceptions::PyIndexError, _>(
                "move index out of range",
            ));
        }

        if from_index == to_index {
            return Ok(());
        }

        let item = py_list.get_item(from_index)?;
        Self::del_item_base(py_list, from_index)?;
        py_list.insert(to_index, &item)?;

        let operation = ContainerOperation::Moved {
            from_index,
            to_index,
            payload: item.unbind(),
        };
        self_.get().record_operation(py, operation, self_)
    }

    pub fn __setitem__<'py>(
        self_: &Bound<'py, NotifyingList>,
        index: usize,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = value.py();
        let valid = self_.get().validate_item(py, value)?;
        let py_list = unsafe { self_.cast_unchecked::<PyList>() };

        if index >= py_list.len() {
            return Err(PyErr::new::<pyo3::exceptions::PyIndexError, _>(
                "list index out of range",
            ));
        }

        py_list.set_item(index, &valid)?;

        let operation = ContainerOperation::Added {
            index,
            payload: valid.unbind(),
        };
        self_.get().record_operation(py, operation, self_)
    }

    pub fn __delitem__<'py>(self_: &Bound<'py, NotifyingList>, index: usize) -> PyResult<()> {
        let py = self_.py();
        let py_list = unsafe { self_.cast_unchecked::<PyList>() };

        if index >= py_list.len() {
            return Err(PyErr::new::<pyo3::exceptions::PyIndexError, _>(
                "list index out of range",
            ));
        }

        let item = py_list.get_item(index)?;
        Self::del_item_base(py_list, index)?;

        let operation = ContainerOperation::Removed {
            old_index: index,
            payload: item.unbind(),
        };
        self_.get().record_operation(py, operation, self_)
    }

    pub fn __iadd__<'py>(
        self_: &Bound<'py, NotifyingList>,
        other: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        NotifyingList::extend(self_, other)
    }

    pub fn __imul__<'py>(self_: &Bound<'py, NotifyingList>, count: isize) -> PyResult<()> {
        let py = self_.py();
        let py_list = unsafe { self_.cast_unchecked::<PyList>() };

        if count <= 0 {
            let items: Vec<_> = py_list.iter().collect();
            let indices: Vec<_> = (0..items.len()).collect();
            // Delete from the end backwards to maintain correct indices
            for (idx, item) in indices.into_iter().zip(items).rev() {
                Self::del_item_base(py_list, idx)?;
                let operation = ContainerOperation::Removed {
                    old_index: idx,
                    payload: item.unbind(),
                };
                self_.get().record_operation(py, operation, self_)?;
            }
        } else if count > 1 {
            let original: Vec<_> = py_list.iter().map(|i| i.clone().unbind()).collect();

            for _ in 1..count {
                let start_index = py_list.len();
                for (offset, item_py) in original.iter().enumerate() {
                    let item = item_py.bind(py);
                    py_list.append(item)?;

                    let operation = ContainerOperation::Added {
                        index: start_index + offset,
                        payload: item_py.clone_ref(py),
                    };
                    self_.get().record_operation(py, operation, self_)?;
                }
            }
        }

        Ok(())
    }

    pub fn begin_batch_notifications(self_: &Bound<'_, NotifyingList>) {
        self_.get().begin_batch_inner(self_);
    }

    pub fn end_batch(self_: &Bound<'_, NotifyingList>) -> PyResult<()> {
        self_.get().end_batch_inner(self_.py(), self_)
    }

    pub fn batched_notifications<'py>(
        self_: &Bound<'py, NotifyingList>,
    ) -> PyResult<Bound<'py, NotifyingListBatchNotificationsContext>> {
        Bound::new(
            self_.py(),
            NotifyingListBatchNotificationsContext {
                notifying_list: self_.clone().unbind(),
            },
        )
    }

    #[classmethod]
    pub fn __class_getitem__<'py>(
        cls: &Bound<'py, PyType>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = item.py();
        let generic_alias = py
            .import(intern!(py, "types"))?
            .getattr(intern!(py, "GenericAlias"))?;
        generic_alias.call1((cls, item))
    }

    #[staticmethod]
    pub fn _construct<'py>(
        py: Python<'py>,
        _args: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, NotifyingList>> {
        use crate::validators::types::TypeValidator;
        Bound::new(
            py,
            NotifyingList {
                validator: UnsafeCell::new(Validator {
                    type_validator: TypeValidator::Any {},
                    value_validators: Box::new([]),
                    coercer: None,
                    init_coercer: None,
                }),
                member_name: UnsafeCell::new(None),
                object: UnsafeCell::new(None),
                notification_buffer: UnsafeCell::new(NotificationBuffer::new()),
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
