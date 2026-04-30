/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
/// Core descriptor class defining Ators events and related utilities.
use crate::{
    class::base::{AtorsBase, is_frozen, notify_member_change},
    validators::{TypeValidator, Validator, ValueValidator},
};
use pyo3::{
    Bound, IntoPyObjectExt, Py, PyAny, PyRef, PyRefMut, PyResult, Python, pyclass, pymethods,
    types::{
        PyAnyMethods, PyDict, PyDictMethods, PyGenericAlias, PyString, PyTuple, PyTupleMethods,
    },
};
use std::{clone::Clone, collections::HashMap};

use crate::utils::err_with_cause;

/// Helper function to clone event metadata (same pattern as for members).
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

/// A write-only Python descriptor that defines an event on an Ators class.
///
/// Events validate assigned values, emit observer notifications on successful
/// set, and never store the assigned value.
#[pyclass(module = "ators._ators", frozen, get_all)]
#[derive(Debug)]
pub struct Event {
    pub name: String,
    validator: Validator,
    // Optional metadata dictionary.
    metadata: Option<HashMap<String, Py<PyAny>>>,
}

impl Event {
    /// Access the validator for this event.
    pub fn validator(&self) -> &Validator {
        &self.validator
    }

    /// Access the metadata for this event.
    pub fn metadata(&self) -> &Option<HashMap<String, Py<PyAny>>> {
        &self.metadata
    }
}

/// Cold path: descriptor get for class-level access or error on instance access.
#[cold]
fn try_get_event_descriptor<'py>(
    self_: PyRef<'py, Event>,
    object: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if object.is_none() {
        return self_.into_bound_py_any(object.py());
    }
    Err(pyo3::exceptions::PyAttributeError::new_err(format!(
        "Event '{}' is write-only and cannot be read.",
        self_.name
    )))
}

/// Cold path: validation failure for events.
#[cold]
fn validate_event_set_failed<'py>(
    py: Python<'py>,
    event: &PyRef<'py, Event>,
    object: &Bound<'py, AtorsBase>,
    err: pyo3::PyErr,
) -> PyResult<pyo3::PyErr> {
    // Frozen takes precedence.
    if is_frozen(object) {
        return Ok(pyo3::exceptions::PyTypeError::new_err(format!(
            "Cannot modify {} which is frozen.",
            object.repr()?,
        )));
    }
    Ok(err_with_cause(
        py,
        pyo3::PyErr::from_type(
            err.get_type(py),
            format!(
                "Validation failed for event '{}' of {}",
                event.name,
                object.repr()?,
            ),
        ),
        err,
    ))
}

impl Event {
    pub fn __get__<'py>(
        self_: PyRef<'py, Self>,
        object: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        // Instance access: raise AttributeError (write-only descriptor).
        // Class access (object is None): return the descriptor itself.
        if object.cast::<AtorsBase>().is_ok() {
            // Instance access — always write-only.
            return Err(pyo3::exceptions::PyAttributeError::new_err(format!(
                "Event '{}' is write-only and cannot be read.",
                self_.name
            )));
        }
        try_get_event_descriptor(self_, object)
    }

    pub fn __set__<'py>(
        self_: PyRef<'py, Self>,
        object: &Bound<'py, PyAny>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = self_.py();
        let object = object.cast::<AtorsBase>()?;

        // Reject writes to frozen objects before any validation.
        if is_frozen(object) {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Cannot modify {} which is frozen.",
                object.repr()?
            )));
        }

        // Validate the value.
        let new = match self_
            .validator
            .validate(Some(&self_.name), Some(object), value)
        {
            Ok(v) => v,
            Err(err) => return Err(validate_event_set_failed(py, &self_, object, err)?),
        };

        // Notify observers — no value is stored.
        notify_member_change(object, &self_.name, py.None(), new.unbind())?;

        Ok(())
    }

    pub fn __traverse__(&self, visit: pyo3::PyVisit) -> Result<(), pyo3::PyTraverseError> {
        if let Some(m) = &self.metadata {
            for (_k, v) in m.iter() {
                visit.call(v)?
            }
        }
        Ok(())
    }

    pub fn __class_getitem__<'py>(
        cls: &Bound<'py, PyAny>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = cls.py();
        // Wrap the subscription item in a tuple for GenericAlias. Arity
        // validation (exactly 1 arg) is deferred to the metaclass so that
        // wrong-arity annotations inside a class body produce a clear error.
        let alias_args = match item.cast::<PyTuple>() {
            Ok(t) => t.to_owned(),
            Err(_) => PyTuple::new(py, [item])?,
        };
        Ok(PyGenericAlias::new(py, cls, alias_args.as_any())?.into_any())
    }
}

