/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/

use std::collections::{HashMap, HashSet};

use pyo3::{
    Bound, Py, PyAny, PyErr, PyResult, ffi, intern, pyfunction,
    sync::critical_section::with_critical_section,
    types::{
        IntoPyDict, PyAnyMethods, PyDict, PyDictMethods, PyFunction, PyListMethods, PyMapping,
        PyMappingMethods, PySet, PySetMethods, PyString, PyTuple, PyTupleMethods, PyType,
        PyTypeMethods,
    },
};

use crate::{
    annotations::generate_member_builders_from_cls_namespace,
    class_info::{
        AtorsClassInfo, AtorsGenericInfo, ClassMutability, PicklePolicy, get_class_info,
        insert_definitive_class_info, insert_pending_specialization_bindings,
        insert_temp_class_info, pop_definitive_class_info, pop_temp_class_info,
        take_pending_specialization_bindings_for_inputs,
    },
    core::AtorsBase,
    member::PreGetattrBehavior,
    member::{
        DefaultBehavior, Member, PostGetattrBehavior, PostSetattrBehavior, PreSetattrBehavior,
    },
    member::{MemberBuilder, MemberCustomizationTool},
    utils::Mutability,
    validators::{Coercer, ValueValidator},
};

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

/// Return `true` when `param` is an instance of `typing.ForwardRef`.
fn is_forward_ref(param: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py = param.py();
    // FIXME: importing `typing` every call is wasteful; cache on module state
    // once pyo3 makes that ergonomic.
    let typing = py.import(intern!(py, "typing"))?;
    param.is_instance(&typing.getattr(intern!(py, "ForwardRef"))?)
}

/// Return `true` when `param` is `typing.Any`.
fn is_any_type(param: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py = param.py();
    let typing = py.import(intern!(py, "typing"))?;
    let any = typing.getattr(intern!(py, "Any"))?;
    Ok(param.is(&any))
}

