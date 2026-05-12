/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
use pyo3::{
	Bound, IntoPyObjectExt, Py, PyAny, PyResult, PyTypeInfo, Python, ffi, intern, pyclass,
	pymethods,
	sync::critical_section::with_critical_section,
	types::{PyAnyMethods, PyList, PyListMethods, PySlice},
};
use std::cell::UnsafeCell;

use crate::{
	class::AtorsBase,
	containers::{common::matches_assignment_context, AtorsDict, AtorsSet},
	utils::error_on_minusone,
	validators::Validator,
};

#[pyclass(module = "ators._ators", extends=PyList, frozen)]
pub struct AtorsList {
	validator: UnsafeCell<Validator>,
	member_name: UnsafeCell<Option<String>>,
	// Wrapped in UnsafeCell to allow clearing during GC while keeping the class frozen.
	object: UnsafeCell<Option<Py<AtorsBase>>>,
}

// Safety: validator and member_name are written only once (at construction or during restore
// before any other references exist), and after that are effectively immutable; object is only
// modified during __clear__, which Python's GC calls only once all references to this object
// have been dropped - ensuring no concurrent access (holds for both GIL and free-threaded builds).
unsafe impl Sync for AtorsList {}

impl AtorsList {
	pub(crate) fn new_empty<'py>(
		py: Python<'py>,
		validator: Validator,
		member_name: Option<&str>,
		object: Option<Py<AtorsBase>>,
	) -> PyResult<Bound<'py, AtorsList>> {
		Bound::new(
			py,
			AtorsList {
				validator: UnsafeCell::new(validator),
				member_name: UnsafeCell::new(member_name.map(|m| m.to_string())),
				object: UnsafeCell::new(object),
			},
		)
	}

	fn validate_item<'py>(
		&self,
		py: Python<'py>,
		value: &Bound<'py, PyAny>,
	) -> PyResult<Bound<'py, PyAny>> {
		// Safety: validator and member_name are written only once (at construction or restore)
		// and are effectively immutable during normal use. A live reference is required to call
		// this method, ensuring no concurrent restoration is occurring.
		let validator = unsafe { &*self.validator.get() };
		let m = unsafe { &*self.member_name.get() }.as_deref();
		// Safety: object is only written during __clear__, which can only run after all
		// live references to this object are gone. A live reference is required to call
		// this method, so __clear__ cannot run concurrently (holds for both GIL and
		// free-threaded builds).
		let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
		validator.validate(m, o, value)
	}

	fn validate_iterable<'py>(
		&self,
		py: Python<'py>,
		value: &Bound<'py, PyAny>,
	) -> PyResult<Bound<'py, PyList>> {
		// Safety: same as validate_item.
		let validator = unsafe { &*self.validator.get() };
		let m = unsafe { &*self.member_name.get() }.as_deref();
		let o = unsafe { &*self.object.get() }.as_ref().map(|o| o.bind(py));
		let mut validated_items = Vec::with_capacity(value.len().unwrap_or(0));
		for item in value.try_iter()? {
			let valid = validator.validate(m, o, &item?)?;
			validated_items.push(valid);
		}
		PyList::new(py, validated_items)
	}

	pub(crate) fn matches_assignment_context<'py>(
		&self,
		member_name: Option<&str>,
		object: Option<&Bound<'py, AtorsBase>>,
	) -> bool {
		matches_assignment_context(&self.member_name, &self.object, member_name, object)
	}

	pub(crate) fn clone_for_assignment<'py>(
		source: &Bound<'py, AtorsList>,
	) -> PyResult<Bound<'py, AtorsList>> {
		let list = source.get();
		// Safety: same as validate_item.
		let validator = unsafe { &*list.validator.get() }.clone();
		let member_name = unsafe { &*list.member_name.get() }
			.as_deref()
			.map(|s| s.to_string());
		let object = unsafe { &*list.object.get() }
			.as_ref()
			.map(|object| object.clone_ref(source.py()));
		let alist = AtorsList::new_empty(source.py(), validator, member_name.as_deref(), object)?;
		// Safety: AtorsList is declared as `extends=PyList`, so this cast is always valid.
		let py_list = unsafe { source.cast_unchecked::<PyList>() };
		let alist_as_list = alist.cast::<PyList>()?;
		for item in py_list.iter() {
			alist_as_list.append(&item)?;
		}
		Ok(alist)
	}

	/// Restore Ators-specific metadata after unpickling.
	/// Called by `AtorsBase.__setstate__` before writing the container to a slot.
	pub(crate) fn restore<'py>(
		alist: &Bound<'py, AtorsList>,
		validator: Validator,
		member_name: Option<&str>,
		object: Option<&Bound<'py, AtorsBase>>,
	) {
		use crate::validators::types::TypeValidator;

		// Capture the validator so we can restore nested containers after
		// rebinding this list metadata.
		let item_v = validator.clone();

		with_critical_section(alist.as_any(), || {
			let inner = alist.get();
			// Safety: we hold the critical section lock. These fields are only written
			// here (during restore) and during construction; after restore they are
			// effectively immutable, matching the normal post-construction invariant.
			unsafe {
				(*inner.validator.get()) = validator;
				(*inner.member_name.get()) = member_name.map(|s| s.to_string());
				(*inner.object.get()) = object.map(|o| o.clone().unbind());
			}
		});

		// Restore any nested containers within the list items.
		// Safety: AtorsList is declared as `extends=PyList`, so this cast is always valid.
		let py_list = unsafe { alist.cast_unchecked::<PyList>() };
		for list_item in py_list.iter() {
			match &item_v.type_validator {
				TypeValidator::List {
					item: Some(nested_bv),
				} => {
					if let Ok(nested) = list_item.cast::<AtorsList>() {
						AtorsList::restore(nested, (*nested_bv.0).clone(), member_name, object);
					}
				}
				TypeValidator::Set {
					item: Some(nested_bv),
				} => {
					if let Ok(nested) = list_item.cast::<AtorsSet>() {
						AtorsSet::restore(nested, (*nested_bv.0).clone(), member_name, object);
					}
				}
				TypeValidator::Dict {
					items: Some((key_bv, val_bv)),
				} => {
					if let Ok(nested) = list_item.cast::<AtorsDict>() {
						AtorsDict::restore(
							nested,
							(*key_bv.0).clone(),
							(*val_bv.0).clone(),
							member_name,
							object,
						);
					}
				}
				_ => {}
			}
		}
	}
}