impl Event {
    #[inline]
    unsafe fn slot_self<'py>(
        py: ::pyo3::Python<'py>,
        slf: &*mut ::pyo3::ffi::PyObject,
    ) -> ::pyo3::PyResult<::pyo3::PyRef<'py, Self>> {
        ::std::convert::TryFrom::try_from(unsafe {
            ::pyo3::impl_::pymethods::BoundRef::ref_from_ptr(py, slf).cast_unchecked::<Event>()
        })
        .map_err(::std::convert::Into::into)
    }
}

impl Event {
    #[allow(non_snake_case)]
    unsafe fn __pymethod___set____(
        py: ::pyo3::Python,
        _slf: *mut ::pyo3::ffi::PyObject,
        arg0: *mut ::pyo3::ffi::PyObject,
        arg1: ::std::ptr::NonNull<::pyo3::ffi::PyObject>,
    ) -> ::pyo3::PyResult<()> {
        #[allow(clippy::let_unit_value, reason = "many holders are just `()`")]
        let mut holder_0 = ::pyo3::impl_::extract_argument::FunctionArgumentHolder::INIT;
        let mut holder_1 = ::pyo3::impl_::extract_argument::FunctionArgumentHolder::INIT;
        let result = Event::__set__(
            unsafe { Event::slot_self(py, &_slf) }?,
            {
                #[allow(unused_imports, reason = "`Probe` trait used on negative case only")]
                use ::pyo3::impl_::pyclass::Probe as _;
                ::pyo3::impl_::extract_argument::extract_argument(
                    unsafe { ::pyo3::impl_::extract_argument::cast_function_argument(py, arg0) },
                    &mut holder_0,
                    "object",
                )
            }?,
            {
                #[allow(unused_imports, reason = "`Probe` trait used on negative case only")]
                use ::pyo3::impl_::pyclass::Probe as _;
                ::pyo3::impl_::extract_argument::extract_argument(
                    unsafe {
                        ::pyo3::impl_::extract_argument::cast_non_null_function_argument(py, arg1)
                    },
                    &mut holder_1,
                    "value",
                )
            }?,
        );
        ::pyo3::impl_::callback::convert(py, result)
    }
}

impl Event {
    #[allow(non_snake_case)]
    unsafe fn __pymethod___delete____(
        py: ::pyo3::Python,
        _slf: *mut ::pyo3::ffi::PyObject,
        _arg0: *mut ::pyo3::ffi::PyObject,
    ) -> ::pyo3::PyResult<()> {
        let _ = py;
        let _ = _slf;
        let _ = _arg0;
        Err(pyo3::exceptions::PyAttributeError::new_err(
            "Event descriptors do not support deletion.",
        ))
    }
}

impl Event {
    #[allow(non_snake_case)]
    unsafe fn __pymethod___class_getitem__(
        py: ::pyo3::Python<'_>,
        cls: *mut ::pyo3::ffi::PyObject,
        args: *mut ::pyo3::ffi::PyObject,
        _kwargs: *mut ::pyo3::ffi::PyObject,
    ) -> ::pyo3::PyResult<*mut ::pyo3::ffi::PyObject> {
        let args_any = unsafe { Bound::<PyAny>::from_borrowed_ptr(py, args) };
        let args_tuple = args_any
            .cast_into::<PyTuple>()
            .expect("CPython always provides a PyTuple for METH_VARARGS");
        let item = args_tuple.get_item(0).map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err(
                "Event.__class_getitem__() takes exactly 1 argument",
            )
        })?;
        let cls_bound = unsafe { Bound::<PyAny>::from_borrowed_ptr(py, cls) };
        Event::__class_getitem__(&cls_bound, &item).map(|alias| alias.into_ptr())
    }
}

