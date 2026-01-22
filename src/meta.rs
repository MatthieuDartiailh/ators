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
    types::{
        IntoPyDict, PyAnyMethods, PyDict, PyDictMethods, PyFrozenSet, PyFrozenSetMethods,
        PyFunction, PySet, PySetMethods, PyString, PyTuple, PyTupleMethods, PyType, PyTypeMethods,
    },
};

use crate::member::{
    DefaultBehavior, Member, PostGetattrBehavior, PostSetattrBehavior, PreSetattrBehavior,
};
use crate::{
    annotations::generate_member_builders_from_cls_namespace,
    member::{MemberBuilder, MemberCustomizationTool},
    validators::{Coercer, ValueValidator},
};
use crate::{
    core::{ATORS_MEMBER_CUSTOMIZER, ATORS_MEMBERS, AtorsBase},
    member::PreGetattrBehavior,
};

static ATORS_SPECIFIC_MEMBERS: &str = "__ators_specific_members__";
static ATORS_METHODS: &str = "__ators_methods__";
static ATORS_FROZEN: &str = "__ators_frozen__";

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

#[pyfunction]
pub fn create_ators_subclass<'py>(
    meta: Bound<'py, PyType>,
    name: Bound<'py, PyString>,
    bases: Bound<'py, PyTuple>,
    dct: Bound<'py, PyDict>,
    frozen: bool,
    enable_weakrefs: bool,
    type_containers: i64,
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

    let mut member_builders =
        generate_member_builders_from_cls_namespace(&name, &dct, type_containers)?;

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
            for (k, v) in base
                .getattr(intern!(py, "__dict__"))?
                .cast::<PyDict>()?
                .iter()
            {
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

    // Collect the used indexes and existing conflict
    let mut occupied = HashSet::new();
    let mut conflict = Vec::new();
    for member in members.values() {
        let i = member.borrow().index();
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
        let name = { cm.borrow().name().to_owned() };
        let new = Bound::new(
            py,
            cm.borrow()
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
                mb.borrow_mut().name = Some(name.clone());
            }
            member_builders.insert(name, mb.extract()?);
        }
    }

    let mut specific_members = HashSet::new();
    for (k, mb) in member_builders.iter_mut() {
        // Track members specific to this class (per opposition to members
        // which are on base classes but not on this one).
        specific_members.insert(k.clone());

        // Assign indexes to member builders and inherit behaviors if requested.
        if let Some(m) = members.get(k) {
            mb.slot_index = Some(m.borrow().index());
            if mb.should_inherit() {
                mb.get_inherited_behavior_from_member(&m.borrow());
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

    let new_members = member_builders
        .into_iter()
        .map(|(k, v)| v.clone().build(&name).map(|v| (k, v)))
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
        let existing = cls.getattr(&mname)?.cast::<Member>()?.borrow();
        mb.name = Some(existing.name().to_owned());
        mb.slot_index = Some(existing.index());
        mb.get_inherited_behavior_from_member(&existing);
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
        spec_members.discard(existing)?;
        spec_members.add(new_member)?;
    }
    if let Some(sm) = new_specific_members {
        cls.setattr(ATORS_SPECIFIC_MEMBERS, PyFrozenSet::new(py, sm)?)?;
    }

    // Set the customizer to None to mark that the class has been created.
    cls.setattr(ATORS_MEMBER_CUSTOMIZER, py.None())?;

    Ok(cls)
}
