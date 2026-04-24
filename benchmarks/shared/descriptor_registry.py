# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Shared descriptor benchmark case registry."""

from typing import Any, Callable

from ators import Ators, freeze, member
from benchmarks.shared.registry_types import BenchmarkCase
from benchmarks.shared.runtime import atom_benchmarks_available

ATOM_AVAILABLE = atom_benchmarks_available()

if ATOM_AVAILABLE:
    from atom.api import Atom, Value


class PySlottedClass:
    __slots__ = ("_field",)

    def __init__(self):
        self._field = 0


class PyPlainClass:
    def __init__(self):
        self._field = 0


class AtorsUntypedClass(Ators):
    field: Any = member()


if ATOM_AVAILABLE:

    class AtomUntypedClass(Atom):
        field = Value()


class PropertyUntypedClass:
    def __init__(self):
        self._field = 0

    @property
    def field(self):
        return self._field

    @field.setter
    def field(self, value):
        self._field = value


def _make_case(
    family: str,
    group: str,
    implementation: str,
    factory: Callable[[], Any],
    op_builder: Callable[[Any], Callable[[], None]],
) -> BenchmarkCase:
    return BenchmarkCase(
        family=family,
        group=group,
        implementation=implementation,
        benchmark_name=f"descriptors.{family}.{implementation}",
        operation_factory=lambda: op_builder(factory()),
    )


def _make_frozen_ators() -> AtorsUntypedClass:
    obj = AtorsUntypedClass(field=42)
    freeze(obj)
    return obj


def _build_alternating_setter(setter: Callable[[int], None]) -> Callable[[], None]:
    value = 41

    def op() -> None:
        nonlocal value
        setter(value)
        value = 42 if value == 41 else 41

    return op


def _get_untyped_cases() -> list[BenchmarkCase]:
    implementations: dict[str, Callable[[], Any]] = {
        "py_slotted": PySlottedClass,
        "py_plain": PyPlainClass,
        "ators": lambda: AtorsUntypedClass(field=42),
        "ators_frozen": _make_frozen_ators,
        "property": PropertyUntypedClass,
    }
    if ATOM_AVAILABLE:
        implementations["atom"] = lambda: AtomUntypedClass(field=42)

    cases = []
    for implementation, factory in implementations.items():
        if implementation == "property":
            cases.append(
                _make_case(
                    "get_untyped",
                    "read",
                    implementation,
                    factory,
                    lambda obj: lambda: obj.field,
                )
            )
        elif implementation.startswith("py_"):
            cases.append(
                _make_case(
                    "get_untyped",
                    "read",
                    implementation,
                    factory,
                    lambda obj: lambda: obj._field,
                )
            )
        else:
            cases.append(
                _make_case(
                    "get_untyped",
                    "read",
                    implementation,
                    factory,
                    lambda obj: lambda: obj.field,
                )
            )
    return cases


def _set_untyped_cases() -> list[BenchmarkCase]:
    implementations: dict[str, Callable[[], Any]] = {
        "py_slotted": PySlottedClass,
        "py_plain": PyPlainClass,
        "ators": AtorsUntypedClass,
        "property": PropertyUntypedClass,
    }
    if ATOM_AVAILABLE:
        implementations["atom"] = AtomUntypedClass

    cases = []
    for implementation, factory in implementations.items():
        if implementation == "property":
            cases.append(
                _make_case(
                    "set_untyped",
                    "write",
                    implementation,
                    factory,
                    lambda obj: lambda: setattr(obj, "field", 42),
                )
            )
        elif implementation.startswith("py_"):
            cases.append(
                _make_case(
                    "set_untyped",
                    "write",
                    implementation,
                    factory,
                    lambda obj: lambda: setattr(obj, "_field", 42),
                )
            )
        else:
            cases.append(
                _make_case(
                    "set_untyped",
                    "write",
                    implementation,
                    factory,
                    lambda obj: lambda: setattr(obj, "field", 42),
                )
            )
    return cases


def _set_untyped_alternating_cases() -> list[BenchmarkCase]:
    implementations: dict[str, Callable[[], Any]] = {
        "py_slotted": PySlottedClass,
        "py_plain": PyPlainClass,
        "ators": AtorsUntypedClass,
        "property": PropertyUntypedClass,
    }
    if ATOM_AVAILABLE:
        implementations["atom"] = AtomUntypedClass

    cases = []
    for implementation, factory in implementations.items():
        if implementation == "property":
            cases.append(
                _make_case(
                    "set_untyped_alternating",
                    "write",
                    implementation,
                    factory,
                    lambda obj: _build_alternating_setter(
                        lambda value: setattr(obj, "field", value)
                    ),
                )
            )
        elif implementation.startswith("py_"):
            cases.append(
                _make_case(
                    "set_untyped_alternating",
                    "write",
                    implementation,
                    factory,
                    lambda obj: _build_alternating_setter(
                        lambda value: setattr(obj, "_field", value)
                    ),
                )
            )
        else:
            cases.append(
                _make_case(
                    "set_untyped_alternating",
                    "write",
                    implementation,
                    factory,
                    lambda obj: _build_alternating_setter(
                        lambda value: setattr(obj, "field", value)
                    ),
                )
            )
    return cases


def _get_descriptor_cases() -> list[BenchmarkCase]:
    implementations: dict[str, Callable[[], Any]] = {
        "ators": lambda: AtorsUntypedClass,
        "ators_frozen": lambda: type(_make_frozen_ators()),
        "property": lambda: PropertyUntypedClass,
    }
    if ATOM_AVAILABLE:
        implementations["atom"] = lambda: AtomUntypedClass

    return [
        _make_case(
            "get_descriptor",
            "descriptor",
            implementation,
            factory,
            lambda cls: lambda: cls.field,
        )
        for implementation, factory in implementations.items()
    ]


def iter_descriptor_cases() -> list[BenchmarkCase]:
    return [
        *_get_untyped_cases(),
        *_set_untyped_cases(),
        *_set_untyped_alternating_cases(),
        *_get_descriptor_cases(),
    ]


def select_descriptor_cases(
    *,
    families: set[str] | None = None,
    groups: set[str] | None = None,
    implementations: set[str] | None = None,
) -> list[BenchmarkCase]:
    cases = iter_descriptor_cases()
    if families is not None:
        cases = [case for case in cases if case.family in families]
    if groups is not None:
        cases = [case for case in cases if case.group in groups]
    if implementations is not None:
        cases = [case for case in cases if case.implementation in implementations]
    return sorted(
        cases, key=lambda case: (case.family, case.group, case.implementation)
    )
