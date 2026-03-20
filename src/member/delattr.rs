/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
/// Member delattr behavior definitions and related utilities.
use pyo3::{Bound, PyRef, PyResult, pyclass, types::PyAnyMethods};

/// Member delattr behavior definitions and related utilities.
#[pyclass(module = "ators._ators", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub enum DelattrBehavior {
    #[pyo3(constructor = ())]
    Slot {},
    #[pyo3(constructor = ())]
    Undeletable {},
}

impl DelattrBehavior {
    /// Behavior to execute when deleting a member value.
    pub(crate) fn del<'py>(
        &self,
        member: &PyRef<'py, super::Member>,
        object: &Bound<'py, crate::core::AtorsBase>,
    ) -> PyResult<()> {
        match self {
            Self::Slot {} => {
                crate::core::del_slot(object, member.index());
                Ok(())
            }
            Self::Undeletable {} => Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "The member {} from {} cannot be deleted",
                member.name,
                object.repr()?
            ))),
        }
    }
}
