/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
/// Callable validation decorators implemented in Rust.
use crate::{
    annotations::{build_validator_from_annotation, get_type_tools},
    member::Member,
    validators::Validator,
};
use pyo3::{
    Bound, Py, PyAny, PyResult, Python, pyclass, pyfunction, pymethods,
    sync::PyOnceLock,
    types::{PyAnyMethods, PyDict, PyDictMethods, PyString, PyTuple, PyTupleMethods},
};
use std::collections::HashMap;

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
    validator: Validator,
}

#[derive(Debug)]
struct SyncValidationPlan {
    signature: Py<PyAny>,
    params: Vec<ParamPlan>,
    defaults: HashMap<String, Py<PyAny>>,
    return_validator: Option<Validator>,
}

#[derive(Debug)]
struct AsyncValidationPlan {
    signature: Py<PyAny>,
    params: Vec<ParamPlan>,
    defaults: HashMap<String, Py<PyAny>>,
    return_validator: Option<Validator>,
}

#[derive(Debug)]
enum CompiledCallablePlan {
    Sync(SyncValidationPlan),
    Async(AsyncValidationPlan),
}

fn build_function_validator<'py>(
    py: Python<'py>,
    tools: &crate::annotations::TypeTools<'py>,
    name: &str,
    ann: &Bound<'py, PyAny>,
) -> PyResult<Validator> {
    let class_var = tools.class_var();
    let origin = tools.get_origin(ann)?;
    if origin.is(class_var) || ann.is(class_var) {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Invalid annotation for '{name}': ClassVar is not allowed in function annotations."
        )));
    }

    let member_type = py.get_type::<Member>();
    if origin.is(member_type.as_any()) {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Invalid annotation for '{name}': subscripted Member annotations are not supported in function annotations."
        )));
    }

    let (validator, _) = build_validator_from_annotation(
        PyString::new(py, name).cast()?,
        ann,
        0,
        tools,
        None,
        None,
    )?;
    Ok(validator)
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

fn validate_bound_arguments<'py>(
    py: Python<'py>,
    arguments: &Bound<'py, PyDict>,
    params: &[ParamPlan],
    defaults: &HashMap<String, Py<PyAny>>,
    strict: bool,
    aggregate_errors: bool,
) -> PyResult<()> {
    let mut issues: Vec<(String, pyo3::PyErr)> = Vec::new();
    for param in params {
        let current = match arguments.get_item(&param.name)? {
            Some(value) => value,
            None => match defaults.get(&param.name) {
                Some(default) => default.bind(py).clone(),
                None => continue,
            },
        };

        match param.kind {
            ParamKind::VarPositional => {
                let mut validated = Vec::new();
                for (idx, item) in current.try_iter()?.enumerate() {
                    let item = item?;
                    match param.validator.validate(Some(&param.name), None, &item) {
                        Ok(v) => validated.push(v.into_any().unbind()),
                        Err(err) => {
                            if strict || !aggregate_errors {
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
                    match param.validator.validate(Some(&param.name), None, &item) {
                        Ok(v) => {
                            validated_kw.set_item(key, v)?;
                        }
                        Err(err) => {
                            if strict || !aggregate_errors {
                                return Err(err);
                            }
                            issues.push((format!("{}.{}", param.name, key_str), err));
                        }
                    }
                }
                arguments.set_item(&param.name, validated_kw)?;
            }
            ParamKind::Regular => {
                match param.validator.validate(Some(&param.name), None, &current) {
                    Ok(v) => {
                        arguments.set_item(&param.name, v)?;
                    }
                    Err(err) => {
                        if strict || !aggregate_errors {
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
    Ok(())
}

static METHOD_TYPE: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

#[inline]
fn get_method_type<'py>(py: Python<'py>) -> &'py Bound<'py, PyAny> {
    METHOD_TYPE
        .import(py, "types", "MethodType")
        .expect("types.MethodType should always be present in the types module.")
}

#[pyclass(module = "ators._ators", dict)]
#[derive(Debug)]
pub struct SyncCallableValidator {
    target: Py<PyAny>,
    plan: SyncValidationPlan,
    aggregate_errors: bool,
    strict: bool,
}

#[pymethods]
impl SyncCallableValidator {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
        kwargs: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let target = self.target.bind(py);
        let bound = self
            .plan
            .signature
            .bind(py)
            .call_method("bind", args, kwargs)?;

        let arguments = bound.getattr("arguments")?.cast_into::<PyDict>()?;
        validate_bound_arguments(
            py,
            &arguments,
            &self.plan.params,
            &self.plan.defaults,
            self.strict,
            self.aggregate_errors,
        )?;

        let call_args = bound.getattr("args")?.cast_into::<PyTuple>()?;
        let call_kwargs = bound.getattr("kwargs")?.cast_into::<PyDict>()?;
        let result = target.call(call_args, Some(&call_kwargs))?;

        if let Some(return_validator) = &self.plan.return_validator {
            return return_validator.validate(Some("return"), None, &result);
        }
        Ok(result)
    }

    fn __get__<'py>(
        slf: Bound<'py, Self>,
        obj: Option<&Bound<'py, PyAny>>,
        _owner: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if let Some(instance) = obj {
            return get_method_type(slf.py()).call1((slf, instance));
        }
        Ok(slf.into_any())
    }
}

impl SyncCallableValidator {
    fn new(
        target: Bound<'_, PyAny>,
        plan: SyncValidationPlan,
        aggregate_errors: bool,
        strict: bool,
    ) -> Self {
        Self {
            target: target.unbind(),
            plan,
            aggregate_errors,
            strict,
        }
    }
}

#[pyclass(module = "ators._ators")]
#[derive(Debug)]
pub struct AsyncValidatedIterator {
    iterator: Py<PyAny>,
    return_validator: Validator,
}

fn extract_stop_iteration_value<'py>(
    py: Python<'py>,
    err: &pyo3::PyErr,
) -> PyResult<Bound<'py, PyAny>> {
    let args = err.value(py).getattr("args")?.cast_into::<PyTuple>()?;
    if args.is_empty() {
        Ok(py.None().into_bound(py))
    } else {
        args.get_item(0)
    }
}

fn handle_iterator_error<'py>(
    py: Python<'py>,
    err: pyo3::PyErr,
    return_validator: &Validator,
) -> PyResult<Bound<'py, PyAny>> {
    if err.is_instance_of::<pyo3::exceptions::PyStopIteration>(py) {
        let value = extract_stop_iteration_value(py, &err)?;
        let validated = return_validator.validate(Some("return"), None, &value)?;
        return Err(pyo3::PyErr::new::<pyo3::exceptions::PyStopIteration, _>((
            validated.unbind(),
        )));
    }
    Err(err)
}

#[pymethods]
impl AsyncValidatedIterator {
    fn __iter__(slf: Bound<'_, Self>) -> Bound<'_, Self> {
        slf
    }

    fn __next__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self.iterator.bind(py).call_method0("__next__") {
            Ok(v) => Ok(v),
            Err(err) => handle_iterator_error(py, err, &self.return_validator),
        }
    }

    fn send<'py>(&self, py: Python<'py>, value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        match self.iterator.bind(py).call_method1("send", (value,)) {
            Ok(v) => Ok(v),
            Err(err) => handle_iterator_error(py, err, &self.return_validator),
        }
    }

    #[pyo3(signature = (*args))]
    fn throw<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self.iterator.bind(py).call_method("throw", args, None) {
            Ok(v) => Ok(v),
            Err(err) => handle_iterator_error(py, err, &self.return_validator),
        }
    }

    fn close<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.iterator.bind(py).call_method0("close")
    }
}

