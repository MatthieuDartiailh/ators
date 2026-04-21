/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
/// Callable validation decorators implemented in Rust.
use pyo3::{
    Bound, Py, PyAny, PyResult, Python, pyclass, pyfunction, pymethods,
    types::{PyAnyMethods, PyDict, PyDictMethods, PyTuple, PyTupleMethods},
};
use std::ffi::CString;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParamKind {
    Regular,
    VarPositional,
    VarKeyword,
}

#[derive(Clone, Debug)]
struct ParamPlan {
    name: String,
    kind: ParamKind,
    slot_name: String,
}

#[derive(Debug)]
struct CompiledPlan {
    signature: Py<PyAny>,
    validator_class: Py<PyAny>,
    params: Vec<ParamPlan>,
    return_slot: Option<String>,
    is_async: bool,
}

fn make_slot_name(raw: &str, index: usize) -> String {
    let mut safe = String::with_capacity(raw.len() + 16);
    for ch in raw.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            safe.push(ch);
        } else {
            safe.push('_');
        }
    }
    if safe.is_empty() {
        safe.push_str("value");
    }
    format!("_v_{index}_{safe}")
}

fn validate_single<'py>(
    instance: &Bound<'py, PyAny>,
    slot_name: &str,
    value: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    instance.setattr(slot_name, value)?;
    instance.getattr(slot_name)
}

fn build_localns<'py>(py: Python<'py>, args: &Bound<'py, PyTuple>) -> PyResult<Bound<'py, PyDict>> {
    let localns = PyDict::new(py);
    if !args.is_empty() {
        let first = args.get_item(0)?;
        let owner = if first.is_instance_of::<pyo3::types::PyType>() {
            first
        } else {
            first.get_type().into_any()
        };
        localns.set_item(owner.getattr("__name__")?, owner)?;
    }
    Ok(localns)
}

fn compile_plan<'py>(
    py: Python<'py>,
    target: &Bound<'py, PyAny>,
    validate_return: bool,
    args: &Bound<'py, PyTuple>,
) -> PyResult<CompiledPlan> {
    let inspect = py.import("inspect")?;
    let typing = py.import("typing")?;
    let builtins = py.import("builtins")?;
    let ators_mod = py.import("ators")?;
    let ext_mod = py.import("ators._ators")?;

    let signature = inspect.getattr("signature")?.call1((target,))?;
    let localns = build_localns(py, args)?;
    let globalns = target
        .getattr("__globals__")
        .unwrap_or_else(|_| PyDict::new(py).into_any());
    let th_kwargs = PyDict::new(py);
    th_kwargs.set_item("globalns", &globalns)?;
    th_kwargs.set_item("localns", &localns)?;
    th_kwargs.set_item("include_extras", true)?;

    let type_hints = typing
        .getattr("get_type_hints")?
        .call((target,), Some(&th_kwargs))?;

    let empty_ann = inspect.getattr("Signature")?.getattr("empty")?;
    let var_pos = inspect.getattr("Parameter")?.getattr("VAR_POSITIONAL")?;
    let var_kw = inspect.getattr("Parameter")?.getattr("VAR_KEYWORD")?;

    let annotations = PyDict::new(py);
    let namespace = PyDict::new(py);
    namespace.set_item("__annotations__", &annotations)?;

    let mut params = Vec::new();
    let parameters = signature.getattr("parameters")?;
    for (idx, item) in parameters.call_method0("items")?.try_iter()?.enumerate() {
        let tuple = item?.cast_into::<PyTuple>()?;
        let name_obj = tuple.get_item(0)?;
        let name = name_obj.extract::<String>()?;
        let param = tuple.get_item(1)?;

        let annotation = match type_hints.cast::<PyDict>()?.get_item(&name)? {
            Some(v) => v,
            None => param.getattr("annotation")?,
        };
        if annotation.is(&empty_ann) {
            continue;
        }

        let slot_name = make_slot_name(&name, idx);
        let member_kwargs = PyDict::new(py);
        member_kwargs.set_item("init", false)?;
        let member_builder = ext_mod.getattr("member")?.call((), Some(&member_kwargs))?;

        annotations.set_item(&slot_name, annotation)?;
        namespace.set_item(&slot_name, member_builder)?;

        let kind = {
            let k = param.getattr("kind")?;
            if k.is(&var_pos) {
                ParamKind::VarPositional
            } else if k.is(&var_kw) {
                ParamKind::VarKeyword
            } else {
                ParamKind::Regular
            }
        };

        params.push(ParamPlan {
            name,
            kind,
            slot_name,
        });
    }

    let mut return_slot = None;
    if validate_return {
        let return_annotation = match type_hints.cast::<PyDict>()?.get_item("return")? {
            Some(v) => v,
            None => signature.getattr("return_annotation")?,
        };
        if !return_annotation.is(&empty_ann) {
            let slot = "_v_return".to_string();
            let member_kwargs = PyDict::new(py);
            member_kwargs.set_item("init", false)?;
            let member_builder = ext_mod.getattr("member")?.call((), Some(&member_kwargs))?;
            annotations.set_item(&slot, return_annotation)?;
            namespace.set_item(&slot, member_builder)?;
            return_slot = Some(slot);
        }
    }

    let class_name = format!(
        "_CallableValidation_{}_{}",
        target.getattr("__name__")?.extract::<String>()?,
        target.as_ptr() as usize
    );
    let validator_class =
        builtins
            .getattr("type")?
            .call1((class_name, (ators_mod.getattr("Ators")?,), namespace))?;

    let is_async = inspect
        .getattr("iscoroutinefunction")?
        .call1((target,))?
        .extract()?;

    Ok(CompiledPlan {
        signature: signature.unbind(),
        validator_class: validator_class.unbind(),
        params,
        return_slot,
        is_async,
    })
}

