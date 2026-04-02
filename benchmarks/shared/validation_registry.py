# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Shared validation benchmark case registry."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any

from benchmarks import conftest as benchmark_conftest
from benchmarks.shared.registry_types import BenchmarkCase

ATOM_AVAILABLE = benchmark_conftest.ATOM_AVAILABLE
AtorsValidatedClass = benchmark_conftest.AtorsValidatedClass
CustomClass = benchmark_conftest.CustomClass
PropertyValidatedClass = benchmark_conftest.PropertyValidatedClass
PySlottedClass = benchmark_conftest.PySlottedClass
AtomValidatedClass = getattr(benchmark_conftest, "AtomValidatedClass", None)


def _make_py_typed() -> PySlottedClass:
    obj = PySlottedClass()
    obj._field = 0
    return obj


def _make_ators_typed() -> AtorsValidatedClass:
    return AtorsValidatedClass(
        int_field=0,
        float_field=0.0,
        str_field="",
        bool_field=False,
        complex_field=0j,
        bytes_field=b"",
        set_field=set(),
        dict_field={},
        list_field=[],
        tuple_field=(),
        fixed_tuple_field=(0, 0, ""),
        frozen_set_field=frozenset(),
        optional_int_field=None,
        enum_like_field=1,
        constrained_int_field=0,
        custom_class_field=CustomClass(),
    )


def _make_property_typed() -> PropertyValidatedClass:
    return PropertyValidatedClass()


def _make_atom_typed() -> Any:
    return AtomValidatedClass(
        int_field=0,
        float_field=0.0,
        str_field="",
        bool_field=False,
        bytes_field=b"",
        set_field=set(),
        dict_field={},
        tuple_field=(),
        fixed_tuple_field=(0, 0, ""),
        optional_int_field=None,
        enum_like_field=1,
        custom_class_field=CustomClass(),
    )


def _make_case(
    family: str,
    implementation: str,
    factory: Callable[[], Any],
    op_builder: Callable[[Any], Callable[[], None]],
) -> BenchmarkCase:
    return BenchmarkCase(
        family=family,
        group="write",
        implementation=implementation,
        benchmark_name=f"validations.{family}.{implementation}",
        operation_factory=lambda: op_builder(factory()),
    )


def _setter_op(attr_name: str, value: Any) -> Callable[[Any], Callable[[], None]]:
    return lambda obj: lambda: setattr(obj, attr_name, value)


VALIDATION_SPECS: tuple[tuple[str, str, Any, tuple[str, ...]], ...] = (
    ("validation_bool", "bool_field", True, ("py", "ators", "property", "atom")),
    ("validation_bytes", "bytes_field", b"test", ("py", "ators", "property", "atom")),
    ("validation_complex", "complex_field", 1 + 2j, ("py", "ators", "property")),
    (
        "validation_constrained_int",
        "constrained_int_field",
        50,
        ("py", "ators", "property"),
    ),
    ("validation_dict", "dict_field", {"a": 1}, ("py", "ators", "property", "atom")),
    (
        "validation_fixed_tuple",
        "fixed_tuple_field",
        (10, 20, "test"),
        ("py", "ators", "property", "atom"),
    ),
    ("validation_float", "float_field", 3.14, ("py", "ators", "property", "atom")),
    ("validation_int", "int_field", 42, ("py", "ators", "property", "atom")),
    ("validation_list", "list_field", [1, 2, 3], ("py", "ators", "property", "atom")),
    ("validation_literal", "enum_like_field", 2, ("py", "ators", "property", "atom")),
    (
        "validation_optional_int",
        "optional_int_field",
        100,
        ("py", "ators", "property", "atom"),
    ),
    ("validation_set", "set_field", {1, 2, 3}, ("py", "ators", "property", "atom")),
    ("validation_str", "str_field", "test", ("py", "ators", "property", "atom")),
    ("validation_tuple", "tuple_field", (1, 2, 3), ("py", "ators", "property", "atom")),
)


VALIDATION_FAMILIES = tuple(spec[0] for spec in VALIDATION_SPECS)


def iter_validation_cases() -> list[BenchmarkCase]:
    cases: list[BenchmarkCase] = []
    for family, field_name, value, implementations in VALIDATION_SPECS:
        if "py" in implementations:
            cases.append(
                _make_case(
                    family,
                    "py",
                    _make_py_typed,
                    _setter_op("_field", value),
                )
            )
        if "ators" in implementations:
            cases.append(
                _make_case(
                    family,
                    "ators",
                    _make_ators_typed,
                    _setter_op(field_name, value),
                )
            )
        if "property" in implementations:
            cases.append(
                _make_case(
                    family,
                    "property",
                    _make_property_typed,
                    _setter_op(field_name, value),
                )
            )
        if "atom" in implementations and ATOM_AVAILABLE:
            cases.append(
                _make_case(
                    family,
                    "atom",
                    _make_atom_typed,
                    _setter_op(field_name, value),
                )
            )
    return cases


def select_validation_cases(
    *,
    families: set[str] | None = None,
    groups: set[str] | None = None,
    implementations: set[str] | None = None,
) -> list[BenchmarkCase]:
    cases = iter_validation_cases()
    if families is not None:
        cases = [case for case in cases if case.family in families]
    if groups is not None:
        cases = [case for case in cases if case.group in groups]
    if implementations is not None:
        cases = [case for case in cases if case.implementation in implementations]
    return sorted(cases, key=lambda case: (case.family, case.group, case.implementation))