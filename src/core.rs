/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
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

use crate::get_type_mutability_map;
use crate::member::{Member, MemberCustomizationTool, member_coerce_init};
use crate::validators::types::Mutability;

// FIXME reduce memory footprint
// See for initializing allocated memory https://docs.rs/init_array/latest/src/init_array/stable.rs.html#71-95
// But we need to understand how to make it Send and Sync first

pub static ATORS_MEMBERS: &str = "__ators_members__";
pub static ATORS_MEMBER_CUSTOMIZER: &str = "__ators_member_customizer__";
pub static ATORS_MEMBERS_MUTABILITY: &str = "__ators_members_mutability__";

#[pyclass(module = "ators._ators", subclass)]
pub struct AtorsBase {
    frozen: bool,
    notification_enabled: bool,
    slots: Box<[Option<Py<PyAny>>]>,
}

#[pyclass(module = "ators._ators", frozen)]
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

#[inline]
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

#[inline]
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

#[inline]
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

/// Check if an object can contain mutable values.
/// For Ators objects: returns Immutable if frozen, otherwise checks the class mutability.
/// For other objects: uses is_type_mutable and inspects the object if undecidable.
pub(crate) fn is_object_mutable<'py>(
    py: Python<'py>,
    obj: &Bound<'py, PyAny>,
) -> PyResult<Mutability> {
    let obj_type = obj.get_type();
    let ators_base_type = py.get_type::<AtorsBase>();

    if obj_type.is_subclass(&ators_base_type)? {
        // For Ators objects, check if frozen
        let ators_obj = obj.cast::<AtorsBase>()?;
        if ators_obj.borrow().is_frozen() {
            Ok(Mutability::Immutable)
        } else {
            Ok(Mutability::Mutable)
        }
    } else {
        // For other objects, first check type mutability and then inspect object
        // if undecidable
        let type_mutability = crate::validators::types::is_type_mutable(&obj_type);
        if type_mutability == Mutability::Undecidable {
            // If type mutability is undecidable, inspect the object
            let mut_map = get_type_mutability_map(py);
            mut_map.borrow().get_object_mutability(obj).map(|opt| {
                match opt {
                    // If there is no custom logic for the type err on the side of mutability
                    None => Mutability::Mutable,
                    Some(true) => Mutability::Mutable,
                    Some(false) => Mutability::Immutable,
                }
            })
        } else {
            Ok(type_mutability)
        }
    }
}

#[pyfunction]
pub fn freeze<'py>(obj: Bound<'py, AtorsBase>) -> PyResult<()> {
    let py = obj.py();

    // Check class mutability to determine if freezing is allowed
    let class_type = obj.get_type();
    match class_type.getattr(ATORS_MEMBERS_MUTABILITY) {
        Ok(mutability_obj) => {
            let mutability_enum = mutability_obj.extract::<ClassMutability>()?;
            match mutability_enum {
                ClassMutability::Immutable {} => {
                    // All members are immutable, allow freezing
                    with_critical_section(&obj, || {
                        obj.borrow_mut().frozen = true;
                    });
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

                    for attr_name in &values {
                        let member_obj = PyAnyMethods::get_item(&members_dict, attr_name)?
                            .cast_into::<Member>()?;

                        // Get the slot index and retrieve the value
                        if let Some(slot_value) = get_slot(&obj, member_obj.borrow().index(), py) {
                            let attr_bound = slot_value.bind(py);
                            let attr_mutability = is_object_mutable(py, attr_bound)?;
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
                    with_critical_section(&obj, || {
                        obj.borrow_mut().frozen = true;
                    });
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

/// Retrieve the member customization tool from a class.
#[pyfunction]
pub fn get_member_customization_tool<'py>(
    cls: Bound<'py, PyAny>,
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
