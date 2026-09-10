/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
use pyo3::{
    Bound, Py, PyAny, PyClassInitializer, PyResult, Python, intern, pyclass, pymethods,
    types::PyAnyMethods,
};
use std::cell::UnsafeCell;

use crate::{
    class::{
        AtorsBase,
        base::{get_observer_pool, notifications_enabled},
    },
    observers::AtorsChange,
};

/// Shared operation payload for ordered container mutations.
#[pyclass(module = "ators._ators", frozen, skip_from_py_object)]
#[derive(Debug)]
pub enum ContainerOperation {
    Added {
        index: usize,
        payload: Py<PyAny>,
    },
    Removed {
        old_index: usize,
        payload: Py<PyAny>,
    },
    Moved {
        from_index: usize,
        to_index: usize,
        payload: Py<PyAny>,
    },
}

impl Clone for ContainerOperation {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            ContainerOperation::Added { index, payload } => ContainerOperation::Added {
                index: *index,
                payload: payload.clone_ref(py),
            },
            ContainerOperation::Removed { old_index, payload } => ContainerOperation::Removed {
                old_index: *old_index,
                payload: payload.clone_ref(py),
            },
            ContainerOperation::Moved {
                from_index,
                to_index,
                payload,
            } => ContainerOperation::Moved {
                from_index: *from_index,
                to_index: *to_index,
                payload: payload.clone_ref(py),
            },
        })
    }
}

#[pymethods]
impl ContainerOperation {
    fn __repr__(&self) -> String {
        match self {
            ContainerOperation::Added { index, .. } => format!("Added(index={index})"),
            ContainerOperation::Removed { old_index, .. } => {
                format!("Removed(old_index={old_index})")
            }
            ContainerOperation::Moved {
                from_index,
                to_index,
                ..
            } => format!("Moved(from_index={from_index}, to_index={to_index})"),
        }
    }
}

/// Shared change object for container mutations.
/// Extends `AtorsChange` with a uniform operation list for list/map containers.
#[pyclass(module = "ators._ators", extends=AtorsChange, subclass, frozen)]
pub struct ContainerChange {
    #[pyo3(get)]
    operations: Vec<ContainerOperation>,
}

impl ContainerChange {
    pub(crate) fn new(
        object: Py<AtorsBase>,
        member_name: String,
        oldvalue: Py<PyAny>,
        newvalue: Py<PyAny>,
        operations: Vec<ContainerOperation>,
    ) -> PyClassInitializer<Self> {
        PyClassInitializer::from(AtorsChange::new(object, member_name, oldvalue, newvalue))
            .add_subclass(Self { operations })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum NotificationState {
    #[default]
    Normal,
    Batching,
}

#[derive(Debug, Default)]
pub(crate) struct NotificationBuffer {
    pub(crate) state: NotificationState,
    pub(crate) pending_operations: Vec<ContainerOperation>,
}

impl NotificationBuffer {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn begin_batch(&mut self) {
        self.state = NotificationState::Batching;
    }

    pub(crate) fn end_batch(&mut self) -> Vec<ContainerOperation> {
        self.state = NotificationState::Normal;
        std::mem::take(&mut self.pending_operations)
    }

    pub(crate) fn push_operation(&mut self, operation: ContainerOperation) {
        if self.state == NotificationState::Batching {
            self.pending_operations.push(operation);
        }
    }

    pub(crate) fn emit_container_change<'py>(
        py: Python<'py>,
        object: &Bound<'py, AtorsBase>,
        member_name: &str,
        newvalue: Py<PyAny>,
        operations: Vec<ContainerOperation>,
    ) -> PyResult<()> {
        if !notifications_enabled(object) {
            return Ok(());
        }

        let change = Bound::new(
            py,
            ContainerChange::new(
                object.clone().unbind(),
                member_name.to_owned(),
                py.None(),
                newvalue,
                operations,
            ),
        )?;

        let pool = get_observer_pool(object);
        let base_change = change.cast::<AtorsChange>()?;
        let errors = crate::observers::ObserverPool::fire(pool, member_name, base_change)?;

        if !errors.is_empty() {
            let exception_group = py
                .import(intern!(py, "builtins"))?
                .getattr(intern!(py, "ExceptionGroup"))?
                .call1(("errors in observers", errors))?;
            return Err(pyo3::PyErr::from_value(exception_group));
        }

        Ok(())
    }
}

pub(super) fn matches_assignment_context<'py>(
    member_name_cell: &UnsafeCell<Option<String>>,
    object_cell: &UnsafeCell<Option<Py<AtorsBase>>>,
    member_name: Option<&str>,
    object: Option<&Bound<'py, AtorsBase>>,
) -> bool {
    // Safety: member_name/object cells are initialized during construction and
    // updated only in restore/clear paths under invariants described by callers.
    unsafe { &*member_name_cell.get() }.as_deref() == member_name
        && match (unsafe { &*object_cell.get() }.as_ref(), object) {
            (None, None) => true,
            (Some(stored), Some(current)) => stored.bind(current.py()).as_ptr() == current.as_ptr(),
            _ => false,
        }
}
