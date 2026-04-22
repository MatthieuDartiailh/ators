/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use pyo3::{
    Bound, Py, PyAny, PyResult, Python, intern, pyclass, pyfunction,
    sync::PyOnceLock,
    types::{
        PyAnyMethods, PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple, PyType, PyTypeMethods,
    },
};

use crate::member::{Member, MemberCustomizationTool};

#[pyclass(module = "ators._ators", frozen, from_py_object)]
#[derive(Debug, Clone)]
pub enum ClassMutability {
    #[pyo3(constructor = ())]
    Immutable {},
    #[pyo3(constructor = ())]
    Mutable {},
    #[pyo3(constructor = (values))]
    InspectValues { values: Vec<String> },
}

#[pyclass(module = "ators._ators", frozen, from_py_object, eq, eq_int)]
#[derive(Debug, Clone, PartialEq)]
pub enum PicklePolicy {
    /// Include all members in pickle state (default).
    #[pyo3(name = "ALL")]
    All,
    /// Exclude all members from pickle state.
    #[pyo3(name = "NONE")]
    None,
    /// Include only public members (those not starting with `_`) in pickle state.
    #[pyo3(name = "PUBLIC")]
    Public,
}

pub(crate) struct AtorsGenericInfo {
    /// The type parameters of the generic class, if any.  This is used to
    /// support unspecialized generic classes
    /// (e.g. `class MyGeneric(Generic[T])`), which have generic metadata but
    /// no origin/args specialization.
    type_parameters: Vec<Py<PyAny>>,
    /// The original generic class that was specialized to produce the current
    /// class, if any.
    origin: Option<Py<PyType>>,
    /// The actual type arguments used in the specialization, if any.
    args: Vec<Py<PyAny>>,
    /// The mapping of type variables to their bound values in this
    /// specialization, if any.
    typevar_bindings: Option<Py<PyDict>>,
    /// The mapping of type variables to their specialized values for any
    /// known specializations of this generic class, if any.  This is used to
    /// support partial specialization of already specialized generic classes
    /// (e.g. `MyGeneric[int]`), which may have further specializations
    /// (e.g. `MyGeneric[int][str]`) that require knowledge of the original t
    /// type parameters and their bindings.
    specializations: Option<Py<PyDict>>,
}

