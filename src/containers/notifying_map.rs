/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
use pyo3::{
    Bound, IntoPyObjectExt, Py, PyAny, PyErr, PyResult, Python, intern, pyclass, pymethods,
    sync::critical_section::with_critical_section,
    types::{PyAnyMethods, PyDict, PyDictMethods, PyList, PyTuple, PyType},
};
use std::cell::UnsafeCell;

use crate::{
    class::AtorsBase,
    containers::{
        ContainerOperation,
        common::{NotificationBuffer, matches_assignment_context, notification_context},
    },
    validators::Validator,
};

#[pyclass(module = "ators._ators", frozen)]
pub struct NotifyingMapBatchNotificationsContext {
    notifying_map: Py<NotifyingMap>,
}

#[pymethods]
impl NotifyingMapBatchNotificationsContext {
    pub fn __enter__<'py>(
        self_: &Bound<'py, NotifyingMapBatchNotificationsContext>,
    ) -> Bound<'py, NotifyingMapBatchNotificationsContext> {
        let map = self_.get().notifying_map.bind(self_.py());
        map.get().begin_batch_inner(map);
        Bound::clone(self_)
    }

    pub fn __exit__(
        self_: &Bound<'_, NotifyingMapBatchNotificationsContext>,
        _exc_type: Option<&Bound<'_, PyAny>>,
        _exc_val: Option<&Bound<'_, PyAny>>,
        _exc_tb: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        let map = self_.get().notifying_map.bind(self_.py());
        map.get().end_batch_inner(self_.py(), map)?;
        Ok(false)
    }
}

#[pyclass(module = "ators._ators", frozen)]
pub struct NotifyingMap {
    pub(crate) values: UnsafeCell<Py<PyDict>>,
    order: UnsafeCell<Vec<Py<PyAny>>>,
    key_validator: UnsafeCell<Validator>,
    value_validator: UnsafeCell<Validator>,
    member_name: UnsafeCell<Option<String>>,
    object: UnsafeCell<Option<Py<AtorsBase>>>,
    notification_buffer: UnsafeCell<NotificationBuffer>,
}

// Safety: validator and member_name are written only once (at construction or during restore
// before any other references exist), and after that are effectively immutable; object is only
// modified during __clear__, which Python's GC calls only once all references to this object
// have been dropped — ensuring no concurrent access (holds for both GIL and free-threaded builds).
// notification_state and pending_operations are protected by critical sections.
unsafe impl Sync for NotifyingMap {}

