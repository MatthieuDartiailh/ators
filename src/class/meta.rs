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
        IntoPyDict, PyAnyMethods, PyDict, PyDictMethods, PyFunction, PyListMethods, PyMapping,
        PyMappingMethods, PySet, PySetMethods, PyString, PyTuple, PyTupleMethods, PyType,
        PyTypeMethods,
    },
};

use crate::{
    annotations::generate_member_builders_from_cls_namespace,
    class::base::AtorsBase,
    class::generic::get_generic_params_obj,
    class::info::{
        AtorsClassInfo, AtorsGenericInfo, ClassMutability, PicklePolicy, get_class_info,
        insert_definitive_class_info, insert_temp_class_info, pop_temp_class_info,
        take_pending_specialization_bindings_for_inputs,
    },
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

/// Return `true` if `obj` is marked as abstract via `__isabstractmethod__ == True`.
///
/// This handles plain functions/methods and also inspects the wrapped callable
/// for `classmethod`, `staticmethod`, and `property` objects so that
/// `@classmethod @abstractmethod`, `@staticmethod @abstractmethod`, and
/// `@property @abstractmethod` stacks are detected correctly.  For `property`,
/// if *any* of `fget`, `fset`, or `fdel` is marked abstract the property is
/// considered abstract (matching CPython's `abc` module behaviour).
fn is_abstract_member(obj: &Bound<'_, PyAny>) -> bool {
    let py = obj.py();
    let is_abstract_key = intern!(py, "__isabstractmethod__");
    // Check the object itself first
    if obj
        .getattr(is_abstract_key)
        .ok()
        .and_then(|v| v.extract::<bool>().ok())
        .unwrap_or(false)
    {
        return true;
    }
    // For classmethod/staticmethod, check the wrapped __func__
    if let Ok(func) = obj.getattr(intern!(py, "__func__")) {
        if func
            .getattr(is_abstract_key)
            .ok()
            .and_then(|v| v.extract::<bool>().ok())
            .unwrap_or(false)
        {
            return true;
        }
    }
    // For property, check fget/fset/fdel
    for accessor in &[
        intern!(py, "fget"),
        intern!(py, "fset"),
        intern!(py, "fdel"),
    ] {
        if let Ok(acc) = obj.getattr(accessor) {
            if !acc.is_none()
                && acc
                    .getattr(is_abstract_key)
                    .ok()
                    .and_then(|v| v.extract::<bool>().ok())
                    .unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
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
    // Collect inherited abstract methods from Ators bases
    let mut inherited_abstract_methods: HashSet<String> = HashSet::new();
    for base in bases.iter() {
        if base.cast::<PyType>()?.is_subclass(&ators_base_ty)? {
            if !base.is(&ators_base_ty) {
                let base_info = get_class_info(base.cast::<PyType>()?)?;
                for method_name in base_info.method_names() {
                    methods.add(method_name)?;
                    methods_by_name.insert(method_name.clone());
                }
                for abstract_name in base_info.abstract_methods() {
                    inherited_abstract_methods.insert(abstract_name.clone());
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
            // Collect abstract methods from non-Ators bases via __abstractmethods__
            if let Ok(abs_set) = base.getattr(intern!(py, "__abstractmethods__")) {
                if !abs_set.is_none() {
                    for name in abs_set.try_iter()? {
                        inherited_abstract_methods.insert(name?.extract::<String>()?);
                    }
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

    // Collect member builder without type annotation.
    // Also classify each namespace entry as abstract or concrete in a single pass.
    let mut unannotated_member_builder_ids = HashMap::new();
    let mut declared_abstract_methods: HashSet<String> = HashSet::new();
    let mut concrete_names: HashSet<String> = HashSet::new();
    for (k, v) in dct.iter() {
        let k_str: String = k.extract()?;
        if is_abstract_member(&v) {
            declared_abstract_methods.insert(k_str.clone());
        } else {
            concrete_names.insert(k_str.clone());
        }
        if v.is_exact_instance_of::<PyFunction>() {
            methods.add(&k)?;
            methods_by_name.insert(k_str);
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
            unannotated_member_builder_ids.insert(mb_id, k_str.clone());
            {
                with_critical_section(mb.as_any(), || {
                    mb.borrow_mut().name = Some(k_str.clone());
                });
            }
            member_builders.insert(k_str, mb.extract()?);
        }
    }

    // Compute the final set of unresolved abstract methods:
    // start from inherited set, remove names overridden concretely in this class,
    // then union with newly declared abstracts.
    let mut abstract_methods: HashSet<String> = inherited_abstract_methods;
    for k_str in &concrete_names {
        abstract_methods.remove(k_str);
    }
    abstract_methods.extend(declared_abstract_methods);

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
        abstract_methods,
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
