/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Matthieu C. Dartiailh
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use crate::validators::{Coercer, TypeValidator, Validator, ValueValidator};
use pyo3::{
    Bound, IntoPyObject, IntoPyObjectExt, Py, PyAny, PyRef, PyRefMut, PyResult, Python, intern,
    pyclass, pymethods,
    sync::with_critical_section2,
    types::{PyAnyMethods, PyDict, PyDictMethods, PyFunction},
};
use std::{clone::Clone, collections::HashMap};

mod default;
mod delattr;
mod getattr;
mod pickle;
mod setattr;
use crate::validators::CoercionMode;
pub use default::DefaultBehavior;
pub use delattr::DelattrBehavior;
pub use getattr::{PostGetattrBehavior, PreGetattrBehavior};
pub use setattr::{PostSetattrBehavior, PreSetattrBehavior};

///
fn clone_metadata(
    metadata: &Option<HashMap<String, Py<PyAny>>>,
) -> Option<HashMap<String, Py<PyAny>>> {
    Python::attach(|py| {
        metadata.as_ref().map(|hm| {
            hm.iter()
                .map(|(k, v)| (k.clone(), v.clone_ref(py)))
                .collect()
        })
    })
}

/// A Python descriptor that defines a member of an Ators class.
#[pyclass(frozen, get_all)]
#[derive(Debug)]
pub struct Member {
    pub name: String,
    slot_index: u8,
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
    pub fn clone_with_index(&self, new_index: u8) -> Self {
        Member {
            name: self.name.clone(),
            slot_index: new_index,
            pre_getattr: self.pre_getattr.clone(),
            post_getattr: self.post_getattr.clone(),
            pre_setattr: self.pre_setattr.clone(),
            post_setattr: self.post_setattr.clone(),
            delattr: self.delattr.clone(),
            default: self.default.clone(),
            validator: self.validator.clone(),
            metadata: clone_metadata(&self.metadata),
        }
    }

    pub fn index(&self) -> u8 {
        self.slot_index
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
#[derive(Debug, Default)]
pub struct MemberBuilder {
    pub name: Option<String>,
    pub slot_index: Option<u8>,
    pub pre_getattr: Option<PreGetattrBehavior>,
    pub post_getattr: Option<PostGetattrBehavior>,
    pub pre_setattr: Option<PreSetattrBehavior>,
    pub post_setattr: Option<PostSetattrBehavior>,
    pub delattr: Option<DelattrBehavior>,
    pub default: Option<DefaultBehavior>,
    pub type_validator: Option<TypeValidator>,
    pub value_validators: Option<Vec<ValueValidator>>,
    pub coercer: Option<CoercionMode>,
    pub metadata: Option<HashMap<String, Py<PyAny>>>,
    inherit: bool,
}

#[pymethods]
impl MemberBuilder {
    // FIXME need to pass in args for customization (init)
    #[new]
    pub fn py_new() -> Self {
        MemberBuilder::default()
    }

    pub fn inherit<'py>(mut self_: PyRefMut<'py, Self>) -> PyResult<PyRefMut<'py, Self>> {
        self_.inherit = true;
        Ok(self_)
    }

