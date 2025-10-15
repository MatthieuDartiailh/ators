/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Matthieu C. Dartiailh
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{
    Bound, IntoPyObjectExt, Py, PyAny, PyResult, Python, intern, pyclass, pyfunction, pymethods,
    sync::with_critical_section,
    types::{PyAnyMethods, PyDict, PyDictMethods, PyString, PyType, PyTypeMethods},
};

use crate::member::{Member, member_coerce_init};

// FIXME reduce memory footprint
// See for initializing allocated memory https://docs.rs/init_array/latest/src/init_array/stable.rs.html#71-95
// But we need to understand how to make it Send and Sync first

pub static ATORS_MEMBERS: &str = "__atom_members__";

#[pyclass(subclass)]
pub struct BaseAtors {
    frozen: bool,
    notification_enabled: bool,
    slots: Box<[Option<Py<PyAny>>]>,
}

#[pymethods]
impl BaseAtors {
    #[new]
    #[pyo3(signature = (**_kwargs))]
    #[classmethod]
    fn py_new(cls: &Bound<'_, PyType>, _kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let py = cls.py();
        let slots_count = cls
            .getattr(intern!(py, ATORS_MEMBERS))?
            .extract::<u8>()
            .map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err(format!(
                    "The class {} has more than 255 members which is not supported.",
                    cls.name().unwrap_or(PyString::new(py, "<unknown>"))
                ))
            })?;
        // NOTE using a boxed slice is suboptimal size wise since we do not need a usize
        // when limiting ourselves to 255 members but it is the easiest way to have
        // a fixed size array without using unsafe code.
        // We can revisit this later if needed.
        let slots = (0..slots_count).map(|_| None).collect();
        Ok(Self {
            frozen: false,
            notification_enabled: false,
            slots,
        })
    }

    pub fn __traverse__(&self, visit: pyo3::PyVisit) -> Result<(), pyo3::PyTraverseError> {
        for slot in self.slots.iter().flatten() {
            visit.call(slot)?;
        }
        Ok(())
    }

    pub fn __clear__(&mut self) {
        for o in self.slots.iter_mut() {
            o.take();
        }
    }
}

impl BaseAtors {
    /// Get a clone (ref) of the value stored in the slot at index if any
    pub(crate) fn get_slot<'py>(&self, index: usize, py: Python<'py>) -> Option<Py<PyAny>> {
        self.slots[index].as_ref().map(|v| v.clone_ref(py))
    }

    /// Set the slot at index to the specified value
    pub(crate) fn set_slot<'py>(&mut self, index: usize, value: Bound<'py, PyAny>) {
        let py = value.py();
        // This conversion cannot fail, so unwrap is safe
        self.slots[index].replace(value.into_py_any(py).unwrap());
    }

    /// Check if the slot at index stores a non-None value
    pub(crate) fn is_slot_set(&self, index: usize) -> bool {
        self.slots[index].is_some()
    }

    #[inline]
    pub(crate) fn is_frozen(&self) -> bool {
        self.frozen
    }
}

#[pyfunction]
pub fn init_ators<'py>(self_: Bound<'py, BaseAtors>, kwargs: Bound<'py, PyDict>) -> PyResult<()> {
    let members = self_.getattr(ATORS_MEMBERS)?;
    for (k, v) in kwargs.cast::<PyDict>()?.iter() {
        let key = k.cast::<PyString>()?;
        {
            match self_.setattr(key, v.clone()) {
                Ok(_) => Ok(()),
                Err(err) => {
                    let m = members.as_any().get_item(key)?.cast_into::<Member>()?;
                    if let Some(r) = member_coerce_init(&m, &self_, v) {
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

#[pyfunction]
pub fn freeze<'py>(obj: Bound<'py, BaseAtors>) {
    with_critical_section(&obj, || {
        obj.borrow_mut().frozen = true;
    });
}

#[pyfunction]
pub fn is_frozen<'py>(obj: Bound<'py, BaseAtors>) -> bool {
    with_critical_section(&obj, || {
        return obj.borrow().frozen;
    })
}

// FIXME re-enable once notification are implemented
// #[pyfunction]
// pub fn enable_notification<'py>(obj: Bound<'py, BaseAtors>) {
//     with_critical_section(&obj, || {
//         obj.borrow_mut().notification_enabled = true;
//     });
// }

// #[pyfunction]
// pub fn disable_notification<'py>(obj: Bound<'py, BaseAtors>) {
//     with_critical_section(&obj, || {
//         obj.borrow_mut().notification_enabled = false;
//     });
// }

// #[pyfunction]
// pub fn is_notification_enabled<'py>(obj: Bound<'py, BaseAtors>) -> bool {
//     with_critical_section(&obj, || {
//         return obj.borrow().notification_enabled;
//     })
// }

// XXX add member access functions (with tag filtering)
