/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use crate::{
    core::{AtorsBase, get_slot, set_slot},
    validators::{Coercer, TypeValidator, Validator, ValueValidator},
};
use pyo3::{
    Bound, IntoPyObject, IntoPyObjectExt, Py, PyAny, PyRef, PyRefMut, PyResult, Python, intern,
    pyclass, pymethods,
    types::{PyAnyMethods, PyDict, PyDictMethods, PyModuleMethods, PyString},
};
use std::{clone::Clone, collections::HashMap};

use crate::utils::err_with_cause;

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

/// Helper class to generate a callable from a list of module names.
///
/// Used for forward reference environment creation.
#[pyclass(module = "ators._ators", frozen)]
struct ForwardRefEnvironmentCallable {
    names: Vec<Py<PyString>>,
}

#[pymethods]
impl ForwardRefEnvironmentCallable {
    pub fn __call__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        for name in &self.names {
            let name_bound = name.bind(py);
            let module = py.import(name_bound)?;
            for (key, value) in module.dict().iter() {
                dict.set_item(key, value)
                    .expect("Setting item in dict cannot fail when key is known to be a string.");
            }
        }
        Ok(dict)
    }
}

/// A Python descriptor that defines a member of an Ators class.
#[pyclass(module = "ators._ators", frozen, get_all)]
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

pub fn member_set_unpickled_value<'py>(
    member: &Bound<'py, Member>,
    object: &Bound<'py, AtorsBase>,
    value: Bound<'py, PyAny>,
) -> PyResult<()> {
    // XXX special case our own containers only
    // to restore valid member and object references
    set_slot(object, member.borrow().slot_index, value);
    Ok(())
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
            Some(mb.name()),
            Some(object),
            &value,
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
            let object = object.cast::<crate::core::AtorsBase>()?;

            // Run pre getattr behavior
            if let Err(e) = self_.pre_getattr.pre_get(&self_, object) {
                return Err(err_with_cause(
                    py,
                    pyo3::PyErr::from_type(
                        e.get_type(py),
                        format!(
                            "pre-get failed for member '{}' of {}",
                            self_.name,
                            object.repr()?,
                        ),
                    ),
                    e,
                ));
            };

            // Get the value from the slot and build a default value if needed
            let slot_value = { get_slot(object, self_.slot_index, object.py()) };
            let value = match slot_value {
                Some(v) => v.clone_ref(py).into_bound(py), // Value exist we return it
                None => {
                    // Attempt to create a default value
                    let default = match self_.default.default(&self_, object) {
                        Ok(v) => v,
                        Err(e) => {
                            return Err(err_with_cause(
                                py,
                                pyo3::PyErr::from_type(
                                    e.get_type(py),
                                    format!(
                                        "Failed to get default value for member '{}' of {}",
                                        self_.name,
                                        object.repr()?,
                                    ),
                                ),
                                e,
                            ));
                        }
                    };
                    // Validate and set the default value
                    let new = match self_.validator.validate(
                        Some(&self_.name),
                        Some(object),
                        default,
                    ) {
                        Ok(v) => v,
                        Err(e) => {
                            return Err(err_with_cause(
                                py,
                                pyo3::PyErr::from_type(
                                    e.get_type(py),
                                    format!(
                                        "Failed to validate default value for member '{}' of {}",
                                        self_.name,
                                        object.repr()?,
                                    ),
                                ),
                                e,
                            ));
                        }
                    };
                    set_slot(object, self_.slot_index, new.clone());
                    new
                }
            };

            // Run post getattr behavior
            if let Err(e) = self_.post_getattr.post_get(&self_, object, &value) {
                return Err(err_with_cause(
                    py,
                    pyo3::PyErr::from_type(
                        e.get_type(py),
                        format!(
                            "post-get failed for member '{}' of {}",
                            self_.name,
                            object.repr()?,
                        ),
                    ),
                    e,
                ));
            };
            Ok(value)
        }
    }

    pub fn __set__<'py>(
        self_: PyRef<'py, Self>,
        object: Bound<'py, PyAny>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = self_.py();
        let object = object.cast::<crate::core::AtorsBase>()?;
        let current = get_slot(object, self_.slot_index, py);

        // Check the frozen bit of the object
        if object.borrow().is_frozen() {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Cannot modify {} which is frozen.",
                object.repr()?
            )));
        }

        // Validate it is legitimate to attempt to set the member
        if let Err(e) = self_.pre_setattr.pre_set(&self_, object, &current) {
            return Err(err_with_cause(
                py,
                pyo3::PyErr::from_type(
                    e.get_type(py),
                    format!(
                        "pre-set failed for member '{}' of {}",
                        self_.name,
                        object.repr()?,
                    ),
                ),
                e,
            ));
        };

        // Validate the new value
        let new = match self_
            .validator
            .validate(Some(&self_.name), Some(object), value)
        {
            Ok(v) => v,
            Err(e) => {
                return Err(err_with_cause(
                    py,
                    pyo3::PyErr::from_type(
                        e.get_type(py),
                        format!(
                            "Validation failed for member '{}' of {}",
                            self_.name,
                            object.repr()?,
                        ),
                    ),
                    e,
                ));
            }
        };
        set_slot(object, self_.slot_index, new.clone());

        if let Err(e) = self_.post_setattr.post_set(&self_, object, &current, &new) {
            return Err(err_with_cause(
                py,
                pyo3::PyErr::from_type(
                    e.get_type(py),
                    format!(
                        "post-set failed for member '{}' of {}",
                        self_.name,
                        object.repr()?,
                    ),
                ),
                e,
            ));
        };

        Ok(())
    }

    pub fn __delete__<'py>(
        self_: PyRef<'py, Member>,
        object: Bound<'py, PyAny>,
    ) -> pyo3::PyResult<()> {
        let py = self_.py();
        let object = object.cast::<crate::core::AtorsBase>()?;
        self_.delattr.del(&self_, object)
    }

    // XXX because the class is frozen I cannot implement clear....
}

