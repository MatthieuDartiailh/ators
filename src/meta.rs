/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/

use std::collections::{HashMap, HashSet};

use pyo3::{
    Bound, Py, PyAny, PyErr, PyResult, intern, pyfunction,
    sync::critical_section::with_critical_section,
    types::{
        IntoPyDict, PyAnyMethods, PyDict, PyDictMethods, PyFrozenSet, PyFrozenSetMethods,
        PyFunction, PyListMethods, PyMapping, PyMappingMethods, PySet, PySetMethods, PyString,
        PyTuple, PyTupleMethods, PyType, PyTypeMethods,
    },
};

use crate::member::{
    DefaultBehavior, Member, PostGetattrBehavior, PostSetattrBehavior, PreSetattrBehavior,
};
use crate::{
    annotations::generate_member_builders_from_cls_namespace,
    member::{MemberBuilder, MemberCustomizationTool},
    utils::Mutability,
    validators::{Coercer, ValueValidator},
};
use crate::{
    core::{
        ATORS_MEMBER_CUSTOMIZER, ATORS_MEMBERS, ATORS_OBSERVABLE, ATORS_PICKLE_POLICY, AtorsBase,
        ClassMutability, PicklePolicy,
    },
    member::PreGetattrBehavior,
};

// FIXME: once pyo3 supports writing metaclasses in Rust, all of the generic-
// specialization state below should be stored on the metaclass itself and
// exposed through read-only interfaces rather than as module-level statics.

/// Name of the frozenset attribute that lists the member names specific to
/// each Ators subclass (as opposed to those inherited from a base class).
static ATORS_SPECIFIC_MEMBERS: &str = "__ators_specific_members__";
/// Name of the frozenset attribute that records all callable method names
/// defined on an Ators class, used to validate behavior references.
static ATORS_METHODS: &str = "__ators_methods__";
/// Name of the bool attribute that records whether instances of an Ators class
/// should be automatically frozen after `__init__`.
pub(crate) static ATORS_FROZEN: &str = "__ators_frozen__";
/// Name of the attribute that stores the un-specialized origin class for a
/// specialized generic Ators class.
static ATORS_GENERIC_ORIGIN: &str = "__ators_origin__";
/// Name of the attribute that stores the concrete type arguments used to
/// create a specialized class relative to the origin's full parameter list.
static ATORS_GENERIC_ARGS: &str = "__ators_args__";
/// Name of the attribute that stores the remaining unbound type parameters
/// of a partially-specialized generic Ators class.
static ATORS_GENERIC_TYPE_PARAMS: &str = "__ators_type_params__";
/// Name of the attribute that stores the mapping from each origin type
/// parameter to its current binding (concrete type or remaining TypeVar).
static ATORS_GENERIC_TYPEVAR_BINDINGS: &str = "__ators_typevar_bindings__";
/// Name of the dict attribute on the origin class that caches already-created
/// specializations, keyed by the full argument tuple for all origin params.
/// Storing the cache on the origin guarantees that `A[int, str]` and
/// `A[int][str]` always return the same class object.
static ATORS_GENERIC_SPECIALIZATIONS: &str = "__ators_specializations__";

fn mro_from_bases<'py>(bases: &Bound<'py, PyTuple>) -> PyResult<Vec<Bound<'py, PyType>>> {
    // Collect the MRO of all the base classes
    let mut inputs: Vec<Vec<Bound<'py, PyType>>> = bases
        .iter()
        .map(|b| -> PyResult<Vec<Bound<'py, PyType>>> {
            b.cast()?
                .mro()
                .iter()
                .map(|e| -> PyResult<Bound<'py, PyType>> { Ok(e.cast_into()?) })
                .collect()
        })
        .collect::<PyResult<Vec<Vec<Bound<'py, PyType>>>>>()?;

    // Container to store the computed MRO
    let mut mro = Vec::new();

    while !inputs.is_empty() {
        let mut candidate: Option<Bound<'py, PyType>> = None;
        for imro in inputs.iter() {
            let temp = &imro[0];
            if inputs
                .iter()
                .any(|imro| imro[1..].iter().any(|t| t.is(temp)))
            {
                candidate = None;
            } else {
                candidate = Some(temp.clone().cast_into()?);
                break;
            }
        }

        if let Some(type_) = candidate.take() {
            for imro in inputs.iter_mut() {
                if imro[0].is(&type_) {
                    imro.remove(0);
                }
            }
            mro.push(type_);
            inputs.retain(|item| !item.is_empty());
        } else {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                "Inconsistent class hierarchy with base classes {bases}"
            )));
        }
    }

    Ok(mro)
}

struct FreeSlotIndexFactory {
    occupied: HashSet<u8>,
    next_index: u8,
}