#[pyclass(module = "ators._ators")]
#[derive(Debug)]
pub struct AsyncValidatedResult {
    awaitable: Py<PyAny>,
    return_validator: Validator,
}

#[pymethods]
impl AsyncValidatedResult {
    fn __await__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let iterator = self.awaitable.bind(py).call_method0("__await__")?;
        Ok(Bound::new(
            py,
            AsyncValidatedIterator {
                iterator: iterator.unbind(),
                return_validator: self.return_validator.clone(),
            },
        )?
        .into_any())
    }
}

#[pyclass(module = "ators._ators", dict)]
#[derive(Debug)]
pub struct AsyncCallableValidator {
    target: Py<PyAny>,
    plan: AsyncValidationPlan,
    aggregate_errors: bool,
    strict: bool,
}

#[pymethods]
impl AsyncCallableValidator {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
        kwargs: Option<&Bound<'py, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let target = self.target.bind(py);
        let bound = self
            .plan
            .signature
            .bind(py)
            .call_method("bind", args, kwargs)?;

        let arguments = bound.getattr("arguments")?.cast_into::<PyDict>()?;
        validate_bound_arguments(
            py,
            &arguments,
            &self.plan.params,
            &self.plan.defaults,
            self.strict,
            self.aggregate_errors,
        )?;

        let call_args = bound.getattr("args")?.cast_into::<PyTuple>()?;
        let call_kwargs = bound.getattr("kwargs")?.cast_into::<PyDict>()?;
        let result = target.call(call_args, Some(&call_kwargs))?;

        if let Some(return_validator) = &self.plan.return_validator {
            return Ok(Bound::new(
                py,
                AsyncValidatedResult {
                    awaitable: result.unbind(),
                    return_validator: return_validator.clone(),
                },
            )?
            .into_any());
        }

        Ok(result)
    }

    fn __get__<'py>(
        slf: Bound<'py, Self>,
        obj: Option<&Bound<'py, PyAny>>,
        _owner: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        if let Some(instance) = obj {
            return get_method_type(slf.py()).call1((slf, instance));
        }
        Ok(slf.into_any())
    }
}

impl AsyncCallableValidator {
    fn new(
        target: Bound<'_, PyAny>,
        plan: AsyncValidationPlan,
        aggregate_errors: bool,
        strict: bool,
    ) -> Self {
        Self {
            target: target.unbind(),
            plan,
            aggregate_errors,
            strict,
        }
    }
}

