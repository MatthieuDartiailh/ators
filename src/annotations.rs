/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
/// Tools to manipulate and extract information from type annotations.
use pyo3::{
    Bound, PyAny, PyErr, PyResult, PyTypeInfo, Python, intern, pyclass,
    sync::critical_section::with_critical_section,
    types::{
        PyAnyMethods, PyBool, PyBytes, PyComplex, PyDict, PyDictMethods, PyFloat, PyFrozenSet,
        PyInt, PyList, PyListMethods, PyMapping, PyMappingMethods, PySet, PyString, PyTuple,
        PyTupleMethods, PyType, PyTypeMethods,
    },
};
use std::collections::HashMap;
use std::ffi::CString;

use crate::{
    event::EventBuilder,
    get_generic_attributes_map,
    member::{DefaultBehavior, DelattrBehavior, Member, MemberBuilder, PreSetattrBehavior},
    utils::err_with_cause,
    validators::{
        TypeValidator, ValidValues, Validator, ValueValidator,
        types::{BoxedValidator, LateResolvedValidator},
    },
};

/// Information extracted while building a validator from an annotation.
#[pyclass(module = "ators._ators", frozen, get_all, skip_from_py_object)]
#[derive(Clone, Debug, Default)]
pub struct ValidatorBuildInfo {
    /// Whether the validator contains a ForwardValidator that requires an owner
    requires_owner: bool,
}

impl ValidatorBuildInfo {
    pub fn requires_owner(&self) -> bool {
        self.requires_owner
    }
}

/// Types requiring special treatment when encountered in type annotations.
pub(crate) struct PyTypes<'py> {
    object: Bound<'py, PyAny>,
    any: Bound<'py, PyAny>,
    final_: Bound<'py, PyAny>,
    union_: Bound<'py, PyAny>,
    type_var: Bound<'py, PyAny>,
    new_type: Bound<'py, PyAny>,
    forward_ref: Bound<'py, PyAny>,
    literal: Bound<'py, PyAny>,
    type_alias: Bound<'py, PyAny>,
    unpack: Bound<'py, PyAny>,
    abc_sequence: Bound<'py, PyAny>,
    abc_collection: Bound<'py, PyAny>,
    abc_mapping: Bound<'py, PyAny>,
    // FIXME defaultdict
}

/// Tools to manipulate and extract information from type annotations.
pub(crate) struct TypeTools<'py> {
    get_origin: Bound<'py, PyAny>,
    get_args: Bound<'py, PyAny>,
    call_evaluate_function: Bound<'py, PyAny>,
    forwardref_format: Bound<'py, PyAny>,
    types: PyTypes<'py>,
}

pub(crate) fn get_type_tools<'py>(py: Python<'py>) -> Result<TypeTools<'py>, PyErr> {
    let annotationlib = py.import(intern!(py, "annotationlib"))?;

    let builtins_mod = py.import(intern!(py, "builtins"))?;
    let abc_mod = py.import(intern!(py, "collections.abc"))?;
    let types_mod = py.import(intern!(py, "types"))?;
    let typing_mod = py.import(intern!(py, "typing"))?;

    // FIXME This should be created only once
    // Store the object in the _ators module namespace
    // Require a different object not linked to the py lifetime...
    // #[pymodule_init]
    // fn init(m: &Bound<'_, PyModule>) -> PyResult<()> {
    //     // Arbitrary code to run at the module initialization
    //     m.add("double2", m.getattr("double")?)
    // }
    Ok(TypeTools {
        get_args: typing_mod.getattr(intern!(py, "get_args"))?,
        get_origin: typing_mod.getattr(intern!(py, "get_origin"))?,
        call_evaluate_function: annotationlib.getattr(intern!(py, "call_evaluate_function"))?,
        forwardref_format: annotationlib
            .getattr(intern!(py, "Format"))?
            .getattr(intern!(py, "FORWARDREF"))?,
        types: PyTypes {
            object: builtins_mod.getattr(intern!(py, "object"))?,
            any: typing_mod.getattr(intern!(py, "Any"))?,
            final_: typing_mod.getattr(intern!(py, "Final"))?,
            union_: types_mod.getattr(intern!(py, "UnionType"))?,
            type_var: typing_mod.getattr(intern!(py, "TypeVar"))?,
            new_type: typing_mod.getattr(intern!(py, "NewType"))?,
            forward_ref: annotationlib.getattr(intern!(py, "ForwardRef"))?,
            literal: typing_mod.getattr(intern!(py, "Literal"))?,
            type_alias: typing_mod.getattr(intern!(py, "TypeAliasType"))?,
            unpack: typing_mod.getattr(intern!(py, "Unpack"))?,
            abc_sequence: abc_mod.getattr(intern!(py, "Sequence"))?,
            abc_collection: abc_mod.getattr(intern!(py, "Collection"))?,
            abc_mapping: abc_mod.getattr(intern!(py, "Mapping"))?,
        },
    })
}

