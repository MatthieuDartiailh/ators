/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/

use pyo3::{
    Bound, Py, PyAny, PyErr, PyResult,
    ffi::{PyType_IsSubtype, c_str},
    intern, pyfunction,
    sync::PyOnceLock,
    types::{
        PyAnyMethods, PyDict, PyDictMethods, PyMapping, PyMappingMethods, PyTuple, PyTupleMethods,
        PyType, PyTypeMethods,
    },
};

use crate::{
    class::info::{
        AtorsGenericInfo, class_key, get_ators_specialized_class_for_alias, get_class_info,
        get_class_info_store, insert_definitive_class_info, insert_pending_specialization_bindings,
        pop_definitive_class_info,
    },
    member::MemberBuilder,
    utils::{is_any_type, is_type_var},
};

static ATORS_GENERIC_ALIAS_TYPE: PyOnceLock<Py<PyType>> = PyOnceLock::new();

fn get_or_create_ators_generic_alias_type<'py>(
    py: pyo3::Python<'py>,
) -> PyResult<Bound<'py, PyType>> {
    let alias_cls = ATORS_GENERIC_ALIAS_TYPE.get_or_try_init(py, || {
        let locals = PyDict::new(py);
        locals.set_item("types", py.import(c_str!("types"))?)?;
        locals.set_item(
            "_get_ators_specialized_class_for_alias",
            pyo3::wrap_pyfunction!(get_ators_specialized_class_for_alias, py)?,
        )?;
        locals.set_item(
            "_rust_instancecheck_alias",
            pyo3::wrap_pyfunction!(rust_instancecheck_alias, py)?,
        )?;
        locals.set_item(
            "_rust_subclasscheck_alias",
            pyo3::wrap_pyfunction!(rust_subclasscheck_alias, py)?,
        )?;
        py.run(
            c_str!(
                r#"
class AtorsGenericAlias(types.GenericAlias):
	"""GenericAlias-compatible view over a specialized Ators class."""

	def __getattribute__(self, name: str):
		if name == "__type_params__":
			return self.__ators_specialized_class__.__type_params__
		if name == "__ators_specialized_class__":
			return _get_ators_specialized_class_for_alias(self)
		return super().__getattribute__(name)

	def __call__(self, *args, **kwargs):
		return self.__ators_specialized_class__(*args, **kwargs)

	def __getitem__(self, params):
		return self.__ators_specialized_class__[params]

	def __instancecheck__(self, instance):
		return _rust_instancecheck_alias(self, instance)

	def __subclasscheck__(self, sub):
		return _rust_subclasscheck_alias(self, sub)

	def __mro_entries__(self, bases):
		del bases
		return (self.__ators_specialized_class__,)

	def __getattr__(self, name: str):
		return getattr(self.__ators_specialized_class__, name)
"#
            ),
            Some(&locals),
            None,
        )?;
        let alias_cls = locals
            .get_item("AtorsGenericAlias")?
            .ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("Failed to create AtorsGenericAlias")
            })?
            .cast_into::<PyType>()?;
        Ok::<Py<PyType>, PyErr>(alias_cls.unbind())
    })?;
    Ok(alias_cls.clone_ref(py).into_bound(py))
}

/// Return `true` when `arg` satisfies the bound and/or constraints of `typevar`.
fn typevar_matches_arg(typevar: &Bound<'_, PyAny>, arg: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py = typevar.py();

    let bound = typevar.getattr(intern!(py, "__bound__"))?;
    if !bound.is_none() {
        if let (Ok(arg_t), Ok(bound_t)) = (arg.cast::<PyType>(), bound.cast::<PyType>()) {
            return arg_t.is_subclass(bound_t);
        }
        return arg.eq(&bound);
    }

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

    Ok(true)
}

fn generic_subclass_match_impl<'py>(
    cls: &Bound<'py, PyType>,
    sub: &Bound<'py, PyType>,
) -> PyResult<bool> {
    if sub.is(cls) {
        return Ok(true);
    }
    let py = cls.py();

    let cls_info = get_class_info(cls)?;
    let cls_generic = cls_info.generic().expect("Cls is known to be generic.");
    let cls_origin = cls_generic
        .origin()
        .expect("Cls is known to be a specialised generic.");

    let sub_info = get_class_info(sub)?;
    let Some(sub_generic) = sub_info.generic() else {
        return Ok(false);
    };
    let Some(sub_origin) = sub_generic.origin() else {
        return Ok(false);
    };

    if !sub_origin.is(cls_origin)
        && unsafe {
            PyType_IsSubtype(
                sub_origin.bind(py).as_type_ptr(),
                cls_origin.bind(py).as_type_ptr(),
            ) == 0
        }
    {
        return Ok(false);
    }

    let sub_args = sub_generic.args();
    let cls_args = cls_generic.args();

    for (sub_arg, cls_arg) in sub_args
        .iter()
        .map(|sa| sa.bind(py))
        .zip(cls_args.iter().map(|ca| ca.bind(py)))
    {
        if is_any_type(cls_arg)? {
            continue;
        }

        if is_type_var(cls_arg)? {
            if !typevar_matches_arg(cls_arg, sub_arg)? {
                return Ok(false);
            }
            continue;
        }

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

#[pyfunction]
pub fn rust_instancecheck_alias<'py>(
    alias: &Bound<'py, PyAny>,
    instance: &Bound<'py, PyAny>,
) -> PyResult<bool> {
    let cls = get_ators_specialized_class_for_alias(alias)?;
    generic_subclass_match_impl(&cls, &instance.get_type())
}

#[pyfunction]
pub fn rust_subclasscheck_alias<'py>(
    alias: &Bound<'py, PyAny>,
    sub: &Bound<'py, PyAny>,
) -> PyResult<bool> {
    let cls = get_ators_specialized_class_for_alias(alias)?;
    let sub_cls =
        get_ators_specialized_class_for_alias(sub).or_else(|_| sub.cast::<PyType>().cloned())?;
    generic_subclass_match_impl(&cls, &sub_cls)
}

#[pyfunction]
pub fn wrap_ators_specialized_class<'py>(cls: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyAny>> {
    let py = cls.py();
    let info = get_class_info(cls)?;
    let Some(generic) = info.generic() else {
        return Ok(cls.clone().into_any());
    };
    if generic.origin().is_none() {
        return Ok(cls.clone().into_any());
    }

    let cls_key = class_key(cls);
    let store = get_class_info_store(py);
    if let Some(alias) = store
        .read()
        .expect("Class info store read lock poisoned")
        .alias_by_specialized
        .get(&cls_key)
    {
        return Ok(alias.bind(py).clone());
    }

    let alias_cls = get_or_create_ators_generic_alias_type(py)?;
    let origin = generic
        .origin()
        .expect("Checked above that specialized classes have an origin")
        .bind(py)
        .clone();
    let args = PyTuple::new(py, generic.args().iter().map(|a| a.bind(py)))?;
    let alias = alias_cls.call1((origin, args))?;
    let alias_key = alias.as_ptr().addr();

    {
        let mut store = store.write().expect("Class info store write lock poisoned");
        store
            .alias_by_specialized
            .insert(cls_key, alias.clone().unbind());
        store
            .specialized_by_alias
            .insert(alias_key, cls.clone().unbind());
    }
    Ok(alias)
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
pub(crate) fn get_generic_params_obj<'py>(
    type_obj: &Bound<'py, PyType>,
) -> PyResult<Bound<'py, PyTuple>> {
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
