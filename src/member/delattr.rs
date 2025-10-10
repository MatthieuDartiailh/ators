/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Matthieu C. Dartiailh
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{pyclass, types::PyAnyMethods};

///
#[pyclass(frozen)]
#[derive(Clone, Debug)]
pub enum DelattrBehavior {
    #[pyo3(constructor = ())]
    Slot {},
    #[pyo3(constructor = ())]
    Undeletable {},
}

impl DelattrBehavior {
    ///
    pub(crate) fn del<'py>(
        &self,
        member: &pyo3::Bound<'py, super::Member>,
        object: &pyo3::Bound<'py, crate::core::BaseAtors>,
    ) -> pyo3::PyResult<()> {
        match self {
            Self::Slot {} => todo!("Implement slot deletion"),
            Self::Undeletable {} => Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "The member {} from {} cannot be deleted",
                member.borrow().name,
                object.repr()?
            ))),
        }
    }
}
