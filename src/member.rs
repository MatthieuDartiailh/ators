/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use crate::{
    core::AtorsBase,
    validators::{Coercer, TypeValidator, Validator, ValueValidator},
};
use pyo3::{
    Bound, IntoPyObject, IntoPyObjectExt, Py, PyAny, PyRef, PyRefMut, PyResult, Python, intern,
    pyclass, pymethods,
    sync::with_critical_section2,
    types::{PyAnyMethods, PyDict, PyDictMethods, PyString},
};
use std::{clone::Clone, collections::HashMap};

mod default;
mod delattr;
mod getattr;
mod pickle;
mod setattr;
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
    name: String,
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

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn index(&self) -> u8 {
        self.slot_index
    }

    pub fn metadata(&self) -> &Option<HashMap<String, Py<PyAny>>> {
        &self.metadata
    }
}

///
pub fn member_coerce_init<'py>(
    member: &Bound<'py, Member>,
    object: &Bound<'py, AtorsBase>,
    value: Bound<'py, PyAny>,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let mb = member.borrow();
    mb.validator.init_coercer.as_ref().map(|c| {
        c.coerce_value(
            true,
            &mb.validator.type_validator,
            Some(member),
            Some(object),
            value,
        )
    })
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
                let object = object.cast::<crate::core::AtorsBase>()?;
                let m_ref = member.borrow();
                m_ref.pre_getattr.pre_get(&member, object)?;
                let slot_value = { object.borrow().get_slot(m_ref.slot_index, object.py()) };
                let value = match slot_value {
                    Some(v) => v.clone_ref(py).into_bound(py), // Value exist we return it
                    None => {
                        // Attempt to create a default value
                        let default = m_ref.default.default(&member, object)?;
                        let new = m_ref
                            .validator
                            .validate(Some(&member), Some(object), default)?;
                        object.borrow_mut().set_slot(m_ref.slot_index, new.clone());
                        new
                    }
                };
                m_ref.post_getattr.post_get(&member, object, &value)?;
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
            let object = object.cast::<crate::core::AtorsBase>()?;
            let current = object.borrow().get_slot(m_ref.slot_index, py);

            // Validate it is legitimate to attempt to set the member
            m_ref.pre_setattr.pre_set(&member, object, &current)?;

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
                .validate(Some(&member), Some(object), value)?; // XXX Need to map the error
            object.borrow_mut().set_slot(m_ref.slot_index, new.clone());

            m_ref
                .post_setattr
                .post_set(&member, object, &current, &new)?;

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
            let object = object.cast::<crate::core::AtorsBase>()?;

            // Validate it is legitimate to attempt to set the member
            member.borrow().delattr.del(&member, object)
        })
    }
}

