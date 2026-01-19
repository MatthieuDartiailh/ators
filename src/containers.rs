/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{
    Bound, Py, PyAny, PyErr, PyRefMut, PyResult, Python, ffi, intern, pyclass, pymethods,
    types::{PyAnyMethods, PyDict, PyDictMethods, PyList, PySet, PySetMethods},
};

use crate::{core::AtorsBase, validators::Validator};

// #[pyclass(extends=PyList)]  Possible in PyO3 0.28 to be released soon
// struct AtorsList;

// XXX not pickable ...
#[pyclass(module = "ators._ators", extends=PySet)]
pub struct AtorsSet {
    validator: Validator,
    member_name: Option<String>,
    object: Option<Py<AtorsBase>>, // WeakRef?
}

impl AtorsSet {
    pub(crate) fn new<'py>(
        py: Python<'py>,
        validator: Validator,
        member_name: Option<&str>,
        object: Option<Py<AtorsBase>>,
        values: Vec<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, AtorsSet>> {
        let n = Bound::new(
            py,
            AtorsSet {
                validator,
                member_name: member_name.map(|m| m.to_string()),
                object,
            },
        )?
        .cast_into::<PySet>()?;
        for v in values.into_iter() {
            n.add(v)?;
        }
        Ok(n.cast_into::<AtorsSet>()?)
    }

    fn validate_set<'py>(
        &self,
        py: Python<'py>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PySet>> {
        if unsafe { pyo3::ffi::PyAnySet_Check(value.as_ptr()) } == 0 {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "Expected a set for validation",
            ));
        }
        let mut validated_items = Vec::with_capacity(value.len()?);
        let m = self.member_name.as_deref();
        let o = self.object.as_ref().map(|o| o.bind(py));
        for item in value.try_iter()? {
            let valid = self.validator.validate(m, o, item?)?;
            validated_items.push(valid);
        }
        PySet::new(py, validated_items)
    }
}

// __isub__, difference_update, __iand__ and intersection_update do not need
// item validation since they remove items
// __ior__, update, __ixor__, symmetric_difference_update and add, need
// item validation since they can add items
#[pymethods]
impl AtorsSet {
    pub fn add<'py>(self_: PyRefMut<'py, AtorsSet>, value: Bound<'py, PyAny>) -> PyResult<()> {
        let py = value.py();
        let valid = self_.validator.validate(
            self_.member_name.as_deref(),
            self_.object.as_ref().map(|o| o.bind(py)),
            value,
        )?;
        // Use direct PySet C API to avoid converting into bound to cast to PySet
        crate::utils::error_on_minusone(py, unsafe {
            ffi::PySet_Add(self_.as_ptr(), valid.as_ptr())
        })
    }

    pub fn __ior__<'py>(self_: &Bound<'py, Self>, value: Bound<'py, PyAny>) -> PyResult<()> {
        let py = value.py();
        let valid = self_.borrow().validate_set(py, value)?;

        self_
            .py_super()?
            .call_method1(intern!(py, "__ior__"), (valid,))
            .map(|_| ())
    }

    pub fn update<'py>(self_: &Bound<'py, AtorsSet>, other: Bound<'py, PyAny>) -> PyResult<()> {
        AtorsSet::__ior__(self_, other)
    }

    pub fn __ixor__<'py>(self_: &Bound<'py, Self>, value: Bound<'py, PyAny>) -> PyResult<()> {
        let py = value.py();
        let valid = self_.borrow().validate_set(py, value)?;

        self_
            .py_super()?
            .call_method1(intern!(py, "__ixor__"), (valid,))
            .map(|_| ())
    }

    pub fn symmetric_difference_update<'py>(
        self_: &Bound<'py, AtorsSet>,
        other: Bound<'py, PyAny>,
    ) -> PyResult<()> {
        AtorsSet::__ixor__(self_, other)
    }

    // The traverse method of the parent class (PySet) is called automatically and
    // the type is also traversed so we only need to visit our own references.
    pub fn __traverse__(&self, visit: pyo3::PyVisit) -> Result<(), pyo3::PyTraverseError> {
        if let Some(o) = &self.object {
            visit.call(o)?;
        }
        Ok(())
    }

    // The clear method of the parent class (PySet) is called automatically and
    // so we only need to visit our own references.
    pub fn __clear__(&mut self) {
        self.object = None;
    }
}

#[pyclass(module = "ators._ators", extends=PyDict)]
pub struct AtorsDict {
    key_validator: Validator,
    value_validator: Validator,
    member_name: Option<String>,
    object: Option<Py<AtorsBase>>,
}

impl AtorsDict {
    pub(crate) fn new<'py>(
        py: Python<'py>,
        key_validator: Validator,
        value_validator: Validator,
        member_name: Option<&str>,
        object: Option<Py<AtorsBase>>,
        items: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)>,
    ) -> PyResult<Bound<'py, AtorsDict>> {
        let n = Bound::new(
            py,
            AtorsDict {
                key_validator,
                value_validator,
                member_name: member_name.map(|m| m.to_string()),
                object,
            },
        )?
        .cast_into::<PyDict>()?;
        for (k, v) in items.into_iter() {
            n.set_item(k, v)?;
        }
        Ok(n.cast_into::<AtorsDict>()?)
    }

    /// Validate a key using the key_validator
    fn validate_key<'py>(
        &self,
        py: Python<'py>,
        key: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let m = self.member_name.as_deref();
        let o = self.object.as_ref().map(|o| o.bind(py));
        self.key_validator.validate(m, o, key)
    }

    /// Validate a value for insertion into the dict
    fn validate_value<'py>(
        &self,
        py: Python<'py>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let m = self.member_name.as_deref();
        let o = self.object.as_ref().map(|o| o.bind(py));
        self.value_validator.validate(m, o, value)
    }

    /// Validate both key and value for insertion into the dict
    fn validate_item<'py>(
        &self,
        py: Python<'py>,
        key: Bound<'py, PyAny>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        let m = self.member_name.as_deref();
        let o = self.object.as_ref().map(|o| o.bind(py));
        let valid_key = self.key_validator.validate(m, o, key)?;
        let valid_value = self.value_validator.validate(m, o, value)?;
        Ok((valid_key, valid_value))
    }
}

