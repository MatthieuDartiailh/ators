///
use pyo3::{
    Bound, IntoPyObject, IntoPyObjectExt, Py, PyAny, PyRef, PyResult, Python, pyclass, pymethods,
    sync::with_critical_section2,
    types::{PyAnyMethods, PyDict, PyDictMethods},
};
use std::clone::Clone;

mod default;
use default::DefaultBehavior;
mod delattr;
use delattr::DelattrBehavior;
mod getattr;
use getattr::{PostGetattrBehavior, PreGetattrBehavior};
mod pickle;
mod setattr;
use crate::validators::Validator;
use setattr::{PostSetattrBehavior, PreSetattrBehavior};

/// A Python descriptor that defines a member of an Ators class.
#[pyclass]
pub struct Member {
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    slot_index: u16,
    // All attributes below are frozen enums so they cannot be modified at runtime
    // and we can safely return clones of them.
    #[pyo3(get, set)]
    pre_getattr: PreGetattrBehavior,
    #[pyo3(get, set)]
    post_getattr: PostGetattrBehavior,
    #[pyo3(get, set)]
    pre_setattr: PreSetattrBehavior,
    #[pyo3(get, set)]
    post_setattr: PostSetattrBehavior,
    #[pyo3(get, set)]
    delattr: DelattrBehavior,
    #[pyo3(get, set)]
    default: DefaultBehavior,
    #[pyo3(get, set)]
    validator: Validator,
    // Optional metadata dictionary that can be used to store arbitrary information
    // about the member.
    metadata: Option<Py<PyDict>>,
}

#[pymethods]
impl Member {
    #[new]
    #[allow(clippy::too_many_arguments)]
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
            metadata: None,
        }
    }

    #[pyo3(signature = (**tags))]
    pub fn tag<'py>(&mut self, py: Python<'py>, tags: Option<&Bound<'_, PyDict>>) {
        if self.metadata.is_none() {
            self.metadata = Some(PyDict::new(py).unbind());
        }
        if let Some(tags) = tags
            && let Some(d) = &mut self.metadata
        {
            d.bind(py).update(tags.as_mapping()).unwrap();
        };
    }

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
