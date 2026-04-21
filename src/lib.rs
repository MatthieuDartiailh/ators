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
    types::{PyAnyMethods, PyType},
};

use crate::utils::{GenericAttributesMap, TypeMutabilityMap};

mod annotations;
mod class_info;
mod containers;
mod core;
mod member;
mod meta;
mod observers;
mod utils;
mod validators;

// XXX would prefer to have module state to do this
// static ANNOTATIONS_TOOLS : PyOnceLock

static GENERIC_ATTRIBUTES: PyOnceLock<Py<GenericAttributesMap>> = PyOnceLock::new();

fn get_generic_attributes_map<'py>(py: Python<'py>) -> Bound<'py, GenericAttributesMap> {
    GENERIC_ATTRIBUTES
        .get_or_init(py, || GenericAttributesMap::new(py))
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
        AtorsBase, PicklePolicy, disable_notifications, enable_notifications, freeze, get_member,
        get_member_customization_tool, get_members, get_members_by_tag,
        get_members_by_tag_and_value, is_frozen, is_notifications_enabled, observe, unobserve,
    };
    #[pymodule_export]
    use self::meta::{create_ators_specialized_subclass, create_ators_subclass};

    #[pymodule_export]
    use self::class_info::{
        get_ators_args, get_ators_frozen_flag, get_ators_init_member_names,
        get_ators_members_by_name, get_ators_origin, get_ators_specific_member_names,
        get_ators_type_params,
    };

    #[pymodule_export]
    use self::meta::{rust_instancecheck, rust_subclasscheck};

    #[pymodule_export]
    use self::member::{
        DefaultBehavior, DelattrBehavior, Member, MemberBuilder, PostGetattrBehavior,
        PostSetattrBehavior, PreGetattrBehavior, PreSetattrBehavior,
    };

    #[pymodule_export]
    use self::validators::{Coercer, TypeValidator, Validator, ValueValidator};

    // Exported only to enable pickling
    #[pymodule_export]
    use self::containers::{AtorsDict, AtorsList, AtorsSet};

    #[pymodule_export]
    use self::observers::AtorsChange;

    #[pyfunction]
    /// Register generic attribute names for a Python type.
    ///
    /// Stores the list of attribute names associated with a generic type so they
    /// can be reused by the runtime when handling parametrized type information.
    ///
    /// # Arguments
    ///
    /// * `type_` - The Python type for which generic attribute names are registered.
    /// * `attributes` - The attribute names to associate with the given type.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the registration succeeds.
    /// * `Err(PyErr)` - If inserting the mapping into the internal storage fails.
    pub(crate) fn add_generic_type_attributes<'py>(
        py: Python<'py>,
        type_: &Bound<'py, PyType>,
        attributes: Vec<String>,
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
    /// from ators import register_type_mutability_info
    ///
    /// # Register that list is always mutable
    /// register_type_mutability_info(list, True)
    ///
    /// # Register that tuple is always immutable
    /// register_type_mutability_info(tuple, False)
    ///
    /// # Register custom mutability check
    /// class MyClass:
    ///     def __init__(self, is_mutable):
    ///         self.is_mutable = is_mutable
    ///
    /// def check_mutability(obj):
    ///     return obj.is_mutable
    ///
    /// register_type_mutability_info(MyClass, check_mutability)
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