impl NotifyingMap {
    fn key_to_string<'py>(key: &Bound<'py, PyAny>) -> PyResult<String> {
        key.repr()?.extract::<String>()
    }

    fn order_index<'py>(
        order: &[Py<PyAny>],
        key: &Bound<'py, PyAny>,
        py: Python<'py>,
    ) -> Option<usize> {
        order
            .iter()
            .position(|candidate| candidate.bind(py).eq(key).unwrap_or(false))
    }

    pub(crate) fn new_empty<'py>(
        py: Python<'py>,
        key_validator: Validator,
        value_validator: Validator,
        member_name: Option<&str>,
        object: Option<Py<AtorsBase>>,
    ) -> PyResult<Bound<'py, NotifyingMap>> {
        Bound::new(
            py,
            Self {
                values: UnsafeCell::new(PyDict::new(py).unbind()),
                order: UnsafeCell::new(Vec::new()),
                key_validator: UnsafeCell::new(key_validator),
                value_validator: UnsafeCell::new(value_validator),
                member_name: UnsafeCell::new(member_name.map(str::to_owned)),
                object: UnsafeCell::new(object),
                notification_buffer: UnsafeCell::new(NotificationBuffer::new()),
            },
        )
    }

    pub(crate) fn matches_assignment_context<'py>(
        &self,
        member_name: Option<&str>,
        object: Option<&Bound<'py, AtorsBase>>,
    ) -> bool {
        matches_assignment_context(&self.member_name, &self.object, member_name, object)
    }

    pub(crate) fn clone_for_assignment<'py>(
        source: &Bound<'py, NotifyingMap>,
    ) -> PyResult<Bound<'py, NotifyingMap>> {
        let map = source.get();
        let key_validator = unsafe { &*map.key_validator.get() }.clone();
        let value_validator = unsafe { &*map.value_validator.get() }.clone();
        let member_name = unsafe { &*map.member_name.get() }
            .as_deref()
            .map(str::to_owned);
        let object = unsafe { &*map.object.get() }
            .as_ref()
            .map(|object| object.clone_ref(source.py()));
        let copy = Self::new_empty(
            source.py(),
            key_validator,
            value_validator,
            member_name.as_deref(),
            object,
        )?;
        let values = unsafe { &*copy.get().values.get() }.bind(source.py());
        let order = unsafe { &*map.order.get() };
        for key in order {
            let key_bound = key.bind(source.py());
            let value = values
                .get_item(key_bound)?
                .expect("stored key must still exist");
            values.set_item(key_bound, &value)?;
        }
        unsafe {
            (*copy.get().order.get()) =
                order.iter().map(|key| key.clone_ref(source.py())).collect();
        }
        Ok(copy)
    }

    pub(crate) fn restore<'py>(
        amap: &Bound<'py, NotifyingMap>,
        key_validator: Validator,
        value_validator: Validator,
        member_name: Option<&str>,
        object: Option<&Bound<'py, AtorsBase>>,
    ) {
        with_critical_section(amap.as_any(), || {
            let inner = amap.get();
            unsafe {
                (*inner.key_validator.get()) = key_validator;
                (*inner.value_validator.get()) = value_validator;
                (*inner.member_name.get()) = member_name.map(str::to_owned);
                (*inner.object.get()) = object.map(|o| o.clone().unbind());
            }
        });
    }

    fn validate_key<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let validator = unsafe { &*self.key_validator.get() };
        let name = unsafe { &*self.member_name.get() }.as_deref();
        let owner = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        validator.validate(name, owner, key)
    }

    fn validate_value<'py>(
        &self,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let validator = unsafe { &*self.value_validator.get() };
        let name = unsafe { &*self.member_name.get() }.as_deref();
        let owner = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
        validator.validate(name, owner, value)
    }

    fn validate_item<'py>(
        &self,
        py: Python<'py>,
        key: &Bound<'py, PyAny>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        Ok((self.validate_key(py, key)?, self.validate_value(py, value)?))
    }

    fn record_operation<'py>(
        &self,
        py: Python<'py>,
        operation: ContainerOperation,
        self_bound: &Bound<'py, NotifyingMap>,
    ) -> PyResult<()> {
        let Some((object, member_name)) = notification_context(py, &self.member_name, &self.object)
        else {
            return Ok(());
        };

        let newvalue = {
            let snapshot = PyDict::new(py);
            let values = self_bound.get().values_bound(py);
            for key in unsafe { &*self_bound.get().order.get() } {
                let key_bound = key.bind(py);
                let value = values
                    .get_item(key_bound)?
                    .expect("stored key must still exist");
                snapshot.set_item(key_bound, &value)?;
            }
            snapshot.into_any().unbind()
        };

        let should_emit = with_critical_section(self_bound.as_any(), || unsafe {
            let buffer = &mut *self.notification_buffer.get();
            buffer.record_operation(operation.clone()).is_some()
        });

        if should_emit {
            NotificationBuffer::emit_container_change(
                py,
                object.bind(py),
                &member_name,
                newvalue,
                vec![operation],
            )
        } else {
            Ok(())
        }
    }

    fn begin_batch_inner(&self, self_bound: &Bound<'_, NotifyingMap>) {
        with_critical_section(self_bound.as_any(), || unsafe {
            (*self.notification_buffer.get()).begin_batch();
        });
    }

    fn end_batch_inner<'py>(
        &self,
        py: Python<'py>,
        self_bound: &Bound<'py, NotifyingMap>,
    ) -> PyResult<()> {
        let operations = with_critical_section(self_bound.as_any(), || unsafe {
            (*self.notification_buffer.get()).end_batch()
        });

        if !operations.is_empty() {
            let Some((object, member_name)) =
                notification_context(py, &self.member_name, &self.object)
            else {
                return Ok(());
            };
            let snapshot = PyDict::new(py);
            let values = self_bound.get().values_bound(py);
            for key in unsafe { &*self_bound.get().order.get() } {
                let key_bound = key.bind(py);
                let value = values
                    .get_item(key_bound)?
                    .expect("stored key must still exist");
                snapshot.set_item(key_bound, &value)?;
            }
            let newvalue = snapshot.into_any().unbind();
            NotificationBuffer::emit_container_change(
                py,
                object.bind(py),
                &member_name,
                newvalue,
                operations,
            )?;
        }

        Ok(())
    }

    pub(crate) fn values_bound<'py>(&self, py: Python<'py>) -> Bound<'py, PyDict> {
        unsafe { &*self.values.get() }.clone_ref(py).into_bound(py)
    }

    pub(crate) fn sync_values_from_dict<'py>(
        self_: &Bound<'py, NotifyingMap>,
        mapping: &Bound<'py, PyDict>,
    ) -> PyResult<()> {
        let py = self_.py();
        let values = unsafe { &*self_.get().values.get() }.bind(py);
        values.clear();
        unsafe { (*self_.get().order.get()).clear() };
        for (key, value) in mapping.iter() {
            values.set_item(&key, &value)?;
            unsafe { (*self_.get().order.get()).push(key.unbind()) };
        }
        Ok(())
    }
}

