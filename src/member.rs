/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Matthieu C. Dartiailh
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use crate::validators::{Coercer, Validator, ValueValidator};
use pyo3::{
    Bound, IntoPyObject, IntoPyObjectExt, Py, PyAny, PyRef, PyRefMut, PyResult, intern, pyclass,
    pymethods,
    sync::with_critical_section2,
    types::{PyAnyMethods, PyDict, PyDictMethods, PyFunction},
};
use std::{clone::Clone, collections::HashMap, mem};

mod default;
mod delattr;
mod getattr;
mod pickle;
mod setattr;
pub use default::DefaultBehavior;
pub use delattr::DelattrBehavior;
pub use getattr::{PostGetattrBehavior, PreGetattrBehavior};
pub use setattr::{PostSetattrBehavior, PreSetattrBehavior};

/// A Python descriptor that defines a member of an Ators class.
#[pyclass(frozen, get_all)]
#[derive(Debug)]
pub struct Member {
    pub name: String,
    slot_index: u16,
    // All attributes below are frozen enums so they cannot be modified at runtime
    // and we can safely return clones of them.
    pre_getattr: PreGetattrBehavior,
    post_getattr: PostGetattrBehavior,
    pre_setattr: PreSetattrBehavior,
    post_setattr: PostSetattrBehavior,
    delattr: DelattrBehavior,
    default: DefaultBehavior,
    validator: Validator,
    // Optional metadata dictionary that can be used to store arbitrary information
    // about the member.
    metadata: Option<HashMap<String, Py<PyAny>>>,
}

impl Member {
    fn new(
        name: String,
        slot_index: u16,
        pre_getattr: PreGetattrBehavior,
        post_getattr: PostGetattrBehavior,
        pre_setattr: PreSetattrBehavior,
        post_setattr: PostSetattrBehavior,
        delattr: DelattrBehavior,
        default: DefaultBehavior,
        validator: Validator,
        metadata: Option<HashMap<String, Py<PyAny>>>,
    ) -> Self {
        Self {
            name,
            slot_index,
            pre_getattr,
            post_getattr,
            pre_setattr,
            post_setattr,
            delattr,
            default,
            validator,
            metadata,
        }
    }
}

#[pymethods]
impl Member {
    pub fn __get__<'py>(
        self_: PyRef<'py, Self>,
        object: Bound<'py, PyAny>,
        _obtype: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        // We access the descriptor through the class so we return the descriptor itself
        if object.is_none() {
            Ok(self_.into_py_any(py)?.into_bound(py))
        // We access the descriptor through an instance so we return the value
        } else {
            let member = self_.into_pyobject(py)?;
            with_critical_section2(member.as_any(), object.as_any(), || {
                let object = object.cast::<crate::core::BaseAtors>()?;
                let m_ref = member.borrow();
                m_ref.pre_getattr.pre_get(&member, object)?;
                let value = match object
                    .borrow()
                    .get_slot(m_ref.slot_index as usize, object.py())
                {
                    Some(v) => v.clone_ref(py).into_bound(py), // Value exist we return it
                    None => {
                        // Attempt to create a default value
                        let default = m_ref.default.default(&member, object)?;
                        let new = m_ref
                            .validator
                            .validate(Some(&member), Some(object), default)?;
                        object
                            .borrow_mut()
                            .set_slot(m_ref.slot_index as usize, new.clone());
                        new
                    }
                };
                member
                    .borrow()
                    .post_getattr
                    .post_get(&member, object, &value)?;
                Ok(value)
            })
        }
    }

    pub fn __set__<'py>(
        self_: PyRef<'py, Self>,
        object: Bound<'py, PyAny>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = self_.py();
        let member = self_.into_pyobject(py)?;
        with_critical_section2(member.as_any(), object.as_any(), || {
            let m_ref = member.borrow();
            let object = object.cast::<crate::core::BaseAtors>()?;
            let current = match object.borrow().get_slot(m_ref.slot_index as usize, py) {
                Some(v) => v,
                None => py.None(), // Use UNSET singleton
            };
            let current_bound = current.bind(py);

            // Validate it is legitimate to attempt to set the member
            m_ref.pre_setattr.pre_set(&member, object, current_bound)?;

            // Check the frozen bit of the object
            if object.borrow().is_frozen() {
                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "Cannot modify {} which is frozen.",
                    object.repr()?
                )));
            }

            // Validate the new value
            let new = m_ref
                .validator
                .validate(Some(&member), Some(object), value)?;
            object
                .borrow_mut()
                .set_slot(m_ref.slot_index as usize, new.clone());

            m_ref
                .post_setattr
                .post_set(&member, object, current_bound, &new)?;

            Ok(())
        })
    }

    pub fn __delete__<'py>(
        self_: PyRef<'py, Member>,
        object: Bound<'py, PyAny>,
    ) -> pyo3::PyResult<()> {
        let py = self_.py();
        let member = self_.into_pyobject(py)?;
        with_critical_section2(member.as_any(), object.as_any(), || {
            let object = object.cast::<crate::core::BaseAtors>()?;

            // Validate it is legitimate to attempt to set the member
            member.borrow().delattr.del(&member, object)
        })
    }
}