    #[pyo3(signature = (**tags))]
    pub fn tag<'py>(
        mut self_: PyRefMut<'py, Self>,
        tags: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        if self_.metadata.is_none() {
            self_.metadata = Some(HashMap::with_capacity(tags.map(|d| d.len()).unwrap_or(0)));
        }
        if let Some(tags) = tags
            && let Some(d) = &mut self_.metadata
        {
            // tags are keyword args so keys are guaranteed to be strings making unwrap safe
            d.extend(tags.iter().map(|(k, v)| (k.extract().unwrap(), v.unbind())));
        };
        Ok(self_)
    }

    ///
    #[pyo3(name = "default")]
    pub fn py_default<'py>(
        mut self_: PyRefMut<'py, Self>,
        default_behavior: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let mself = &mut *self_;
        match default_behavior.cast::<DefaultBehavior>() {
            Ok(b) => mself.default = Some(b.as_any().extract()?),
            Err(_) => match default_behavior.cast_exact::<PyFunction>() {
                Ok(func) => {
                    mself.default = Some(DefaultBehavior::ObjectMethod {
                        meth_name: func.getattr(intern!(py, "__name__"))?.cast_into()?.unbind(),
                    });
                    return Ok(default_behavior);
                }
                Err(_) => {
                    mself.default = Some(DefaultBehavior::Static {
                        value: default_behavior.unbind(),
                    })
                }
            },
        };
        self_.into_bound_py_any(py)
    }

    ///
    pub fn coerce<'py>(
        mut self_: PyRefMut<'py, Self>,
        coercer: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let mself = &mut *self_;
        if let Some(c) = coercer {
            match c.cast::<Coercer>() {
                Ok(b) => mself.coercer = Some(CoercionMode::Coerce(b.as_any().extract()?)),
                Err(_) => {
                    let func = c.cast_exact::<PyFunction>()?;
                    mself.coercer = Some(CoercionMode::Coerce(Coercer::ObjectMethod {
                        meth_name: func.getattr(intern!(py, "__name__"))?.cast_into()?.unbind(),
                    }));
                    return Ok(c);
                }
            }
        } else {
            mself.coercer = Some(CoercionMode::No());
        };
        self_.into_bound_py_any(py)
    }

    ///
    pub fn coerce_init<'py>(
        mut self_: PyRefMut<'py, Self>,
        coercer: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let mself = &mut *self_;
        if let Some(c) = coercer {
            match c.cast::<Coercer>() {
                Ok(b) => mself.coercer = Some(CoercionMode::Init(b.as_any().extract()?)),
                Err(_) => {
                    let func = c.cast_exact::<PyFunction>()?;
                    mself.coercer = Some(CoercionMode::Init(Coercer::ObjectMethod {
                        meth_name: func.getattr(intern!(py, "__name__"))?.cast_into()?.unbind(),
                    }));
                    return Ok(c);
                }
            }
        } else {
            mself.coercer = Some(CoercionMode::No());
        };
        self_.into_bound_py_any(py)
    }

    ///
    pub fn preget<'py>(
        mut self_: PyRefMut<'py, Self>,
        pre_getattr: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let mself = &mut *self_;
        match pre_getattr.cast::<PreGetattrBehavior>() {
            Ok(b) => {
                mself.pre_getattr = Some(b.as_any().extract()?);
                self_.into_bound_py_any(py)
            }
            Err(_) => {
                let func = pre_getattr.cast_exact::<PyFunction>()?;
                mself.pre_getattr = Some(PreGetattrBehavior::ObjectMethod {
                    meth_name: func.getattr(intern!(py, "__name__"))?.cast_into()?.unbind(),
                });
                Ok(pre_getattr)
            }
        }
    }

    ///
    pub fn postget<'py>(
        mut self_: PyRefMut<'py, Self>,
        post_getattr: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let mself = &mut *self_;
        match post_getattr.cast::<PostGetattrBehavior>() {
            Ok(b) => {
                mself.post_getattr = Some(b.as_any().extract()?);
                self_.into_bound_py_any(py)
            }
            Err(_) => {
                let func = post_getattr.cast_exact::<PyFunction>()?;
                mself.post_getattr = Some(PostGetattrBehavior::ObjectMethod {
                    meth_name: func.getattr(intern!(py, "__name__"))?.cast_into()?.unbind(),
                });
                Ok(post_getattr)
            }
        }
    }

    ///
    pub fn preset<'py>(
        mut self_: PyRefMut<'py, Self>,
        pre_setattr: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let mself = &mut *self_;
        match pre_setattr.cast::<PreSetattrBehavior>() {
            Ok(b) => {
                mself.pre_setattr = Some(b.as_any().extract()?);
                self_.into_bound_py_any(py)
            }
            Err(_) => {
                let func = pre_setattr.cast_exact::<PyFunction>()?;
                mself.pre_setattr = Some(PreSetattrBehavior::ObjectMethod {
                    meth_name: func.getattr(intern!(py, "__name__"))?.cast_into()?.unbind(),
                });
                Ok(pre_setattr)
            }
        }
    }

    ///
    pub fn constant<'py>(mut self_: PyRefMut<'py, Self>) -> PyResult<PyRefMut<'py, Self>> {
        {
            let mself = &mut *self_;
            mself.pre_setattr = Some(PreSetattrBehavior::Constant {});
            mself.delattr = Some(DelattrBehavior::Undeletable {});
        }
        Ok(self_)
    }

    ///
    pub fn postset<'py>(
        mut self_: PyRefMut<'py, Self>,
        post_setattr: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let mself = &mut *self_;
        match post_setattr.cast::<PostSetattrBehavior>() {
            Ok(b) => {
                mself.post_setattr = Some(b.as_any().extract()?);
                self_.into_bound_py_any(py)
            }
            Err(_) => {
                let func = post_setattr.cast_exact::<PyFunction>()?;
                mself.post_setattr = Some(PostSetattrBehavior::ObjectMethod {
                    meth_name: func.getattr(intern!(py, "__name__"))?.cast_into()?.unbind(),
                });
                Ok(post_setattr)
            }
        }
    }

    ///
    pub fn del_<'py>(
        mut self_: PyRefMut<'py, Self>,
        delattr_behavior: Bound<'py, DelattrBehavior>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        {
            let mself = &mut *self_;
            mself.delattr = Some(delattr_behavior.as_any().extract()?);
        }
        Ok(self_)
    }
}

