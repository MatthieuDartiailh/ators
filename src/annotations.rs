use std::{collections::HashMap, mem};

/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Matthieu C. Dartiailh
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
///
use pyo3::{
    Bound, PyAny, PyRefMut, PyResult, intern,
    types::{
        PyAnyMethods, PyBool, PyBytes, PyDict, PyDictMethods, PyFloat, PyInt, PyString, PyType,
    },
};

use crate::validators::{CoercionMode, Validator, ValueValidator};
use crate::{
    member::{DelattrBehavior, MemberBuilder, PreSetattrBehavior},
    validators::TypeValidator,
};

///
struct PyTypes<'py> {
    any: Bound<'py, PyType>,
    final_: Bound<'py, PyType>,
    generic_alias: Bound<'py, PyType>,
    union_: Bound<'py, PyType>,
    type_var: Bound<'py, PyType>,
    new_type: Bound<'py, PyType>,
    forward_ref: Bound<'py, PyType>,
}

///
struct TypeTools<'py> {
    get_origin: Bound<'py, PyAny>,
    get_args: Bound<'py, PyAny>,
    types: PyTypes<'py>,
}

// XXX bad idea for ators since I need to look into generic when building validators
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

fn build_validator_from_annotation<'py>(
    name: &Bound<'py, PyString>,
    ann: &Bound<'py, PyAny>,
    type_containers: i64,
    tools: &TypeTools<'py>,
) -> PyResult<Validator> {
    if ann.is_instance_of::<PyString>() || ann.is_instance(&tools.types.forward_ref)? {
        return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
            "Str and ForwardRef annotations ({}) are not supported in ators classes",
            ann.repr()?
        )));
    }

    let py = name.py();

    // in 3.14, Union[int, float] and int | float share the same type
    if ann.is_instance(&tools.types.generic_alias)? {
        let origin = tools.get_origin.call1((ann,))?;
        let args = tools.get_args.call1((ann,))?;
        // FIXME treat Literal
        Err(pyo3::exceptions::PyTypeError::new_err(
            "Unsupported Generic",
        )) // FIXME
    } else if ann.is_instance(&tools.types.union_)? {
        Err(pyo3::exceptions::PyTypeError::new_err("Unsupported Union")) // FIXME
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
        )
    } else if ann.is(&tools.types.any) {
        Ok(Validator::default())
    } else if ann.is(py.get_type::<PyBool>()) {
        Ok(Validator::new(
            TypeValidator::Bool {},
            None,
            CoercionMode::No(),
        ))
    } else if ann.is(py.get_type::<PyInt>()) {
        Ok(Validator::new(
            TypeValidator::Int {},
            None,
            CoercionMode::No(),
        ))
    } else if ann.is(py.get_type::<PyFloat>()) {
        Ok(Validator::new(
            TypeValidator::Float {},
            None,
            CoercionMode::No(),
        ))
    } else if ann.is(py.get_type::<PyBytes>()) {
        Ok(Validator::new(
            TypeValidator::Bytes {},
            None,
            CoercionMode::No(),
        ))
    } else if ann.is(py.get_type::<PyString>()) {
        Ok(Validator::new(
            TypeValidator::Str {},
            None,
            CoercionMode::No(),
        ))
    } else {
        //f"Failed to extract types from {kind}. "
        // f"The extraction yielded {t} which is not a type. "
        // "One case in which this can occur is when using unions of "
        // "Literal, and the issues can be worked around by using a "
        // "single literal containing all the values."
        Ok(Validator::new(
            TypeValidator::Typed {
                type_: ann.clone().cast_into()?.unbind(),
            },
            None,
            CoercionMode::No(),
        ))
    }
}

