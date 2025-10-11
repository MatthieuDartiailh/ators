/*-----------------------------------------------------------------------------
| Copyright (c) 2025, Matthieu C. Dartiailh
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/

use std::collections::{HashMap, HashSet};

use pyo3::{
    Bound, Py, PyAny, PyResult, intern, pyfunction,
    types::{
        PyAnyMethods, PyDict, PyDictMethods, PyList, PyListMethods, PyString, PyTuple,
        PyTupleMethods, PyType,
    },
};

static ATORS_SPECIFIC_MEMBERS: &str = "__ators_specific_members__";
static ATORS_FROZEN: &str = "__ators_frozen__";

fn mro_from_bases<'py>(bases: Bound<'py, PyTuple>) -> PyResult<Vec<Bound<'py, PyType>>> {
    let py = bases.py();

    // Collect the MRO of all the base classes
    let mut inputs: Vec<Bound<'py, PyList>> = bases
        .iter()
        .map(|b| -> Bound<'py, PyList> {
            b.call_method0(intern!(py, "mro"))
                .unwrap()
                .cast_into()
                .unwrap()
        })
        .collect();

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
            inputs = inputs.into_iter().filter(|item| item.len() != 0).collect();
        } else {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                "Inconsistent class hierarchy with base classes {}",
                bases
            )));
        }
    }

    Ok(mro)
}

// XXX should provide a mapping from name to index to avoid modifying members
// multiple times
fn assign_member_indexes() {}

#[derive(Default)]
struct AtorsMetaHelper<'py> {
    members: HashMap<String, Bound<'py, crate::member::Member>>,
    owned_members: HashSet<Bound<'py, crate::member::Member>>,
    specific_members: HashSet<String>,
}

#[pyfunction]
fn create_ators_subclass<'py>(
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

    let mro = mro_from_bases(bases)?;
    let mut new_dct = PyDict::new(dct.py());

    // Store whether or not the instance should be frozen after creation.
    new_dct.set_item(intern!(py, ATORS_FROZEN), frozen);

    // Since all classes deriving from Ators are slotted, we only need to check
    // for non-empty slots to know if a base class supports weakrefs.
    if enable_weakrefs && !mro.iter().any(|b| b.hasattr(slot_name).unwrap()) {
        new_dct.set_item(slot_name, (intern!(py, "__weakref__"),))?;
    } else {
        new_dct.set_item(slot_name, ())?;
    }

    let member_builders = generate_members_from_cls_namespace(&name, &dct, &type_containers)?;

    // Create the helper used to analyze the namespace and customize members
    let helper = AtorsMetaHelper::default();

    // Further processing can be done here using `helper`

    // Set the class level information as aggregated during the analysis
    new_dct.set_item(ATORS_SPECIFIC_MEMBERS, helper.specific_members)?;
    new_dct.set_item(crate::core::ATORS_MEMBERS, helper.specific_members)?;

    // Since the only slot we use is __weakref__ we do not need copyreg

    meta.py_super()?
        .call_method1(intern!(py, "__new__"), (meta, name, bases, new_dct))
}