/// Check if a TypeValidator represents a mutable built-in container (list, dict, or set).
/// These cannot be used as item types in abstract collections.
/// This includes both explicit List/Dict/Set validators and Typed validators where the type
/// is one of the mutable built-in container types.
fn is_mutable_container<'py>(tv: &TypeValidator, py: Python<'py>) -> bool {
    match tv {
        TypeValidator::List { .. } | TypeValidator::Dict { .. } | TypeValidator::Set { .. } => true,
        TypeValidator::Typed { type_ } => {
            // Check if the type is one of the mutable built-in container types
            // by comparing against the Python type objects
            let type_bound = type_.bind(py);
            
            // Check if it's the list, dict, or set type
            py.get_type::<PyList>().is(&type_bound)
                || py.get_type::<PyDict>().is(&type_bound)
                || py.get_type::<PySet>().is(&type_bound)
        }
        _ => false,
    }
}

/// Build a validator from a type annotation, extracting as much information as
/// possible to optimize validation and behavior definition. The returned
/// ValidatorBuildInfo contains information about the built validator that may
/// be useful to configure the member builder or the behaviors.
pub fn build_validator_from_annotation<'py>(
    name: &Bound<'py, PyString>,
    ann: &Bound<'py, PyAny>,
    type_containers: i64, // not sure this is worth keeping it
    tools: &TypeTools<'py>,
    ctx_provider: Option<&Bound<'py, PyAny>>,
    typevar_bindings: Option<&Bound<'py, PyDict>>,
) -> PyResult<(Validator, ValidatorBuildInfo)> {
    // Ators generic specializations can be represented as GenericAlias wrappers
    // on the Python side; unwrap them to the canonical specialized class for
    // validator inference.
    let ann = if ann.hasattr(intern!(name.py(), "__ators_specialized_class__"))? {
        ann.getattr(intern!(name.py(), "__ators_specialized_class__"))?
    } else {
        ann.clone()
    };

    if ann.is_instance_of::<PyString>() {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
            "Str annotations ({}) are not supported in ators classes, use ForwardRef instead",
            ann.repr()?
        )));
    } else if ann.is_instance(&tools.types.forward_ref)? {
        return Ok((
            Validator::new(
                TypeValidator::ForwardValidator {
                    late_validator: LateResolvedValidator::new(
                        &ann,
                        ctx_provider,
                        type_containers,
                        name,
                        typevar_bindings,
                    ),
                },
                None,
                None,
                None,
            ),
            ValidatorBuildInfo {
                requires_owner: true,
            },
        ));
    } else if ann.is_instance(&tools.types.type_alias)? {
        return build_validator_from_annotation(
            name,
            &tools
                .call_evaluate_function
                .call1((ann.getattr("evaluate_value")?, &tools.forwardref_format))?
                .cast_into()?,
            type_containers,
            tools,
            ctx_provider,
            typevar_bindings,
        );
    }

    let py = name.py();

    // Applicable on any type and return None for non generic types
    // Using this rather than is_instance is easier to get right since
    // some generics such as Literal use specific private classes.
    let origin = tools.get_origin.call1((&ann,))?;

    // In 3.14, Union[int, float] and int | float share the same type
    if !origin.is_none() {
        // FIXME extract in a dedicated function since it will be expanded
        // to cover list, dict, Numpy.NDArray etc
        let args = tools.get_args.call1((&ann,))?.cast_into::<PyTuple>()?;
        if origin.is(&tools.types.literal) {
            Ok((
                Validator::new(
                    TypeValidator::Any {},
                    Some(vec![ValueValidator::Values {
                        values: ValidValues(
                            PyFrozenSet::type_object(py)
                                .call1((args,))?
                                .cast_into()?
                                .unbind(),
                        ),
                    }]),
                    None,
                    None,
                ),
                ValidatorBuildInfo {
                    requires_owner: false,
                },
            ))
        } else if origin.is(py.get_type::<PyTuple>()) {
            if args.len() == 2 && args.get_item(1).expect("Known 2-tuple").is(py.Ellipsis()) {
                // VarTuple
                let (item_validator, item_info) = build_validator_from_annotation(
                    PyString::new(py, &format!("{name}-item")).cast()?,
                    &args.get_item(0).expect("Known 2-tuple"),
                    type_containers,
                    tools,
                    ctx_provider,
                    typevar_bindings,
                )?;
                Ok((
                    Validator::new(
                        TypeValidator::VarTuple {
                            item: Some(BoxedValidator::from(item_validator)),
                        },
                        None,
                        None,
                        None,
                    ),
                    ValidatorBuildInfo {
                        requires_owner: item_info.requires_owner,
                    },
                ))
            } else {
                // Fixed length tuple
                let mut items = Vec::new();
                let mut requires_owner = false;
                for item in args.iter() {
                    let (item_validator, item_info) = build_validator_from_annotation(
                        PyString::new(py, &format!("{name}-item")).cast()?,
                        &item,
                        type_containers,
                        tools,
                        ctx_provider,
                        typevar_bindings,
                    )?;
                    requires_owner = requires_owner || item_info.requires_owner;
                    items.push(item_validator);
                }
                Ok((
                    Validator::new(TypeValidator::Tuple { items }, None, None, None),
                    ValidatorBuildInfo { requires_owner },
                ))
            }
        } else if origin.is(py.get_type::<PyFrozenSet>()) {
            let (item_val, requires_owner) = if let Ok(item_arg) = args.get_item(0) {
                let (item_validator, item_info) = build_validator_from_annotation(
                    PyString::new(py, &format!("{name}-item")).cast()?,
                    &item_arg,
                    type_containers,
                    tools,
                    ctx_provider,
                    typevar_bindings,
                )?;
                (
                    Some(BoxedValidator::from(item_validator)),
                    item_info.requires_owner,
                )
            } else {
                (None, false)
            };
            Ok((
                Validator::new(
                    TypeValidator::FrozenSet { item: item_val },
                    None,
                    None,
                    None,
                ),
                ValidatorBuildInfo { requires_owner },
            ))
        } else if origin.is(py.get_type::<PySet>()) {
            let (item_val, requires_owner) = if let Ok(item_arg) = args.get_item(0) {
                let (item_validator, item_info) = build_validator_from_annotation(
                    PyString::new(py, &format!("{name}-item")).cast()?,
                    &item_arg,
                    type_containers,
                    tools,
                    ctx_provider,
                    typevar_bindings,
                )?;
                (
                    Some(BoxedValidator::from(item_validator)),
                    item_info.requires_owner,
                )
            } else {
                (None, false)
            };
            Ok((
                Validator::new(TypeValidator::Set { item: item_val }, None, None, None),
                ValidatorBuildInfo { requires_owner },
            ))
        } else if origin.is(py.get_type::<PyList>()) {
            let (item_val, requires_owner) = if let Ok(item_arg) = args.get_item(0) {
                let (item_validator, item_info) = build_validator_from_annotation(
                    PyString::new(py, &format!("{name}-item")).cast()?,
                    &item_arg,
                    type_containers,
                    tools,
                    ctx_provider,
                    typevar_bindings,
                )?;
                (
                    Some(BoxedValidator::from(item_validator)),
                    item_info.requires_owner,
                )
            } else {
                (None, false)
            };
            Ok((
                Validator::new(TypeValidator::List { item: item_val }, None, None, None),
                ValidatorBuildInfo { requires_owner },
            ))
        } else if origin.is(py.get_type::<PyDict>()) {
            let (items_validator, requires_owner) = if let Ok((key_arg, val_arg)) = args.extract() {
                let (key_validator, key_info) = build_validator_from_annotation(
                    PyString::new(py, &format!("{name}-key")).cast()?,
                    &key_arg,
                    type_containers,
                    tools,
                    ctx_provider,
                    typevar_bindings,
                )?;
                let (val_validator, val_info) = build_validator_from_annotation(
                    PyString::new(py, &format!("{name}-value")).cast()?,
                    &val_arg,
                    type_containers,
                    tools,
                    ctx_provider,
                    typevar_bindings,
                )?;
                (
                    Some((
                        BoxedValidator::from(key_validator),
                        BoxedValidator::from(val_validator),
                    )),
                    key_info.requires_owner || val_info.requires_owner,
                )
            } else {
                (None, false)
            };
            Ok((
                Validator::new(
                    TypeValidator::Dict {
                        items: items_validator,
                    },
                    None,
                    None,
                    None,
                ),
                ValidatorBuildInfo { requires_owner },
            ))
        } else if origin.is(&tools.types.abc_sequence) {
            let (item_val, requires_owner) = if let Ok(item_arg) = args.get_item(0) {
                let (item_validator, item_info) = build_validator_from_annotation(
                    PyString::new(py, &format!("{name}-item")).cast()?,
                    &item_arg,
                    type_containers,
                    tools,
                    ctx_provider,
                    typevar_bindings,
                )?;
                if is_mutable_container(&item_validator.type_validator, py) {
                    return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                        "Cannot use mutable container {} as item type for Sequence. \
                         Mutable containers (list, dict, set) cannot be used in abstract collections \
                         because ators cannot insert wrapped versions inside them.",
                        item_arg.repr()?
                    )));
                }
                (
                    Some(BoxedValidator::from(item_validator)),
                    item_info.requires_owner,
                )
            } else {
                (None, false)
            };
            Ok((
                Validator::new(
                    TypeValidator::Sequence { item: item_val },
                    None,
                    None,
                    None,
                ),
                ValidatorBuildInfo { requires_owner },
            ))
        } else if origin.is(&tools.types.abc_collection) {
            let (item_val, requires_owner) = if let Ok(item_arg) = args.get_item(0) {
                let (item_validator, item_info) = build_validator_from_annotation(
                    PyString::new(py, &format!("{name}-item")).cast()?,
                    &item_arg,
                    type_containers,
                    tools,
                    ctx_provider,
                    typevar_bindings,
                )?;
                if is_mutable_container(&item_validator.type_validator, py) {
                    return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                        "Cannot use mutable container {} as item type for Collection. \
                         Mutable containers (list, dict, set) cannot be used in abstract collections \
                         because ators cannot insert wrapped versions inside them.",
                        item_arg.repr()?
                    )));
                }
                (
                    Some(BoxedValidator::from(item_validator)),
                    item_info.requires_owner,
                )
            } else {
                (None, false)
            };
            Ok((
                Validator::new(
                    TypeValidator::Collection { item: item_val },
                    None,
                    None,
                    None,
                ),
                ValidatorBuildInfo { requires_owner },
            ))
        } else if origin.is(&tools.types.abc_mapping) {
            let (items_validator, requires_owner) =
                if let Ok((key_arg, val_arg)) = args.extract() {
                    let (key_validator, key_info) = build_validator_from_annotation(
                        PyString::new(py, &format!("{name}-key")).cast()?,
                        &key_arg,
                        type_containers,
                        tools,
                        ctx_provider,
                        typevar_bindings,
                    )?;
                    if is_mutable_container(&key_validator.type_validator, py) {
                        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                            "Cannot use mutable container {} as key type for Mapping. \
                             Mutable containers (list, dict, set) cannot be used in abstract collections \
                             because ators cannot insert wrapped versions inside them.",
                            key_arg.repr()?
                        )));
                    }
                    let (val_validator, val_info) = build_validator_from_annotation(
                        PyString::new(py, &format!("{name}-value")).cast()?,
                        &val_arg,
                        type_containers,
                        tools,
                        ctx_provider,
                        typevar_bindings,
                    )?;
                    if is_mutable_container(&val_validator.type_validator, py) {
                        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                            "Cannot use mutable container {} as value type for Mapping. \
                             Mutable containers (list, dict, set) cannot be used in abstract collections \
                             because ators cannot insert wrapped versions inside them.",
                            val_arg.repr()?
                        )));
                    }
                    (
                        Some((
                            BoxedValidator::from(key_validator),
                            BoxedValidator::from(val_validator),
                        )),
                        key_info.requires_owner || val_info.requires_owner,
                    )
                } else {
                    (None, false)
                };
            Ok((
                Validator::new(
                    TypeValidator::Mapping {
                        items: items_validator,
                    },
                    None,
                    None,
                    None,
                ),
                ValidatorBuildInfo { requires_owner },
            ))
        } else if origin.is(&tools.types.union_) {
            // FIXME: low priority
            // merge Typed/Instance together if relevant
            let mut members = Vec::new();
            let mut requires_owner = false;
            for member_ann in args.iter() {
                let (validator, info) = build_validator_from_annotation(
                    name,
                    &member_ann,
                    type_containers,
                    tools,
                    ctx_provider,
                    typevar_bindings,
                )?;
                requires_owner = requires_owner || info.requires_owner;
                members.push(validator);
            }
            Ok((
                Validator::new(TypeValidator::Union { members }, None, None, None),
                ValidatorBuildInfo { requires_owner },
            ))
        } else if origin.is(&tools.types.unpack) {
            Err(pyo3::exceptions::PyTypeError::new_err("Unsupported Unpack")) // FIXME
        } else {
            let attr_names_opt: Option<Vec<String>> = {
                let generic_attrs_bound = get_generic_attributes_map(py);
                with_critical_section(generic_attrs_bound.as_any(), || {
                    let generic_attrs = generic_attrs_bound.borrow();
                    origin
                        .cast::<PyType>()
                        .ok()
                        .and_then(|t| generic_attrs.get_attributes(t))
                        .cloned()
                })
            };
            if let Some(attr_names) = attr_names_opt {
                let origin_type = origin.cast_into::<PyType>()?;
                let mut attributes = Vec::new();
                let mut requires_owner = false;
                for (attr_name_str, attr_type) in attr_names.into_iter().zip(args.iter()) {
                    let (attr_validator, attr_info) = build_validator_from_annotation(
                        PyString::new(py, &format!("{name}-{attr_name_str}")).cast()?,
                        &attr_type,
                        type_containers,
                        tools,
                        ctx_provider,
                        typevar_bindings,
                    )?;
                    requires_owner = requires_owner || attr_info.requires_owner;
                    attributes.push((attr_name_str, attr_validator));
                }
                Ok((
                    Validator::new(
                        TypeValidator::GenericAttributes {
                            type_: origin_type.unbind(),
                            attributes,
                        },
                        None,
                        None,
                        None,
                    ),
                    ValidatorBuildInfo { requires_owner },
                ))
            } else {
                // Block G: check if the origin is a subclass of a handled ABC.
                let origin_type = origin.cast::<PyType>()?;
                if origin_type.is_subclass(&tools.types.abc_mapping)? {
                    let (items_validator, requires_owner) =
                        if let Ok((key_arg, val_arg)) = args.extract() {
                            let (key_validator, key_info) = build_validator_from_annotation(
                                PyString::new(py, &format!("{name}-key")).cast()?,
                                &key_arg,
                                type_containers,
                                tools,
                                ctx_provider,
                                typevar_bindings,
                            )?;
                            if is_mutable_container(&key_validator.type_validator, py) {
                                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                                    "Cannot use mutable container {} as key type for Mapping. \
                                     Mutable containers (list, dict, set) cannot be used in abstract collections \
                                     because ators cannot insert wrapped versions inside them.",
                                    key_arg.repr()?
                                )));
                            }
                            let (val_validator, val_info) = build_validator_from_annotation(
                                PyString::new(py, &format!("{name}-value")).cast()?,
                                &val_arg,
                                type_containers,
                                tools,
                                ctx_provider,
                                typevar_bindings,
                            )?;
                            if is_mutable_container(&val_validator.type_validator, py) {
                                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                                    "Cannot use mutable container {} as value type for Mapping. \
                                     Mutable containers (list, dict, set) cannot be used in abstract collections \
                                     because ators cannot insert wrapped versions inside them.",
                                    val_arg.repr()?
                                )));
                            }
                            (
                                Some((
                                    BoxedValidator::from(key_validator),
                                    BoxedValidator::from(val_validator),
                                )),
                                key_info.requires_owner || val_info.requires_owner,
                            )
                        } else {
                            (None, false)
                        };
                    Ok((
                        Validator::new(
                            TypeValidator::Mapping { items: items_validator },
                            None,
                            None,
                            None,
                        ),
                        ValidatorBuildInfo { requires_owner },
                    ))
                } else if origin_type.is_subclass(&tools.types.abc_sequence)? {
                    let (item_val, requires_owner) = if let Ok(item_arg) = args.get_item(0) {
                        let (item_validator, item_info) = build_validator_from_annotation(
                            PyString::new(py, &format!("{name}-item")).cast()?,
                            &item_arg,
                            type_containers,
                            tools,
                            ctx_provider,
                            typevar_bindings,
                        )?;
                        if is_mutable_container(&item_validator.type_validator, py) {
                            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                                "Cannot use mutable container {} as item type for Sequence. \
                                 Mutable containers (list, dict, set) cannot be used in abstract collections \
                                 because ators cannot insert wrapped versions inside them.",
                                item_arg.repr()?
                            )));
                        }
                        (Some(BoxedValidator::from(item_validator)), item_info.requires_owner)
                    } else {
                        (None, false)
                    };
                    Ok((
                        Validator::new(
                            TypeValidator::Sequence { item: item_val },
                            None,
                            None,
                            None,
                        ),
                        ValidatorBuildInfo { requires_owner },
                    ))
                } else if origin_type.is_subclass(&tools.types.abc_collection)? {
                    let (item_val, requires_owner) = if let Ok(item_arg) = args.get_item(0) {
                        let (item_validator, item_info) = build_validator_from_annotation(
                            PyString::new(py, &format!("{name}-item")).cast()?,
                            &item_arg,
                            type_containers,
                            tools,
                            ctx_provider,
                            typevar_bindings,
                        )?;
                        if is_mutable_container(&item_validator.type_validator, py) {
                            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                                "Cannot use mutable container {} as item type for Collection. \
                                 Mutable containers (list, dict, set) cannot be used in abstract collections \
                                 because ators cannot insert wrapped versions inside them.",
                                item_arg.repr()?
                            )));
                        }
                        (Some(BoxedValidator::from(item_validator)), item_info.requires_owner)
                    } else {
                        (None, false)
                    };
                    Ok((
                        Validator::new(
                            TypeValidator::Collection { item: item_val },
                            None,
                            None,
                            None,
                        ),
                        ValidatorBuildInfo { requires_owner },
                    ))
                } else {
                    // No ABC subclass recognized: warn and fall back to Typed.
                    let origin_name = origin.get_type().name()?;
                    PyErr::warn(
                        py,
                        &py.get_type::<pyo3::exceptions::PyUserWarning>(),
                        CString::new(format!(
                            "No specific validation strategy recorded for generic type                              {origin_name}. Falling back to Typed validator."
                        ))?
                        .as_c_str(),
                        0,
                    )?;
                    Ok((
                        Validator::new(
                            TypeValidator::Typed {
                                type_: origin_type.clone().unbind(),
                            },
                            None,
                            None,
                            None,
                        ),
                        ValidatorBuildInfo { requires_owner: false },
                    ))
                }
            }
        }
    } else if ann.is_instance(&tools.types.type_var)? {
        if let Some(bindings) = typevar_bindings
            && let Some(bound_ann) = bindings.get_item(&ann)?
        {
            return build_validator_from_annotation(
                name,
                &bound_ann.cast_into()?,
                type_containers,
                tools,
                ctx_provider,
                typevar_bindings,
            );
        }

        // Constrained TypeVars (e.g. `T = TypeVar('T', int, str)`) are treated
        // as a union of their constraints.
        let constraints = ann.getattr(intern!(py, "__constraints__"))?;
        if let Ok(constraints_tuple) = constraints.cast::<PyTuple>()
            && !constraints_tuple.is_empty()
        {
            let mut members = Vec::new();
            let mut requires_owner = false;
            for constraint in constraints_tuple.iter() {
                let (validator, info) = build_validator_from_annotation(
                    name,
                    &constraint,
                    type_containers,
                    tools,
                    ctx_provider,
                    typevar_bindings,
                )?;
                requires_owner = requires_owner || info.requires_owner;
                members.push(validator);
            }
            return Ok((
                Validator::new(TypeValidator::Union { members }, None, None, None),
                ValidatorBuildInfo { requires_owner },
            ));
        }

        let bound = ann.getattr(intern!(py, "__bound__"))?;
        if !bound.is_none() {
            return build_validator_from_annotation(
                name,
                &bound.cast_into()?,
                type_containers,
                tools,
                ctx_provider,
                typevar_bindings,
            );
        }

        Ok((
            Validator::default(),
            ValidatorBuildInfo {
                requires_owner: false,
            },
        ))
    } else if ann.is_instance(&tools.types.new_type)? {
        build_validator_from_annotation(
            name,
            &ann.getattr(intern!(py, "__supertype__"))?,
            type_containers,
            tools,
            ctx_provider,
            typevar_bindings,
        )
    } else if ann.is(&tools.types.any) || ann.is(&tools.types.object) {
        Ok((
            Validator::default(),
            ValidatorBuildInfo {
                requires_owner: false,
            },
        ))
    } else if ann.is(py.get_type::<PyBool>()) {
        Ok((
            Validator::new(TypeValidator::Bool {}, None, None, None),
            ValidatorBuildInfo {
                requires_owner: false,
            },
        ))
    } else if ann.is(py.get_type::<PyInt>()) {
        Ok((
            Validator::new(TypeValidator::Int {}, None, None, None),
            ValidatorBuildInfo {
                requires_owner: false,
            },
        ))
    } else if ann.is(py.get_type::<PyFloat>()) {
        Ok((
            Validator::new(TypeValidator::Float {}, None, None, None),
            ValidatorBuildInfo {
                requires_owner: false,
            },
        ))
    } else if ann.is(py.get_type::<PyComplex>()) {
        Ok((
            Validator::new(TypeValidator::Complex {}, None, None, None),
            ValidatorBuildInfo {
                requires_owner: false,
            },
        ))
    } else if ann.is(py.get_type::<PyBytes>()) {
        Ok((
            Validator::new(TypeValidator::Bytes {}, None, None, None),
            ValidatorBuildInfo {
                requires_owner: false,
            },
        ))
    } else if ann.is(py.get_type::<PyString>()) {
        Ok((
            Validator::new(TypeValidator::Str {}, None, None, None),
            ValidatorBuildInfo {
                requires_owner: false,
            },
        ))
    } else if ann.is(py.get_type::<PyTuple>()) {
        Ok((
            Validator::new(TypeValidator::VarTuple { item: None }, None, None, None),
            ValidatorBuildInfo {
                requires_owner: false,
            },
        ))
    } else if ann.is(&tools.types.abc_sequence) {
        Ok((
            Validator::new(TypeValidator::Sequence { item: None }, None, None, None),
            ValidatorBuildInfo { requires_owner: false },
        ))
    } else if ann.is(&tools.types.abc_collection) {
        Ok((
            Validator::new(TypeValidator::Collection { item: None }, None, None, None),
            ValidatorBuildInfo { requires_owner: false },
        ))
    } else if ann.is(&tools.types.abc_mapping) {
        Ok((
            Validator::new(TypeValidator::Mapping { items: None }, None, None, None),
            ValidatorBuildInfo { requires_owner: false },
        ))
    } else {
        let ty = ann.clone().cast_into::<PyType>()?;
        // Block G: detect subclasses of handled ABCs before falling back to Typed.
        let is_concrete_container = ty.is(py.get_type::<PyList>())
            || ty.is(py.get_type::<PySet>())
            || ty.is(py.get_type::<PyDict>())
            || ty.is(py.get_type::<PyFrozenSet>());
        if !is_concrete_container {
            if ty.is_subclass(&tools.types.abc_mapping)? {
                return Ok((
                    Validator::new(TypeValidator::Mapping { items: None }, None, None, None),
                    ValidatorBuildInfo { requires_owner: false },
                ));
            } else if ty.is_subclass(&tools.types.abc_sequence)? {
                return Ok((
                    Validator::new(TypeValidator::Sequence { item: None }, None, None, None),
                    ValidatorBuildInfo { requires_owner: false },
                ));
            } else if ty.is_subclass(&tools.types.abc_collection)? {
                return Ok((
                    Validator::new(TypeValidator::Collection { item: None }, None, None, None),
                    ValidatorBuildInfo { requires_owner: false },
                ));
            }
        }
        Ok((
            Validator::new(
                TypeValidator::Typed { type_: ty.unbind() },
                None,
                None,
                None,
            ),
            ValidatorBuildInfo { requires_owner: false },
        ))
    }
}

