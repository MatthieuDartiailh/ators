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
    Bound, Py, PyAny, PyErr, PyResult,
    ffi::c_str,
    intern, pyclass, pyfunction, pymethods,
    sync::PyOnceLock,
    types::{PyAnyMethods, PyDict, PyDictMethods, PyFrozenSet, PyString, PyTuple, PyType},
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

#[pyclass(module = "ators._ators", mapping)]
pub struct MembersByNameMapping {
    members_by_name: HashMap<String, Py<Member>>,
}

#[pyclass(module = "ators._ators")]
struct MembersByNameKeysIter {
    keys: Vec<Py<PyString>>,
    index: usize,
}

#[pymethods]
impl MembersByNameKeysIter {
    fn __iter__(slf: pyo3::PyRef<'_, Self>) -> pyo3::PyRef<'_, Self> {
        slf
    }

    fn __next__<'py>(mut slf: pyo3::PyRefMut<'py, Self>) -> Option<Bound<'py, PyString>> {
        let key = slf.keys.get(slf.index)?.clone_ref(slf.py());
        slf.index += 1;
        Some(key.bind(slf.py()).clone())
    }
}

#[pymethods]
impl MembersByNameMapping {
    fn __len__(&self) -> usize {
        self.members_by_name.len()
    }

    fn __getitem__<'py>(
        &self,
        py: pyo3::Python<'py>,
        key: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, Member>> {
        let key_name: String = key.extract()?;
        self.members_by_name
            .get(&key_name)
            .map(|member| member.bind(py).clone())
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(key_name))
    }

    fn __iter__(&self, py: pyo3::Python<'_>) -> PyResult<Py<MembersByNameKeysIter>> {
        Py::new(
            py,
            MembersByNameKeysIter {
                keys: self
                    .members_by_name
                    .keys()
                    .map(|k| PyString::new(py, k).unbind())
                    .collect(),
                index: 0,
            },
        )
    }
}

impl MembersByNameMapping {
    fn from_member_lookup(members_by_name: HashMap<String, Py<Member>>) -> Self {
        Self { members_by_name }
    }

    /// Return the number of members currently stored in the mapping.
    pub(crate) fn len(&self) -> usize {
        self.members_by_name.len()
    }

    /// Check whether a member with the provided name exists.
    pub(crate) fn contains_key(&self, name: &str) -> bool {
        self.members_by_name.contains_key(name)
    }

    /// Retrieve a member by name from the underlying Rust map.
    pub(crate) fn get(&self, name: &str) -> Option<&Py<Member>> {
        self.members_by_name.get(name)
    }

    /// Iterate over member-name/member-object pairs.
    pub(crate) fn iter(&self) -> std::collections::hash_map::Iter<'_, String, Py<Member>> {
        self.members_by_name.iter()
    }

    /// Iterate over member names.
    pub(crate) fn keys(&self) -> std::collections::hash_map::Keys<'_, String, Py<Member>> {
        self.members_by_name.keys()
    }
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
    pickle_policy: PicklePolicy,
    mutability: Option<ClassMutability>,
    members_by_name: Py<MembersByNameMapping>,
    specific_member_names: HashSet<String>,
    optional_init_member_names: Vec<Py<PyString>>,
    required_init_member_names: Vec<Py<PyString>>,
    method_names: HashSet<String>,
    generic: Option<AtorsGenericInfo>,
    customizer_tool: Option<Py<MemberCustomizationTool>>,
}

