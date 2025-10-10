///
use pyo3::{
    Bound, IntoPyObjectExt, Py, PyAny, PyResult, Python, intern, pyclass, pyfunction, pymethods,
    sync::with_critical_section,
    types::{PyAnyMethods, PyString, PyType, PyTypeMethods},
};

// FIXME reduce memory footprint
// See for initializing allocated memory https://docs.rs/init_array/latest/src/init_array/stable.rs.html#71-95
// But we need to understand how to make it Send and Sync first

static ATORS_MEMBERS: &str = "__atom_members__";

#[pyclass(subclass)]
pub struct BaseAtors {
    frozen: bool,
    notification_enabled: bool,
    slots: Box<[Option<Py<PyAny>>]>,
}

#[pymethods]
impl BaseAtors {
    #[new]
    #[classmethod]
    fn py_new(cls: &Bound<'_, PyType>) -> PyResult<Self> {
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
