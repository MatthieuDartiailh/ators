/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Matthieu C. Dartiailh
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/

use std::collections::{HashMap, HashSet};

use pyo3::{
    Bound, PyAny, PyResult, intern, pyfunction,
    types::{
        IntoPyDict, PyAnyMethods, PyDict, PyDictMethods, PyFrozenSet, PyFunction, PySet,
        PySetMethods, PyString, PyTuple, PyTupleMethods, PyType, PyTypeMethods,
    },
};

use crate::{
    annotations::generate_member_builders_from_cls_namespace,
    member::MemberBuilder,
    validators::{Coercer, ValueValidator},
};
use crate::{
    core::{ATORS_MEMBERS, BaseAtors},
    member::PreGetattrBehavior,
};
use crate::{
    member::{
        DefaultBehavior, Member, PostGetattrBehavior, PostSetattrBehavior, PreSetattrBehavior,
    },
    validators::CoercionMode,
};

static ATORS_SPECIFIC_MEMBERS: &str = "__ators_specific_members__";
static ATORS_METHODS: &str = "__ators_methods__";
static ATORS_FROZEN: &str = "__ators_frozen__";

fn mro_from_bases<'py>(bases: &Bound<'py, PyTuple>) -> PyResult<Vec<Bound<'py, PyType>>> {
    // Collect the MRO of all the base classes
    let mut inputs: Vec<Bound<'py, PyTuple>> = bases
        .iter()
        .map(|b| -> PyResult<Bound<'py, PyTuple>> { Ok(b.cast()?.mro()) })
        .collect::<PyResult<Vec<Bound<'py, PyTuple>>>>()?;

    // Container to store teh computed MRO
    let mut mro = Vec::new();

    while !inputs.is_empty() {
        let mut candidate: Option<Bound<'py, PyType>> = None;
        for imro in inputs.iter() {
            let temp = imro.get_item(0)?;
            if inputs
                .iter()
                .any(|imro| imro.get_slice(1, imro.len()).contains(&temp).unwrap())
            {
                candidate = None;
            } else {
                candidate = Some(temp.cast_into()?);
                break;
            }
        }

        if let Some(type_) = candidate.take() {
            for imro in inputs.iter_mut() {
                if imro.get_item(0)?.is(&type_) {
                    imro.del_item(0).unwrap();
                }
            }
            mro.push(type_);
            inputs.retain(|item| item.len() != 0);
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
        Ok(self.next_index)
    }
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

    let mro = mro_from_bases(&bases)?;

    // Store whether or not the instance should be frozen after creation.
    dct.set_item(intern!(py, ATORS_FROZEN), frozen)?;

    // Since all classes deriving from Ators are slotted, we only need to check
    // for non-empty slots to know if a base class supports weakrefs.
    if enable_weakrefs && !mro.iter().any(|b| b.hasattr(slot_name).unwrap()) {
        dct.set_item(slot_name, (intern!(py, "__weakref__"),))?;
    } else {
        dct.set_item(slot_name, ())?;
    }

    let mut member_builders =
        generate_member_builders_from_cls_namespace(&name, &dct, type_containers)?;

    // Gather the name of the methods defined on the base classes.
    let methods = PySet::empty(py)?;
    for base in bases.iter() {
        // Methods are stored as a frozenset so we can safely iterate over it.
        for method_name in base.getattr(ATORS_METHODS)?.as_any().try_iter()? {
            methods.add(method_name?)?;
        }
    }

    // Walk the mro of the class, excluding itself, in reverse order collecting
    // all of the members into a single dict. The reverse update preserves the
    // mro of overridden members. We use only known specific members to also
    // preserve the mro in presence of multiple inheritance.
    let bat = py.get_type::<BaseAtors>();
    let mut members = HashMap::new();
    for base in mro.iter().skip(1).rev() {
        if base.is_subclass(&bat)? && !base.is(&bat) {
            let spm = base.getattr(ATORS_SPECIFIC_MEMBERS)?;
            members.extend(
                base.getattr(ATORS_MEMBERS)?
                    .cast::<PyDict>()?
                    .iter()
                    // SAFETY we know k is a string and that checking if it is in
                    // the set of specific member is safe.
                    .filter(|(k, _)| spm.contains(k).unwrap())
                    .map(|(k, v)| {
                        (
                            k.extract::<String>().unwrap(),
                            v.cast_into::<Member>().unwrap(),
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
        let name = { cm.borrow().name.clone() };
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
    for (k, v) in dct.iter() {
        if v.is_exact_instance_of::<PyFunction>() {
            methods.add(k)?;
        } else if let Ok(mb) = v.cast_into::<MemberBuilder>() {
            members.insert(k.extract()?, mb.extract()?);
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

        // XXX nice error messages
        // Ensure all the method the members are using do exist.
        if let Some(PreGetattrBehavior::ObjectMethod { meth_name }) = &mb.pre_getattr
            && !methods.contains(meth_name.bind(py))?
        {}
        if let Some(PostGetattrBehavior::ObjectMethod { meth_name }) = &mb.post_getattr
            && !methods.contains(meth_name.bind(py))?
        {}
        if let Some(PreSetattrBehavior::ObjectMethod { meth_name }) = &mb.pre_setattr
            && !methods.contains(meth_name.bind(py))?
        {}
        if let Some(PostSetattrBehavior::ObjectMethod { meth_name }) = &mb.post_setattr
            && !methods.contains(meth_name.bind(py))?
        {}
        if let Some(DefaultBehavior::ObjectMethod { meth_name }) = &mb.default
            && !methods.contains(meth_name.bind(py))?
        {}
        if let Some(meth_name) = match &mb.coercer {
            Some(CoercionMode::Coerce(Coercer::ObjectMethod { meth_name })) => Some(meth_name),
            Some(CoercionMode::Init(Coercer::ObjectMethod { meth_name })) => Some(meth_name),
            _ => None,
        } && !methods.contains(meth_name.bind(py))?
        {}
        for vv in mb.value_validators.as_ref().unwrap_or(&Vec::new()) {
            if let ValueValidator::ObjectMethod { meth_name } = vv
                && !methods.contains(meth_name.bind(py))?
            {}
        }
    }

    dct.update(
        member_builders
            .into_iter()
            // SAFETY The above logic guarantee the name and slot_index are set so
            // unwrapping on build is safe
            .map(|(k, v)| (k, v.clone().build().unwrap()))
            .into_py_dict(py)?
            .as_mapping(),
    )?;

    // Set the class level information as aggregated during the analysis
    dct.set_item(
        ATORS_SPECIFIC_MEMBERS,
        PyFrozenSet::new(py, specific_members)?,
    )?;
    dct.set_item(ATORS_METHODS, PyFrozenSet::new(py, methods)?)?;
    dct.set_item(crate::core::ATORS_MEMBERS, members)?;

    // Since the only slot we use is __weakref__ we do not need copyreg

    // Finally create the class
    meta.py_super()?
        .call_method1(intern!(py, "__new__"), (meta, name, bases, dct))
}