impl FreeSlotIndexFactory {
    fn next_index(&mut self) -> Result<u8, ()> {
        while self.occupied.contains(&self.next_index) {
            if self.next_index == u8::MAX {
                return Err(());
            }
            self.next_index += 1;
        }
        self.occupied.insert(self.next_index);
        Ok(self.next_index)
    }
}

fn make_unknown_method_error<'py>(
    member_name: &String,
    behavior_name: &str,
    meth_name: &Py<PyString>,
    methods: &Bound<'py, PySet>,
) -> PyErr {
    pyo3::exceptions::PyTypeError::new_err(format!(
        "Member {member_name} {behavior_name} behavior reference method {} \
        which does not exist. Known methods are {}",
        meth_name
            .bind(methods.py())
            .repr()
            .expect("String is safe to get a repr from."),
        methods
            .repr()
            .expect("Set of string is safe to get a repr from.")
    ))
}

/// Return a human-readable display string for a single type parameter.
/// For concrete types the qualified name is used; for TypeVars and other
/// parameter objects the `__name__` attribute is preferred before falling
/// back to `repr`.
fn type_param_display(param: &Bound<'_, PyAny>) -> PyResult<String> {
    if let Ok(type_) = param.cast::<PyType>() {
        return Ok(type_.name()?.to_string());
    }
    if let Ok(name) = param.getattr(intern!(param.py(), "__name__")) {
        return name.extract();
    }
    Ok(param.repr()?.to_string())
}

/// Return the exposed type parameters for `type_obj` as a tuple.
///
/// Prefers PEP 695 runtime metadata (`__type_params__`), falling back to the
/// legacy `typing.Generic` attribute (`__parameters__`) so that classes
/// declared with the old `Generic[T, ...]` style are also specializable.
/// Returns an empty tuple for non-generic classes.
#[inline]
fn get_generic_params_obj<'py>(type_obj: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyTuple>> {
    let py = type_obj.py();

    // Prefer PEP 695 runtime metadata (__type_params__), but fall back to
    // legacy typing metadata (__parameters__) so Generic[...] classes from
    // older style declarations are still specializable.
    let obj = match type_obj.getattr(intern!(py, "__type_params__")) {
        Ok(obj) if !obj.is_none() => obj,
        _ => type_obj
            .getattr(intern!(py, "__parameters__"))
            .unwrap_or_else(|_| PyTuple::empty(py).into_any()),
    };

    match obj.clone().cast_into::<PyTuple>() {
        Ok(tuple) => Ok(tuple),
        Err(_) => {
            // Some typing implementations expose an iterable but not a tuple;
            // normalize to tuple so downstream zip/len logic stays uniform.
            let mut items = Vec::new();
            for item in obj.try_iter()? {
                items.push(item?);
            }
            PyTuple::new(py, items)
        }
    }
}

fn is_type_var(param: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py = param.py();
    // FIXME: importing `typing` every call is wasteful; this lookup should
    // be cached on the module state once pyo3 makes that ergonomic.
    let typing = py.import(intern!(py, "typing"))?;
    param.is_instance(&typing.getattr(intern!(py, "TypeVar"))?)
}

/// Verify that `replacement` TypeVar has a bound that is at least as narrow
/// as the bound declared on the `parent` TypeVar.
///
/// If `parent` has no bound the replacement is accepted unconditionally.
/// Raises `TypeError` when the bound of `replacement` is either absent or
/// not a subtype of the bound of `parent`.
fn enforce_narrower_typevar_bound(
    parent: &Bound<'_, PyAny>,
    replacement: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py = parent.py();
    let parent_bound = parent.getattr(intern!(py, "__bound__"))?;
    if parent_bound.is_none() {
        return Ok(());
    }

    let replacement_bound = replacement.getattr(intern!(py, "__bound__"))?;
    if replacement_bound.is_none() {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Replacement type parameter {} must define a bound narrower than {}",
            replacement.repr()?,
            parent.repr()?
        )));
    }

    let narrower = if let (Ok(replacement_type), Ok(parent_type)) = (
        replacement_bound.cast::<PyType>(),
        parent_bound.cast::<PyType>(),
    ) {
        replacement_type.is_subclass(parent_type)?
    } else {
        // For non-class bounds, defer to Python's dynamic issubclass; if that
        // is unsupported (for example typing constructs), require exact match.
        let builtins = py.import(intern!(py, "builtins"))?;
        let issubclass = builtins.getattr(intern!(py, "issubclass"))?;
        match issubclass.call1((replacement_bound.clone(), parent_bound.clone())) {
            Ok(v) => v.extract::<bool>()?,
            Err(_) => replacement_bound.eq(&parent_bound)?,
        }
    };

    if !narrower {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Replacement type parameter {} has bound {} which is not narrower than parent bound {}",
            replacement.repr()?,
            replacement_bound.repr()?,
            parent_bound.repr()?
        )));
    }

    Ok(())
}