impl AtorsGenericInfo {
    pub(crate) fn new(
        type_parameters: Vec<Py<PyAny>>,
        origin: Option<Py<PyType>>,
        args: Vec<Py<PyAny>>,
        typevar_bindings: Option<Py<PyDict>>,
        specializations: Option<Py<PyDict>>,
    ) -> Self {
        Self {
            type_parameters,
            origin,
            args,
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

    pub(crate) fn type_parameters(&self) -> &[Py<PyAny>] {
        &self.type_parameters
    }

    pub(crate) fn typevar_bindings(&self) -> Option<&Py<PyDict>> {
        self.typevar_bindings.as_ref()
    }

    pub(crate) fn specializations(&self) -> Option<&Py<PyDict>> {
        self.specializations.as_ref()
    }
}

pub(crate) struct AtorsClassInfo {
    frozen: bool,
    observable: bool,
    enable_weakrefs: bool,
    validate_attr: bool,
    type_containers: i64,
    pickle_policy: PicklePolicy,
    mutability: Option<ClassMutability>,
    members_by_name: HashMap<String, Py<Member>>,
    specific_member_names: HashSet<String>,
    optional_init_member_names: Vec<String>,
    required_init_member_names: Vec<String>,
    method_names: HashSet<String>,
    generic: Option<AtorsGenericInfo>,
    customizer_tool: Option<Py<MemberCustomizationTool>>,
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
        mutability: Option<ClassMutability>,
        members_by_name: HashMap<String, Py<Member>>,
        specific_member_names: HashSet<String>,
        optional_init_member_names: Vec<String>,
        required_init_member_names: Vec<String>,
        method_names: HashSet<String>,
        generic: Option<AtorsGenericInfo>,
        customizer_tool: Option<Py<MemberCustomizationTool>>,
    ) -> Self {
        Self {
            frozen,
            observable,
            enable_weakrefs,
            validate_attr,
            type_containers,
            pickle_policy,
            mutability,
            members_by_name,
            specific_member_names,
            optional_init_member_names,
            required_init_member_names,
            method_names,
            generic,
            customizer_tool,
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

    pub(crate) fn with_mutability(self, mutability: Option<ClassMutability>) -> Self {
        Self { mutability, ..self }
    }

    pub(crate) fn customizer(&self) -> Option<&Py<MemberCustomizationTool>> {
        self.customizer_tool.as_ref()
    }

    pub(crate) fn take_customizer(&mut self) -> Py<MemberCustomizationTool> {
        self.customizer_tool
            .take()
            .expect("Member customizer should be set at this point")
    }

    pub(crate) fn frozen(&self) -> bool {
        self.frozen
    }

    pub(crate) fn observable(&self) -> bool {
        self.observable
    }

    pub(crate) fn mutability(&self) -> Option<&ClassMutability> {
        self.mutability.as_ref()
    }

    pub(crate) fn pickle_policy(&self) -> &PicklePolicy {
        &self.pickle_policy
    }

    pub(crate) fn members_by_name(&self) -> &HashMap<String, Py<Member>> {
        &self.members_by_name
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
}

// Python exposed functions to access class info.
// XXX audit

#[pyfunction]
pub fn get_ators_members_by_name<'py>(cls: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyAny>> {
    let py = cls.py();
    let info = get_class_info(cls)?;
    let members_dict = info.members_by_name();
    py.import(intern!(py, "types"))?
        .getattr(intern!(py, "MappingProxyType"))?
        .call1((members_dict,))
}

#[pyfunction]
pub fn get_ators_specific_member_names<'py>(
    cls: &Bound<'py, PyType>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = cls.py();
    let info = get_class_info(cls)?;
    Ok(PyFrozenSet::new(py, info.specific_member_names())?.into_any())
}

#[pyfunction]
pub fn get_ators_init_member_names<'py>(cls: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyAny>> {
    let info = get_class_info(cls)?;
    let init_member_names: Vec<&String> = info
        .required_init_member_names()
        .iter()
        .chain(info.optional_init_member_names().iter())
        .collect();
    Ok(PyTuple::new(cls.py(), init_member_names)?.into_any())
}

#[pyfunction]
pub fn get_ators_frozen_flag(cls: &Bound<'_, PyType>) -> PyResult<bool> {
    Ok(get_class_info(cls)?.frozen())
}

#[pyfunction]
pub fn get_ators_origin<'py>(cls: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyAny>> {
    let py = cls.py();
    let info = get_class_info(cls)?;
    Ok(info
        .generic()
        .and_then(|generic| generic.origin())
        .map_or_else(
            || py.None().into_bound(py),
            |origin| origin.bind(py).clone().into_any(),
        ))
}

#[pyfunction]
pub fn get_ators_args<'py>(cls: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyAny>> {
    let py = cls.py();
    let info = get_class_info(cls)?;
    match info.generic() {
        Some(generic) if generic.origin().is_some() => {
            PyTuple::new(py, generic.args().iter().map(|a| a.bind(py))).map(|t| t.into_any())
        }
        // Unspecialized generic class: generic metadata is present for
        // parameters, but there is no origin/args specialization yet.
        None | Some(_) => Ok(py.None().into_bound(py)),
    }
}

#[pyfunction]
pub fn get_ators_type_params<'py>(cls: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyAny>> {
    let py = cls.py();
    let info = get_class_info(cls)?;
    match info.generic() {
        Some(generic) => PyTuple::new(py, generic.type_parameters().iter().map(|p| p.bind(py)))
            .map(|t| t.into_any()),
        None => Ok(py.None().into_bound(py)),
    }
}

#[inline]
fn get_ators_args_tuple<'py>(cls: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyTuple>> {
    let info = get_class_info(cls)?;
    let Some(generic) = info.generic().filter(|g| g.origin().is_some()) else {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "{} is not a specialised generic Ators class",
            cls.qualname()?
        )));
    };
    PyTuple::new(cls.py(), generic.args().iter().map(|a| a.bind(cls.py())))
}

struct ClassInfoStore {
    definitive: HashMap<usize, Arc<AtorsClassInfo>>,
    temporary: HashMap<String, Arc<AtorsClassInfo>>,
}

