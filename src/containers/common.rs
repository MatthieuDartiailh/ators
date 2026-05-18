/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
use pyo3::{Bound, Py};
use std::cell::UnsafeCell;

use crate::class::AtorsBase;

pub(super) fn matches_assignment_context<'py>(
    member_name_cell: &UnsafeCell<Option<String>>,
    object_cell: &UnsafeCell<Option<Py<AtorsBase>>>,
    member_name: Option<&str>,
    object: Option<&Bound<'py, AtorsBase>>,
) -> bool {
    // Safety: member_name/object cells are initialized during construction and
    // updated only in restore/clear paths under invariants described by callers.
    unsafe { &*member_name_cell.get() }.as_deref() == member_name
        && match (unsafe { &*object_cell.get() }.as_ref(), object) {
            (None, None) => true,
            (Some(stored), Some(current)) => stored.bind(current.py()).as_ptr() == current.as_ptr(),
            _ => false,
        }
}
