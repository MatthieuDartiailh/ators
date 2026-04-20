/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
use std::collections::{HashMap, HashSet};

use pyo3::{
    Py, PyAny, Python,
    types::{PyDict, PyDictMethods, PyType},
};

use crate::{core::PicklePolicy, member::Member};

pub(crate) struct AtorsGenericInfo {
    origin: Option<Py<PyType>>,
    args: Vec<Py<PyAny>>,
    parameters: Vec<Py<PyAny>>,
    typevar_bindings: Option<Py<PyDict>>,
    specializations: Option<Py<PyDict>>,
}

impl AtorsGenericInfo {
    pub(crate) fn new(
        origin: Option<Py<PyType>>,
        args: Vec<Py<PyAny>>,
        parameters: Vec<Py<PyAny>>,
        typevar_bindings: Option<Py<PyDict>>,
        specializations: Option<Py<PyDict>>,
    ) -> Self {
        Self {
            origin,
            args,
            parameters,
            typevar_bindings,
            specializations,
        }
    }

    pub(crate) fn origin(&self) -> Option<&Py<PyType>> {
        self.origin.as_ref()
    }

    pub(crate) fn args(&self) -> &[Py<PyAny>] {
        &self.args
    }

    pub(crate) fn parameters(&self) -> &[Py<PyAny>] {
        &self.parameters
    }

    pub(crate) fn typevar_bindings(&self) -> Option<&Py<PyDict>> {
        self.typevar_bindings.as_ref()
    }

    pub(crate) fn specializations(&self) -> Option<&Py<PyDict>> {
        self.specializations.as_ref()
    }

    pub(crate) fn clone_ref(&self, py: Python<'_>) -> Self {
        Self {
            origin: self.origin.as_ref().map(|o| o.clone_ref(py)),
            args: self.args.iter().map(|a| a.clone_ref(py)).collect(),
            parameters: self.parameters.iter().map(|p| p.clone_ref(py)).collect(),
            typevar_bindings: self.typevar_bindings.as_ref().map(|m| m.clone_ref(py)),
            specializations: self.specializations.as_ref().map(|m| m.clone_ref(py)),
        }
    }
}

pub(crate) struct AtorsClassInfo {
    frozen: bool,
    observable: bool,
    enable_weakrefs: bool,
    validate_attr: bool,
    type_containers: i64,
    pickle_policy: PicklePolicy,
    members_by_name: HashMap<String, Py<Member>>,
    members_dict: Py<PyDict>,
    specific_member_names: HashSet<String>,
    optional_init_member_names: Vec<String>,
    required_init_member_names: Vec<String>,
    method_names: HashSet<String>,
    generic: Option<AtorsGenericInfo>,
}

impl AtorsClassInfo {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        frozen: bool,
        observable: bool,
        enable_weakrefs: bool,
        validate_attr: bool,
        type_containers: i64,
        pickle_policy: PicklePolicy,
        members_by_name: HashMap<String, Py<Member>>,
        members_dict: Py<PyDict>,
        specific_member_names: HashSet<String>,
        optional_init_member_names: Vec<String>,
        required_init_member_names: Vec<String>,
        method_names: HashSet<String>,
        generic: Option<AtorsGenericInfo>,
    ) -> Self {
        Self {
            frozen,
            observable,
            enable_weakrefs,
            validate_attr,
            type_containers,
            pickle_policy,
            members_by_name,
            members_dict,
            specific_member_names,
            optional_init_member_names,
            required_init_member_names,
            method_names,
            generic,
        }
    }

    pub(crate) fn with_generic(self, generic: Option<AtorsGenericInfo>) -> Self {
        Self { generic, ..self }
    }

    pub(crate) fn with_members(
        self,
        py: Python<'_>,
        members_by_name: HashMap<String, Py<Member>>,
    ) -> Self {
        let members_dict = PyDict::new(py);
        for (name, member) in &members_by_name {
            members_dict
                .set_item(name, member.bind(py))
                .expect("Failed to build members dict from members_by_name");
        }
        Self {
            members_by_name,
            members_dict: members_dict.unbind(),
            ..self
        }
    }

    pub(crate) fn frozen(&self) -> bool {
        self.frozen
    }

    pub(crate) fn observable(&self) -> bool {
        self.observable
    }

    pub(crate) fn pickle_policy(&self) -> &PicklePolicy {
        &self.pickle_policy
    }

    pub(crate) fn members_by_name(&self) -> &HashMap<String, Py<Member>> {
        &self.members_by_name
    }

    pub(crate) fn members_dict(&self) -> &Py<PyDict> {
        &self.members_dict
    }

    pub(crate) fn specific_member_names(&self) -> &HashSet<String> {
        &self.specific_member_names
    }

    pub(crate) fn optional_init_member_names(&self) -> &[String] {
        &self.optional_init_member_names
    }

    pub(crate) fn required_init_member_names(&self) -> &[String] {
        &self.required_init_member_names
    }

    pub(crate) fn init_member_count(&self) -> usize {
        self.optional_init_member_names.len() + self.required_init_member_names.len()
    }

    pub(crate) fn is_init_member(&self, name: &str) -> bool {
        self.required_init_member_names.iter().any(|n| n == name)
            || self.optional_init_member_names.iter().any(|n| n == name)
    }

    pub(crate) fn method_names(&self) -> &HashSet<String> {
        &self.method_names
    }

    pub(crate) fn generic(&self) -> Option<&AtorsGenericInfo> {
        self.generic.as_ref()
    }

    pub(crate) fn clone_ref(&self, py: Python<'_>) -> Self {
        Self {
            frozen: self.frozen,
            observable: self.observable,
            enable_weakrefs: self.enable_weakrefs,
            validate_attr: self.validate_attr,
            type_containers: self.type_containers,
            pickle_policy: self.pickle_policy.clone(),
            members_by_name: self
                .members_by_name
                .iter()
                .map(|(k, v)| (k.clone(), v.clone_ref(py)))
                .collect(),
            members_dict: self.members_dict.clone_ref(py),
            specific_member_names: self.specific_member_names.clone(),
            optional_init_member_names: self.optional_init_member_names.clone(),
            required_init_member_names: self.required_init_member_names.clone(),
            method_names: self.method_names.clone(),
            generic: self.generic.as_ref().map(|g| g.clone_ref(py)),
        }
    }
}