#[pyclass(module = "ators._ators", name = "member")]
#[derive(Debug, Default)]
pub struct MemberBuilder {
    // `name` and `slot_index` are public for direct Rust-level access
    pub name: Option<String>,
    pub slot_index: Option<u8>,
    pre_getattr: Option<PreGetattrBehavior>,
    post_getattr: Option<PostGetattrBehavior>,
    pre_setattr: Option<PreSetattrBehavior>,
    post_setattr: Option<PostSetattrBehavior>,
    delattr: Option<DelattrBehavior>,
    default: Option<DefaultBehavior>,
    type_validator: Option<TypeValidator>,
    value_validators: Option<Vec<ValueValidator>>,
    coerce: Option<Coercer>,
    coerce_init: Option<Coercer>,
    metadata: Option<HashMap<String, Py<PyAny>>>,
    forward_ref_environment_factory: Option<Py<PyAny>>,
    pickle: bool,
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

    ///
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
            d.extend(tags.iter().map(|(k, v)| {
                (
                    k.extract()
                        .expect("Tags keys are string by construction making unwrap safe"),
                    v.unbind(),
                )
            }));
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
                    // For mutable containers we will always create a new instance
                    // as part of the validation process so we can use a static value
                    // approach here.
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

    ///
    #[pyo3(name = "pickle")]
    pub fn py_pickle<'py>(
        mut self_: PyRefMut<'py, Self>,
        pickle: bool,
    ) -> PyResult<PyRefMut<'py, Self>> {
        {
            let mself = &mut *self_;
            if mself.pickle != pickle {
                mself
                    .multiple_settings
                    .entry("pickle".into())
                    .and_modify(|e| *e += 1)
                    .or_insert(2);
            }
            mself.pickle = pickle;
        }
        Ok(self_)
    }

    ///
    pub fn forward_ref_environment<'py>(
        mut self_: PyRefMut<'py, Self>,
        factory_or_modules: Bound<'py, PyAny>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let mself = &mut *self_;
        let fc;
        if factory_or_modules.is_callable() {
            let py = factory_or_modules.py();
            let sig = py
                .import(intern!(py, "inspect"))?
                .getattr(intern!(py, "signature"))?;
            let ob_sig_len = sig
                .call1((&factory_or_modules,))?
                .getattr(intern!(py, "parameters"))?
                .len()?;
            if ob_sig_len != 0 {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "forward_ref_environment expect a callable taking 0 got \
                    {factory_or_modules} which takes {ob_sig_len}."
                )));
            }
            fc = factory_or_modules.unbind();
        } else if factory_or_modules.is_exact_instance_of::<PyString>() {
            fc = ForwardRefEnvironmentCallable {
                names: vec![factory_or_modules.clone().cast_into::<PyString>()?.unbind()],
            }
            .into_py_any(factory_or_modules.py())?;
        } else if factory_or_modules.cast::<pyo3::types::PySequence>().is_ok() {
            fc = ForwardRefEnvironmentCallable {
                names: factory_or_modules
                    .try_iter()?
                    .map(|item| Ok(item?.cast_into::<PyString>()?.unbind()))
                    .collect::<PyResult<Vec<Py<PyString>>>>()?,
            }
            .into_py_any(factory_or_modules.py())?;
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "forward_ref_environment expect a callable taking 0, \
                a class fully qualified name or a sequence of fully qualified names.",
            ));
        }

        if mself.forward_ref_environment_factory.is_some() {
            mself
                .multiple_settings
                .entry("forward_ref_environment".into())
                .and_modify(|e| *e += 1)
                .or_insert(2);
        }
        mself.forward_ref_environment_factory = Some(fc);
        Ok(self_)
    }
}