impl AtorsClassInfo {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        py: pyo3::Python<'_>,
        frozen: bool,
        observable: bool,
        pickle_policy: PicklePolicy,
        mutability: Option<ClassMutability>,
        members_by_name: HashMap<String, Py<Member>>,
        specific_member_names: HashSet<String>,
        optional_init_member_names: Vec<Py<PyString>>,
        required_init_member_names: Vec<Py<PyString>>,
        method_names: HashSet<String>,
        generic: Option<AtorsGenericInfo>,
        customizer_tool: Option<Py<MemberCustomizationTool>>,
    ) -> PyResult<Self> {
        let members_by_name = Py::new(
            py,
            MembersByNameMapping::from_member_lookup(members_by_name),
        )?;
        Ok(Self {
            frozen,
            observable,
            pickle_policy,
            mutability,
            members_by_name,
            specific_member_names,
            optional_init_member_names,
            required_init_member_names,
            method_names,
            generic,
            customizer_tool,
        })
    }

    pub(crate) fn with_generic(self, generic: Option<AtorsGenericInfo>) -> Self {
        Self { generic, ..self }
    }

    pub(crate) fn with_members(
        self,
        py: pyo3::Python<'_>,
        members_by_name: HashMap<String, Py<Member>>,
    ) -> PyResult<Self> {
        let members_by_name = Py::new(
            py,
            MembersByNameMapping::from_member_lookup(members_by_name),
        )?;
        Ok(Self {
            members_by_name,
            ..self
        })
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

    pub(crate) fn members_by_name(&self) -> &Py<MembersByNameMapping> {
        &self.members_by_name
    }

    pub(crate) fn members_by_name_ref<'py>(
        &self,
        py: pyo3::Python<'py>,
    ) -> pyo3::PyRef<'py, MembersByNameMapping> {
        self.members_by_name.bind(py).borrow()
    }

    pub(crate) fn specific_member_names(&self) -> &HashSet<String> {
        &self.specific_member_names
    }

    pub(crate) fn optional_init_member_names(&self) -> &[Py<PyString>] {
        &self.optional_init_member_names
    }

    pub(crate) fn required_init_member_names(&self) -> &[Py<PyString>] {
        &self.required_init_member_names
    }

    pub(crate) fn is_init_member_name(&self, py: pyo3::Python<'_>, name: &str) -> bool {
        self.members_by_name
            .bind(py)
            .borrow()
            .get(name)
            .map(|member| member.bind(py).get().init)
            .unwrap_or(false)
    }

    pub(crate) fn method_names(&self) -> &HashSet<String> {
        &self.method_names
    }

    pub(crate) fn generic(&self) -> Option<&AtorsGenericInfo> {
        self.generic.as_ref()
    }
}

// Python exposed functions to access class info.

/// Return the mapping of member names to `Member` descriptors for an Ators class.
#[pyfunction]
pub fn get_ators_members_by_name<'py>(cls: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyAny>> {
    let info = get_class_info(cls)?;
    Ok(info.members_by_name().bind(cls.py()).clone().into_any())
}

/// Return the set of member names defined specifically on `cls`.
#[pyfunction]
pub fn get_ators_specific_member_names<'py>(
    cls: &Bound<'py, PyType>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = cls.py();
    let info = get_class_info(cls)?;
    Ok(PyFrozenSet::new(py, info.specific_member_names())?.into_any())
}

/// Return the tuple of init-participating member names for `cls`.
#[pyfunction]
pub fn get_ators_init_member_names<'py>(cls: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyAny>> {
    let py = cls.py();
    let info = get_class_info(cls)?;
    let init_member_names: Vec<Bound<'_, PyString>> = info
        .required_init_member_names()
        .iter()
        .chain(info.optional_init_member_names().iter())
        .map(|name| name.bind(py).clone())
        .collect();
    Ok(PyTuple::new(py, init_member_names)?.into_any())
}

/// Return whether `cls` is configured as frozen.
#[pyfunction]
pub fn get_ators_frozen_flag(cls: &Bound<'_, PyType>) -> PyResult<bool> {
    Ok(get_class_info(cls)?.frozen())
}

/// Return the generic origin class for `cls`, or `None` if unspecialized.
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

/// Return specialization type arguments for `cls`, or `None` when unavailable.
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

/// Return generic type parameters for `cls`, or `None` for non-generic classes.
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

/// Create (or reuse) a specialized Ators class for generic parameters.
///
/// This is the Python-facing entry point used by `AtorsMeta.__getitem__`.
#[pyfunction]
pub fn create_ators_specialized_alias<'py>(
    cls: &Bound<'py, PyType>,
    params: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let specialized = crate::meta::create_ators_specialized_subclass(cls, params)?;
    let specialized_type = specialized.cast::<PyType>()?;
    wrap_ators_specialized_class(specialized_type)
}

struct ClassInfoStore {
    definitive: HashMap<usize, Arc<AtorsClassInfo>>,
    temporary: HashMap<String, Arc<AtorsClassInfo>>,
    pending_specialization_bindings: HashMap<String, Py<PyDict>>,
    alias_by_specialized: HashMap<usize, Py<PyAny>>,
    specialized_by_alias: HashMap<usize, Py<PyType>>,
}

impl ClassInfoStore {
    fn new() -> Self {
        Self {
            definitive: HashMap::new(),
            temporary: HashMap::new(),
            pending_specialization_bindings: HashMap::new(),
            alias_by_specialized: HashMap::new(),
            specialized_by_alias: HashMap::new(),
        }
    }
}