#[pymethods]
impl NotifyingMap {
    #[new]
    fn new(py: Python<'_>) -> PyResult<Self> {
        Ok(Self {
            values: UnsafeCell::new(PyDict::new(py).unbind()),
            order: UnsafeCell::new(Vec::new()),
            key_validator: UnsafeCell::new(Validator::new(
                crate::validators::TypeValidator::Any {},
                None,
                None,
                None,
            )),
            value_validator: UnsafeCell::new(Validator::new(
                crate::validators::TypeValidator::Any {},
                None,
                None,
                None,
            )),
            member_name: UnsafeCell::new(None),
            object: UnsafeCell::new(None),
            notification_buffer: UnsafeCell::new(NotificationBuffer::new()),
        })
    }

    fn __len__(&self) -> usize {
        unsafe { (*self.order.get()).len() }
    }

    fn __iter__<'py>(self_: &Bound<'py, NotifyingMap>) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let order = unsafe { &*self_.get().order.get() };
        let mut keys = Vec::with_capacity(order.len());
        for key in order {
            keys.push(key.bind(py).clone());
        }
        let list = PyList::new(py, keys)?;
        list.call_method0("__iter__")
    }

    fn __contains__<'py>(
        self_: &Bound<'py, NotifyingMap>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<bool> {
        Ok(self_
            .get()
            .values_bound(self_.py())
            .get_item(key)?
            .is_some())
    }

    fn __getitem__<'py>(
        self_: &Bound<'py, NotifyingMap>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        if let Some(value) = self_.get().values_bound(py).get_item(key)? {
            Ok(value)
        } else {
            let key_repr = NotifyingMap::key_to_string(key)?;
            Err(PyErr::new::<pyo3::exceptions::PyKeyError, _>(key_repr))
        }
    }

    fn __setitem__<'py>(
        self_: &Bound<'py, NotifyingMap>,
        key: &Bound<'py, PyAny>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<()> {
        let py = self_.py();
        let (valid_key, valid_value) = self_.get().validate_item(py, key, value)?;
        let dict = self_.get().values_bound(py);
        if dict.get_item(&valid_key)?.is_some() {
            let old_value = dict
                .get_item(&valid_key)?
                .expect("stored key must still exist");
            let index =
                NotifyingMap::order_index(unsafe { &*self_.get().order.get() }, &valid_key, py)
                    .expect("stored key must still exist in order metadata");
            dict.set_item(&valid_key, &valid_value)?;
            let operation = ContainerOperation::Replaced {
                index,
                old_value: PyTuple::new(py, [valid_key.clone(), old_value.clone()])?
                    .into_any()
                    .unbind(),
                new_value: PyTuple::new(py, [valid_key.clone(), valid_value.clone()])?
                    .into_any()
                    .unbind(),
            };
            return self_.get().record_operation(py, operation, self_);
        }
        NotifyingMap::add(self_, &valid_key, &valid_value, None)
    }

    #[pyo3(signature = (key, default=None))]
    fn get<'py>(
        self_: &Bound<'py, NotifyingMap>,
        key: &Bound<'py, PyAny>,
        default: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let valid_key = self_.get().validate_key(py, key)?;
        let dict = self_.get().values_bound(py);
        match dict.get_item(&valid_key)? {
            Some(value) => Ok(value),
            None => match default {
                Some(value) => Ok(value.clone()),
                None => Ok(py.None().into_bound(py)),
            },
        }
    }

    #[pyo3(signature = (key, default=None))]
    fn setdefault<'py>(
        self_: &Bound<'py, NotifyingMap>,
        key: &Bound<'py, PyAny>,
        default: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let valid_key = self_.get().validate_key(py, key)?;
        let dict = self_.get().values_bound(py);
        if let Some(existing) = dict.get_item(&valid_key)? {
            return Ok(existing);
        }

        let default_value = default
            .map(|value| self_.get().validate_value(py, value))
            .transpose()?
            .unwrap_or_else(|| py.None().into_bound(py));

        NotifyingMap::add(self_, &valid_key, &default_value, None)?;
        Ok(default_value)
    }

    #[pyo3(signature = (other=None, **kwargs))]
    fn update<'py>(
        self_: &Bound<'py, NotifyingMap>,
        other: Option<&Bound<'py, PyAny>>,
        kwargs: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<()> {
        let py = self_.py();

        if let Some(other) = other {
            if let Ok(mapping) = other.cast::<PyDict>() {
                for (key, value) in mapping.iter() {
                    self_.get().validate_item(py, &key, &value)?;
                    NotifyingMap::__setitem__(self_, &key, &value)?;
                }
            } else if other.hasattr(intern!(py, "keys"))? {
                let keys = other.call_method0(intern!(py, "keys"))?;
                for key in keys.try_iter()? {
                    let key = key?;
                    let value = other.getattr(intern!(py, "__getitem__"))?.call1((&key,))?;
                    self_.get().validate_item(py, &key, &value)?;
                    NotifyingMap::__setitem__(self_, &key, &value)?;
                }
            } else {
                for item in other.try_iter()? {
                    let (key, value) = item?.extract::<(Bound<'py, PyAny>, Bound<'py, PyAny>)>()?;
                    self_.get().validate_item(py, &key, &value)?;
                    NotifyingMap::__setitem__(self_, &key, &value)?;
                }
            }
        }

        if let Some(kwargs) = kwargs {
            for (key, value) in kwargs.iter() {
                self_.get().validate_item(py, &key, &value)?;
                NotifyingMap::__setitem__(self_, &key, &value)?;
            }
        }

        Ok(())
    }

    #[pyo3(signature = (key, default=None))]
    fn pop<'py>(
        self_: &Bound<'py, NotifyingMap>,
        key: &Bound<'py, PyAny>,
        default: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let valid_key = self_.get().validate_key(py, key)?;
        let dict = self_.get().values_bound(py);
        match dict.get_item(&valid_key)? {
            Some(_) => NotifyingMap::remove(self_, &valid_key),
            None => match default {
                Some(value) => Ok(value.clone()),
                None => {
                    let key_repr = NotifyingMap::key_to_string(&valid_key)?;
                    Err(PyErr::new::<pyo3::exceptions::PyKeyError, _>(key_repr))
                }
            },
        }
    }

    fn __delitem__<'py>(self_: &Bound<'py, NotifyingMap>, key: &Bound<'py, PyAny>) -> PyResult<()> {
        NotifyingMap::remove(self_, key)?;
        Ok(())
    }

    fn __repr__<'py>(self_: &Bound<'py, NotifyingMap>) -> PyResult<String> {
        let py = self_.py();
        let dict = self_.get().values_bound(py);
        let mut parts = Vec::new();
        for key in unsafe { &*self_.get().order.get() } {
            let key_bound = key.bind(py);
            let value = dict
                .get_item(key_bound)?
                .expect("stored key must still exist");
            parts.push(format!("{}: {}", key_bound.repr()?, value.repr()?));
        }
        Ok(format!("NotifyingMap({{{}}})", parts.join(", ")))
    }

    fn keys<'py>(self_: &Bound<'py, NotifyingMap>) -> PyResult<Bound<'py, PyList>> {
        let py = self_.py();
        let order = unsafe { &*self_.get().order.get() };
        let mut keys = Vec::with_capacity(order.len());
        for key in order {
            keys.push(key.bind(py).clone());
        }
        PyList::new(py, keys)
    }

    fn values<'py>(self_: &Bound<'py, NotifyingMap>) -> PyResult<Bound<'py, PyList>> {
        let py = self_.py();
        let dict = self_.get().values_bound(py);
        let order = unsafe { &*self_.get().order.get() };
        let mut values = Vec::with_capacity(order.len());
        for key in order {
            let key_bound = key.bind(py);
            values.push(
                dict.get_item(key_bound)?
                    .expect("stored key must still exist"),
            );
        }
        PyList::new(py, values)
    }

    fn items<'py>(self_: &Bound<'py, NotifyingMap>) -> PyResult<Bound<'py, PyList>> {
        let py = self_.py();
        let dict = self_.get().values_bound(py);
        let order = unsafe { &*self_.get().order.get() };
        let mut pairs = Vec::with_capacity(order.len());
        for key in order {
            let key_bound = key.bind(py);
            let value = dict
                .get_item(key_bound)?
                .expect("stored key must still exist");
            pairs.push((key_bound.clone(), value));
        }
        PyList::new(py, pairs)
    }

    #[pyo3(signature = (key, value, before=None))]
    fn add<'py>(
        self_: &Bound<'py, NotifyingMap>,
        key: &Bound<'py, PyAny>,
        value: &Bound<'py, PyAny>,
        before: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<()> {
        let py = self_.py();
        let (valid_key, valid_value) = self_.get().validate_item(py, key, value)?;
        let dict = self_.get().values_bound(py);
        if dict.get_item(&valid_key)?.is_some() {
            let key_repr = NotifyingMap::key_to_string(&valid_key)?;
            return Err(PyErr::new::<pyo3::exceptions::PyKeyError, _>(key_repr));
        }

        let order_snapshot: &[Py<PyAny>] = unsafe { &*self_.get().order.get() };
        let insertion = if let Some(before_key) = before {
            let before_valid = self_.get().validate_key(py, before_key)?;
            if dict.get_item(&before_valid)?.is_none() {
                let key_repr = NotifyingMap::key_to_string(&before_valid)?;
                return Err(PyErr::new::<pyo3::exceptions::PyKeyError, _>(key_repr));
            }
            NotifyingMap::order_index(order_snapshot, &before_valid, py)
                .unwrap_or(order_snapshot.len())
        } else {
            order_snapshot.len()
        };

        dict.set_item(&valid_key, &valid_value)?;
        let order: &mut Vec<Py<PyAny>> = unsafe { &mut *self_.get().order.get() };
        order.insert(insertion, valid_key.clone().unbind());

        let op = ContainerOperation::Added {
            index: insertion,
            payload: PyTuple::new(py, [valid_key.clone(), valid_value.clone()])?
                .into_any()
                .unbind(),
        };
        self_.get().record_operation(py, op, self_)
    }

    #[pyo3(name = "move", signature = (key, before=None))]
    fn move_item<'py>(
        self_: &Bound<'py, NotifyingMap>,
        key: &Bound<'py, PyAny>,
        before: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<()> {
        let py = self_.py();
        let dict = self_.get().values_bound(py);
        let valid_key = self_.get().validate_key(py, key)?;
        let order_snapshot: &[Py<PyAny>] = unsafe { &*self_.get().order.get() };
        let current_index =
            NotifyingMap::order_index(order_snapshot, &valid_key, py).ok_or_else(|| {
                PyErr::new::<pyo3::exceptions::PyKeyError, _>(
                    NotifyingMap::key_to_string(&valid_key).unwrap_or_else(|_| "key".to_string()),
                )
            })?;

        if let Some(before_key) = before {
            let before_valid = self_.get().validate_key(py, before_key)?;
            if dict.get_item(&before_valid)?.is_none() {
                let key_repr = NotifyingMap::key_to_string(&before_valid)?;
                return Err(PyErr::new::<pyo3::exceptions::PyKeyError, _>(key_repr));
            }
            if before_valid.eq(&valid_key)? {
                return Ok(());
            }
            let target_index = NotifyingMap::order_index(order_snapshot, &before_valid, py)
                .unwrap_or(current_index);
            let order: &mut Vec<Py<PyAny>> = unsafe { &mut *self_.get().order.get() };
            let item = order.remove(current_index);
            let final_index = if current_index < target_index {
                target_index - 1
            } else {
                target_index
            };
            order.insert(final_index, item);
            let op = ContainerOperation::Moved {
                from_index: current_index,
                to_index: final_index,
                payload: PyTuple::new(
                    py,
                    [
                        valid_key.clone(),
                        dict.get_item(&valid_key)?
                            .expect("stored key must still exist"),
                    ],
                )?
                .into_any()
                .unbind(),
            };
            self_.get().record_operation(py, op, self_)?;
            return Ok(());
        }

        let order: &mut Vec<Py<PyAny>> = unsafe { &mut *self_.get().order.get() };
        let item = order.remove(current_index);
        order.push(item);
        let op = ContainerOperation::Moved {
            from_index: current_index,
            to_index: order.len().saturating_sub(1),
            payload: PyTuple::new(
                py,
                [
                    valid_key.clone(),
                    dict.get_item(&valid_key)?
                        .expect("stored key must still exist"),
                ],
            )?
            .into_any()
            .unbind(),
        };
        self_.get().record_operation(py, op, self_)
    }

    fn remove<'py>(
        self_: &Bound<'py, NotifyingMap>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = self_.py();
        let valid_key = self_.get().validate_key(py, key)?;
        let dict = self_.get().values_bound(py);
        let value = match dict.get_item(&valid_key)? {
            Some(value) => value,
            None => {
                let key_repr = NotifyingMap::key_to_string(&valid_key)?;
                return Err(PyErr::new::<pyo3::exceptions::PyKeyError, _>(key_repr));
            }
        };
        let order_snapshot: &[Py<PyAny>] = unsafe { &*self_.get().order.get() };
        let index = NotifyingMap::order_index(order_snapshot, &valid_key, py).unwrap_or(0);
        let order: &mut Vec<Py<PyAny>> = unsafe { &mut *self_.get().order.get() };
        order.remove(index);
        dict.del_item(&valid_key)?;
        let op = ContainerOperation::Removed {
            old_index: index,
            payload: PyTuple::new(py, [valid_key.clone(), value.clone()])?
                .into_any()
                .unbind(),
        };
        self_.get().record_operation(py, op, self_)?;
        Ok(value)
    }

    pub fn begin_batch_notifications(self_: &Bound<'_, NotifyingMap>) {
        self_.get().begin_batch_inner(self_);
    }

    pub fn end_batch(self_: &Bound<'_, NotifyingMap>) -> PyResult<()> {
        self_.get().end_batch_inner(self_.py(), self_)
    }

    pub fn batched_notifications<'py>(
        self_: &Bound<'py, NotifyingMap>,
    ) -> PyResult<Bound<'py, NotifyingMapBatchNotificationsContext>> {
        Bound::new(
            self_.py(),
            NotifyingMapBatchNotificationsContext {
                notifying_map: self_.clone().unbind(),
            },
        )
    }

    #[classmethod]
    pub fn __class_getitem__<'py>(
        cls: &Bound<'py, PyType>,
        item: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let py = item.py();
        let generic_alias = py
            .import(intern!(py, "types"))?
            .getattr(intern!(py, "GenericAlias"))?;
        generic_alias.call1((cls, item))
    }

    #[staticmethod]
    pub fn _construct<'py>(
        py: Python<'py>,
        _args: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, NotifyingMap>> {
        Bound::new(
            py,
            NotifyingMap {
                values: UnsafeCell::new(PyDict::new(py).unbind()),
                order: UnsafeCell::new(Vec::new()),
                key_validator: UnsafeCell::new(Validator::new(
                    crate::validators::TypeValidator::Any {},
                    None,
                    None,
                    None,
                )),
                value_validator: UnsafeCell::new(Validator::new(
                    crate::validators::TypeValidator::Any {},
                    None,
                    None,
                    None,
                )),
                member_name: UnsafeCell::new(None),
                object: UnsafeCell::new(None),
                notification_buffer: UnsafeCell::new(NotificationBuffer::new()),
            },
        )
    }

    pub fn __reduce_ex__<'py>(
        self_: &Bound<'py, Self>,
        py: Python<'py>,
        _protocol: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        let values = self_.get().values_bound(py);
        let items: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)> =
            unsafe { &*self_.get().order.get() }
                .iter()
                .map(|key| {
                    let key_bound = key.bind(py);
                    let value = values
                        .get_item(key_bound)?
                        .expect("stored key must still exist");
                    Ok((key_bound.clone(), value))
                })
                .collect::<PyResult<Vec<_>>>()?;
        let items_iter = items.into_bound_py_any(py)?.try_iter()?;
        (
            self_.getattr(intern!(py, "_construct"))?,
            (py.None(),),
            py.None(),
            py.None(),
            items_iter,
        )
            .into_bound_py_any(py)
    }
}