#[pyclass(name = "member")]
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
    pub coerce: Option<Coercer>,
    pub coerce_init: Option<Coercer>,
    pub metadata: Option<HashMap<String, Py<PyAny>>>,
    inherit: bool,
    multiple_settings: HashMap<String, u8>,
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
        if mself.default.is_some() {
            mself
                .multiple_settings
                .entry("default".into())
                .and_modify(|e| *e += 1)
                .or_insert(2);
        }
        match default_behavior.cast::<DefaultBehavior>() {
            Ok(b) => mself.default = Some(b.as_any().extract()?),
            Err(_) => {
                mself.default = Some(DefaultBehavior::Static {
                    value: default_behavior.unbind(),
                })
            }
        }

        self_.into_bound_py_any(py)
    }

    ///
    #[pyo3( signature= ( coercer = None))]
    pub fn coerce<'py>(
        mut self_: PyRefMut<'py, Self>,
        coercer: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let mself = &mut *self_;
        if mself.coerce.is_some() {
            mself
                .multiple_settings
                .entry("coerce".into())
                .and_modify(|e| *e += 1)
                .or_insert(2);
        }
        if let Some(c) = coercer {
            // XXX accept a callable directly
            let bc = c.cast::<Coercer>()?;
            mself.coerce = Some(bc.as_any().extract()?);
        } else {
            // Use the Type Inferred coercer by default
            // (people should not call coerce if they do not want to coerce).
            mself.coerce = Some(Coercer::TypeInferred {});
        };
        self_.into_bound_py_any(py)
    }

    ///
    #[pyo3( signature= ( coercer = None))]
    pub fn coerce_init<'py>(
        mut self_: PyRefMut<'py, Self>,
        coercer: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let mself = &mut *self_;
        if mself.coerce_init.is_some() {
            mself
                .multiple_settings
                .entry("coerce_init".into())
                .and_modify(|e| *e += 1)
                .or_insert(2);
        }
        if let Some(c) = coercer {
            // XXX accept a callable directly
            let bc = c.cast::<Coercer>()?;
            mself.coerce_init = Some(bc.as_any().extract()?);
        } else {
            // Use the Type Inferred coercer by default
            // (people should not call coerce if they do not want to coerce).
            mself.coerce_init = Some(Coercer::TypeInferred {});
        };
        self_.into_bound_py_any(py)
    }

    pub fn append_value_validator<'py>(
        mut self_: PyRefMut<'py, Self>,
        value_validator: Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let mself = &mut *self_;
        match value_validator.cast::<ValueValidator>() {
            Ok(b) => {
                if let Some(vv) = &mut mself.value_validators {
                    vv.push(b.as_any().extract()?);
                } else {
                    mself.value_validators.replace(vec![b.as_any().extract()?]);
                }
            }
            Err(err) => return Err(err.into()),
        };
        Ok(self_)
    }

    ///
    pub fn preget<'py>(
        mut self_: PyRefMut<'py, Self>,
        pre_getattr: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let mself = &mut *self_;
        if mself.pre_getattr.is_some() {
            mself
                .multiple_settings
                .entry("preget".into())
                .and_modify(|e| *e += 1)
                .or_insert(2);
        }
        match pre_getattr.cast::<PreGetattrBehavior>() {
            Ok(b) => mself.pre_getattr = Some(b.as_any().extract()?),
            Err(err) => return Err(err.into()),
        }
        self_.into_bound_py_any(py)
    }

    ///
    pub fn postget<'py>(
        mut self_: PyRefMut<'py, Self>,
        post_getattr: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let mself = &mut *self_;
        if mself.post_getattr.is_some() {
            mself
                .multiple_settings
                .entry("postget".into())
                .and_modify(|e| *e += 1)
                .or_insert(2);
        }
        match post_getattr.cast::<PostGetattrBehavior>() {
            Ok(b) => mself.post_getattr = Some(b.as_any().extract()?),
            Err(err) => return Err(err.into()),
        }
        self_.into_bound_py_any(py)
    }

    ///
    pub fn preset<'py>(
        mut self_: PyRefMut<'py, Self>,
        pre_setattr: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let mself = &mut *self_;
        if mself.pre_setattr.is_some() {
            mself
                .multiple_settings
                .entry("preset".into())
                .and_modify(|e| *e += 1)
                .or_insert(2);
        }
        match pre_setattr.cast::<PreSetattrBehavior>() {
            Ok(b) => mself.pre_setattr = Some(b.as_any().extract()?),
            Err(err) => return Err(err.into()),
        }
        self_.into_bound_py_any(py)
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
        if mself.post_setattr.is_some() {
            mself
                .multiple_settings
                .entry("postset".into())
                .and_modify(|e| *e += 1)
                .or_insert(2);
        }
        match post_setattr.cast::<PostSetattrBehavior>() {
            Ok(b) => mself.post_setattr = Some(b.as_any().extract()?),
            Err(err) => return Err(err.into()),
        }
        self_.into_bound_py_any(py)
    }

    ///
    pub fn del_<'py>(
        mut self_: PyRefMut<'py, Self>,
        delattr_behavior: Bound<'py, DelattrBehavior>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        {
            let mself = &mut *self_;
            if mself.delattr.is_some() {
                mself
                    .multiple_settings
                    .entry("del_".into())
                    .and_modify(|e| *e += 1)
                    .or_insert(2);
            }
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
        if self.coerce.is_none() {
            self.coerce = member.validator.coercer.clone();
        }
        if self.coerce_init.is_none() {
            self.coerce_init = member.validator.init_coercer.clone();
        }
        if self.metadata.is_none() {
            self.metadata = clone_metadata(&member.metadata);
        }
    }

    ///
    pub fn build<'py>(self, type_name: &Bound<'py, PyString>) -> PyResult<Member> {
        let Some(name) = self.name else {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Cannot build member belonging to {type_name} without an assigned name."
            )));
        };
        let Some(index) = self.slot_index else {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Cannot build member {name} of {type_name} without an assigned slot."
            )));
        };
        let tv = self.type_validator.unwrap_or(TypeValidator::Any {});
        if !self.multiple_settings.is_empty() {
            let py = type_name.py();
            let warnings_mod = py.import(intern!(py, "warnings"))?;
            warnings_mod.getattr(intern!(py, "warn"))?.call1((
                pyo3::exceptions::PyUserWarning::new_err(format!(
                    "The followng behaviors of member {} of {type_name} were \
                        set multiple times: {:#?}",
                    &name, &self.multiple_settings
                )),
            ))?;
        }

        // XXX warn if type validator is any and coercer is set

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
                coercer: self.coerce,
                init_coercer: self.coerce_init,
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
            coerce: self.coerce.clone(),
            coerce_init: self.coerce_init.clone(),
            metadata: clone_metadata(&self.metadata),
            inherit: self.inherit,
            multiple_settings: self.multiple_settings.clone(),
        }
    }
}