/// Return `true` when `arg` satisfies the bound and/or constraints of `typevar`.
///
/// Rules (mirroring PEP 484 compatibility):
/// - If `typevar` has a bound `B`, `arg` must be a subclass of `B`.
/// - If `typevar` has constraints `[C1, C2, …]`, `arg` must be a subclass of
///   at least one constraint.
/// - An unconstrained, unbound TypeVar is a wildcard and matches anything.
fn typevar_matches_arg(typevar: &Bound<'_, PyAny>, arg: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py = typevar.py();

    // Bounded TypeVar: arg must be a subclass of the bound (checked first as more common).
    let bound = typevar.getattr(intern!(py, "__bound__"))?;
    if !bound.is_none() {
        if let (Ok(arg_t), Ok(bound_t)) = (arg.cast::<PyType>(), bound.cast::<PyType>()) {
            return arg_t.is_subclass(bound_t);
        }
        // Non-type bound: fall back to equality.
        return arg.eq(&bound);
    }

    // Constrained TypeVar: arg must be subclass of (at least) one constraint.
    let constraints = typevar.getattr(intern!(py, "__constraints__"))?;
    let constraints_tuple = constraints.cast_into::<PyTuple>()?;
    if !constraints_tuple.is_empty() {
        for constraint in constraints_tuple.iter() {
            if arg.is(&constraint) {
                return Ok(true);
            }
            if let (Ok(arg_t), Ok(con_t)) = (arg.cast::<PyType>(), constraint.cast::<PyType>())
                && arg_t.is_subclass(con_t)?
            {
                return Ok(true);
            }
        }
        return Ok(false);
    }

    // Unconstrained, unbound TypeVar: wildcard.
    Ok(true)
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
                t.is_subclass(c.as_any())?
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

/// Core generic-subclass matching logic (no caching).
///
/// Returns `true` when `sub` is generically compatible with the specialised
/// generic `cls`.  Both `cls` and `sub` are expected to be Ators-specialised
/// classes (i.e. they carry `__origin__` and `__args__`).
#[cold]
fn generic_subclass_match_impl<'py>(
    cls: &Bound<'py, PyType>,
    sub: &Bound<'py, PyType>,
) -> PyResult<bool> {
    // Identity short-circuit.
    if sub.is(cls) {
        return Ok(true);
    }
    let py = cls.py();

    // `cls` must be a specialised generic (non-None origin).
    let cls_info = get_class_info(cls)?;
    let Some(cls_generic) = cls_info.generic() else {
        return Ok(false);
    };
    let Some(cls_origin) = cls_generic.origin() else {
        return Ok(false);
    };

    // `sub` must also be a specialised generic.
    let sub_info = get_class_info(sub)?;
    let Some(sub_generic) = sub_info.generic() else {
        return Ok(false);
    };
    let Some(sub_origin) = sub_generic.origin() else {
        return Ok(false);
    };

    // Origins must be compatible: same object, or sub_origin is a (normal)
    // subclass of cls_origin.
    if !sub_origin.is(cls_origin) {
        // Both origins are already PyType; use C-level check to avoid going
        // through Python dispatch (which would re-enter __subclasscheck__).
        if unsafe {
            ffi::PyType_IsSubtype(
                sub_origin.bind(py).as_type_ptr(),
                cls_origin.bind(py).as_type_ptr(),
            ) == 0
        } {
            return Ok(false);
        }
    }

    // Retrieve argument tuples.
    let sub_args = sub_generic.args();
    let cls_args = cls_generic.args();

    // Arity mismatch => False.
    if sub_args.len() != cls_args.len() {
        return Ok(false);
    }

    // Match each argument position.
    for (sub_arg, cls_arg) in sub_args
        .iter()
        .map(|sa| sa.bind(py))
        .zip(cls_args.iter().map(|ca| ca.bind(py)))
    {
        // `Any` on the RHS matches everything.
        if is_any_type(cls_arg)? {
            continue;
        }

        // TypeVar on the RHS acts as a wildcard (subject to bound/constraints).
        if is_type_var(cls_arg)? {
            if !typevar_matches_arg(cls_arg, sub_arg)? {
                return Ok(false);
            }
            continue;
        }

        // Concrete type: sub_arg must be identical or a subclass.
        if sub_arg.is(cls_arg) {
            continue;
        }
        match (sub_arg.cast::<PyType>(), cls_arg.cast::<PyType>()) {
            (Ok(s), Ok(c)) => {
                if !s.is_subclass(c)? {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        }
    }

    Ok(true)
}

/// Generic-aware runtime subclass check (Rust side).
///
/// Assigned as `AtorsMeta.__subclasscheck__`.  When `cls` is a specialised
/// Ators generic (carries a non-`None` `__origin__`), the generic match
/// engine is used.  Otherwise a direct C-level type hierarchy check is used to
/// avoid recursion through Python's `__subclasscheck__` dispatch.
#[pyfunction]
pub fn rust_subclasscheck<'py>(
    cls: &Bound<'py, PyType>,
    sub: &Bound<'py, PyType>,
) -> PyResult<bool> {
    if get_class_info(cls)?
        .generic()
        .and_then(|generic| generic.origin())
        .is_some()
    {
        return generic_subclass_match_impl(cls, sub);
    }
    // Non-generic fallback: input types are already known; use the C-level
    // subtype check to avoid going back through Python's __subclasscheck__
    // dispatch (which would recurse here).
    Ok(unsafe { ffi::PyType_IsSubtype(sub.as_type_ptr(), cls.as_type_ptr()) != 0 })
}

/// Generic-aware runtime instance check (Rust side).
///
/// Assigned as `AtorsMeta.__instancecheck__`.  When `cls` is a specialised
/// Ators generic, the generic match engine is applied to `type(instance)`.
/// Otherwise a C-level subtype check is used.
#[pyfunction]
pub fn rust_instancecheck<'py>(
    cls: &Bound<'py, PyType>,
    instance: &Bound<'py, PyAny>,
) -> PyResult<bool> {
    let instance_type = instance.get_type();
    if get_class_info(cls)?
        .generic()
        .and_then(|generic| generic.origin())
        .is_some()
    {
        return generic_subclass_match_impl(cls, &instance_type);
    }
    Ok(unsafe { ffi::PyType_IsSubtype(instance_type.as_type_ptr(), cls.as_type_ptr()) != 0 })
}

