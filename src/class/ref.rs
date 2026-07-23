/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
/// Lightweight non-owning handle for Ators instances.
use pyo3::{
    Bound, Py, PyAny, PyResult, Python, exceptions::PyTypeError, pyclass, pyfunction, pymethods,
    types::PyAnyMethods,
};
use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

use super::base::AtorsBase;

static REF_REGISTRY: OnceLock<Mutex<HashSet<usize>>> = OnceLock::new();

fn ref_registry() -> &'static Mutex<HashSet<usize>> {
    REF_REGISTRY.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(crate) fn register_ators_instance(target: &Bound<'_, AtorsBase>) -> PyResult<()> {
    let mut registry = ref_registry().lock().expect("ref registry lock poisoned");
    registry.insert(target.as_ptr() as usize);
    Ok(())
}

/// Called from __clear__ to invalidate the registry entry for this object pointer.
/// This is idempotent—if the entry is not in the registry, remove() is a no-op.
pub(crate) fn unregister_ators_instance(target: &Bound<'_, AtorsBase>) {
    let mut registry = ref_registry().lock().expect("ref registry lock poisoned");
    let _ = registry.remove(&(target.as_ptr() as usize));
}

pub(crate) fn instance_ref_active_for_ptr(target_ptr: usize) -> bool {
    let registry = ref_registry().lock().expect("ref registry lock poisoned");
    registry.contains(&target_ptr)
}

#[pyclass(module = "ators._ators", frozen, immutable_type)]
pub struct AtorsRef {
    /// Raw pointer to the target AtorsBase object, stored as usize.
    /// Validity is determined by presence in the registry; if absent, the ref is inert.
    target: Option<usize>,
}

#[pymethods]
impl AtorsRef {
    #[new]
    #[pyo3(signature = (obj))]
    fn new(obj: &Bound<'_, PyAny>) -> PyResult<Self> {
        let instance = obj
            .cast::<AtorsBase>()
            .map_err(|_| PyTypeError::new_err("Expected an Ators instance"))?;
        register_ators_instance(instance)?;
        let target = instance.as_ptr().cast::<()>() as usize;
        Ok(Self {
            target: Some(target),
        })
    }

    #[pyo3(signature = ())]
    fn __call__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let Some(target_ptr) = self.target else {
            return Ok(py.None());
        };
        // Check registry: if not marked as valid, return None
        if !instance_ref_active_for_ptr(target_ptr) {
            self.target = None; // Invalidate the ref since the target is no longer valid
            return Ok(py.None());
        }
        // Registry says it's valid, safe to construct borrowed reference and return
        let obj = unsafe {
            Bound::<PyAny>::from_borrowed_ptr(py, target_ptr as *mut pyo3::ffi::PyObject)
        };
        Ok(obj.unbind())
    }

    fn __bool__(&self, _py: Python<'_>) -> bool {
        let Some(target_ptr) = self.target else {
            return false;
        };
        // Check if registry marks this pointer as valid
        instance_ref_active_for_ptr(target_ptr)
    }

    fn __repr__(&self, py: Python<'_>) -> String {
        let Some(target_ptr) = self.target else {
            return "AtorsRef(target=None)".to_string();
        };

        // Check registry: if not marked as valid, return None
        if !instance_ref_active_for_ptr(target_ptr) {
            return "AtorsRef(target=None)".to_string();
        }

        // Registry says it's valid, safe to construct borrowed reference and get repr
        let obj = unsafe {
            Bound::<PyAny>::from_borrowed_ptr(py, target_ptr as *mut pyo3::ffi::PyObject)
        };

        // Call the object's __repr__ method
        match obj.repr() {
            Ok(result) => match result.extract::<String>() {
                Ok(repr_str) => format!("AtorsRef(target={})", repr_str),
                Err(_) => "AtorsRef(target=<error>)".to_string(),
            },
            Err(_) => "AtorsRef(target=<error>)".to_string(),
        }
    }
}

#[pyfunction]
pub fn atorsref<'py>(py: Python<'py>, obj: &Bound<'py, PyAny>) -> PyResult<Py<AtorsRef>> {
    let ref_obj = AtorsRef::new(obj)?;
    Ok(Bound::new(py, ref_obj)?.unbind())
}
