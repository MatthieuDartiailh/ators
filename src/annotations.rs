/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{
    Bound, Py, PyAny, PyErr, PyResult, PyTypeInfo, Python, intern,
    types::{
        PyAnyMethods, PyBool, PyBytes, PyDict, PyDictMethods, PyFloat, PyFrozenSet, PyInt,
        PyString, PyTuple, PyTupleMethods, PyType, PyTypeMethods,
    },
};
use std::collections::HashMap;
use std::ffi::CString;

use crate::{
    get_generic_attributes_map,
    member::{DefaultBehavior, DelattrBehavior, MemberBuilder, PreSetattrBehavior},
    validators::{
        TypeValidator, ValidValues, Validator, ValueValidator, types::LateResolvedValidator,
    },
};

///
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
    // sequence: Bound<'py, PyAny>,
    // mapping: Bound<'py, PyAny>,
    // XXX defaultdict
}

///
pub(crate) struct TypeTools<'py> {
    get_origin: Bound<'py, PyAny>,
    get_args: Bound<'py, PyAny>,
    types: PyTypes<'py>,
}

pub(crate) fn get_type_tools<'py>(py: Python<'py>) -> Result<TypeTools<'py>, PyErr> {
    let annotationlib = py.import(intern!(py, "annotationlib"))?;

    let builtins_mod = py.import(intern!(py, "builtins"))?;
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
            // sequence: builtins_mod.getattr(intern!(py, "tuple"))?,  // XXX wrong module
            // mapping: builtins_mod.getattr(intern!(py, "tuple"))?,
        },
    })
}

// NOTE bad idea for ators since I need to look into generic when building validators
// NOTE in ators I will never get a tuple of types only Unions !!! so much simpler
// This should map to _extract_types
// pub fn extract_types<'py>(
//     kind: Bound<'py, PyAny>,
//     tools: &TypeTools,
// ) -> PyResult<Vec<Bound<'py, PyType>>> {
//     let mut types = Vec::new();

//     if kind.is_instance_of::<PyString>() || kind.is_instance(&tools.types.forward_ref)? {
//         return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
//             "Str and ForwardRef annotations ({}) are not supported in ators classes",
//             kind.repr()?
//         )));
//     }

//     let to_inspect = Vec::new();
//     if kind.is_instance(&tools.types.generic_alias) {
//         let origin = tools.get_origin.call1((kind,))?;
//         if origin.is_instance(ty)

//     }

//     Ok(types)
// }

// NOTE I should not need is_optional since I won't rely on it for instance
// validation