fn configure_member_builder_from_annotation<'py>(
    builder: &mut MemberBuilder,
    name: &Bound<'py, PyString>,
    ann: &Bound<'py, PyAny>,
    type_containers: i64,
    tools: &TypeTools<'py>,
    final_annotated: bool,
    typevar_bindings: Option<&Bound<'py, PyDict>>,
) -> PyResult<()> {
    let origin = tools.get_origin.call1((ann,))?;

    // If the type is annotated Final, ensure the behaviors match
    // If the builder already set the member as constant we ignore it.
    // Finally ensure a member that has a ReadOnly or Constant pre set behavior
    // is marked final.
    if origin.is(&tools.types.final_) {
        let args = &tools.get_args.call1((ann,))?.cast_into::<PyTuple>()?;
        if args.len() != 1 {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Final should only contain 1 item got {args}"
            )));
        }
        configure_member_builder_from_annotation(
            builder,
            name,
            // SAFETY we just checked the tuple did contains one element
            &args
                .get_item(0)
                .expect("The tuple is known to have one element as per previous check"),
            type_containers,
            tools,
            true,
            typevar_bindings,
        )?;
        match builder.pre_setattr() {
            Some(PreSetattrBehavior::Constant {}) => {}
            _ => builder.set_pre_setattr(PreSetattrBehavior::ReadOnly {}),
        };
        builder.set_delattr(DelattrBehavior::Undeletable {});
        return Ok(());
    }

    // Ensure we do not have a pre set behavior that mandates the use of Final
    if !final_annotated {
        match builder.pre_setattr() {
            Some(PreSetattrBehavior::Constant {}) | Some(PreSetattrBehavior::ReadOnly {}) => {
                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "Member {} prevents mutation but type is not annotated as final {}.",
                    name,
                    ann.repr()?
                )));
            }
            _ => (),
        };
    }

    // Next analyze the annotation to build the validators (Final is not
    // permitted within container or generic).
    let (new, build_info) = match build_validator_from_annotation(
        name,
        ann,
        type_containers,
        tools,
        builder
            .forward_ref_environment_factory()
            .map(|f| f.bind(name.py())),
        typevar_bindings,
    ) {
        Ok(v) => Ok(v),
        Err(err) => Err(err_with_cause(
            name.py(),
            pyo3::exceptions::PyRuntimeError::new_err(format!(
                "Failed to build validator for member {} from annotation {}",
                name,
                ann.repr()?,
            )),
            err,
        )),
    }?;

    // Set the type validator
    builder.set_type_validator(new.type_validator.clone());

    // Store the validator build info in the builder for later use when building
    // the member
    builder.require_owner = build_info.requires_owner();

    // Append the user specified value validators to the ones inferred from type
    // annotation.
    let temp: Option<Vec<ValueValidator>> = builder.take_value_validators();
    if !new.value_validators.is_empty() || temp.is_some() {
        builder.set_value_validators(
            new.value_validators
                .into_iter()
                .chain(temp.unwrap_or_default())
                .collect(),
        );
    }

    Ok(())
}

