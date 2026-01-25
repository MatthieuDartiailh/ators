/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
#![deny(unused_must_use)]

use pyo3::{
    Bound, Py, PyResult, Python, pymodule,
    sync::PyOnceLock,
    types::{PyAnyMethods, PyDict, PyTuple, PyType},
};

use crate::utils::TypeMutabilityMap;

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

static TYPE_MUTABILITY: PyOnceLock<Py<utils::TypeMutabilityMap>> = PyOnceLock::new();

fn get_type_mutability_map<'py>(py: Python<'py>) -> Bound<'py, TypeMutabilityMap> {
    TYPE_MUTABILITY
        .get_or_init(py, || TypeMutabilityMap::new(py))
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
        AtorsBase, freeze, get_member, get_member_customization_tool, get_members,
        get_members_by_tag, get_members_by_tag_and_value, init_ators, is_frozen,
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

    // Exported only to enable pickling
    #[pymodule_export]
    use self::containers::{AtorsDict, AtorsSet};

    #[pyfunction]
    pub(crate) fn add_generic_type_attributes<'py>(
        py: Python<'py>,
        type_: &Bound<'py, PyType>,
        attributes: Bound<'py, PyTuple>,
    ) -> PyResult<()> {
        let map = get_generic_attributes_map(py);
        map.set_item(type_, attributes)
    }

    #[pyfunction]
    /// Register a mutability specification for a given type.
    ///
    /// This function allows registering custom mutability information for a Python type.
    /// The mutability specification can be either a boolean or a callable.
    ///
    /// # Arguments
    ///
    /// * `type_` - The Python type for which to register mutability information (type: type[T])
    /// * `mutability` - Either:
    ///   - `True` (bool): The type is always considered mutable
    ///   - `False` (bool): The type is always considered immutable
    ///   - A callable `Callable[[T], bool]`: A function that takes an instance of type T
    ///     and returns a bool indicating whether that specific instance is mutable (True)
    ///     or immutable (False)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the registration was successful
    /// * `Err(PyErr)` - If the mutability value is neither a bool nor a valid callable
    ///   with the appropriate signature (must accept exactly one argument)
    ///
    /// # Example
    ///
    /// ```python
    /// from ators import add_type_mutability
    ///
    /// # Register that list is always mutable
    /// add_type_mutability(list, True)
    ///
    /// # Register that tuple is always immutable
    /// add_type_mutability(tuple, False)
    ///
    /// # Register custom mutability check
    /// class MyClass:
    ///     def __init__(self, is_mutable):
    ///         self.is_mutable = is_mutable
    ///
    /// def check_mutability(obj):
    ///     return obj.is_mutable
    ///
    /// add_type_mutability(MyClass, check_mutability)
    /// ```
    pub(crate) fn register_type_mutability_info<'py>(
        py: Python<'py>,
        type_: &Bound<'py, PyType>,
        mutability: &Bound<'py, pyo3::PyAny>,
    ) -> PyResult<()> {
        let map_py = TYPE_MUTABILITY.get_or_init(py, || TypeMutabilityMap::new(py));
        let map_bound = map_py.clone_ref(py).into_bound(py);
        // Use Python's setitem protocol
        map_bound.set_item(type_, mutability)
    }
}