#[allow(unknown_lints, non_local_definitions)]
impl ::pyo3::impl_::pyclass::PyMethods<Event>
    for ::pyo3::impl_::pyclass::PyClassImplCollector<Event>
{
    fn py_methods(self) -> &'static ::pyo3::impl_::pyclass::PyClassItems {
        static ITEMS: ::pyo3::impl_::pyclass::PyClassItems = ::pyo3::impl_::pyclass::PyClassItems {
            methods: &[::pyo3::impl_::pymethods::PyMethodDefType::Method(
                ::pyo3::impl_::pymethods::PyMethodDef::cfunction_with_keywords(
                    c"__class_getitem__",
                    {
                        struct ClassGetItemDef;
                        impl
                            ::pyo3::impl_::trampoline::MethodDef<
                                ::pyo3::impl_::trampoline::cfunction_with_keywords::Func,
                            > for ClassGetItemDef
                        {
                            const METH: ::pyo3::impl_::trampoline::cfunction_with_keywords::Func =
                                Event::__pymethod___class_getitem__;
                        }
                        ::pyo3::impl_::trampoline::cfunction_with_keywords::<ClassGetItemDef>
                    },
                    c"",
                )
                .flags(::pyo3::ffi::METH_CLASS),
            )],
            slots: &[
                ::pyo3::ffi::PyType_Slot {
                    slot: ::pyo3::ffi::Py_tp_descr_get,
                    pfunc: {
                        struct Def;
                        impl
                            pyo3::impl_::trampoline::MethodDef<
                                pyo3::impl_::trampoline::descrgetfunc::Func,
                            > for Def
                        {
                            const METH: pyo3::impl_::trampoline::descrgetfunc::Func =
                                Event::__pymethod___get____;
                        }
                        pyo3::impl_::trampoline::descrgetfunc::<Def>
                    } as ::pyo3::ffi::descrgetfunc as _,
                },
                ::pyo3::ffi::PyType_Slot {
                    slot: ::pyo3::ffi::Py_tp_traverse,
                    pfunc: Event::__pymethod_traverse__ as ::pyo3::ffi::traverseproc as _,
                },
                {
                    unsafe fn slot_impl(
                        py: pyo3::Python<'_>,
                        _slf: *mut pyo3::ffi::PyObject,
                        attr: *mut pyo3::ffi::PyObject,
                        value: *mut pyo3::ffi::PyObject,
                    ) -> pyo3::PyResult<::std::ffi::c_int> {
                        use ::std::option::Option::*;
                        use pyo3::impl_::callback::IntoPyCallbackOutput;
                        if let Some(_value) = ::std::ptr::NonNull::new(value) {
                            unsafe {
                                Event::__pymethod___set____(py, _slf, attr, _value).convert(py)
                            }
                        } else {
                            unsafe { Event::__pymethod___delete____(py, _slf, attr).convert(py) }
                        }
                    }
                    pyo3::ffi::PyType_Slot {
                        slot: pyo3::ffi::Py_tp_descr_set,
                        pfunc: {
                            struct Def;
                            impl
                                pyo3::impl_::trampoline::MethodDef<
                                    pyo3::impl_::trampoline::setattrofunc::Func,
                                > for Def
                            {
                                const METH: pyo3::impl_::trampoline::setattrofunc::Func =
                                    slot_impl;
                            }
                            pyo3::impl_::trampoline::setattrofunc::<Def>
                        } as pyo3::ffi::descrsetfunc as _,
                    }
                },
            ],
        };
        &ITEMS
    }
}

#[doc(hidden)]
#[allow(non_snake_case)]
impl Event {
    #[allow(non_snake_case)]
    unsafe fn __pymethod___get____(
        py: ::pyo3::Python<'_>,
        _slf: *mut ::pyo3::ffi::PyObject,
        arg0: *mut ::pyo3::ffi::PyObject,
        _arg1: *mut ::pyo3::ffi::PyObject,
    ) -> ::pyo3::PyResult<*mut ::pyo3::ffi::PyObject> {
        #[allow(clippy::let_unit_value, reason = "many holders are just `()`")]
        let mut holder_0 = ::pyo3::impl_::extract_argument::FunctionArgumentHolder::INIT;
        let result = Event::__get__(unsafe { Event::slot_self(py, &_slf) }?, {
            #[allow(unused_imports, reason = "`Probe` trait used on negative case only")]
            use ::pyo3::impl_::pyclass::Probe as _;
            ::pyo3::impl_::extract_argument::extract_argument(
                unsafe {
                    ::pyo3::impl_::extract_argument::cast_function_argument(
                        py,
                        if arg0.is_null() {
                            ::pyo3::ffi::Py_None()
                        } else {
                            arg0
                        },
                    )
                },
                &mut holder_0,
                "object",
            )
        }?);
        ::pyo3::impl_::callback::convert(py, result)
    }
    pub unsafe extern "C" fn __pymethod_traverse__(
        slf: *mut ::pyo3::ffi::PyObject,
        visit: ::pyo3::ffi::visitproc,
        arg: *mut ::std::ffi::c_void,
    ) -> ::std::ffi::c_int {
        unsafe {
            ::pyo3::impl_::pymethods::_call_traverse::<Event>(
                slf,
                Event::__traverse__,
                visit,
                arg,
                Event::__pymethod_traverse__,
            )
        }
    }
}