fn aggregate_error(issues: &[(String, pyo3::PyErr)]) -> pyo3::PyErr {
    let mut details = String::new();
    for (idx, (location, err)) in issues.iter().enumerate() {
        let line = format!("- {location}: {err}\n");
        if idx == 0 {
            details.push_str(&line);
        } else {
            details.push_str(&line);
        }
    }
    pyo3::exceptions::PyTypeError::new_err(format!(
        "Callable validation failed with {} issue(s):\n{}",
        issues.len(),
        details.trim_end()
    ))
}

#[pyclass(module = "ators._ators")]
#[derive(Debug)]
pub struct CallableValidator {
    target: Py<PyAny>,
    aggregate_errors: bool,
    validate_return: bool,
    strict: bool,
}

impl CallableValidator {
    fn validate_return_with_plan<'py>(
        &self,
        py: Python<'py>,
        plan: &CompiledPlan,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if let Some(slot) = &plan.return_slot {
            let validator_instance = plan.validator_class.bind(py).call0()?;
            validate_single(&validator_instance, slot, value)
        } else {
            Ok(value.clone())
        }
    }
}

#[pymethods]
impl CallableValidator {
    #[new]
    pub fn new(
        target: Bound<'_, PyAny>,
        aggregate_errors: bool,
        validate_return: bool,
        strict: bool,
    ) -> Self {
        Self {
            target: target.unbind(),
            aggregate_errors,
            validate_return,
            strict,
        }
    }

    #[pyo3(signature = (*args, **kwargs))]
    fn __call__<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
        kwargs: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let target = self.target.bind(py);
        let plan = compile_plan(py, target, self.validate_return, args)?;

        let bound = plan.signature.bind(py).call_method("bind", args, kwargs)?;
        bound.call_method0("apply_defaults")?;

        let arguments = bound.getattr("arguments")?.cast_into::<PyDict>()?;
        let validator_instance = plan.validator_class.bind(py).call0()?;

        let mut issues: Vec<(String, pyo3::PyErr)> = Vec::new();
        for param in &plan.params {
            let current = arguments
                .get_item(&param.name)?
                .expect("Bound arguments should include validated parameters");

            match param.kind {
                ParamKind::VarPositional => {
                    let mut validated = Vec::new();
                    for (idx, item) in current.try_iter()?.enumerate() {
                        let item = item?;
                        match validate_single(&validator_instance, &param.slot_name, &item) {
                            Ok(v) => validated.push(v.into_any().unbind()),
                            Err(err) => {
                                if self.strict || !self.aggregate_errors {
                                    return Err(err);
                                }
                                issues.push((format!("{}[{idx}]", param.name), err));
                            }
                        }
                    }
                    arguments.set_item(&param.name, PyTuple::new(py, validated)?)?;
                }
                ParamKind::VarKeyword => {
                    let validated_kw = PyDict::new(py);
                    for kv in current.call_method0("items")?.try_iter()? {
                        let kv = kv?.cast_into::<PyTuple>()?;
                        let key = kv.get_item(0)?;
                        let item = kv.get_item(1)?;
                        let key_str = key
                            .extract::<String>()
                            .unwrap_or_else(|_| "<key>".to_string());
                        match validate_single(&validator_instance, &param.slot_name, &item) {
                            Ok(v) => {
                                validated_kw.set_item(key, v)?;
                            }
                            Err(err) => {
                                if self.strict || !self.aggregate_errors {
                                    return Err(err);
                                }
                                issues.push((format!("{}.{}", param.name, key_str), err));
                            }
                        }
                    }
                    arguments.set_item(&param.name, validated_kw)?;
                }
                ParamKind::Regular => {
                    match validate_single(&validator_instance, &param.slot_name, &current) {
                        Ok(v) => {
                            arguments.set_item(&param.name, v)?;
                        }
                        Err(err) => {
                            if self.strict || !self.aggregate_errors {
                                return Err(err);
                            }
                            issues.push((param.name.clone(), err));
                        }
                    }
                }
            }
        }

