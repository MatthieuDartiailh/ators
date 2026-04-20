/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
use std::collections::{HashMap, HashSet};

use pyo3::{Py, PyAny, Python, types::PyType};

use crate::{core::PicklePolicy, member::Member};

pub(crate) struct AtorsGenericInfo {
    origin: Option<Py<PyType>>,
    args: Vec<Py<PyAny>>,
    parameters: Vec<Py<PyAny>>,
}

impl AtorsGenericInfo {
    pub(crate) fn new(
        origin: Option<Py<PyType>>,
        args: Vec<Py<PyAny>>,
        parameters: Vec<Py<PyAny>>,
    ) -> Self {
        Self {
            origin,
            args,
            parameters,
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

    pub(crate) fn clone_ref(&self, py: Python<'_>) -> Self {
        Self {
            origin: self.origin.as_ref().map(|o| o.clone_ref(py)),
            args: self.args.iter().map(|a| a.clone_ref(py)).collect(),
            parameters: self.parameters.iter().map(|p| p.clone_ref(py)).collect(),
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
    specific_member_names: HashSet<String>,
    init_member_names: Vec<String>,
    required_init_member_names: Vec<String>,
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
        specific_member_names: HashSet<String>,
        init_member_names: Vec<String>,
        required_init_member_names: Vec<String>,
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
            specific_member_names,
            init_member_names,
            required_init_member_names,
            generic,
        }
    }

    pub(crate) fn with_generic(self, generic: Option<AtorsGenericInfo>) -> Self {
        Self { generic, ..self }
    }

    pub(crate) fn with_members(self, members_by_name: HashMap<String, Py<Member>>) -> Self {
        Self {
            members_by_name,
            ..self
        }
    }

    pub(crate) fn frozen(&self) -> bool {
        self.frozen
    }

    pub(crate) fn observable(&self) -> bool {
        self.observable
    }

    pub(crate) fn members_by_name(&self) -> &HashMap<String, Py<Member>> {
        &self.members_by_name
    }

    pub(crate) fn specific_member_names(&self) -> &HashSet<String> {
        &self.specific_member_names
    }

    pub(crate) fn init_member_names(&self) -> &[String] {
        &self.init_member_names
    }

    pub(crate) fn required_init_member_names(&self) -> &[String] {
        &self.required_init_member_names
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
            specific_member_names: self.specific_member_names.clone(),
            init_member_names: self.init_member_names.clone(),
            required_init_member_names: self.required_init_member_names.clone(),
            generic: self.generic.as_ref().map(|g| g.clone_ref(py)),
        }
    }
}