/// Create (or reuse from cache) a specialised Ators generic subclass.
///
/// This resolves and validates type arguments, computes the canonical origin
/// argument mapping, and reuses an existing specialisation when available.
#[pyfunction]
pub fn create_ators_specialized_subclass<'py>(
    cls: &Bound<'py, PyType>,
    params: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = cls.py();

    let cls_info = get_class_info(cls)?;
    let Some(cls_generic_info) = cls_info.generic() else {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "{} is not a generic Ators class",
            cls.qualname()?
        )));
    };
    let exposed_params = cls_generic_info.type_parameters();

    let params_tuple = if params.is_instance_of::<PyTuple>() {
        params.cast::<PyTuple>()?.clone()
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

    // ForwardRef is forbidden at specialisation time: it cannot be resolved
    // without a concrete namespace and would silently produce wrong results.
    for arg in params_tuple.iter() {
        if is_forward_ref(&arg)? {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "ForwardRef is not supported in Ators generic specialisations; \
                 use concrete types or TypeVar instead",
            ));
        }
    }

    // If all type var are the type var involved in the definition of the class,
    // we can skip the specialization and return the class itself.
    let fully_passthrough = exposed_params
        .iter()
        .zip(params_tuple.iter())
        .all(|(tp, p)| tp.is(&p));
    if fully_passthrough {
        return Ok(cls.clone().into_any());
    }

    // None: cls is the first time it is being specialized; it IS the origin.
    // Some: cls is already a specialization – use its recorded origin.
    let origin = cls_generic_info.origin().map(|o| o.bind(py)).unwrap_or(cls);

    // Always bind against the origin definition so repeated partial
    // specializations compose transitively.
    // Apply the same compatibility rule to the origin class metadata.
    let origin_params = get_generic_params_obj(origin)?;

    // Determine how the type variables of the origin class map to the provided
    // arguments,  starting from the existing bindings on the class (if any)
    // or the identity mapping on the origin parameters.
    // This allows partial specializations to be defined in terms of the
    // original type parameters, and for users to re-use the same argument in
    // multiple positions (e.g. `A[int, int]`).
    let full_bindings = PyDict::new(py);
    if let Some(existing) = cls_generic_info.typevar_bindings() {
        for (k, v) in existing.bind(py).iter() {
            full_bindings.set_item(k, v)?;
        }
    } else {
        for tp in origin_params.iter() {
            full_bindings.set_item(&tp, &tp)?;
        }
    }
    for (exposed, arg) in exposed_params
        .iter()
        .map(|p| p.bind(py))
        .zip(params_tuple.iter())
    {
        if exposed.is(&arg) {
            continue;
        }

        if is_type_var(&arg)? {
            enforce_narrower_typevar_bound(exposed, &arg)?;
        }
        enforce_within_constraints(exposed, &arg)?;

        let mut to_replace = Vec::new();
        for (key, value) in full_bindings.iter() {
            if value.is(exposed) {
                to_replace.push(key.unbind());
            }
        }
        // Replace by identity, not equality: two distinct TypeVars can be
        // equal by name but still represent different generic slots.
        for key in to_replace {
            full_bindings.set_item(key.bind(py), &arg)?;
        }
    }

    // Compute the set of unresolved type variables in the final bindings.
    // This is used to determine which type variables remain free in the
    // specialized class and should be recorded as typevar_bindings metadata
    // for downstream specializations.
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

    // Compute the full argument tuple for all origin params.  This is the
    // canonical cache key: using the full args (relative to the origin) means
    // that `A[int, str]` and `A[int][str]` always resolve to the same class.
    let typevar_bindings = full_bindings;
    let full_args = origin_params
        .iter()
        .map(|tp| Ok(typevar_bindings.get_item(&tp)?.unwrap_or(tp)))
        .collect::<PyResult<Vec<Bound<'_, PyAny>>>>()?;
    let full_args_tuple = PyTuple::new(py, full_args.iter())?;

    // The cache always lives on the origin class so that independent
    // specialization paths (direct vs. step-wise) share the same result.
    let origin_info = get_class_info(origin)?;
    let cache = origin_info
        .generic()
        .and_then(|g| g.specializations())
        .ok_or_else(|| {
            let origin_name = origin
                .qualname()
                .ok()
                .map(|n| n.to_string())
                .unwrap_or_else(|| "<unknown>".to_string());
            pyo3::exceptions::PyRuntimeError::new_err(format!(
                "Missing generic specialization cache for {}",
                origin_name
            ))
        })?
        .bind(py)
        .clone();
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

    for member_name in cls_info.members_by_name_ref(py).keys() {
        if annotations.contains(member_name)? {
            let mut inherited_builder = MemberBuilder::default();
            inherited_builder.set_inherit(true);
            namespace.set_item(member_name, Bound::new(py, inherited_builder)?)?;
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
    kwargs.set_item(intern!(py, "frozen"), cls_info.frozen())?;

    let typevar_bindings_py = typevar_bindings.unbind();
    let origin_module: String = cls.getattr(intern!(py, "__module__"))?.extract()?;
    let specialized_fqname = format!("{origin_module}.{specialized_name}");
    insert_pending_specialization_bindings(
        py,
        specialized_fqname,
        typevar_bindings_py.clone_ref(py),
    );

    let specialized = cls.get_type().call(
        (
            specialized_name,
            PyTuple::new(py, [cls.as_any()])?,
            namespace,
        ),
        Some(&kwargs),
    )?;

    if let Ok(specialized_type) = specialized.cast::<PyType>() {
        let info = pop_definitive_class_info(py, specialized_type);
        // AtorsGenericInfo::new takes (type_parameters, origin, args, ...)
        // so unresolved type vars are first, followed by concrete specialization args.
        let updated = info.with_generic(Some(AtorsGenericInfo::new(
            unresolved_tuple.iter().map(|a| a.unbind()).collect(),
            Some(origin.clone().unbind()),
            full_args_tuple.iter().map(|a| a.unbind()).collect(),
            Some(typevar_bindings_py),
            None,
        )));
        insert_definitive_class_info(py, specialized_type, updated);
    }
    cache.set_item(&full_args_tuple, &specialized)?;

    Ok(specialized)
}

/// Create an Ators subclass from metaclass inputs.
///
/// This computes member layout and inherited behaviors, enforces Ators class
/// constraints, records class metadata, and returns the newly created class.
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
            b.cast::<PyType>()
                .ok()
                .and_then(|base_ty| get_class_info(base_ty).ok())
                .is_some_and(|info| info.observable())
        });

    // Resolve the pickle policy: honour an explicit value, then inherit from the first
    // base class that defines one; fall back to `ALL` (the default) if none does.
    let pickle_policy_overridden = pickle_policy.is_some();
    let pickle_policy = if let Some(p) = pickle_policy {
        p
    } else {
        mro.iter()
            .filter_map(|base| base.cast::<PyType>().ok())
            .find_map(|base_ty| {
                get_class_info(base_ty)
                    .ok()
                    .map(|info| info.pickle_policy().clone())
            })
            .unwrap_or(PicklePolicy::All)
    };

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

    let typevar_bindings = take_pending_specialization_bindings_for_inputs(py, &name, &dct)?;
    let typevar_bindings_ref = typevar_bindings.as_ref().map(|tb| tb.bind(py));

    let mut member_builders = generate_member_builders_from_cls_namespace(
        &name,
        &dct,
        type_containers,
        typevar_bindings_ref,
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
    let methods = PySet::empty(py)?;
    let mut methods_by_name = HashSet::new();
    for base in bases.iter() {
        if base.cast::<PyType>()?.is_subclass(&ators_base_ty)? {
            if !base.is(&ators_base_ty) {
                let base_info = get_class_info(base.cast::<PyType>()?)?;
                for method_name in base_info.method_names() {
                    methods.add(method_name)?;
                    methods_by_name.insert(method_name.clone());
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
                    methods.add(&k)?;
                    methods_by_name.insert(k.extract::<String>()?);
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
            let base_info = get_class_info(base)?;
            if !frozen && base_info.frozen() {
                return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                    "{name} is not frozen but inherit from {} which is.",
                    base.name()?
                )));
            }
            let spm = base_info.specific_member_names();
            members.extend(
                base_info
                    .members_by_name_ref(py)
                    .iter()
                    .filter(|(k, _)| spm.contains(k.as_str()))
                    .map(|(k, v)| (k.clone(), v.bind(py).clone())),
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
            methods.add(&k)?;
            methods_by_name.insert(k.extract::<String>()?);
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
        if mb.init.is_none() {
            mb.init = Some(!k.starts_with('_'));
        }
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

    // Since the only slot we use is __weakref__ we do not need copyreg

    // Build class info with member customization tool to be used in
    // __init__subclass__
    let members_by_name = all_members
        .iter()
        .map(|(k, v)| Ok((k.extract::<String>()?, v.cast::<Member>()?.clone().unbind())))
        .collect::<PyResult<HashMap<String, Py<Member>>>>()?;
    let mut required_init_member_names = Vec::new();
    let mut optional_init_member_names = Vec::new();
    for (member_name, member) in &members_by_name {
        let member = member.bind(py).get();
        if member.init {
            let n = PyString::new(py, member_name).unbind();
            if member.has_default() {
                optional_init_member_names.push(n);
            } else {
                required_init_member_names.push(n);
            }
        }
    }
    let class_info = AtorsClassInfo::new(
        py,
        frozen,
        is_observable,
        pickle_policy.clone(),
        None,
        members_by_name,
        specific_members,
        optional_init_member_names,
        required_init_member_names,
        methods_by_name,
        None,
        Some(Py::new(py, MemberCustomizationTool::new(&all_members))?),
    )?;
    let fqname = insert_temp_class_info(py, &name, &dct, class_info)?;

    let cls_result = py
        .import(intern!(py, "builtins"))?
        .getattr(intern!(py, "type"))?
        .call_method1(intern!(py, "__new__"), (meta, name.clone(), bases, dct))?;
    let cls = match cls_result.cast_into::<PyType>() {
        Ok(c) => c,
        Err(err) => {
            pop_temp_class_info(py, &fqname);
            return Err(err.into());
        }
    };
    let mut class_info = pop_temp_class_info(py, &fqname);
    let mut updated_members_by_name = class_info
        .members_by_name_ref(py)
        .iter()
        .map(|(k, v)| (k.clone(), v.clone_ref(py)))
        .collect::<HashMap<_, _>>();

    // Retrieve and clear the customization tool and customize the members as needed
    let mut tool = class_info.take_customizer().bind(py).borrow_mut();
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
        all_members.set_item(&mname, Bound::clone(&new_member))?;
        updated_members_by_name.insert(mname.clone(), new_member.clone().unbind());
    }

    // Determine class mutability based on member type validators
    let members_dict = &all_members;
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
        let member_name_str = member_name.extract::<String>()?;
        cls.setattr(&member_name_str, Bound::clone(&new_member))?;
        updated_members_by_name.insert(member_name_str.clone(), new_member.clone().unbind());

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

    // Initialize specialization cache once for generic classes so it always
    // lives on the origin (non-specialized) class.
    let generic_params = get_generic_params_obj(&cls)?;
    let generic = if !generic_params.is_empty() {
        let typevar_bindings = PyDict::new(py);
        for param in generic_params.iter() {
            typevar_bindings.set_item(&param, &param)?;
        }
        Some(AtorsGenericInfo::new(
            generic_params.iter().map(|p| p.unbind()).collect(),
            None,
            Vec::new(),
            Some(typevar_bindings.unbind()),
            Some(PyDict::new(py).unbind()),
        ))
    } else {
        None
    };

    let final_class_info = class_info
        .with_members(py, updated_members_by_name)?
        .with_generic(generic)
        .with_mutability(Some(class_mutability));
    insert_definitive_class_info(py, &cls, final_class_info);

    Ok(cls.into_any())
}