pub fn generate_member_builders_from_cls_namespace<'py>(
    name: &Bound<'py, PyString>,
    dct: &Bound<'py, PyDict>,
    type_containers: i64,
    typevar_bindings: Option<&Bound<'py, PyDict>>,
    validate_attr: bool,
) -> PyResult<(
    HashMap<String, MemberBuilder>,
    HashMap<String, EventBuilder>,
)> {
    let py = name.py();

    let annotationlib = py.import(intern!(py, "annotationlib"))?;
    // `__annotations__` is guaranteed by Python to be a mapping; cast it
    // directly rather than checking `isinstance(…, dict)` and copying.
    let annotations: Bound<'py, PyMapping> = if dct.contains(intern!(py, "__annotations__"))? {
        dct.as_any()
            .get_item(intern!(py, "__annotations__"))?
            .cast_into()?
    } else {
        let annotate = annotationlib
            .getattr(intern!(py, "get_annotate_from_class_namespace"))?
            .call1((dct,))?;
        if annotate.is_none() {
            PyDict::new(py).into_any().cast_into()?
        } else {
            annotationlib
                .getattr(intern!(py, "call_annotate_function"))?
                .call1((
                    annotate,
                    annotationlib
                        .getattr(intern!(py, "Format"))?
                        .getattr(intern!(py, "FORWARDREF"))?,
                ))?
                .cast_into()?
        }
    };

    let typing_mod = py.import(intern!(py, "typing"))?;
    let class_var = typing_mod.getattr(intern!(py, "ClassVar"))?;

    let tools = get_type_tools(py)?;

    // Type objects used for annotation dispatch.
    let member_type = py.get_type::<Member>();
    let event_type = py.get_type::<crate::event::Event>();

    let mut member_builders = HashMap::new();
    let mut event_builders = HashMap::new();

    for item in annotations.items()?.iter() {
        let (attr_key, ann) = item.extract::<(Bound<'py, PyAny>, Bound<'py, PyAny>)>()?;
        // Get the origin of the type annotation — computed once, used by both branches.
        let origin = tools.get_origin.call1((&ann,))?;

        // Check we are not dealing with a ClassVar (parameterized ClassVar[T] or
        // bare ClassVar).  Both cases must be skipped even when validate_attr=False.
        if origin.is(&class_var) || ann.is(&class_var) {
            continue;
        }

        let attr_name: String = attr_key.extract()?;

        // ── Event branch ────────────────────────────────────────────────────
        if ann.is(event_type.as_any()) {
            // Bare `Event` annotation — never valid; must be subscripted.
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Attribute '{attr_name}': Event must be subscripted as Event[T]."
            )));
        }

        if origin.is(event_type.as_any()) {
            // Event[T…] annotation: validate arity (exactly one type argument).
            let args = tools.get_args.call1((&ann,))?.cast_into::<PyTuple>()?;
            let arg_count = args.len();
            if arg_count != 1 {
                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "Attribute '{attr_name}': Event[T] expects exactly 1 type argument, \
                     got {arg_count}."
                )));
            }

            // Retrieve the user-provided EventBuilder or create a default one.
            let mut builder = if dct.contains(&attr_key)? {
                let value = dct.as_any().get_item(&attr_key)?;
                // Remove from dct so it is not processed again as a bare builder.
                dct.del_item(&attr_key)?;
                if let Ok(eb) = value.cast::<EventBuilder>() {
                    eb.clone().extract()?
                } else {
                    return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                        "Attribute '{attr_name}': Event annotation requires an event() builder \
                         on the RHS, got {} instead.",
                        value.get_type().name()?
                    )));
                }
            } else {
                EventBuilder::default()
            };

            // Configure validator from the single type argument.
            if validate_attr {
                let type_arg = args.get_item(0).expect("Arity already checked to be 1");
                let (validator, _build_info) = build_validator_from_annotation(
                    attr_key.cast()?,
                    &type_arg,
                    type_containers,
                    &tools,
                    None,
                    typevar_bindings,
                )
                .map_err(|err| {
                    err_with_cause(
                        py,
                        pyo3::exceptions::PyTypeError::new_err(format!(
                            "Failed to build validator for event {attr_key} \
                             from annotation {ann:?}"
                        )),
                        err,
                    )
                })?;

                builder.set_type_validator(validator.type_validator.clone());

                // Append type-inferred value validators before any user-specified ones.
                let user_vvs = builder.take_value_validators();
                if !validator.value_validators.is_empty() || user_vvs.is_some() {
                    builder.set_value_validators(
                        validator
                            .value_validators
                            .into_iter()
                            .chain(user_vvs.unwrap_or_default())
                            .collect(),
                    );
                }
            }

            builder.name = Some(attr_name.clone());
            event_builders.insert(attr_name, builder);
            continue; // annotation processed; skip member branch
        }

        // ── Member branch ────────────────────────────────────────────────────

        // Retrieve the user provided builder, or build one with or without
        // a default value
        let mut builder = if dct.contains(&attr_key)? {
            let value = dct.as_any().get_item(&attr_key)?;
            // Remove the builder from the dict so that we can extract builder
            // without annotations at a later stage.
            dct.del_item(&attr_key)?;
            // Reject accidental use of an event() builder on a non-Event annotation.
            if value.is_instance_of::<EventBuilder>() {
                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "Attribute '{attr_name}': event() builder used as RHS for a \
                     non-Event annotation. Use Event[T] as the annotation instead."
                )));
            }
            if let Ok(mb) = value.cast::<MemberBuilder>() {
                mb.clone().extract()?
            } else {
                let mut mb = MemberBuilder::default();
                mb.set_default(DefaultBehavior::Static {
                    value: value.into(),
                });
                mb
            }
        } else {
            MemberBuilder::default()
        };

        // Enforce the annotation–coerce pairing contract.
        //
        // Bare `Member` is never a valid annotation.  `Member[T1, T2]` is only
        // valid when the RHS member explicitly calls `.coerce(...)`.
        // Conversely, `.coerce(...)` on an annotated member requires the
        // annotation to be `Member[T1, T2]` with exactly two type arguments.
        //
        // The `effective_ann` resolved here replaces the outer `Member[T1, T2]`
        // with its first type argument (T1) for subsequent validator inference.
        let has_coerce = builder.coercer().is_some();

        // Track whether `effective_ann` differs from the original `ann`
        // so the validate_attr=False path can reuse the already-computed
        // `origin` instead of making a redundant `get_origin` call.
        let effective_ann: Bound<'py, PyAny>;
        let ann_replaced: bool;

        if ann.is(member_type.as_any()) {
            // Bare `Member` annotation — never valid.
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Attribute '{attr_name}': Member must be subscripted as Member[T1, T2]."
            )));
        } else if origin.is(member_type.as_any()) {
            // `Member[T1, T2]` annotation: validate arity and require coerce.
            // This is done here because raising in __class_getitem__ simply turns
            // the annotations into a forward reference which leads to
            // generating poor and late error messages.
            let args = tools.get_args.call1((&ann,))?.cast_into::<PyTuple>()?;
            let arg_count = args.len();
            if arg_count != 2 {
                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "Attribute '{attr_name}': Member[T1, T2] expects exactly 2 type \
                     arguments, got {arg_count}."
                )));
            }
            if !has_coerce {
                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "Attribute '{attr_name}': Member annotation requires an explicitly \
                     coerced RHS member (call .coerce(...))."
                )));
            }
            // Use T1 (the first type arg) for validator inference.
            effective_ann = args.get_item(0)?;
            ann_replaced = true;
        } else {
            if has_coerce {
                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "Attribute '{attr_name}': coerced RHS member requires a \
                     Member[T1, T2] annotation."
                )));
            }
            effective_ann = ann.clone();
            ann_replaced = false;
        }

        if validate_attr {
            // Analyze the annotation to configure the builder
            configure_member_builder_from_annotation(
                &mut builder,
                attr_key.cast()?,
                &effective_ann,
                type_containers,
                &tools,
                false,
                typevar_bindings,
            )
            .map_err(|err| {
                err_with_cause(
                    py,
                    pyo3::exceptions::PyTypeError::new_err(format!(
                        "Failed to configure Member {attr_key} from annotation {ann:?}"
                    )),
                    err,
                )
            })?;
        } else {
            // When validate_attr=False, skip building type/value validators and
            // coercers.  Only Final semantics (read-only + undeletable) are
            // still enforced; everything else is treated as untyped.
            // Reuse the already-computed `origin` when the annotation was not
            // replaced (the common non-Member case) to avoid a redundant
            // `get_origin` Python call.
            let effective_origin = if ann_replaced {
                tools.get_origin.call1((&effective_ann,))?
            } else {
                origin.clone()
            };
            let is_final =
                effective_origin.is(&tools.types.final_) || effective_ann.is(&tools.types.final_);
            if is_final {
                match builder.pre_setattr() {
                    Some(PreSetattrBehavior::Constant {}) => {}
                    _ => builder.set_pre_setattr(PreSetattrBehavior::ReadOnly {}),
                };
                builder.set_delattr(DelattrBehavior::Undeletable {});
            }
        }

        // Set the member name
        builder.name = Some(attr_name.clone());

        member_builders.insert(attr_name, builder);
    }

    Ok((member_builders, event_builders))
}