impl MemberBuilder {
    #[inline]
    pub fn should_inherit(&self) -> bool {
        self.inherit
    }

    #[inline]
    pub fn pre_getattr(&self) -> Option<&PreGetattrBehavior> {
        self.pre_getattr.as_ref()
    }

    #[inline]
    pub fn post_getattr(&self) -> Option<&PostGetattrBehavior> {
        self.post_getattr.as_ref()
    }

    #[inline]
    pub fn pre_setattr(&self) -> Option<&PreSetattrBehavior> {
        self.pre_setattr.as_ref()
    }

    #[inline]
    pub fn post_setattr(&self) -> Option<&PostSetattrBehavior> {
        self.post_setattr.as_ref()
    }

    #[inline]
    pub fn delattr(&self) -> Option<&DelattrBehavior> {
        self.delattr.as_ref()
    }

    #[inline]
    pub fn default_behavior(&self) -> Option<&DefaultBehavior> {
        self.default.as_ref()
    }

    #[inline]
    pub fn value_validators(&self) -> Option<&Vec<ValueValidator>> {
        self.value_validators.as_ref()
    }

    #[inline]
    pub fn coercer(&self) -> Option<&Coercer> {
        self.coerce.as_ref()
    }

    #[inline]
    pub fn init_coercer(&self) -> Option<&Coercer> {
        self.coerce_init.as_ref()
    }

    #[inline]
    pub fn pickle(&self) -> bool {
        self.pickle
    }

    #[inline]
    pub fn metadata(&self) -> &Option<HashMap<String, Py<PyAny>>> {
        &self.metadata
    }

    #[inline]
    pub fn forward_ref_environment_factory(&self) -> Option<&Py<PyAny>> {
        self.forward_ref_environment_factory.as_ref()
    }

    #[inline]
    pub fn set_default(&mut self, d: DefaultBehavior) {
        self.default = Some(d);
    }

    #[inline]
    pub fn set_pre_setattr(&mut self, v: PreSetattrBehavior) {
        self.pre_setattr = Some(v);
    }

    #[inline]
    pub fn set_delattr(&mut self, v: DelattrBehavior) {
        self.delattr = Some(v);
    }

    #[inline]
    pub fn set_type_validator(&mut self, tv: TypeValidator) {
        self.type_validator = Some(tv);
    }

    #[inline]
    pub fn take_value_validators(&mut self) -> Option<Vec<ValueValidator>> {
        self.value_validators.take()
    }

    #[inline]
    pub fn set_value_validators(&mut self, v: Vec<ValueValidator>) {
        self.value_validators = Some(v);
    }

    #[inline]
    pub fn set_pickle(&mut self, new: bool) {
        self.pickle = new;
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
        let mut tv = self.type_validator.unwrap_or(TypeValidator::Any {});
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

        if (self.coerce.is_some() || self.coerce_init.is_some())
            && let TypeValidator::Any {} = &tv
            && self
                .value_validators
                .as_ref()
                .is_none_or(|vv| vv.is_empty())
        {
            let py = type_name.py();
            let warnings_warn = py
                .import(intern!(py, "warnings"))?
                .getattr(intern!(py, "warn"))?;
            warnings_warn.call1((pyo3::exceptions::PyUserWarning::new_err(format!(
                "Member {} of {} specify a coercion behavior but no type nor value validation.\
             As a consequence, the coercer will never be invoked.",
                &name, &type_name
            )),))?;
        }

        // For union type validators, if type inferred coercion is requested at
        // the member level we set coercion on all union validators if no specific
        // was set.
        if let TypeValidator::Union { ref mut members } = tv {
            if let Some(Coercer::TypeInferred {}) = &self.coerce {
                for m in members.iter_mut() {
                    if m.coercer.is_none() {
                        m.coercer = Some(Coercer::TypeInferred {});
                    }
                }
            }
            if let Some(Coercer::TypeInferred {}) = &self.coerce_init {
                for m in members.iter_mut() {
                    if m.init_coercer.is_none() {
                        m.init_coercer = Some(Coercer::TypeInferred {});
                    }
                }
            }
        }

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
            forward_ref_environment_factory: {
                if let Some(fr) = self.forward_ref_environment_factory.as_ref() {
                    Python::attach(|py| Some(fr.clone_ref(py)))
                } else {
                    None
                }
            },
            inherit: self.inherit,
            multiple_settings: self.multiple_settings.clone(),
            pickle: self.pickle,
        }
    }
}
