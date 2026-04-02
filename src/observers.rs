/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
/// Observer support for Ators objects.
use pyo3::{
    Bound, Py, PyAny, PyErr, PyResult, Python, intern, pyclass, pymethods,
    sync::critical_section::with_critical_section, types::PyAnyMethods,
};
use std::{cell::UnsafeCell, collections::HashMap};

use crate::core::AtorsBase;

#[pyclass(module = "ators._ators", frozen, get_all)]
#[derive(Debug)]
pub struct AtorsChange {
    object: Py<AtorsBase>,
    member_name: String,
    oldvalue: Py<PyAny>,
    newvalue: Py<PyAny>,
}

impl AtorsChange {
    pub(crate) fn new(
        object: Py<AtorsBase>,
        member_name: String,
        oldvalue: Py<PyAny>,
        newvalue: Py<PyAny>,
    ) -> Self {
        Self {
            object,
            member_name,
            oldvalue,
            newvalue,
        }
    }
}

enum ObserverCallback {
    WeakMethod { weak_method: Py<PyAny> },
    Callable { callable: Py<PyAny> },
}

impl Clone for ObserverCallback {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            Self::WeakMethod { weak_method } => Self::WeakMethod {
                weak_method: weak_method.clone_ref(py),
            },
            Self::Callable { callable } => Self::Callable {
                callable: callable.clone_ref(py),
            },
        })
    }
}

#[pyclass(module = "ators._ators", frozen)]
pub struct ObserverPool {
    callbacks: UnsafeCell<HashMap<String, Vec<ObserverCallback>>>,
}

// Safety: all mutations of callbacks are protected by critical sections.
unsafe impl Sync for ObserverPool {}

impl ObserverPool {
    pub(crate) fn new() -> Self {
        Self {
            callbacks: UnsafeCell::new(HashMap::new()),
        }
    }

    pub(crate) fn add<'py>(
        pool: &Bound<'py, ObserverPool>,
        member_name: &str,
        callback: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = pool.py();
        with_critical_section(pool.as_any(), || {
            let callbacks = unsafe { &mut *pool.get().callbacks.get() };
            let observers = callbacks.entry(member_name.to_string()).or_default();
            observers.retain(|observer| match observer {
                ObserverCallback::Callable { .. } => true,
                ObserverCallback::WeakMethod { weak_method } => weak_method
                    .bind(py)
                    .call0()
                    .map(|target| !target.is_none())
                    .unwrap_or(true),
            });

            let method_type = py
                .import(intern!(py, "types"))?
                .getattr(intern!(py, "MethodType"))?;
            if callback.is_instance(&method_type)? {
                let weak_method = py
                    .import(intern!(py, "weakref"))?
                    .getattr(intern!(py, "WeakMethod"))?
                    .call1((callback,))?
                    .unbind();
                observers.push(ObserverCallback::WeakMethod { weak_method });
            } else {
                observers.push(ObserverCallback::Callable {
                    callable: callback.clone().unbind(),
                });
            }

            Ok(())
        })
    }

    pub(crate) fn remove<'py>(
        pool: &Bound<'py, ObserverPool>,
        member_name: &str,
        callback: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = pool.py();
        with_critical_section(pool.as_any(), || {
            let callbacks = unsafe { &mut *pool.get().callbacks.get() };
            if let Some(observers) = callbacks.get_mut(member_name) {
                let mut kept = Vec::with_capacity(observers.len());
                let mut removed = false;

                for observer in observers.drain(..) {
                    match &observer {
                        ObserverCallback::Callable { callable } => {
                            if !removed && callable.bind(py).as_ptr() == callback.as_ptr() {
                                removed = true;
                                continue;
                            }
                            kept.push(observer);
                        }
                        ObserverCallback::WeakMethod { weak_method } => {
                            let target = weak_method.bind(py).call0()?;
                            if target.is_none() {
                                continue;
                            }
                            if !removed && target.as_ptr() == callback.as_ptr() {
                                removed = true;
                                continue;
                            }
                            kept.push(observer);
                        }
                    }
                }

                *observers = kept;
                if observers.is_empty() {
                    callbacks.remove(member_name);
                }
            }
            Ok(())
        })
    }

    pub(crate) fn fire<'py>(
        pool: &Bound<'py, ObserverPool>,
        member_name: &str,
        change: &Bound<'py, AtorsChange>,
    ) -> PyResult<Vec<PyErr>> {
        let py = pool.py();
        let observers = with_critical_section(pool.as_any(), || unsafe {
            (*pool.get().callbacks.get())
                .get(member_name)
                .cloned()
                .unwrap_or_default()
        });

        let mut errors = Vec::new();
        for observer in observers {
            match observer {
                ObserverCallback::Callable { callable } => {
                    if let Err(err) = callable.bind(py).call1((change,)) {
                        errors.push(err);
                    }
                }
                ObserverCallback::WeakMethod { weak_method } => {
                    match weak_method.bind(py).call0() {
                        Ok(cb) => {
                            if !cb.is_none()
                                && let Err(err) = cb.call1((change,))
                            {
                                errors.push(err);
                            }
                        }
                        Err(err) => errors.push(err),
                    }
                }
            }
        }

        with_critical_section(pool.as_any(), || {
            let callbacks = unsafe { &mut *pool.get().callbacks.get() };
            if let Some(observers) = callbacks.get_mut(member_name) {
                observers.retain(|observer| match observer {
                    ObserverCallback::Callable { .. } => true,
                    ObserverCallback::WeakMethod { weak_method } => weak_method
                        .bind(py)
                        .call0()
                        .map(|target| !target.is_none())
                        .unwrap_or(true),
                });
                if observers.is_empty() {
                    callbacks.remove(member_name);
                }
            }
        });

        Ok(errors)
    }
}

#[pymethods]
impl ObserverPool {
    pub fn __traverse__(&self, visit: pyo3::PyVisit) -> Result<(), pyo3::PyTraverseError> {
        let callbacks = unsafe { &*self.callbacks.get() };
        for observers in callbacks.values() {
            for observer in observers {
                match observer {
                    ObserverCallback::WeakMethod { weak_method } => visit.call(weak_method)?,
                    ObserverCallback::Callable { callable } => visit.call(callable)?,
                }
            }
        }
        Ok(())
    }

    pub fn __clear__(&self) {
        unsafe {
            (*self.callbacks.get()).clear();
        }
    }
}