        if !issues.is_empty() {
            return Err(aggregate_error(&issues));
        }

        let call_args = bound.getattr("args")?.cast_into::<PyTuple>()?;
        let call_kwargs = bound.getattr("kwargs")?.cast_into::<PyDict>()?;
        let result = target.call(call_args, Some(&call_kwargs))?;

        if !self.validate_return || plan.return_slot.is_none() {
            return Ok(result);
        }

        if plan.is_async {
            let locals = PyDict::new(py);
            let validator_obj = Bound::new(
                py,
                CallableValidator {
                    target: self.target.clone_ref(py),
                    aggregate_errors: self.aggregate_errors,
                    validate_return: self.validate_return,
                    strict: self.strict,
                },
            )?;
            let code = CString::new(
                "async def _ators_await_and_validate(coro, validator):\n    result = await coro\n    return validator._validate_return_after_await(result)",
            )
            .expect("string literal should be a valid C string");
            py.run(code.as_c_str(), None, Some(&locals))?;
            let helper = locals
                .get_item("_ators_await_and_validate")?
                .expect("helper should be defined by run");
            return helper.call1((&result, &validator_obj));
        }

        self.validate_return_with_plan(py, &plan, &result)
    }

    fn _validate_return_after_await<'py>(
        &self,
        py: Python<'py>,
        value: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let empty = PyTuple::empty(py);
        let plan = compile_plan(py, self.target.bind(py), self.validate_return, &empty)?;
        self.validate_return_with_plan(py, &plan, value)
    }
}

fn make_wrapped_callable<'py>(
    py: Python<'py>,
    validator: Bound<'py, CallableValidator>,
    target: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let locals = PyDict::new(py);
    locals.set_item("__validator", validator)?;
    let wrapper_code =
        CString::new("(lambda *a, __validator=__validator, **k: __validator(*a, **k))")
            .expect("string literal should be a valid C string");
    let wrapper = py.eval(wrapper_code.as_c_str(), None, Some(&locals))?;

    let functools = py.import("functools")?;
    let wrapped = functools
        .getattr("wraps")?
        .call1((target,))?
        .call1((wrapper,))?;
    let inspect = py.import("inspect")?;
    wrapped.setattr(
        "__signature__",
        inspect.getattr("signature")?.call1((target,))?,
    )?;
    Ok(wrapped)
}

fn decorate_target<'py>(
    py: Python<'py>,
    target: &Bound<'py, PyAny>,
    aggregate_errors: bool,
    validate_return: bool,
    strict: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let builtins = py.import("builtins")?;
    let staticmethod_ty = builtins.getattr("staticmethod")?;
    if target.is_instance(&staticmethod_ty)? {
        let inner = target.getattr("__func__")?;
        let wrapped = decorate_target(py, &inner, aggregate_errors, validate_return, strict)?;
        return staticmethod_ty.call1((wrapped,));
    }

    let classmethod_ty = builtins.getattr("classmethod")?;
    if target.is_instance(&classmethod_ty)? {
        let inner = target.getattr("__func__")?;
        let wrapped = decorate_target(py, &inner, aggregate_errors, validate_return, strict)?;
        return classmethod_ty.call1((wrapped,));
    }

    if !target.is_callable() {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "validated can only be applied to callables",
        ));
    }

    let validator = Bound::new(
        py,
        CallableValidator::new(target.clone(), aggregate_errors, validate_return, strict),
    )?;
    make_wrapped_callable(py, validator, target)
}

#[pyfunction(signature=(func=None, *, aggregate_errors=true, validate_return=true, strict=false))]
pub fn validated<'py>(
    py: Python<'py>,
    func: Option<Bound<'py, PyAny>>,
    aggregate_errors: bool,
    validate_return: bool,
    strict: bool,
) -> PyResult<Bound<'py, PyAny>> {
    if let Some(target) = func {
        return decorate_target(py, &target, aggregate_errors, validate_return, strict);
    }

    let functools = py.import("functools")?;
    let module = py.import("ators._ators")?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("aggregate_errors", aggregate_errors)?;
    kwargs.set_item("validate_return", validate_return)?;
    kwargs.set_item("strict", strict)?;
    functools
        .getattr("partial")?
        .call((module.getattr("validated")?,), Some(&kwargs))
}
