/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{
    Bound, IntoPyObjectExt, Py, PyAny, PyRef, PyResult, Python, intern, pyclass, pymethods,
    types::{PyAnyMethods, PyDict, PyList, PySet, PySetMethods},
};

use crate::{core::AtorsBase, member::Member, validators::Validator};

// #[pyclass(extends=PyList)]
// struct AtorsList;

#[pyclass(extends=PySet)]
pub struct AtorsSet {
    validator: Validator,
    member: Option<Py<Member>>,
    object: Option<Py<AtorsBase>>,
}

impl AtorsSet {
    pub(crate) fn new<'py>(
        py: Python<'py>,
        validator: Validator,
        member: Option<Py<Member>>,
        object: Option<Py<AtorsBase>>,
        values: Vec<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, AtorsSet>> {
        let n = Bound::new(
            py,
            AtorsSet {
                validator,
                member,
                object,
            },
        )?
        .cast_into::<PySet>()?;
        for v in values.into_iter() {
            n.add(v)?;
        }
        Ok(n.cast_into::<AtorsSet>()?)
    }
}

impl AtorsSet {
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
        let m = self.member.as_ref().map(|m| m.bind(py));
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
    pub fn add<'py>(self_: PyRef<'py, AtorsSet>, value: Bound<'py, PyAny>) -> PyResult<()> {
        let py = value.py();
        let valid = self_.validator.validate(
            self_.member.as_ref().map(|m| m.bind(py)),
            self_.object.as_ref().map(|o| o.bind(py)),
            value,
        )?;
        self_.into_py_any(py)?.cast_bound::<PySet>(py)?.add(valid)
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
}

#[pyclass(extends=PyDict)]
struct AtorsDict;