// ─── EventBuilder ────────────────────────────────────────────────────────────

/// Builder class for Event that allows ergonomic declaration of events
/// in the class body.
#[pyclass(module = "ators._ators", name = "event", from_py_object)]
#[derive(Debug, Default)]
pub struct EventBuilder {
    /// Name resolved by the metaclass.
    pub name: Option<String>,
    type_validator: Option<TypeValidator>,
    value_validators: Option<Vec<ValueValidator>>,
    metadata: Option<HashMap<String, Py<PyAny>>>,
    pub inherit: bool,
}

#[pymethods]
impl EventBuilder {
    /// Create a new event builder.
    ///
    /// Does not accept `default`, `default_factory`, pre/post get/set hooks,
    /// or any other member-only capability.
    #[new]
    pub fn py_new() -> Self {
        EventBuilder::default()
    }

    /// Mark this event as inheriting from the declaration on a parent class.
    pub fn inherit<'py>(mut self_: PyRefMut<'py, Self>) -> PyResult<PyRefMut<'py, Self>> {
        self_.inherit = true;
        Ok(self_)
    }

    /// Attach arbitrary metadata to this event.
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
                        .expect("Tag keys are strings by construction, unwrap is safe"),
                    v.unbind(),
                )
            }));
        }
        Ok(self_)
    }

    /// Append a value validator to this event.
    pub fn append_value_validator<'py>(
        mut self_: PyRefMut<'py, Self>,
        value_validator: Bound<'py, crate::validators::ValueValidator>,
    ) -> PyResult<PyRefMut<'py, Self>> {
        let vv: ValueValidator = value_validator.as_any().extract()?;
        if let Some(vvs) = &mut self_.value_validators {
            vvs.push(vv);
        } else {
            self_.value_validators = Some(vec![vv]);
        }
        Ok(self_)
    }
}

impl EventBuilder {
    /// Whether this builder requests inheritance from a base-class event.
    #[inline]
    pub fn should_inherit(&self) -> bool {
        self.inherit
    }

    #[inline]
    pub fn type_validator(&self) -> Option<&TypeValidator> {
        self.type_validator.as_ref()
    }

    #[inline]
    pub fn value_validators(&self) -> Option<&Vec<ValueValidator>> {
        self.value_validators.as_ref()
    }

    #[inline]
    pub fn metadata(&self) -> &Option<HashMap<String, Py<PyAny>>> {
        &self.metadata
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

    /// Populate unset fields from an existing `Event` instance (inherit mode).
    pub fn get_inherited_behavior_from_event(&mut self, event: &Event) {
        if self.type_validator.is_none() {
            self.type_validator = Some(event.validator.type_validator.clone());
        }
        if self.value_validators.is_none() {
            self.value_validators = Some(event.validator.value_validators.to_vec());
        }
        if self.metadata.is_none() {
            self.metadata = clone_metadata(&event.metadata);
        }
    }

    /// Finalize the builder and construct an `Event` descriptor.
    pub fn build(self, type_name: &Bound<'_, PyString>) -> PyResult<Event> {
        let Some(name) = self.name else {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Cannot build event belonging to {type_name} without an assigned name."
            )));
        };
        Ok(Event {
            name,
            validator: Validator::new(
                self.type_validator.unwrap_or(TypeValidator::Any {}),
                self.value_validators,
                None,
                None,
            ),
            metadata: self.metadata,
        })
    }
}

impl Clone for EventBuilder {
    fn clone(&self) -> Self {
        Python::attach(|py| EventBuilder {
            name: self.name.clone(),
            type_validator: self.type_validator.clone(),
            value_validators: self.value_validators.clone(),
            metadata: self.metadata.as_ref().map(|hm| {
                hm.iter()
                    .map(|(k, v)| (k.clone(), v.clone_ref(py)))
                    .collect()
            }),
            inherit: self.inherit,
        })
    }
}