fn dict_set_item<'py>(
    py: Python<'py>,
    dict: *mut ffi::PyObject,
    key: Bound<'py, PyAny>,
    value: Bound<'py, PyAny>,
) -> PyResult<()> {
    // Use direct PyDict C API to avoid converting into bound to cast to PyDict
    crate::utils::error_on_minusone(py, unsafe {
        ffi::PyDict_SetItem(dict, key.as_ptr(), value.as_ptr())
    })
}

#[pymethods]
impl AtorsDict {
    pub fn __setitem__<'py>(
        self_: PyRefMut<'py, AtorsDict>,
        key: Bound<'py, PyAny>,
        value: Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = key.py();
        let (valid_key, valid_value) = self_.validate_item(py, key, value)?;
        // Use direct PyDict C API to avoid converting into bound to cast to PyDict
        dict_set_item(py, self_.as_ptr(), valid_key, valid_value)
    }

    #[pyo3(signature = (other=None, **kwargs))]
    pub fn update<'py>(
        self_: PyRefMut<'py, AtorsDict>,
        other: Option<&Bound<'py, PyAny>>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let py = self_.py();
        if let Some(o) = other {
            // Shortcut for dicts for which we can safely iterate over
            if let Ok(od) = o.cast::<PyDict>() {
                for (k, v) in od.iter() {
                    let (valid_key, valid_value) = self_.validate_item(self_.py(), k, v)?;
                    dict_set_item(py, self_.as_ptr(), valid_key, valid_value)?;
                }
            }
            // Handle object providing keys() method
            else if o.hasattr(intern!(self_.py(), "keys"))? {
                let keys = o.call_method0(intern!(self_.py(), "keys"))?;
                for key in keys.try_iter()? {
                    let k = key?;
                    let v = o
                        .getattr(intern!(self_.py(), "__getitem__"))?
                        .call1((&k,))?;
                    let (valid_key, valid_value) = self_.validate_item(self_.py(), k, v)?;
                    dict_set_item(py, self_.as_ptr(), valid_key, valid_value)?;
                }
            }
            // Handle iterable of key-value pairs
            else {
                for t in o.try_iter()? {
                    let (k, v) = t?.extract::<(Bound<'py, PyAny>, Bound<'py, PyAny>)>()?;
                    let (valid_key, valid_value) = self_.validate_item(self_.py(), k, v)?;
                    dict_set_item(py, self_.as_ptr(), valid_key, valid_value)?;
                }
            }
        }

        // Handle keyword arguments
        if let Some(kw) = kwargs {
            for (k, v) in kw.iter() {
                let (valid_key, valid_value) = self_.validate_item(self_.py(), k, v)?;
                dict_set_item(py, self_.as_ptr(), valid_key, valid_value)?;
            }
        }

        Ok(())
    }

    pub fn setdefault<'py>(
        self_: PyRefMut<'py, AtorsDict>,
        key: Bound<'py, PyAny>,
        default: Option<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = key.py();
        let valid_key = self_.validate_key(py, key)?;
        // Use direct PyDict C API to avoid converting into bound to cast to PyDict
        // Such a casting would consume the ref and we cannot clone it.
        if let Some(existing) = {
            let mut result: *mut ffi::PyObject = std::ptr::null_mut();
            // Safety: All pointers are valid Python objects
            match unsafe {
                ffi::compat::PyDict_GetItemRef(self_.as_ptr(), valid_key.as_ptr(), &mut result)
            } {
                std::ffi::c_int::MIN..=-1 => Err(PyErr::fetch(py)),
                0 => Ok(None),
                1..=std::ffi::c_int::MAX => {
                    // Safety: PyDict_GetItemRef positive return value means the result is a valid
                    // owned reference
                    Ok(Some(unsafe { Bound::from_owned_ptr(py, result) }))
                }
            }?
        } {
            return Ok(existing);
        }

        // The key does not exist, insert the default value
        let value = if let Some(def) = default {
            def
        } else {
            py.None().into_bound(py)
        };
        let valid_value = self_.validate_value(py, value)?;
        dict_set_item(py, self_.as_ptr(), valid_key, Bound::clone(&valid_value))?;

        Ok(valid_value)
    }

    pub fn __ior__<'py>(self_: PyRefMut<'py, AtorsDict>, other: Bound<'py, PyAny>) -> PyResult<()> {
        AtorsDict::update(self_, Some(&other), None)
    }

    // The traverse method of the parent class (PyDict) is called automatically and
    // the type is also traversed so we only need to visit our own references.
    pub fn __traverse__(&self, visit: pyo3::PyVisit) -> Result<(), pyo3::PyTraverseError> {
        if let Some(o) = &self.object {
            visit.call(o)?;
        }
        Ok(())
    }

    // The clear method of the parent class (PyDict) is called automatically and
    // so we only need to clear our own references.
    pub fn __clear__(&mut self) {
        self.object = None;
    }

    // XXX can simply implement __getstate__ and __setstate__ without dealing with items
}