/// Verify that `arg` (a concrete type or TypeVar) is compatible with the
/// constraints declared on the `parent` TypeVar.
///
/// If `parent` has no `__constraints__` the check is a no-op.
/// For a concrete type `arg`: it must be a subtype of at least one constraint.
/// For a TypeVar `arg` with constraints: every constraint must be a subtype of
/// at least one of the parent's constraints.
/// For a TypeVar `arg` with only a bound (no constraints): the bound must be a
/// subtype of at least one parent constraint.
/// A TypeVar `arg` with neither constraints nor a bound is rejected.
fn enforce_within_constraints(parent: &Bound<'_, PyAny>, arg: &Bound<'_, PyAny>) -> PyResult<()> {
    let py = parent.py();
    let parent_constraints = parent.getattr(intern!(py, "__constraints__"))?;
    let Ok(parent_constraints_tuple) = parent_constraints.cast::<PyTuple>() else {
        return Ok(());
    };
    if parent_constraints_tuple.is_empty() {
        return Ok(());
    }

    // Returns true if `ty` is a subtype of at least one parent constraint.
    let is_within = |ty: &Bound<'_, PyAny>| -> PyResult<bool> {
        for constraint in parent_constraints_tuple.iter() {
            let within = if let (Ok(t), Ok(c)) = (ty.cast::<PyType>(), constraint.cast::<PyType>())
            {
                t.is_subclass(c)?
            } else {
                let builtins = py.import(intern!(py, "builtins"))?;
                let issubclass = builtins.getattr(intern!(py, "issubclass"))?;
                match issubclass.call1((ty.clone(), constraint.clone())) {
                    Ok(v) => v.extract::<bool>()?,
                    Err(_) => ty.eq(&constraint)?,
                }
            };
            if within {
                return Ok(true);
            }
        }
        Ok(false)
    };

    if is_type_var(arg)? {
        // TypeVar replacement: check its constraints against the parent constraints.
        let arg_constraints = arg.getattr(intern!(py, "__constraints__"))?;
        if let Ok(arg_constraints_tuple) = arg_constraints.cast::<PyTuple>()
            && !arg_constraints_tuple.is_empty()
        {
            for arg_constraint in arg_constraints_tuple.iter() {
                if !is_within(&arg_constraint)? {
                    return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                        "Replacement TypeVar {} has constraint {} which is not within \
                         the constraints {} of {}",
                        arg.repr()?,
                        arg_constraint.repr()?,
                        parent_constraints_tuple.repr()?,
                        parent.repr()?
                    )));
                }
            }
            return Ok(());
        }

        // TypeVar without constraints: fall back to its bound.
        let arg_bound = arg.getattr(intern!(py, "__bound__"))?;
        if !arg_bound.is_none() {
            if is_within(&arg_bound)? {
                return Ok(());
            }
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Replacement TypeVar {} has bound {} which is not within \
                 the constraints {} of {}",
                arg.repr()?,
                arg_bound.repr()?,
                parent_constraints_tuple.repr()?,
                parent.repr()?
            )));
        }

        // Unconstrained and unbound TypeVar: too broad for a constrained parent.
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Replacement TypeVar {} must define constraints or a bound \
             compatible with the constraints {} of {}",
            arg.repr()?,
            parent_constraints_tuple.repr()?,
            parent.repr()?
        )));
    }

    // Concrete type: must be a subtype of at least one constraint.
    if !is_within(arg)? {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Type argument {} is not within the constraints {} of {}",
            arg.repr()?,
            parent_constraints_tuple.repr()?,
            parent.repr()?
        )));
    }

    Ok(())
}

