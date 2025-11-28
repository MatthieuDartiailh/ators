/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
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

pub static ATORS_MEMBERS: &str = "__ators_members__";

#[pyclass(subclass)]
pub struct AtorsBase {
    frozen: bool,
    notification_enabled: bool,
    slots: Box<[Option<Py<PyAny>>]>,
}

#[pymethods]
impl AtorsBase {
    #[new]
    #[pyo3(signature = (**_kwargs))]
    #[classmethod]
    fn py_new(cls: &Bound<'_, PyType>, _kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let py = cls.py();
        let slots_count = cls.getattr(intern!(py, ATORS_MEMBERS))?.len()?;
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
        let slots = (0..=slots_count).map(|_| None).collect();
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

impl AtorsBase {
    /// Check if a Ators instance is frozen
    #[inline]
    pub(crate) fn is_frozen(&self) -> bool {
        self.frozen
    }
}

/// Get a clone (ref) of the value stored in the slot at index if any
/// A critical section is used only if the object is not frozen.
pub(crate) fn get_slot<'py>(
    object: &Bound<'py, AtorsBase>,
    index: u8,
    py: Python<'py>,
) -> Option<Py<PyAny>> {
    let oref = object.borrow();
    if oref.is_frozen() {
        oref.slots[index as usize].as_ref().map(|v| v.clone_ref(py))
    } else {
        with_critical_section(object, || {
            oref.slots[index as usize].as_ref().map(|v| v.clone_ref(py))
        })
    }
}

/// Set the slot at index to the specified value
pub(crate) fn set_slot<'py>(object: &Bound<'py, AtorsBase>, index: u8, value: Bound<'py, PyAny>) {
    let py = object.py();
    with_critical_section(object, || {
        object.borrow_mut().slots[index as usize].replace(
            value
                .into_py_any(py)
                .expect("Unfaillible conversion to Py<PyAny>"),
        );
    })
}

/// Del the slot value at index
pub(crate) fn del_slot<'py>(object: &Bound<'py, AtorsBase>, index: u8) {
    with_critical_section(object, || {
        object.borrow_mut().slots[index as usize] = None;
    })
}

// FIXME move once #[init] has landed
#[pyfunction]
pub fn init_ators<'py>(self_: Bound<'py, AtorsBase>, kwargs: Bound<'py, PyDict>) -> PyResult<()> {
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
pub fn freeze<'py>(obj: Bound<'py, AtorsBase>) {
    with_critical_section(&obj, || {
        obj.borrow_mut().frozen = true;
    });
}

#[pyfunction]
pub fn is_frozen<'py>(obj: Bound<'py, AtorsBase>) -> bool {
    with_critical_section(&obj, || {
        return obj.borrow().frozen;
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
pub fn get_members<'py>(obj: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    obj.getattr(ATORS_MEMBERS)?.cast::<PyDict>()?.copy()
}

/// Retrieve all members with a specific metadata key and the value associated with it.
#[pyfunction]
pub fn get_members_by_tag<'py>(
    obj: Bound<'py, PyAny>,
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
    obj: Bound<'py, PyAny>,
    tag: String,
    value: Bound<'py, PyAny>,
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

// FIXME re-enable once notification are implemented
// #[pyfunction]
// pub fn enable_notification<'py>(obj: Bound<'py, AtorsBase>) {
//     with_critical_section(&obj, || {
//         obj.borrow_mut().notification_enabled = true;
//     });
// }

// #[pyfunction]
// pub fn disable_notification<'py>(obj: Bound<'py, AtorsBase>) {
//     with_critical_section(&obj, || {
//         obj.borrow_mut().notification_enabled = false;
//     });
// }

// #[pyfunction]
// pub fn is_notification_enabled<'py>(obj: Bound<'py, AtorsBase>) -> bool {
//     with_critical_section(&obj, || {
//         return obj.borrow().notification_enabled;
//     })
// }

// XXX add member access functions (with tag filtering)