impl ClassInfoStore {
    fn new() -> Self {
        Self {
            definitive: HashMap::new(),
            temporary: HashMap::new(),
        }
    }
}

static CLASS_INFO_STORE: PyOnceLock<RwLock<ClassInfoStore>> = PyOnceLock::new();

#[inline]
fn get_class_info_store<'py>(py: pyo3::Python<'py>) -> &'py RwLock<ClassInfoStore> {
    CLASS_INFO_STORE.get_or_init(py, || RwLock::new(ClassInfoStore::new()))
}

#[inline]
fn class_key(cls: &Bound<'_, PyType>) -> usize {
    cls.as_ptr().addr()
}

#[inline]
fn class_fqname<'py>(cls: &Bound<'py, PyType>) -> PyResult<String> {
    let py = cls.py();
    let module: String = cls.getattr(intern!(py, "__module__"))?.extract()?;
    let qualname: String = cls.getattr(intern!(py, "__qualname__"))?.extract()?;
    Ok(format!("{module}.{qualname}"))
}

#[inline]
pub(crate) fn class_fqname_from_inputs<'py>(
    name: &Bound<'py, PyString>,
    dct: &Bound<'py, PyDict>,
) -> PyResult<String> {
    let py = name.py();
    let module = dct
        .get_item(intern!(py, "__module__"))?
        .map_or(Ok(String::from("<unknown>")), |m| m.extract::<String>())?;
    let qualname = dct
        .get_item(intern!(py, "__qualname__"))?
        .map_or_else(|| name.extract::<String>(), |q| q.extract::<String>())?;
    Ok(format!("{module}.{qualname}"))
}

#[inline]
pub(crate) fn insert_temp_class_info(py: pyo3::Python<'_>, fqname: String, info: AtorsClassInfo) {
    let store = get_class_info_store(py);
    store
        .write()
        .expect("Class info store write lock poisoned")
        .temporary
        .insert(fqname, Arc::new(info));
}

/// Pop a temporary class info from the store by fully qualified name.
///
/// This should only be used once per class, and only for classes that are in
/// the process of being created (i.e. before the info is transferred to the
/// definitive store).  Panics if the class info is not found or if there are
/// multiple strong references to the info (which would indicate a logic error
/// in the creation process).
#[inline]
pub(crate) fn pop_temp_class_info(py: pyo3::Python<'_>, fqname: &str) -> AtorsClassInfo {
    let store = get_class_info_store(py);
    Arc::into_inner(
        store
            .write()
            .expect("Class info store write lock poisoned")
            .temporary
            .remove(fqname)
            .expect("Class info is known to be present at this point."),
    )
    .expect("No other strong reference should exists.")
}

#[inline]
pub(crate) fn insert_definitive_class_info(
    py: pyo3::Python<'_>,
    cls: &Bound<'_, PyType>,
    info: AtorsClassInfo,
) {
    let store = get_class_info_store(py);
    store
        .write()
        .expect("Class info store write lock poisoned")
        .definitive
        .insert(class_key(cls), Arc::new(info));
}

#[inline]
pub(crate) fn pop_definitive_class_info(
    py: pyo3::Python<'_>,
    cls: &Bound<'_, PyType>,
) -> AtorsClassInfo {
    let key = class_key(cls);
    let store = get_class_info_store(py);
    Arc::into_inner(
        store
            .write()
            .expect("Class info store write lock poisoned")
            .definitive
            .remove(&key)
            .expect("Class info is known to be present at this point."),
    )
    .expect("No other strong reference should exists.")
}

pub(crate) fn get_class_info<'py>(cls: &Bound<'py, PyType>) -> PyResult<Arc<AtorsClassInfo>> {
    let key = class_key(cls);
    let store = get_class_info_store(cls.py());
    if let Some(info) = store
        .read()
        .expect("Class info store read lock poisoned")
        .definitive
        .get(&key)
    {
        return Ok(Arc::clone(info));
    }
    let fqname = class_fqname(cls)?;
    if let Some(info) = store
        .read()
        .expect("Class info store read lock poisoned")
        .temporary
        .get(&fqname)
    {
        return Ok(Arc::clone(info));
    } else {
        Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
            "No Ators class info registered for {fqname}"
        )))
    }
}