#[pyfunction]
pub fn create_ators_specialized_subclass<'py>(
    cls: Bound<'py, PyType>,
    params: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = cls.py();

    let exposed_params = get_generic_params_obj(&cls)?;

    if exposed_params.is_empty() {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "{} is not a generic Ators class",
            cls.qualname()?
        )));
    }

    let params_tuple = if params.is_instance_of::<PyTuple>() {
        params.cast_into::<PyTuple>()?
    } else {
        PyTuple::new(py, [params])?
    };

    if params_tuple.len() != exposed_params.len() {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "{} expects {} type arguments, got {}",
            cls.qualname()?,
            exposed_params.len(),
            params_tuple.len()
        )));
    }

    // If all type var are the type var involved in the definition of the class,
    // we can skip the specialization and return the class itself.
    let fully_passthrough = exposed_params
        .iter()
        .zip(params_tuple.iter())
        .all(|(tp, p)| tp.is(&p));
    if fully_passthrough {
        return Ok(cls.into_any());
    }

    let origin = match cls.getattr(intern!(py, ATORS_GENERIC_ORIGIN)) {
        // cls is already a specialization – use its recorded origin.
        Ok(o) => o.cast_into::<PyType>()?,
        // cls is the first time it is being specialized; it IS the origin.
        Err(_) => cls.clone(),
    };
    // Always bind against the origin definition so repeated partial
    // specializations compose transitively.
    // Apply the same compatibility rule to the origin class metadata.
    let origin_params = get_generic_params_obj(&origin)?;

    let full_bindings = PyDict::new(py);
    if let Ok(existing) = cls.getattr(intern!(py, ATORS_GENERIC_TYPEVAR_BINDINGS)) {
        for (k, v) in existing.cast_into::<PyDict>()?.iter() {
            full_bindings.set_item(k, v)?;
        }
    } else {
        for tp in origin_params.iter() {
            full_bindings.set_item(&tp, &tp)?;
        }
    }

    for (exposed, arg) in exposed_params.iter().zip(params_tuple.iter()) {
        if exposed.is(&arg) {
            continue;
        }

        if is_type_var(&arg)? {
            enforce_narrower_typevar_bound(&exposed, &arg)?;
        }
        enforce_within_constraints(&exposed, &arg)?;

        let mut to_replace = Vec::new();
        for (key, value) in full_bindings.iter() {
            if value.is(&exposed) {
                to_replace.push(key.unbind());
            }
        }
        // Replace by identity, not equality: two distinct TypeVars can be
        // equal by name but still represent different generic slots.
        for key in to_replace {
            full_bindings.set_item(key.bind(py), &arg)?;
        }
    }

    let mut unresolved = Vec::new();
    for origin_param in origin_params.iter() {
        let value = full_bindings
            .get_item(&origin_param)?
            .unwrap_or(origin_param.clone());
        // Remaining type params must preserve first-seen order while dropping
        // duplicates introduced by transitive substitutions.
        if is_type_var(&value)? && !unresolved.iter().any(|p: &Bound<'_, PyAny>| p.is(&value)) {
            unresolved.push(value);
        }
    }
    let unresolved_tuple = PyTuple::new(py, unresolved.iter())?;

    let typevar_bindings = full_bindings;

    // Compute the full argument tuple for all origin params.  This is the
    // canonical cache key: using the full args (relative to the origin) means
    // that `A[int, str]` and `A[int][str]` always resolve to the same class.
    let full_args = origin_params
        .iter()
        .map(|tp| Ok(typevar_bindings.get_item(&tp)?.unwrap_or(tp)))
        .collect::<PyResult<Vec<Bound<'_, PyAny>>>>()?;
    let full_args_tuple = PyTuple::new(py, full_args.iter())?;

    // The cache always lives on the origin class so that independent
    // specialization paths (direct vs. step-wise) share the same result.
    let cache = origin
        .getattr(intern!(py, ATORS_GENERIC_SPECIALIZATIONS))?
        .cast_into::<PyDict>()?;
    if let Some(cached) = cache.get_item(&full_args_tuple)? {
        return Ok(cached);
    }

    // `__annotations__` is guaranteed by Python to be a mapping; cast
    // directly rather than copying through `builtins.dict`.
    let annotations = cls
        .getattr(intern!(py, "__annotations__"))?
        .cast_into::<PyMapping>()?;

    let namespace = PyDict::new(py);
    namespace.set_item(
        intern!(py, "__module__"),
        cls.getattr(intern!(py, "__module__"))?,
    )?;
    namespace.set_item(intern!(py, "__annotations__"), &annotations)?;
    namespace.set_item(
        intern!(py, ATORS_GENERIC_TYPEVAR_BINDINGS),
        &typevar_bindings,
    )?;

    let members = cls
        .getattr(intern!(py, "__ators_members__"))?
        .cast_into::<PyDict>()?;
    for member_name in members.keys().iter() {
        if annotations.contains(&member_name)? {
            let mut inherited_builder = MemberBuilder::default();
            inherited_builder.set_inherit(true);
            namespace.set_item(&member_name, Bound::new(py, inherited_builder)?)?;
        }
    }

    let base_name = origin.name()?;
    let rendered = full_args_tuple
        .iter()
        .map(|p| type_param_display(&p))
        .collect::<PyResult<Vec<String>>>()?
        .join(", ");
    let specialized_name = format!("{base_name}[{rendered}]");

    let kwargs = PyDict::new(py);
    kwargs.set_item(
        intern!(py, "frozen"),
        cls.getattr(intern!(py, "__ators_frozen__"))?,
    )?;
    let specialized = cls.get_type().call(
        (
            specialized_name,
            PyTuple::new(py, [cls.as_any()])?,
            namespace,
        ),
        Some(&kwargs),
    )?;

    specialized.setattr(intern!(py, ATORS_GENERIC_ORIGIN), origin.as_any())?;
    specialized.setattr(intern!(py, ATORS_GENERIC_ARGS), &full_args_tuple)?;
    specialized.setattr(intern!(py, ATORS_GENERIC_TYPE_PARAMS), &unresolved_tuple)?;
    specialized.setattr(intern!(py, "__type_params__"), &unresolved_tuple)?;
    specialized.setattr(
        intern!(py, ATORS_GENERIC_TYPEVAR_BINDINGS),
        &typevar_bindings,
    )?;
    cache.set_item(&full_args_tuple, &specialized)?;

    Ok(specialized)
}

