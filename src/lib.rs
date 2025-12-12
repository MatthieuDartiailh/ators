/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
use pyo3::{
    Bound, Py, PyResult, Python, pymodule,
    sync::PyOnceLock,
    types::{PyAnyMethods, PyDict, PyTuple, PyType},
};

mod annotations;
mod containers;
mod core;
mod member;
mod meta;
mod utils;
mod validators;

// XXX would prefer to have module state to do this
// static ANNOTATIONS_TOOLS : PyOnceLock

static GENERIC_ATTRIBUTES: PyOnceLock<Py<PyDict>> = PyOnceLock::new();

fn get_generic_attributes_map<'py>(py: Python<'py>) -> Bound<'py, PyDict> {
    GENERIC_ATTRIBUTES
        .get_or_init(py, || PyDict::new(py).into())
        .clone_ref(py)
        .into_bound(py)
}

/// A Python module implemented in Rust.
#[pymodule]
mod _ators {
    use pyo3::pyfunction;

    use super::*;

    #[pymodule_export]
    use self::core::{
        AtorsBase, freeze, get_member, get_members, get_members_by_tag,
        get_members_by_tag_and_value, init_ators, is_frozen,
    };
    #[pymodule_export]
    use self::meta::create_ators_subclass;

    #[pymodule_export]
    use self::member::{
        DefaultBehavior, DelattrBehavior, Member, MemberBuilder, PostGetattrBehavior,
        PostSetattrBehavior, PreGetattrBehavior, PreSetattrBehavior,
    };

    #[pymodule_export]
    use self::validators::{Coercer, TypeValidator, Validator, ValueValidator};

    #[pyfunction]
    pub(crate) fn add_generic_type_attributes<'py>(
        py: Python<'py>,
        type_: &Bound<'py, PyType>,
        attributes: Bound<'py, PyTuple>,
    ) -> PyResult<()> {
        let map = get_generic_attributes_map(py);
        map.set_item(type_, attributes)
    }
}