fn configure_member_builder_from_annotation<'py>(
    builder: &mut PyRefMut<'py, MemberBuilder>,
    name: &Bound<'py, PyString>,
    ann: &Bound<'py, PyAny>,
    type_containers: i64,
    tools: &TypeTools<'py>,
) -> PyResult<()> {
    let origin = tools.get_origin.call1((ann,))?;

    // If the type is annotated Final, ensure the behaviors match
    // If the builder already set the member as constant we ignore it.
    // Finally ensure a member that has a ReadOnly or Constant pre set behavior
    // is marked final.
    if origin.is(&tools.types.final_) {
        configure_member_builder_from_annotation(builder, name, ann, type_containers, &tools);
        match builder.pre_setattr {
            PreSetattrBehavior::Constant {} => {}
            _ => builder.pre_setattr = PreSetattrBehavior::ReadOnly {},
        };
        builder.delattr = DelattrBehavior::Undeletable {};
    } else {
        match builder.pre_setattr {
            PreSetattrBehavior::Constant {} | PreSetattrBehavior::ReadOnly {} => {
                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "Member {} prevents mutation but type is not annotated as final {}.",
                    name,
                    ann.repr()?
                )));
            }
            _ => builder.pre_setattr = PreSetattrBehavior::ReadOnly {},
        };
    }

    // Next analyze the annotation to build the validators (Final is not
    // permitted within container or generic).
    // FIXME should not override the existing validators
    let new = match build_validator_from_annotation(name, ann, type_containers, &tools) {
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
    builder.validator.type_validator = new.type_validator;

    // Append the user specified value validators to the ones inferred from type
    // annotation.
    let temp = mem::replace(
        &mut builder.validator.value_validators,
        Vec::new().into_boxed_slice(),
    );
    builder.validator.value_validators = new
        .value_validators
        .into_iter()
        .chain(temp.into_iter())
        .collect::<Box<[ValueValidator]>>();

    Ok(())
}

pub fn generate_member_builders_from_cls_namespace<'py>(
    name: &Bound<'py, PyString>,
    dct: &Bound<'py, PyDict>,
    type_containers: i64,
) -> PyResult<HashMap<String, Bound<'py, MemberBuilder>>> {
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

    // FIXME This should be created only once
    // Store the object in the _ators module namespace
    // Require a different object not linked to the py lifetime...
    // #[pymodule_init]
    // fn init(m: &Bound<'_, PyModule>) -> PyResult<()> {
    //     // Arbitrary code to run at the module initialization
    //     m.add("double2", m.getattr("double")?)
    // }
    let tools = TypeTools {
        get_args: typing_mod.getattr(intern!(py, "get_args"))?,
        get_origin: typing_mod.getattr(intern!(py, "get_origin"))?,
        types: PyTypes {
            any: typing_mod
                .getattr(intern!(py, "Any"))?
                .cast_into::<PyType>()?,
            final_: typing_mod
                .getattr(intern!(py, "Final"))?
                .cast_into::<PyType>()?,
            generic_alias: typing_mod
                .getattr(intern!(py, "GenericAlias"))?
                .cast_into::<PyType>()?,
            union_: typing_mod
                .getattr(intern!(py, "UnionType"))?
                .cast_into::<PyType>()?,
            type_var: typing_mod
                .getattr(intern!(py, "TypeVar"))?
                .cast_into::<PyType>()?,
            new_type: typing_mod
                .getattr(intern!(py, "NewType"))?
                .cast_into::<PyType>()?,
            forward_ref: annotationlib
                .getattr(intern!(py, "ForwardRef"))?
                .cast_into::<PyType>()?,
        },
    };

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
        let builder = if dct.contains(&name)? {
            let value = dct.as_any().get_item(&name)?;
            if let Ok(mb) = value.cast::<MemberBuilder>() {
                mb.clone()
            } else {
                let mut mb = MemberBuilder::default();
                mb.default = crate::member::DefaultBehavior::Static {
                    value: value.into(),
                };
                Bound::new(py, mb)?
            }
        } else {
            Bound::new(py, MemberBuilder::default())?
        };

        // Analyze the annotation to configure the builder
        {
            let mut bmr = builder.borrow_mut();
            configure_member_builder_from_annotation(
                &mut bmr,
                name.cast()?,
                &ann,
                type_containers,
                &tools,
            )?;

            bmr.name = Some(name.extract()?);
        }

        builders.insert(name.extract()?, builder);
    }

    Ok(builders)
}