impl MemberBuilder {
    ///
    pub fn should_inherit(&self) -> bool {
        self.inherit
    }

    ///
    pub fn get_inherited_behavior_from_member(&mut self, member: &Member) {
        if self.pre_getattr.is_none() {
            self.pre_getattr = Some(member.pre_getattr.clone());
        }
        if self.post_getattr.is_none() {
            self.post_getattr = Some(member.post_getattr.clone());
        }
        if self.pre_setattr.is_none() {
            self.pre_setattr = Some(member.pre_setattr.clone());
        }
        if self.post_setattr.is_none() {
            self.post_setattr = Some(member.post_setattr.clone());
        }
        if self.delattr.is_none() {
            self.delattr = Some(member.delattr.clone());
        }
        if self.default.is_none() {
            self.default = Some(member.default.clone());
        }
        if self.type_validator.is_none() {
            self.type_validator = Some(member.validator.type_validator.clone());
        }
        if self.value_validators.is_none() {
            self.value_validators = Some(member.validator.value_validators.to_vec());
        }
        if self.coercer.is_none() {
            self.coercer = Some(member.validator.coercer.clone());
        }
        if self.metadata.is_none() {
            self.metadata = clone_metadata(&member.metadata);
        }
    }

    ///
    pub fn build(self) -> PyResult<Member> {
        let Some(name) = self.name else { todo!() };
        let Some(index) = self.slot_index else {
            todo!()
        };
        let Some(tv) = self.type_validator else {
            todo!()
        };
        Ok(Member {
            name,
            slot_index: index,
            pre_getattr: self.pre_getattr.unwrap_or(PreGetattrBehavior::NoOp {}),
            post_getattr: self.post_getattr.unwrap_or(PostGetattrBehavior::NoOp {}),
            pre_setattr: self.pre_setattr.unwrap_or(PreSetattrBehavior::NoOp {}),
            post_setattr: self.post_setattr.unwrap_or(PostSetattrBehavior::NoOp {}),
            delattr: self.delattr.unwrap_or(DelattrBehavior::Slot {}),
            default: self.default.unwrap_or(DefaultBehavior::NoDefault {}),
            validator: Validator {
                type_validator: tv,
                value_validators: self.value_validators.unwrap_or_default().into_boxed_slice(),
                coercer: self.coercer.unwrap_or(CoercionMode::No()),
            },
            metadata: self.metadata,
        })
    }
}

impl Clone for MemberBuilder {
    fn clone(&self) -> Self {
        MemberBuilder {
            name: self.name.clone(),
            slot_index: self.slot_index,
            pre_getattr: self.pre_getattr.clone(),
            post_getattr: self.post_getattr.clone(),
            pre_setattr: self.pre_setattr.clone(),
            post_setattr: self.post_setattr.clone(),
            delattr: self.delattr.clone(),
            default: self.default.clone(),
            type_validator: self.type_validator.clone(),
            value_validators: self.value_validators.clone(),
            coercer: self.coercer.clone(),
            metadata: clone_metadata(&self.metadata),
            inherit: self.inherit,
        }
    }
}