fn compile_plan<'py>(
    py: Python<'py>,
    target: &Bound<'py, PyAny>,
    validate_return: bool,
) -> PyResult<CompiledCallablePlan> {
    let inspect = py.import("inspect")?;
    let typing = py.import("typing")?;
    let tools = get_type_tools(py)?;

    let signature = inspect.getattr("signature")?.call1((target,))?;
    let globalns = target
        .getattr("__globals__")
        .unwrap_or_else(|_| PyDict::new(py).into_any());
    let th_kwargs = PyDict::new(py);
    th_kwargs.set_item("globalns", &globalns)?;
    th_kwargs.set_item("include_extras", true)?;
    let type_hints = typing
        .getattr("get_type_hints")?
        .call((target,), Some(&th_kwargs))?
        .cast_into::<PyDict>()?;

    let empty_ann = inspect.getattr("Signature")?.getattr("empty")?;
    let var_pos = inspect.getattr("Parameter")?.getattr("VAR_POSITIONAL")?;
    let var_kw = inspect.getattr("Parameter")?.getattr("VAR_KEYWORD")?;

    let mut params = Vec::new();
    let mut defaults = HashMap::new();
    let parameters = signature.getattr("parameters")?;
    for item in parameters.call_method0("items")?.try_iter()? {
        let tuple = item?.cast_into::<PyTuple>()?;
        let name_obj = tuple.get_item(0)?;
        let name = name_obj.extract::<String>()?;
        let param = tuple.get_item(1)?;

        let annotation = match type_hints.get_item(&name)? {
            Some(v) => v,
            None => param.getattr("annotation")?,
        };
        if annotation.is(&empty_ann) {
            continue;
        }
        let default = param.getattr("default")?;
        if !default.is(&empty_ann) {
            defaults.insert(name.clone(), default.unbind());
        }

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
            name: name.clone(),
            kind,
            validator: build_function_validator(py, &tools, &name, &annotation)?,
        });
    }

    let return_validator = if validate_return {
        let return_annotation = match type_hints.get_item("return")? {
            Some(v) => v,
            None => signature.getattr("return_annotation")?,
        };
        if return_annotation.is(&empty_ann) {
            None
        } else {
            Some(build_function_validator(
                py,
                &tools,
                "return",
                &return_annotation,
            )?)
        }
    } else {
        None
    };

    let is_async = inspect
        .getattr("iscoroutinefunction")?
        .call1((target,))?
        .extract()?;

    if is_async {
        Ok(CompiledCallablePlan::Async(AsyncValidationPlan {
            signature: signature.unbind(),
            params,
            defaults,
            return_validator,
        }))
    } else {
        Ok(CompiledCallablePlan::Sync(SyncValidationPlan {
            signature: signature.unbind(),
            params,
            defaults,
            return_validator,
        }))
    }
}

fn update_wrapper_metadata<'py>(
    py: Python<'py>,
    wrapped: &Bound<'py, PyAny>,
    target: &Bound<'py, PyAny>,
) -> PyResult<()> {
    let functools = py.import("functools")?;
    functools
        .getattr("update_wrapper")?
        .call1((wrapped, target))?;
    let inspect = py.import("inspect")?;
    wrapped.setattr(
        "__signature__",
        inspect.getattr("signature")?.call1((target,))?,
    )?;
    Ok(())
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
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "validated cannot be applied to staticmethod objects; decorate the underlying function before wrapping it as staticmethod.",
        ));
    }

    let classmethod_ty = builtins.getattr("classmethod")?;
    if target.is_instance(&classmethod_ty)? {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "validated cannot be applied to classmethod objects; decorate the underlying function before wrapping it as classmethod.",
        ));
    }

    if !target.is_callable() {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "validated can only be applied to callables",
        ));
    }

    let wrapped = match compile_plan(py, target, validate_return)? {
        CompiledCallablePlan::Sync(plan) => Bound::new(
            py,
            SyncCallableValidator::new(target.clone(), plan, aggregate_errors, strict),
        )?
        .into_any(),
        CompiledCallablePlan::Async(plan) => Bound::new(
            py,
            AsyncCallableValidator::new(target.clone(), plan, aggregate_errors, strict),
        )?
        .into_any(),
    };
    update_wrapper_metadata(py, &wrapped, target)?;
    Ok(wrapped)
}

#[pyclass(module = "ators._ators", frozen)]
#[derive(Debug)]
pub struct ValidatedDecorator {
    aggregate_errors: bool,
    validate_return: bool,
    strict: bool,
}

#[pymethods]
impl ValidatedDecorator {
    #[new]
    #[pyo3(signature = (*, aggregate_errors=true, validate_return=true, strict=false))]
    fn new(aggregate_errors: bool, validate_return: bool, strict: bool) -> Self {
        Self {
            aggregate_errors,
            validate_return,
            strict,
        }
    }

    fn __call__<'py>(
        &self,
        py: Python<'py>,
        target: Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        decorate_target(
            py,
            &target,
            self.aggregate_errors,
            self.validate_return,
            self.strict,
        )
    }
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

    Ok(Bound::new(
        py,
        ValidatedDecorator {
            aggregate_errors,
            validate_return,
            strict,
        },
    )?
    .into_any())
}