#[pyfunction]
#[allow(clippy::too_many_arguments)]
pub fn create_ators_subclass<'py>(
    meta: Bound<'py, PyType>,
    name: Bound<'py, PyString>,
    bases: Bound<'py, PyTuple>,
    dct: Bound<'py, PyDict>,
    frozen: bool,
    observable: bool,
    enable_weakrefs: bool,
    type_containers: i64,
    pickle_policy: Option<PicklePolicy>,
    validate_attr: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py = name.py();

    // Ators subclasses do not support slots (beyond support for weakrefs
    // through the enable_weakrefs metaclass argument), so we error if any slot
    // are found in the class dict.
    let slot_name = intern!(py, "__slots__");
    if dct.contains(slot_name)? {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "__slots__ not supported in Ators subclasses",
        ));
    }

    let ators_base_ty = py.get_type::<AtorsBase>();
    let mro = mro_from_bases(&bases)?;
    let is_observable = observable
        || mro.iter().any(|b| {
            b.getattr(ATORS_OBSERVABLE)
                .ok()
                .and_then(|v| v.extract::<bool>().ok())
                .unwrap_or(false)
        });
    dct.set_item(ATORS_OBSERVABLE, is_observable)?;

    // Resolve the pickle policy: honour an explicit value, then inherit from the first
    // base class that defines one; fall back to `ALL` (the default) if none does.
    let pickle_policy_overridden = pickle_policy.is_some();
    let pickle_policy = if let Some(p) = pickle_policy {
        p
    } else {
        mro.iter()
            .find_map(|base| {
                base.getattr(ATORS_PICKLE_POLICY)
                    .ok()
                    .and_then(|v| v.extract::<PicklePolicy>().ok())
            })
            .unwrap_or(PicklePolicy::All)
    };
    dct.set_item(ATORS_PICKLE_POLICY, Bound::new(py, pickle_policy.clone())?)?;

    // Since all classes deriving from Ators are slotted, we only need to check
    // for non-empty slots to know if a base class supports weakrefs.
    if enable_weakrefs
        && !mro
            .iter()
            .any(|b| b.hasattr(slot_name).expect("Hasattr cannot fail."))
    {
        dct.set_item(slot_name, (intern!(py, "__weakref__"),))?;
    } else {
        dct.set_item(slot_name, ())?;
    }

    let typevar_bindings =
        if let Some(tb) = dct.get_item(intern!(py, ATORS_GENERIC_TYPEVAR_BINDINGS))? {
            Some(tb.cast_into::<PyDict>()?)
        } else {
            None
        };
    if typevar_bindings.is_some() {
        dct.del_item(intern!(py, ATORS_GENERIC_TYPEVAR_BINDINGS))?;
    }

    let mut member_builders = generate_member_builders_from_cls_namespace(
        &name,
        &dct,
        type_containers,
        typevar_bindings.as_ref(),
        validate_attr,
    )?;

    // Collect the new members defined in this class that require the owning
    // class to be set to resolve ForwardRef
    let members_requiring_owner = member_builders
        .values()
        .filter(|mb| mb.require_owner)
        .map(|mb| {
            mb.name
                .clone()
                .expect("Member builders should have their name set")
        })
        .collect::<Vec<String>>();

    // Gather the name of the methods defined on the base classes.
    // For subclasses of AtorsBase we grab the names from the special class
    // attribute __ators__methods__, for other types we scan the type dictionary
    let methods = PySet::empty(py)?;
    for base in bases.iter() {
        if base.cast::<PyType>()?.is_subclass(&ators_base_ty)? {
            if !base.is(&ators_base_ty) {
                // Methods are stored as a frozenset so we can safely iterate over it.
                for method_name in base.getattr(ATORS_METHODS)?.as_any().try_iter()? {
                    methods.add(method_name?)?;
                }
            }
        } else {
            // Some metaclasses expose __dict__ as a mapping proxy-like object.
            // Iterate through the mapping protocol instead of requiring PyDict.
            let base_mapping = base
                .getattr(intern!(py, "__dict__"))?
                .cast_into::<PyMapping>()?;
            for item in base_mapping.items()?.iter() {
                let (k, v) = item.extract::<(Bound<'py, PyAny>, Bound<'py, PyAny>)>()?;
                if v.is_exact_instance_of::<PyFunction>() {
                    methods.add(k)?;
                }
            }
        }
    }

    // Walk the mro of the class, in reverse order collecting all of the
    // members into a single dict. The reverse update preserves the mro of
    // overridden members. We use only known specific members to also
    // preserve the mro in presence of multiple inheritance.
    // Note that the custom computed mro does not contain ourself.
    let mut members = HashMap::new();
    for base in mro.iter().rev() {
        // Ensure there is no frozen class among our ancestors if we are not frozen
        if base.is_subclass(&ators_base_ty)? && !base.is(&ators_base_ty) {
            if !frozen && base.getattr(ATORS_FROZEN)?.extract()? {
                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "{name} is not frozen but inherit from {} which is.",
                    base.name()?
                )));
            }
            let spm = base.getattr(ATORS_SPECIFIC_MEMBERS)?;
            members.extend(
                base.getattr(ATORS_MEMBERS)?
                    .cast::<PyDict>()?
                    .iter()
                    // SAFETY we know k is a string and that checking if it is in
                    // the set of specific member is safe.
                    .filter(|(k, _)| spm.contains(k).expect("Checking str in set[str] is safe"))
                    .map(|(k, v)| {
                        (
                            k.extract::<String>()
                                .expect("__ators_members__ keys should only be keys"),
                            v.cast_into::<Member>()
                                .expect("__ators_members__ values should only be Member"),
                        )
                    }),
            );
        }
    }

    // If this class explicitly overrides `pickle_policy`, re-evaluate inherited
    // members that did not opt in/out explicitly via `member().pickle(...)`.
    if pickle_policy_overridden {
        let mut updated_members = HashMap::new();
        for (name, member) in &members {
            let m = member.get();
            if m.pickle_explicit {
                continue;
            }
            let new_pickle = match pickle_policy {
                PicklePolicy::All => true,
                PicklePolicy::None => false,
                PicklePolicy::Public => !name.starts_with('_'),
            };
            if m.pickle != new_pickle {
                updated_members.insert(
                    name.clone(),
                    Bound::new(py, m.clone_with_pickle(new_pickle))?,
                );
            }
        }
        members.extend(updated_members);
    }

    // Collect the used indexes and existing conflict
    let mut occupied = HashSet::new();
    if is_observable {
        occupied.insert(0);
    }
    let mut conflict = Vec::new();
    for member in members.values() {
        let i = member.get().index();
        if occupied.contains(&i) {
            conflict.push(member);
        } else {
            occupied.insert(i);
        }
    }

    // Resolve index conflict for existing members
    let mut conflict_free_members = HashMap::new();
    let mut index_factory = FreeSlotIndexFactory {
        occupied,
        next_index: 0,
    };
    for cm in conflict.iter() {
        let name = { cm.get().name().to_owned() };
        let new = Bound::new(
            py,
            cm.get()
                .clone_with_index(index_factory.next_index().map_err(|_| {
                    pyo3::exceptions::PyTypeError::new_err(format!(
                        "Class {name} has more than 255 members"
                    ))
                })?),
        )?;
        conflict_free_members.insert(name.clone(), Bound::clone(&new));
        dct.set_item(name.clone(), new)?;
    }
    members.extend(conflict_free_members);

    // Collect member builder without type annotation
    let mut unannotated_member_builder_ids = HashMap::new();
    for (k, v) in dct.iter() {
        if v.is_exact_instance_of::<PyFunction>() {
            methods.add(k)?;
        } else if let Ok(mb) = v.cast_into::<MemberBuilder>() {
            let mb_id: usize = mb.as_ptr().addr();
            if unannotated_member_builder_ids.contains_key(&mb_id) {
                return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                    "'{k}' and '{}' are assigned the same member which is not supported",
                    // SAFETY we checked the key is in the HashMap so unwrapping is safe.
                    unannotated_member_builder_ids
                        .get(&mb_id)
                        .expect("Key is known to be in the map")
                )));
            }
            let name: String = k.extract()?;
            unannotated_member_builder_ids.insert(mb_id, name.clone());
            {
                with_critical_section(mb.as_any(), || {
                    mb.borrow_mut().name = Some(name.clone());
                });
            }
            member_builders.insert(name, mb.extract()?);
        }
    }

    let mut specific_members = HashSet::new();
    for (k, mb) in member_builders.iter_mut() {
        // Track members specific to this class (per opposition to members
        // which are on base classes but not on this one).
        specific_members.insert(k.clone());

        // Resolve the init flag: honour an explicit user value, then fall back
        // to the name-based default (public → true, private → false).
        // Resolve the init flag: honour an explicit user value, then fall back
        // to the name-based default (public → true, private → false).
        mb.init = Some(mb.init.unwrap_or_else(|| !k.starts_with('_')));

        // Resolve the pickle flag: honour an explicit user value, then fall back
        // to the class policy.
        if !mb.pickle_explicit {
            mb.pickle = Some(match pickle_policy {
                PicklePolicy::All => true,
                PicklePolicy::None => false,
                PicklePolicy::Public => !k.starts_with('_'),
            });
        }

        // Assign indexes to member builders and inherit behaviors if requested.
        if let Some(m) = members.get(k) {
            mb.slot_index = Some(m.get().index());
            if mb.should_inherit() {
                mb.get_inherited_behavior_from_member(m.get());
            }
        } else {
            mb.slot_index = Some(index_factory.next_index().map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err(format!(
                    "Class {name} has more than 255 members"
                ))
            })?);
            if mb.should_inherit() {
                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "Member {} is marked as inheriting from the member defined \
                    on a parent of {} but so such member exists. \
                    Known members are {:?}",
                    k,
                    name,
                    members.keys()
                )));
            }
        }

        // FIXME low prio (use a macro to reduce repetition)
        // Ensure all the method the members are using do exist.
        if let Some(PreGetattrBehavior::ObjectMethod { meth_name }) = mb.pre_getattr()
            && !methods.contains(meth_name.bind(py))?
        {
            return Err(make_unknown_method_error(
                k,
                "pre_getattr",
                meth_name,
                &methods,
            ));
        }
        if let Some(PostGetattrBehavior::ObjectMethod { meth_name }) = mb.post_getattr()
            && !methods.contains(meth_name.bind(py))?
        {
            return Err(make_unknown_method_error(
                k,
                "post_getattr",
                meth_name,
                &methods,
            ));
        }
        if let Some(PreSetattrBehavior::ObjectMethod { meth_name }) = mb.pre_setattr()
            && !methods.contains(meth_name.bind(py))?
        {
            return Err(make_unknown_method_error(
                k,
                "pre_setattr",
                meth_name,
                &methods,
            ));
        }
        if let Some(PostSetattrBehavior::ObjectMethod { meth_name }) = mb.post_setattr()
            && !methods.contains(meth_name.bind(py))?
        {
            return Err(make_unknown_method_error(
                k,
                "post_setattr",
                meth_name,
                &methods,
            ));
        }
        if let Some(DefaultBehavior::ObjectMethod { meth_name }) = mb.default_behavior()
            && !methods.contains(meth_name.bind(py))?
        {
            return Err(make_unknown_method_error(k, "default", meth_name, &methods));
        }
        if let Some(Coercer::ObjectMethod { meth_name }) = mb.coercer()
            && !methods.contains(meth_name.bind(py))?
        {
            return Err(make_unknown_method_error(k, "coerce", meth_name, &methods));
        }
        if let Some(Coercer::ObjectMethod { meth_name }) = mb.init_coercer()
            && !methods.contains(meth_name.bind(py))?
        {
            return Err(make_unknown_method_error(
                k,
                "coerce_init",
                meth_name,
                &methods,
            ));
        }
        for vv in mb.value_validators().map_or(&Vec::new(), |v| v) {
            if let ValueValidator::ObjectMethod { meth_name } = vv.clone()
                && !methods.contains(meth_name.bind(py))?
            {
                return Err(make_unknown_method_error(
                    k,
                    "value_validator",
                    &meth_name,
                    &methods,
                ));
            }
        }
    }

    // When validate_attr is False, coercion cannot function without a type
    // validator.  Fail early if any member – whether newly defined or
    // inherited from a base class – has a coercer configured.
    if !validate_attr {
        for (k, mb) in &member_builders {
            if mb.coercer().is_some() || mb.init_coercer().is_some() {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Class creation failed: attribute '{}' requires coercion \
                     but validate_attr is False",
                    k
                )));
            }
        }
        for (k, m) in &members {
            if !member_builders.contains_key(k) {
                let mv = m.get();
                if mv.validator().coercer.is_some() || mv.validator().init_coercer.is_some() {
                    return Err(pyo3::exceptions::PyValueError::new_err(format!(
                        "Class creation failed: attribute '{}' requires coercion \
                         but validate_attr is False",
                        k
                    )));
                }
            }
        }
    }

    let new_members = member_builders
        .into_iter()
        .map(|(k, v)| v.build(&name).map(|v| (k, v)))
        .collect::<PyResult<Vec<(String, Member)>>>()?
        .into_py_dict(py)?;
    let all_members = members.into_py_dict(py)?;
    all_members.update(new_members.as_mapping())?;

    dct.update(new_members.as_mapping())?;

    // Set the class level information as aggregated during the analysis
    dct.set_item(
        ATORS_SPECIFIC_MEMBERS,
        PyFrozenSet::new(py, specific_members)?,
    )?;
    dct.set_item(ATORS_METHODS, PyFrozenSet::new(py, methods)?)?;
    dct.set_item(crate::core::ATORS_MEMBERS, Bound::clone(&all_members))?;

    // Store whether or not the instance should be frozen after creation.
    dct.set_item(intern!(py, ATORS_FROZEN), frozen)?;

    // Since the only slot we use is __weakref__ we do not need copyreg

    // Add member customization tool to be used in __init__subclass__ and
    // create the class
    dct.set_item(
        ATORS_MEMBER_CUSTOMIZER,
        MemberCustomizationTool::new(&all_members),
    )?;
    let cls = py
        .import(intern!(py, "builtins"))?
        .getattr(intern!(py, "type"))?
        .call_method1(intern!(py, "__new__"), (meta, name.clone(), bases, dct))?;

    // Retrieve the customization tool and customize the members as needed
    let mut tool = cls
        .getattr(ATORS_MEMBER_CUSTOMIZER)?
        .cast_exact::<MemberCustomizationTool>()?
        .borrow_mut();
    let mut new_specific_members = None;
    for (mname, mut mb) in tool.get_builders(py) {
        // Create the new member inheriting behaviors from the existing ones.
        let existing_member = cls.getattr(&mname)?.cast_into::<Member>()?;
        let em = existing_member.get();
        mb.name = Some(em.name().to_owned());
        mb.slot_index = Some(em.index());
        mb.get_inherited_behavior_from_member(em);
        let new_member = Bound::new(py, mb.build(&name)?)?;

        // Replace the exiting member references by references by the new member
        cls.setattr(&mname, Bound::clone(&new_member))?;
        cls.getattr(ATORS_MEMBERS)?
            .cast_exact::<PyDict>()?
            .set_item(&mname, Bound::clone(&new_member))?;

        // Manual initialization to make it easier to handle result
        if new_specific_members.is_none() {
            new_specific_members = Some(PySet::new(
                py,
                cls.getattr(ATORS_SPECIFIC_MEMBERS)?
                    .cast_into_exact::<PyFrozenSet>()?
                    .iter(),
            )?);
        }
        let spec_members = new_specific_members.as_ref().expect("Initialized");
        spec_members.discard(existing_member)?;
        spec_members.add(new_member)?;
    }
    if let Some(sm) = new_specific_members {
        cls.setattr(ATORS_SPECIFIC_MEMBERS, PyFrozenSet::new(py, sm)?)?;
    }

    // Set the customizer to None to mark that the class has been created.
    cls.setattr(ATORS_MEMBER_CUSTOMIZER, py.None())?;

    // Determine class mutability based on member type validators
    let members_dict_obj = cls.getattr(ATORS_MEMBERS)?;
    let members_dict = members_dict_obj.cast::<PyDict>()?;
    let mut class_mutability = ClassMutability::Immutable {};
    let mut inspect_values_names = Vec::new();

    for (member_name, member_obj) in members_dict.iter() {
        let member = member_obj.cast::<Member>()?;

        // Set the owner if the validator contains a ForwardValidator requiring
        // a owner to resolve it.
        let requires_owner = members_requiring_owner.contains(&member_name.extract::<String>()?);
        let new_member = if requires_owner {
            Bound::new(py, member.get().with_owner(py, &cls))?
        } else {
            member_obj.clone().cast_into()?
        };
        members_dict.set_item(&member_name, Bound::clone(&new_member))?;
        cls.setattr(&member_name.extract::<String>()?, Bound::clone(&new_member))?;

        // Get the validator from the member using the accessor method
        let member_borrow = new_member.get();
        let validator = member_borrow.validator();

        // Examine the mutability of the member and update the class mutability
        // accordingly. For validation involving forward references we do not
        // have enough information to determine mutability (since the class has
        // not yet been added to the module dict), so we mark it as Undecidable
        // and keep track of the member name to later set the class mutability
        // to InspectValues if needed.
        let mutability = if requires_owner {
            Mutability::Undecidable
        } else {
            validator.type_validator.is_type_mutable(py)
        };
        match mutability {
            Mutability::Mutable => {
                class_mutability = ClassMutability::Mutable {};
                break;
            }
            Mutability::Undecidable => {
                inspect_values_names.push(member_name.extract::<String>()?);
            }
            Mutability::Immutable => {
                // Keep iterating
            }
        }
    }

    // If we haven't found a mutable type and we have undecidable types, set to InspectValues
    match class_mutability {
        ClassMutability::Immutable {} if !inspect_values_names.is_empty() => {
            class_mutability = ClassMutability::InspectValues {
                values: inspect_values_names,
            };
        }
        _ => {}
    }

    cls.setattr(
        crate::core::ATORS_MEMBERS_MUTABILITY,
        Bound::new(py, class_mutability)?,
    )?;

    // Initialize the specialization cache on generic classes so it always
    // lives on the origin (non-specialized) class. This ensures that
    // `A[int, str]` and `A[int][str]` resolve to the same class object.
    if let Ok(cls_type) = cls.cast::<PyType>()
        && !get_generic_params_obj(cls_type)?.is_empty()
    {
        cls.setattr(intern!(py, ATORS_GENERIC_SPECIALIZATIONS), PyDict::new(py))?;
    }

    Ok(cls)
}
