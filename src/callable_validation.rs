/*-----------------------------------------------------------------------------
| Copyright (c) 2025-2026, Ators contributors, see git history for details
|
| Distributed under the terms of the Modified BSD License.
|
| The full license is in the file LICENSE, distributed with this software.
|----------------------------------------------------------------------------*/
/// Callable validation decorators implemented in Rust.
use crate::{
    annotations::{build_function_argument_validator, get_type_tools},
    validators::Validator,
};
use pyo3::{
    Bound, Py, PyAny, PyResult, Python, intern, pyclass, pyfunction, pymethods,
    sync::PyOnceLock,
    types::{
        PyAnyMethods, PyDict, PyDictMethods, PyString, PyStringMethods, PyTuple, PyTupleMethods,
    },
};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParamKind {
    PositionalOnly,
    PositionalOrKeyword,
    KeywordOnly,
    VarPositional,
    VarKeyword,
}

#[derive(Debug)]
struct ParamPlan {
    name: String,
    py_name: Py<PyString>,
    kind: ParamKind,
    validator: Option<Validator>,
    default: Option<Py<PyAny>>,
}

#[derive(Debug)]
struct SyncValidationPlan {
    params: Vec<ParamPlan>,
    return_validator: Option<Validator>,
}

#[derive(Debug)]
struct AsyncValidationPlan {
    params: Vec<ParamPlan>,
    return_validator: Option<Validator>,
}

#[derive(Debug)]
enum CompiledCallablePlan {
    Sync(SyncValidationPlan),
    Async(AsyncValidationPlan),
}


fn ensure_positional_storage<'py, 'a>(
    args: &Bound<'py, PyTuple>,
    out_positional: &'a mut Option<Vec<Py<PyAny>>>,
    prefix_len: usize,
) -> PyResult<&'a mut Vec<Py<PyAny>>> {
    if out_positional.is_none() {
        let mut created = Vec::with_capacity(args.len());
        for i in 0..prefix_len {
            created.push(args.get_item(i)?.unbind());
        }
        *out_positional = Some(created);
    }
    Ok(out_positional
        .as_mut()
        .expect("out_positional should be initialized"))
}

fn ensure_kwargs_storage<'py>(
    py: Python<'py>,
    kwargs: Option<&Bound<'py, PyDict>>,
    out_kwargs: &mut Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    if let Some(current) = out_kwargs.as_ref() {
        return Ok(current.clone());
    }
    let created = if let Some(kw) = kwargs {
        kw.copy()?
    } else {
        PyDict::new(py)
    };
    *out_kwargs = Some(created.clone());
    Ok(created)
}