#[pyclass]
#[derive(Debug)]
struct MemberBuilder {
    pub name: Option<String>,
    pub slot_index: Option<u16>,
    pub pre_getattr: PreGetattrBehavior,
    pub post_getattr: PostGetattrBehavior,
    pub pre_setattr: PreSetattrBehavior,
    pub post_setattr: PostSetattrBehavior,
    pub delattr: DelattrBehavior,
    pub default: DefaultBehavior,
    pub validator: Validator,
    pub metadata: Option<HashMap<String, Py<PyAny>>>,
}

#[pymethods]
impl MemberBuilder {
    #[new]
    fn py_new() -> Self {
        MemberBuilder {
            name: None,
            slot_index: None,
            pre_getattr: PreGetattrBehavior::NoOp {},
            post_getattr: PostGetattrBehavior::NoOp {},
            pre_setattr: PreSetattrBehavior::NoOp {},
            post_setattr: PostSetattrBehavior::NoOp {},
            delattr: DelattrBehavior::Slot {},
            default: DefaultBehavior::NoDefault {},
            validator: Validator::default(),
            metadata: None,
        }
    }

    #[pyo3(signature = (**tags))]
    pub fn tag<'py>(&mut self, tags: Option<&Bound<'_, PyDict>>) {
        if self.metadata.is_none() {
            self.metadata = Some(HashMap::with_capacity(
                tags.and_then(|d| Some(d.len())).unwrap_or(0),
            ));
        }
        if let Some(tags) = tags
            && let Some(d) = &mut self.metadata
        {
            // tags are keyword args so keys are guaranteed to be strings making unwrap safe
            d.extend(tags.iter().map(|(k, v)| (k.extract().unwrap(), v.unbind())));
        };
    }

    ///
    fn default<'py>(
        mut self_: PyRefMut<'py, Self>,
        default_behavior: Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let py = self_.py();
        let behavior = match default_behavior.cast::<DefaultBehavior>() {
            Ok(b) => b.clone().unbind().extract(py)?,
            Err(_) => match default_behavior.cast_exact::<PyFunction>() {
                Ok(func) => DefaultBehavior::ObjectMethod {
                    meth_name: func.getattr(intern!(py, "__name__"))?.cast_into()?.unbind(),
                },
                Err(_) => DefaultBehavior::Static {
                    value: default_behavior.unbind(),
                },
            },
        };
        {
            let mself = &mut *self_;
            mself.default = behavior;
        }
        Ok(self_)
    }

    ///
    fn coerce<'py>(
        mut self_: PyRefMut<'py, Self>,
        coercer: Option<Bound<'py, PyAny>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let py = self_.py();
        let behavior = if let Some(c) = coercer {
            Some(match c.cast::<Coercer>() {
                Ok(b) => b.clone().unbind().extract(py)?,
                Err(_) => {
                    let func = c.cast_exact::<PyFunction>()?;
                    Coercer::ObjectMethod {
                        meth_name: func.getattr(intern!(py, "__name__"))?.cast_into()?.unbind(),
                    }
                }
            })
        } else {
            None
        };
        {
            let mself = &mut *self_;
            // The default Validator is cheap to construct.
            let v = mem::replace(&mut mself.validator, Validator::default());
            mself.validator = v.with_coercer(behavior);
        }
        Ok(self_)
    }

    ///
    fn append_value_validator<'py>(
        mut self_: PyRefMut<'py, Self>,
        value_validator: Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let py = self_.py();
        let behavior = match value_validator.cast::<ValueValidator>() {
            Ok(b) => b.clone().unbind().extract(py)?,
            Err(_) => {
                let func = value_validator.cast_exact::<PyFunction>()?;
                ValueValidator::ObjectMethod {
                    meth_name: func.getattr(intern!(py, "__name__"))?.cast_into()?.unbind(),
                }
            }
        };
        {
            let mself = &mut *self_;
            // The default Validator is cheap to construct.
            let v = mem::replace(&mut mself.validator, Validator::default());
            mself.validator = v.with_appended_value_validator(behavior);
        }
        Ok(self_)
    }

    // This come with a foot gun if not used on a function assigned to a method
    // of the same name
    // XXX implement sanity check on metaclass
    ///
    fn preget<'py>(
        mut self_: PyRefMut<'py, Self>,
        pre_getattr: Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let py = self_.py();
        let behavior = match pre_getattr.cast::<PreGetattrBehavior>() {
            Ok(b) => b.clone().unbind().extract(py)?,
            Err(_) => {
                let func = pre_getattr.cast_exact::<PyFunction>()?;
                PreGetattrBehavior::ObjectMethod {
                    meth_name: func.getattr(intern!(py, "__name__"))?.cast_into()?.unbind(),
                }
            }
        };
        {
            let mself = &mut *self_;
            mself.pre_getattr = behavior;
        }
        Ok(self_)
    }

    ///
    fn postget<'py>(
        mut self_: PyRefMut<'py, Self>,
        post_getattr: Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let py = self_.py();
        let behavior = match post_getattr.cast::<PostGetattrBehavior>() {
            Ok(b) => b.clone().unbind().extract(py)?,
            Err(_) => {
                let func = post_getattr.cast_exact::<PyFunction>()?;
                PostGetattrBehavior::ObjectMethod {
                    meth_name: func.getattr(intern!(py, "__name__"))?.cast_into()?.unbind(),
                }
            }
        };
        {
            let mself = &mut *self_;
            mself.post_getattr = behavior;
        }
        Ok(self_)
    }

    ///
    fn preset<'py>(
        mut self_: PyRefMut<'py, Self>,
        pre_setattr: Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let py = self_.py();
        let behavior = match pre_setattr.cast::<PreSetattrBehavior>() {
            Ok(b) => b.clone().unbind().extract(py)?,
            Err(_) => {
                let func = pre_setattr.cast_exact::<PyFunction>()?;
                PreSetattrBehavior::ObjectMethod {
                    meth_name: func.getattr(intern!(py, "__name__"))?.cast_into()?.unbind(),
                }
            }
        };
        {
            let mself = &mut *self_;
            mself.pre_setattr = behavior;
        }
        Ok(self_)
    }

    ///
    fn postset<'py>(
        mut self_: PyRefMut<'py, Self>,
        post_setattr: Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let py = self_.py();
        let behavior = match post_setattr.cast::<PostSetattrBehavior>() {
            Ok(b) => b.clone().unbind().extract(py)?,
            Err(_) => {
                let func = post_setattr.cast_exact::<PyFunction>()?;
                PostSetattrBehavior::ObjectMethod {
                    meth_name: func.getattr(intern!(py, "__name__"))?.cast_into()?.unbind(),
                }
            }
        };
        {
            let mself = &mut *self_;
            mself.post_setattr = behavior;
        }
        Ok(self_)
    }

    ///
    fn del_<'py>(
        mut self_: PyRefMut<'py, Self>,
        delattr_behavior: Bound<'py, DelattrBehavior>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let py = self_.py();
        {
            let mself = &mut *self_;
            mself.delattr = delattr_behavior.unbind().extract(py)?;
        }
        Ok(self_)
    }
}

impl MemberBuilder {
    ///
    fn build(self) -> PyResult<Member> {
        let Some(name) = self.name else { todo!() };
        let Some(index) = self.slot_index else {
            todo!()
        };
        Ok(Member {
            name,
            slot_index: index,
            pre_getattr: self.pre_getattr,
            post_getattr: self.post_getattr,
            pre_setattr: self.pre_setattr,
            post_setattr: self.post_setattr,
            delattr: self.delattr,
            default: self.default,
            validator: self.validator,
            metadata: self.metadata,
        })
    }
}