// remove, pop, clear, sort, reverse and __imul__ do not need
// item validation since they only remove or rearrange existing items.
// append, insert, __setitem__, extend and __iadd__ need item validation
// since they can add new items.
#[pymethods]
impl AtorsList {
	/// Append a value after validating it with the list item validator.
	pub fn append<'py>(self_: &Bound<'py, AtorsList>, value: &Bound<'py, PyAny>) -> PyResult<()> {
		let py = value.py();
		let valid = self_.get().validate_item(py, value)?;
		// SAFETY: AtorsList is declared as `extends=PyList`, so this cast is
		// always valid, and the resulting PyList is valid for calling append.
		unsafe { self_.cast_unchecked::<PyList>() }.append(&valid)
	}

	/// Insert a value at `index` after validating it with the item validator.
	pub fn insert<'py>(
		self_: &Bound<'py, AtorsList>,
		index: usize,
		value: &Bound<'py, PyAny>,
	) -> PyResult<()> {
		let py = value.py();
		let valid = self_.get().validate_item(py, value)?;
		// SAFETY: AtorsList is declared as `extends=PyList`, so this cast is
		// always valid, and the resulting PyList is valid for calling append.
		unsafe { self_.cast_unchecked::<PyList>() }.insert(index, &valid)
	}

	pub fn __setitem__<'py>(
		self_: &Bound<'py, AtorsList>,
		index: &Bound<'py, PyAny>,
		value: &Bound<'py, PyAny>,
	) -> PyResult<()> {
		let py = index.py();

		// Cast once to PyList (AtorsList extends PyList). Use unchecked cast to avoid
		// an extra runtime check and to get access to PyList helper methods.
		let list = unsafe { self_.cast_unchecked::<PyList>() };

		// Slice assignment path
		if index.is_instance_of::<PySlice>() {
			// Validate the list on the RHS
			let validated_list = self_.get().validate_iterable(py, value)?;

			// Use direct slo access to use the proper PyList method (since we have no super)
			return error_on_minusone(py, unsafe {
				(*(*PyList::type_object_raw(py)).tp_as_mapping)
					.mp_ass_subscript
					.unwrap()(self_.as_ptr(), index.as_ptr(), validated_list.as_ptr())
			});
		}

		// Non-slice: single-index assignment
		// Validate the new value under critical section
		let valid = self_.get().validate_item(py, value)?;

		// Convert index to integer using PyO3's extract (honours __index__/index-like subclasses)
		let idx = index.as_any().extract::<isize>()?;

		// Normalize negative indices relative to list length
		let len = list.len() as isize;
		let normalized = if idx < 0 { idx + len } else { idx };
		if normalized < 0 || normalized >= len {
			return Err(pyo3::exceptions::PyIndexError::new_err(
				"list assignment index out of range",
			));
		}

		// Use high-level set_item (no ffi), conversion is safe since normalized is > 0
		list.set_item(normalized as usize, valid.as_any())?;
		Ok(())
	}

	// Required since CPython uses a single slot for setitem/delitem which prevents
	// inheriting the delitem behavior from PyList when __setitem__ is overridden.
	pub fn __delitem__<'py>(
		self_: &Bound<'py, AtorsList>,
		index: &Bound<'py, PyAny>,
	) -> PyResult<()> {
		let py = self_.py();
		error_on_minusone(py, unsafe {
			(*(*PyList::type_object_raw(py)).tp_as_mapping)
				.mp_ass_subscript
				.unwrap()(
				self_.as_ptr(),
				index.as_ptr(),
				std::ptr::null_mut::<ffi::PyObject>(),
			)
		})
	}

	/// Extend the list with values from `other` after validating each item.
	pub fn extend<'py>(self_: &Bound<'py, AtorsList>, other: &Bound<'py, PyAny>) -> PyResult<()> {
		let valid = with_critical_section(self_.as_any(), || {
			self_.get().validate_iterable(other.py(), other)
		})?;
		let list = unsafe { self_.cast_unchecked::<PyList>() };
		unsafe {
			error_on_minusone(
				self_.py(),
				ffi::compat::PyList_Extend(list.as_ptr(), valid.as_ptr()),
			)
		}
	}

	pub fn __iadd__<'py>(self_: &Bound<'py, Self>, value: &Bound<'py, PyAny>) -> PyResult<()> {
		AtorsList::extend(self_, value)
	}

	// The traverse method of the parent class (PyList) is called automatically and
	// the type is also traversed so we only need to visit our own references.
	pub fn __traverse__(&self, visit: pyo3::PyVisit) -> Result<(), pyo3::PyTraverseError> {
		// Safety: Python guarantees exclusive access when calling GC methods, ensuring
		// no concurrent mutation (holds for both GIL and free-threaded builds).
		if let Some(o) = unsafe { &*self.object.get() } {
			visit.call(o)?;
		}
		Ok(())
	}

	// The clear method of the parent class (PyList) is called automatically and
	// so we only need to visit our own references.
	pub fn __clear__(&self) {
		// Safety: Python guarantees exclusive access when calling GC methods, ensuring
		// no concurrent mutation (holds for both GIL and free-threaded builds).
		unsafe { *self.object.get() = None };
	}

	#[staticmethod]
	pub fn _construct<'py>(
		py: Python<'py>,
		_args: &Bound<'py, PyAny>,
	) -> PyResult<Bound<'py, AtorsList>> {
		// This is a dummy constructor used solely for unpickling. It creates an empty AtorsList
		// without any meaningful metadata; the actual validator and related metadata will be
		// populated by the restore method called from AtorsBase.__setstate__ after construction.
		use crate::validators::types::TypeValidator;
		Bound::new(
			py,
			AtorsList {
				validator: UnsafeCell::new(Validator {
					type_validator: TypeValidator::Any {},
					value_validators: Box::new([]),
					coercer: None,
					init_coercer: None,
				}),
				member_name: UnsafeCell::new(None),
				object: UnsafeCell::new(None),
			},
		)
	}

	pub fn __reduce_ex__<'py>(
		self_: &Bound<'py, Self>,
		py: Python<'py>,
		_protocol: usize,
	) -> PyResult<Bound<'py, PyAny>> {
		(
			self_.getattr(intern!(py, "_construct"))?,
			(py.None(),),
			py.None(),
			unsafe { self_.cast_unchecked::<PyList>() }.try_iter()?,
		)
			.into_bound_py_any(py)
	}
}