fn validate_call_arguments<'py>(
    py: Python<'py>,
    name: &str,
    args: &Bound<'py, PyTuple>,
    kwargs: Option<&Bound<'py, PyDict>>,
    params: &[ParamPlan],
    aggregate_errors: bool,
) -> PyResult<(Bound<'py, PyTuple>, Option<Bound<'py, PyDict>>)> {
    let mut issues: Option<Vec<(String, pyo3::PyErr)>> = None;
    let mut out_positional: Option<Vec<Py<PyAny>>> = None;
    let mut out_kwargs: Option<Bound<'py, PyDict>> = None;
    let mut pending_positional_defaults: Vec<Py<PyAny>> = Vec::new();
    let mut consumed_kwargs: HashSet<String> = HashSet::new();
    let mut pos_index = 0usize;

    for param in params {
        match param.kind {
            ParamKind::PositionalOnly => {
                let from_positional = pos_index < args.len();
                let current = if from_positional {
                    let value = args.get_item(pos_index)?;
                    pos_index += 1;
                    Some(value)
                } else if param.validator.is_some() {
                    param
                        .default
                        .as_ref()
                        .map(|default| default.bind(py).clone())
                } else {
                    None
                };
                let Some(current) = current else {
                    continue;
                };
                if let Some(validator) = &param.validator {
                    match validator.validate(Some(&param.name), None, &current) {
                        Ok(v) => {
                            let validated = v.into_any();
                            if from_positional {
                                let arg_index = pos_index - 1;
                                if let Some(ref mut out) = out_positional {
                                    out.push(validated.unbind());
                                } else if !validated.is(&args.get_item(arg_index)?) {
                                    ensure_positional_storage(
                                        args,
                                        &mut out_positional,
                                        arg_index,
                                    )?
                                    .push(validated.unbind());
                                }
                            } else if let Some(ref mut out) = out_positional {
                                out.push(validated.unbind());
                            } else if !validated.is(&current) {
                                let out = ensure_positional_storage(
                                    args,
                                    &mut out_positional,
                                    pos_index,
                                )?;
                                out.append(&mut pending_positional_defaults);
                                out.push(validated.unbind());
                            } else {
                                pending_positional_defaults.push(validated.unbind());
                            }
                        }
                        Err(err) => {
                            std::hint::cold_path();
                            if !aggregate_errors {
                                return Err(err);
                            }
                            if issues.is_none() {
                                issues = Some(Vec::new());
                            }
                            issues.as_mut().unwrap().push((param.name.clone(), err));
                        }
                    }
                } else if let Some(ref mut out) = out_positional {
                    out.push(current.unbind());
                }
            }
            ParamKind::PositionalOrKeyword => {
                let mut from_positional = false;
                let mut from_keyword = false;
                let current = if pos_index < args.len() {
                    let value = args.get_item(pos_index)?;
                    pos_index += 1;
                    from_positional = true;
                    Some(value)
                } else if let Some(kw) = kwargs {
                    if let Some(value) = kw.get_item(param.py_name.bind(py))? {
                        consumed_kwargs.insert(param.name.clone());
                        from_keyword = true;
                        Some(value)
                    } else {
                        param
                            .default
                            .as_ref()
                            .map(|default| default.bind(py).clone())
                    }
                } else {
                    param
                        .default
                        .as_ref()
                        .map(|default| default.bind(py).clone())
                };
                let Some(current) = current else {
                    continue;
                };
                if let Some(validator) = &param.validator {
                    match validator.validate(Some(&param.name), None, &current) {
                        Ok(v) => {
                            if from_keyword {
                                if !v.is(&current) {
                                    ensure_kwargs_storage(py, kwargs, &mut out_kwargs)?
                                        .set_item(param.py_name.bind(py), &v)?;
                                }
                            } else if from_positional {
                                let arg_index = pos_index - 1;
                                if let Some(ref mut out) = out_positional {
                                    out.push(v.into_any().unbind());
                                } else if !v.is(&args.get_item(arg_index)?) {
                                    ensure_positional_storage(
                                        args,
                                        &mut out_positional,
                                        arg_index,
                                    )?
                                    .push(v.into_any().unbind());
                                }
                            } else if !v.is(&current) {
                                ensure_kwargs_storage(py, kwargs, &mut out_kwargs)?
                                    .set_item(param.py_name.bind(py), &v)?;
                            }
                        }
                        Err(err) => {
                            if !aggregate_errors {
                                return Err(err);
                            }
                            if issues.is_none() {
                                issues = Some(Vec::new());
                            }
                            issues.as_mut().unwrap().push((param.name.clone(), err));
                        }
                    }
                } else if from_positional && let Some(ref mut out) = out_positional {
                    out.push(current.unbind());
                }
            }
            ParamKind::KeywordOnly => {
                let current = if let Some(kw) = kwargs {
                    if let Some(value) = kw.get_item(param.py_name.bind(py))? {
                        consumed_kwargs.insert(param.name.clone());
                        Some(value)
                    } else if param.validator.is_some() {
                        param
                            .default
                            .as_ref()
                            .map(|default| default.bind(py).clone())
                    } else {
                        None
                    }
                } else if param.validator.is_some() {
                    param
                        .default
                        .as_ref()
                        .map(|default| default.bind(py).clone())
                } else {
                    None
                };
                let Some(current) = current else {
                    continue;
                };
                if let Some(validator) = &param.validator {
                    match validator.validate(Some(&param.name), None, &current) {
                        Ok(v) => {
                            if !v.is(&current) {
                                ensure_kwargs_storage(py, kwargs, &mut out_kwargs)?
                                    .set_item(param.py_name.bind(py), &v)?;
                            }
                        }
                        Err(err) => {
                            std::hint::cold_path();
                            if !aggregate_errors {
                                return Err(err);
                            }
                            if issues.is_none() {
                                issues = Some(Vec::new());
                            }
                            issues.as_mut().unwrap().push((param.name.clone(), err));
                        }
                    }
                }
            }
            ParamKind::VarPositional => {
                let mut idx = 0usize;
                while pos_index < args.len() {
                    let item = args.get_item(pos_index)?;
                    let arg_index = pos_index;
                    pos_index += 1;
                    if let Some(validator) = &param.validator {
                        match validator.validate(Some(&param.name), None, &item) {
                            Ok(v) => {
                                if let Some(ref mut out) = out_positional {
                                    out.push(v.into_any().unbind());
                                } else if !v.is(&args.get_item(arg_index)?) {
                                    ensure_positional_storage(
                                        args,
                                        &mut out_positional,
                                        arg_index,
                                    )?
                                    .push(v.into_any().unbind());
                                }
                            }
                            Err(err) => {
                                if !aggregate_errors {
                                    return Err(err);
                                }
                                if issues.is_none() {
                                    issues = Some(Vec::new());
                                }
                                issues.as_mut().unwrap().push((format!("{}[{idx}]", param.name), err));
                            }
                        }
                    } else if let Some(ref mut out) = out_positional {
                        out.push(item.unbind());
                    }
                    idx += 1;
                }
            }
            ParamKind::VarKeyword => {
                let Some(kw) = kwargs else {
                    continue;
                };
                for (key, item) in kw.iter() {
                    let key_str = key
                        .extract::<String>()
                        .unwrap_or_else(|_| "<non-string-key>".to_string());
                    if consumed_kwargs.contains(&key_str) {
                        continue;
                    }
                    if let Some(validator) = &param.validator {
                        match validator.validate(Some(&param.name), None, &item) {
                            Ok(v) => {
                                if !v.is(&item) {
                                    ensure_kwargs_storage(py, kwargs, &mut out_kwargs)?
                                        .set_item(key, v)?;
                                }
                            }
                            Err(err) => {
                                std::hint::cold_path();
                                if !aggregate_errors {
                                    return Err(err);
                                }
                                if issues.is_none() {
                                    issues = Some(Vec::new());
                                }
                                issues.as_mut().unwrap().push((format!("{}.{}", param.name, key_str), err));
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(issues) = issues {
        let exception_group = py
            .import(intern!(py, "builtins"))?
            .getattr(intern!(py, "ExceptionGroup"))?
            .call1((format!("Failed to validate {} arguments", self.target_name), issues))?;
        return Err(pyo3::PyErr::from_value(exception_group));
    }
    let final_args = if let Some(mut out) = out_positional {
        out.extend(pending_positional_defaults);
        PyTuple::new(py, out)?
    } else {
        args.clone()
    };
    let final_kwargs = if let Some(out) = out_kwargs {
        Some(out)
    } else {
        kwargs.cloned()
    };
    Ok((final_args, final_kwargs))
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
    target_name: String,
    target: Py<PyAny>,
    plan: SyncValidationPlan,
    aggregate_errors: bool,
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
        let (call_args, call_kwargs) = validate_call_arguments(
            py,
            self.target_name.as_str(),
            args,
            kwargs,
            &self.plan.params,
            self.aggregate_errors,
        )?;

        let result = target.call(&call_args, call_kwargs.as_ref())?;

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
    ) -> Self {
        Self {
            target_name: target.getattr(intern!(target.py(), "__name__"))
                .and_then(|name| name.cast_into::<PyString>())
                .map(|name| name.to_string())
                .unwrap_or_else(|_| "<unknown>".to_string()),
            target: target.unbind(),
            plan,
            aggregate_errors,
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
    let args = err
        .value(py)
        .getattr(intern!(py, "args"))?
        .cast_into::<PyTuple>()?;
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
        match self.iterator.bind(py).call_method0(intern!(py, "__next__")) {
            Ok(v) => Ok(v),
            Err(err) => handle_iterator_error(py, err, &self.return_validator),
        }
    }

    fn send<'py>(&self, py: Python<'py>, value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        match self
            .iterator
            .bind(py)
            .call_method1(intern!(py, "send"), (value,))
        {
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
        match self
            .iterator
            .bind(py)
            .call_method(intern!(py, "throw"), args, None)
        {
            Ok(v) => Ok(v),
            Err(err) => handle_iterator_error(py, err, &self.return_validator),
        }
    }

    fn close<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.iterator.bind(py).call_method0(intern!(py, "close"))
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
        let iterator = self
            .awaitable
            .bind(py)
            .call_method0(intern!(py, "__await__"))?;
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
    target_name: String,
    plan: AsyncValidationPlan,
    aggregate_errors: bool,
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
        let (call_args, call_kwargs) = validate_call_arguments(
            py,
            self.target_name.as_str(),
            args,
            kwargs,
            &self.plan.params,
            self.aggregaste_errors,
        )?;

        let result = target.call(&call_args, call_kwargs.as_ref())?;

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
    ) -> Self {
        Self {
            target: target.unbind(),
            target_name: target.getattr(intern!(target.py(), "__name__"))
                .and_then(|name| name.cast_into::<PyString>())
                .map(|name| name.to_string())
                .unwrap_or_else(|_| "<unknown>".to_string()),
            plan,
            aggregate_errors,
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

    let signature = inspect
        .getattr(intern!(py, "signature"))?
        .call1((target,))?;
    let globalns = target
        .getattr(intern!(py, "__globals__"))
        .unwrap_or_else(|_| PyDict::new(py).into_any());
    let th_kwargs = PyDict::new(py);
    th_kwargs.set_item(intern!(py, "globalns"), &globalns)?;
    th_kwargs.set_item(intern!(py, "include_extras"), true)?;
    let type_hints = typing
        .getattr(intern!(py, "get_type_hints"))?
        .call((target,), Some(&th_kwargs))?
        .cast_into::<PyDict>()?;

    let empty_ann = inspect
        .getattr(intern!(py, "Signature"))?
        .getattr(intern!(py, "empty"))?;
    let parameter = inspect.getattr(intern!(py, "Parameter"))?;
    let pos_only = parameter.getattr(intern!(py, "POSITIONAL_ONLY"))?;
    let pos_or_kw = parameter.getattr(intern!(py, "POSITIONAL_OR_KEYWORD"))?;
    let kw_only = parameter.getattr(intern!(py, "KEYWORD_ONLY"))?;
    let var_pos = parameter.getattr(intern!(py, "VAR_POSITIONAL"))?;
    let var_kw = parameter.getattr(intern!(py, "VAR_KEYWORD"))?;

    let mut params = Vec::new();
    let parameters = signature.getattr(intern!(py, "parameters"))?;
    for item in parameters.call_method0(intern!(py, "items"))?.try_iter()? {
        let tuple = item?.cast_into::<PyTuple>()?;
        let name_obj = tuple.get_item(0)?.cast_into::<PyString>()?;
        let name = name_obj.to_cow()?.into_owned();
        let param = tuple.get_item(1)?;

        let annotation = match type_hints.get_item(&name)? {
            Some(v) => v,
            None => param.getattr(intern!(py, "annotation"))?,
        };
        let validator = if annotation.is(&empty_ann) {
            None
        } else {
            Some(build_function_argument_validator(
                &name_obj,
                &annotation,
                &tools,
            )?)
        };
        let default = param.getattr(intern!(py, "default"))?;
        let default = if validator.is_some() && !default.is(&empty_ann) {
            Some(default.unbind())
        } else {
            None
        };

        let kind = {
            let k = param.getattr(intern!(py, "kind"))?;
            if k.is(&pos_only) {
                ParamKind::PositionalOnly
            } else if k.is(&pos_or_kw) {
                ParamKind::PositionalOrKeyword
            } else if k.is(&kw_only) {
                ParamKind::KeywordOnly
            } else if k.is(&var_pos) {
                ParamKind::VarPositional
            } else if k.is(&var_kw) {
                ParamKind::VarKeyword
            } else {
                ParamKind::PositionalOrKeyword
            }
        };

        params.push(ParamPlan {
            name: name.clone(),
            py_name: name_obj.clone().unbind(),
            kind,
            validator,
            default,
        });
    }

    let return_validator = if validate_return {
        let return_annotation = match type_hints.get_item(intern!(py, "return"))? {
            Some(v) => v,
            None => signature.getattr(intern!(py, "return_annotation"))?,
        };
        if return_annotation.is(&empty_ann) {
            None
        } else {
            let return_name = PyString::new(py, "return");
            Some(build_function_argument_validator(
                &return_name,
                &return_annotation,
                &tools,
            )?)
        }
    } else {
        None
    };

    let is_async = inspect
        .getattr(intern!(py, "iscoroutinefunction"))?
        .call1((target,))?
        .extract()?;

    if is_async {
        Ok(CompiledCallablePlan::Async(AsyncValidationPlan {
            params,
            return_validator,
        }))
    } else {
        Ok(CompiledCallablePlan::Sync(SyncValidationPlan {
            params,
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
        .getattr(intern!(py, "update_wrapper"))?
        .call1((wrapped, target))?;
    let inspect = py.import("inspect")?;
    wrapped.setattr(
        intern!(py, "__signature__"),
        inspect
            .getattr(intern!(py, "signature"))?
            .call1((target,))?,
    )?;
    Ok(())
}

fn decorate_target<'py>(
    py: Python<'py>,
    target: &Bound<'py, PyAny>,
    aggregate_errors: bool,
    validate_return: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let builtins = py.import("builtins")?;
    let staticmethod_ty = builtins.getattr(intern!(py, "staticmethod"))?;
    if target.is_instance(&staticmethod_ty)? {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "validated cannot be applied to staticmethod objects; decorate the underlying function before wrapping it as staticmethod.",
        ));
    }

    let classmethod_ty = builtins.getattr(intern!(py, "classmethod"))?;
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
            SyncCallableValidator::new(target.clone(), plan, aggregate_errors),
        )?
        .into_any(),
        CompiledCallablePlan::Async(plan) => Bound::new(
            py,
            AsyncCallableValidator::new(target.clone(), plan, aggregate_errors),
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
}

#[pymethods]
impl ValidatedDecorator {
    #[new]
    #[pyo3(signature = (*, aggregate_errors=true, validate_return=true))]
    fn new(aggregate_errors: bool, validate_return: bool) -> Self {
        Self {
            aggregate_errors,
            validate_return,
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
        )
    }
}

/// Decorate a callable (or create a decorator) that validates call arguments and return values.
///
/// When called as `@validated`, defaults are:
/// - `aggregate_errors=True`: collect all argument errors before raising.
/// - `validate_return=True`: validate the return annotation when present.
///
/// It can also be used as a decorator factory, for example:
/// `@validated(aggregate_errors=False, validate_return=False)`.
#[pyfunction(signature=(func=None, *, aggregate_errors=true, validate_return=true))]
pub fn validated<'py>(
    py: Python<'py>,
    func: Option<Bound<'py, PyAny>>,
    aggregate_errors: bool,
    validate_return: bool,
) -> PyResult<Bound<'py, PyAny>> {
    if let Some(target) = func {
        return decorate_target(py, &target, aggregate_errors, validate_return);
    }

    Ok(Bound::new(
        py,
        ValidatedDecorator {
            aggregate_errors,
            validate_return,
        },
    )?
    .into_any())
}