///
pub fn build_validator_from_annotation<'py>(
    name: &Bound<'py, PyString>,
    ann: &Bound<'py, PyAny>,
    type_containers: i64, // not sure this is worth keeping it
    tools: &TypeTools<'py>,
    ctx_provider: Option<&Bound<'py, PyAny>>,
) -> PyResult<Validator> {
    if ann.is_instance_of::<PyString>() {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
            "Str annotations ({}) are not supported in ators classes, use ForwardRef instead",
            ann.repr()?
        )));
    } else if ann.is_instance(&tools.types.forward_ref)? {
        return Ok(Validator::new(
            TypeValidator::ForwardValidator {
                late_validator: LateResolvedValidator::new(
                    ann,
                    ctx_provider,
                    type_containers,
                    name,
                ),
            },
            None,
            None,
            None,
        ));
    }

    let py = name.py();

    // Applicable on any type and return None for non generic types
    // Using this rather than is_instance is easier to get right since
    // some generics such as Literal use specific private classes.
    let origin = tools.get_origin.call1((ann,))?;

    // In 3.14, Union[int, float] and int | float share the same type
    if !origin.is_none() {
        // FIXME extract in a dedicated function since it will be expanded
        // to cover list, dict, Numpy.NDArray etc
        let args = tools.get_args.call1((ann,))?.cast_into::<PyTuple>()?;
        if origin.is(&tools.types.literal) {
            Ok(Validator::new(
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
            ))
        } else if origin.is(py.get_type::<PyTuple>()) {
            if args.len() == 2 && args.get_item(1).expect("Known 2-tuple").is(py.Ellipsis()) {
                // VarTuple
                let item_validator = build_validator_from_annotation(
                    PyString::new(py, &format!("{name}-item")).cast()?,
                    &args.get_item(0).expect("Known 2-tuple"),
                    type_containers,
                    tools,
                    ctx_provider,
                )?;
                Ok(Validator::new(
                    TypeValidator::VarTuple {
                        item: Some(Py::new(py, item_validator)?),
                    },
                    None,
                    None,
                    None,
                ))
            } else {
                // Fixed length tuple
                let mut items = Vec::new();
                for item in args.iter() {
                    let item_validator = build_validator_from_annotation(
                        PyString::new(py, &format!("{name}-item")).cast()?,
                        &item,
                        type_containers,
                        tools,
                        ctx_provider,
                    )?;
                    items.push(item_validator);
                }
                Ok(Validator::new(
                    TypeValidator::Tuple { items },
                    None,
                    None,
                    None,
                ))
            }
        } else if origin.is(&tools.types.union_) {
            // FIXME: low priority
            // merge Typed/Instance together if relevant
            Ok(Validator::new(
                TypeValidator::Union {
                    members: args
                        .iter()
                        .map(|member| {
                            build_validator_from_annotation(
                                name,
                                &member,
                                type_containers,
                                tools,
                                ctx_provider,
                            )
                        })
                        .collect::<PyResult<Vec<Validator>>>()?,
                },
                None,
                None,
                None,
            ))
        } else if origin.is(&tools.types.type_alias) {
            Err(pyo3::exceptions::PyTypeError::new_err(
                "Unsupported TypeAlias",
            )) // FIXME
        } else if origin.is(&tools.types.unpack) {
            Err(pyo3::exceptions::PyTypeError::new_err("Unsupported Unpack")) // FIXME
        } else {
            let generic_attrs = get_generic_attributes_map(py);
            if let Some(attr_list) = generic_attrs.get_item(&origin)? {
                let mut attributes = Vec::new();
                for (attr_name, attr_type) in
                    attr_list.cast_into::<PyTuple>()?.iter().zip(args.iter())
                {
                    let attr_name_str = attr_name.extract::<String>()?;
                    let attr_validator = build_validator_from_annotation(
                        PyString::new(py, &format!("{name}-{attr_name_str}")).cast()?,
                        &attr_type,
                        type_containers,
                        tools,
                        ctx_provider,
                    )?;
                    attributes.push((attr_name_str, attr_validator));
                }
                Ok(Validator::new(
                    TypeValidator::GenericAttributes {
                        type_: origin.cast_into::<PyType>()?.unbind(),
                        attributes,
                    },
                    None,
                    None,
                    None,
                ))
            } else {
                let origin_name = origin.get_type().name()?;
                PyErr::warn(
                    py,
                    &py.get_type::<pyo3::exceptions::PyUserWarning>(),
                    CString::new(format!(
                        "No specific validation strategy recorded for generic type {origin_name}.\
                         Falling back to Typed validator."
                    ))?
                    .as_c_str(),
                    0,
                )?;
                Ok(Validator::new(
                    TypeValidator::Typed {
                        type_: origin.cast_into::<PyType>()?.unbind(),
                    },
                    None,
                    None,
                    None,
                ))
            }
        }
    } else if ann.is_instance(&tools.types.type_var)? {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "Unsupported TypeVar",
        )) // FIXME
    } else if ann.is_instance(&tools.types.new_type)? {
        build_validator_from_annotation(
            name,
            &ann.getattr(intern!(py, "__supertype__"))?,
            type_containers,
            tools,
            ctx_provider,
        )
    } else if ann.is(&tools.types.any) || ann.is(&tools.types.object) {
        Ok(Validator::default())
    } else if ann.is(py.get_type::<PyBool>()) {
        Ok(Validator::new(TypeValidator::Bool {}, None, None, None))
    } else if ann.is(py.get_type::<PyInt>()) {
        Ok(Validator::new(TypeValidator::Int {}, None, None, None))
    } else if ann.is(py.get_type::<PyFloat>()) {
        Ok(Validator::new(TypeValidator::Float {}, None, None, None))
    } else if ann.is(py.get_type::<PyBytes>()) {
        Ok(Validator::new(TypeValidator::Bytes {}, None, None, None))
    } else if ann.is(py.get_type::<PyString>()) {
        Ok(Validator::new(TypeValidator::Str {}, None, None, None))
    } else if ann.is(py.get_type::<PyTuple>()) {
        Ok(Validator::new(
            TypeValidator::VarTuple { item: None },
            None,
            None,
            None,
        ))
    } else {
        let ty = ann.clone().cast_into::<PyType>()?;
        Ok(Validator::new(
            TypeValidator::Typed { type_: ty.unbind() },
            None,
            None,
            None,
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
            &args.get_item(0).unwrap(),
            type_containers,
            tools,
            true,
        )?;
        match builder.pre_setattr {
            Some(PreSetattrBehavior::Constant {}) => {}
            _ => builder.pre_setattr = Some(PreSetattrBehavior::ReadOnly {}),
        };
        builder.delattr = Some(DelattrBehavior::Undeletable {});
        return Ok(());
    }

    // Ensure we do not have a pre set behavior that mandates the use of Final
    if !final_annotated {
        match builder.pre_setattr {
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
    let new = match build_validator_from_annotation(
        name,
        ann,
        type_containers,
        tools,
        builder
            .forward_ref_environment_factory
            .as_ref()
            .map(|f| f.bind(name.py())),
    ) {
        Ok(v) => Ok(v),
        Err(err) => {
            let new_err = pyo3::exceptions::PyRuntimeError::new_err(
                pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "Failed to build validator for member {} from annotation {}",
                    name,
                    ann.repr()?,
                )),
            );
            new_err.set_cause(name.py(), Some(err));
            Err(new_err)
        }
    }?;

    // Set the type validator
    builder.type_validator = Some(new.type_validator);

    // Append the user specified value validators to the ones inferred from type
    // annotation.
    let temp: Option<Vec<ValueValidator>> = builder.value_validators.take();
    if !new.value_validators.is_empty() || temp.is_some() {
        builder.value_validators = Some(
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
) -> PyResult<HashMap<String, MemberBuilder>> {
    let py = name.py();

    let annotationlib = py.import(intern!(py, "annotationlib"))?;
    let annotations = if dct.contains(intern!(py, "__annotations__"))? {
        dct.as_any()
            .get_item(intern!(py, "__annotations__"))?
            .cast_into()
    } else {
        let annotate = annotationlib
            .getattr(intern!(py, "get_annotate_from_class_namespace"))?
            .call1((dct,))?;
        if annotate.is_none() {
            Ok(PyDict::new(py))
        } else {
            annotationlib
                .getattr(intern!(py, "call_annotate_function"))?
                .call1((
                    annotate,
                    annotationlib
                        .getattr(intern!(py, "Format"))?
                        .getattr(intern!(py, "FORWARDREF"))?,
                ))?
                .cast_into()
        }
    }?;

    let typing_mod = py.import(intern!(py, "typing"))?;
    let class_var = typing_mod.getattr(intern!(py, "ClassVar"))?;

    let tools = get_type_tools(py)?;

    let mut builders = HashMap::new();
    for (name, ann) in annotations.iter() {
        // Get the origin of the type annotation
        let origin = tools.get_origin.call1((&ann,))?;

        // Check we are not dealing with a ClassVar
        if origin.is(&class_var) {
            continue;
        }

        // Retrieve the user provided builder, or build one with or without
        // a default value
        let mut builder = if dct.contains(&name)? {
            let value = dct.as_any().get_item(&name)?;
            // Remove the builder from the dict so that we can extract builder
            // without annotations at a later stage.
            dct.del_item(&name)?;
            if let Ok(mb) = value.cast::<MemberBuilder>() {
                mb.clone().extract()?
            } else {
                let mut mb = MemberBuilder::default();
                mb.default = Some(DefaultBehavior::Static {
                    value: value.into(),
                });
                mb
            }
        } else {
            MemberBuilder::default()
        };

        // Analyze the annotation to configure the builder
        configure_member_builder_from_annotation(
            &mut builder,
            name.cast()?,
            &ann,
            type_containers,
            &tools,
            false,
        )
        .map_err(|err| {
            let new_err = pyo3::exceptions::PyTypeError::new_err(format!(
                "Failed to configure Member {name} from annotation {ann:?}"
            ));
            new_err.set_cause(py, Some(err));
            new_err
        })?;

        // Set the member name
        builder.name = Some(name.extract()?);

        builders.insert(name.extract()?, builder);
    }

    Ok(builders)
}