static CLASS_INFO_STORE: PyOnceLock<RwLock<ClassInfoStore>> = PyOnceLock::new();
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
            "_rust_instancecheck",
            pyo3::wrap_pyfunction!(crate::meta::rust_instancecheck, py)?,
        )?;
        locals.set_item(
            "_rust_subclasscheck",
            pyo3::wrap_pyfunction!(crate::meta::rust_subclasscheck, py)?,
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
fn class_fqname_from_inputs<'py>(
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
pub(crate) fn insert_temp_class_info<'py>(
    py: pyo3::Python<'py>,
    name: &Bound<'py, PyString>,
    dct: &Bound<'py, PyDict>,
    info: AtorsClassInfo,
) -> PyResult<String> {
    let fqname = class_fqname_from_inputs(name, dct)?;
    let store = get_class_info_store(py);
    store
        .write()
        .expect("Class info store write lock poisoned")
        .temporary
        .insert(fqname.clone(), Arc::new(info));
    Ok(fqname)
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
pub(crate) fn insert_pending_specialization_bindings(
    py: pyo3::Python<'_>,
    fqname: String,
    bindings: Py<PyDict>,
) {
    let store = get_class_info_store(py);
    store
        .write()
        .expect("Class info store write lock poisoned")
        .pending_specialization_bindings
        .insert(fqname, bindings);
}

#[inline]
pub(crate) fn take_pending_specialization_bindings(
    py: pyo3::Python<'_>,
    fqname: &str,
) -> Option<Py<PyDict>> {
    let store = get_class_info_store(py);
    store
        .write()
        .expect("Class info store write lock poisoned")
        .pending_specialization_bindings
        .remove(fqname)
}

#[inline]
pub(crate) fn take_pending_specialization_bindings_for_inputs<'py>(
    py: pyo3::Python<'py>,
    name: &Bound<'py, PyString>,
    dct: &Bound<'py, PyDict>,
) -> PyResult<Option<Py<PyDict>>> {
    let fqname = class_fqname_from_inputs(name, dct)?;
    Ok(take_pending_specialization_bindings(py, &fqname))
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
        Ok(Arc::clone(info))
    } else {
        Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
            "No Ators class info registered for {fqname}"
        )))
    }
}

/// Remove class metadata for `cls` from all class-info stores.
#[pyfunction]
pub fn drop_class_info(cls: &Bound<'_, PyType>) {
    let py = cls.py();
    let key = class_key(cls);
    let fqname = class_fqname(cls).ok();
    let store = get_class_info_store(py);
    let mut store = store.write().expect("Class info store write lock poisoned");
    store.definitive.remove(&key);
    if let Some(alias) = store.alias_by_specialized.remove(&key) {
        store.specialized_by_alias.remove(&alias.as_ptr().addr());
    }
    if let Some(fqname) = fqname {
        store.temporary.remove(&fqname);
        store.pending_specialization_bindings.remove(&fqname);
    }
}

/// Return the backing specialized Ators class for an AtorsGenericAlias instance.
#[pyfunction]
pub fn get_ators_specialized_class_for_alias<'py>(
    alias: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyType>> {
    let key = alias.as_ptr().addr();
    let store = get_class_info_store(alias.py());
    let store = store.read().expect("Class info store read lock poisoned");
    store
        .specialized_by_alias
        .get(&key)
        .map(|cls| cls.bind(alias.py()).clone())
        .ok_or_else(|| {
            pyo3::exceptions::PyTypeError::new_err(
                "Expected an AtorsGenericAlias created by Ators specialization",
            )
        })
}

/// Alias-aware instance check using the backing specialized Ators class.
#[pyfunction]
pub fn rust_instancecheck_alias<'py>(
    alias: &Bound<'py, PyAny>,
    instance: &Bound<'py, PyAny>,
) -> PyResult<bool> {
    let cls = get_ators_specialized_class_for_alias(alias)?;
    crate::meta::rust_instancecheck(&cls, instance)
}

/// Alias-aware subclass check using the backing specialized Ators class.
#[pyfunction]
pub fn rust_subclasscheck_alias<'py>(
    alias: &Bound<'py, PyAny>,
    sub: &Bound<'py, PyAny>,
) -> PyResult<bool> {
    let cls = get_ators_specialized_class_for_alias(alias)?;
    let sub_cls =
        get_ators_specialized_class_for_alias(sub).or_else(|_| sub.cast::<PyType>().cloned())?;
    crate::meta::rust_subclasscheck(&cls, &sub_cls)
}

/// Wrap a specialized Ators class into a cached AtorsGenericAlias.
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

/// Return the number of definitive class-info entries currently tracked.
#[pyfunction]
pub fn get_tracked_class_info_size(py: pyo3::Python<'_>) -> usize {
    // Keep this debugging helper deterministic across test runs by forcing a
    // collection cycle before counting tracked classes.
    let _ = py
        .import(intern!(py, "gc"))
        .and_then(|gc| gc.call_method0(intern!(py, "collect")));
    let store = get_class_info_store(py);
    store
        .read()
        .expect("Class info store read lock poisoned")
        .definitive
        .len()
}
