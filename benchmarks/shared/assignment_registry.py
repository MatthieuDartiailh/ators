# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Shared container-assignment benchmark case registry."""

import importlib.util
from collections.abc import Callable
from typing import Any, cast

from ators import Ators, member
from benchmarks.shared.registry_types import BenchmarkCase

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))

if ATOM_AVAILABLE:
    import atom.api as atom_api


LIST_VALUES = [1, 2, 3, 4]
SET_VALUES = {1, 2, 3, 4}
DICT_VALUES = {"a": 1, "b": 2, "c": 3, "d": 4}


def _copy_list(value):
    return list(value)


def _copy_set(value):
    return set(value)


def _copy_dict(value):
    return dict(value)


def _validate_list_of_ints(value):
    copied = list(value)
    for item in copied:
        if not isinstance(item, int):
            raise TypeError(f"Expected int item, got {type(item).__name__}")
    return copied


def _validate_set_of_ints(value):
    copied = set(value)
    for item in copied:
        if not isinstance(item, int):
            raise TypeError(f"Expected int item, got {type(item).__name__}")
    return copied


def _validate_dict_str_int(value):
    copied = dict(value)
    for key, item in copied.items():
        if not isinstance(key, str):
            raise TypeError(f"Expected str key, got {type(key).__name__}")
        if not isinstance(item, int):
            raise TypeError(f"Expected int value, got {type(item).__name__}")
    return copied


class PyContainerClass:
    __slots__ = ("dict_field", "list_field", "set_field")

    def __init__(self):
        self.list_field = []
        self.set_field = set()
        self.dict_field = {}


class PropertyContainerClass:
    def __init__(self):
        self._list_field = []
        self._set_field = set()
        self._dict_field = {}

    @property
    def list_field(self):
        return self._list_field

    @list_field.setter
    def list_field(self, value):
        self._list_field = _copy_list(value)

    @property
    def set_field(self):
        return self._set_field

    @set_field.setter
    def set_field(self, value):
        self._set_field = _copy_set(value)

    @property
    def dict_field(self):
        return self._dict_field

    @dict_field.setter
    def dict_field(self, value):
        self._dict_field = _copy_dict(value)


class TypedPropertyContainerClass:
    def __init__(self):
        self._list_field = []
        self._set_field = set()
        self._dict_field = {}

    @property
    def list_field(self):
        return self._list_field

    @list_field.setter
    def list_field(self, value):
        self._list_field = _validate_list_of_ints(value)

    @property
    def set_field(self):
        return self._set_field

    @set_field.setter
    def set_field(self, value):
        self._set_field = _validate_set_of_ints(value)

    @property
    def dict_field(self):
        return self._dict_field

    @dict_field.setter
    def dict_field(self, value):
        self._dict_field = _validate_dict_str_int(value)


class AtorsContainerClass(Ators):
    list_field: list[int] = member()
    set_field: set[int] = member()
    dict_field: dict[str, int] = member()


if ATOM_AVAILABLE:

    class AtomContainerClass(atom_api.Atom):
        list_field = cast(Any, atom_api.List)(cast(Any, atom_api.Int)())
        set_field = cast(Any, atom_api.Set)(cast(Any, atom_api.Int)())
        dict_field = cast(Any, atom_api.Dict)(
            cast(Any, atom_api.Str)(),
            cast(Any, atom_api.Int)(),
        )


def _make_case(
    group: str,
    implementation: str,
    factory: Callable[[], Any],
    op_builder: Callable[[Any], Callable[[], None]],
) -> BenchmarkCase:
    return BenchmarkCase(
        family="container_assignment",
        group=group,
        implementation=implementation,
        benchmark_name=f"assignments.{group}.{implementation}",
        operation_factory=lambda: op_builder(factory()),
    )


def _make_py_container() -> PyContainerClass:
    return PyContainerClass()


def _make_property_container() -> PropertyContainerClass:
    return PropertyContainerClass()


def _make_typed_property_container() -> TypedPropertyContainerClass:
    return TypedPropertyContainerClass()


def _make_ators_container() -> AtorsContainerClass:
    return AtorsContainerClass(list_field=[], set_field=set(), dict_field={})


def _make_atom_container() -> Any:
    return AtomContainerClass(list_field=[], set_field=set(), dict_field={})


def _assignment_cases_for_group(group: str, value: Any) -> list[BenchmarkCase]:
    cases = [
        _make_case(
            group,
            "py",
            _make_py_container,
            lambda obj: (
                lambda: setattr(
                    obj,
                    f"{group}_field",
                    value.copy() if hasattr(value, "copy") else list(value),
                )
            ),
        ),
        _make_case(
            group,
            "property",
            _make_property_container,
            lambda obj: (
                lambda: setattr(
                    obj,
                    f"{group}_field",
                    value.copy() if hasattr(value, "copy") else list(value),
                )
            ),
        ),
        _make_case(
            group,
            "property_typed",
            _make_typed_property_container,
            lambda obj: (
                lambda: setattr(
                    obj,
                    f"{group}_field",
                    value.copy() if hasattr(value, "copy") else list(value),
                )
            ),
        ),
        _make_case(
            group,
            "ators",
            _make_ators_container,
            lambda obj: (
                lambda: setattr(
                    obj,
                    f"{group}_field",
                    value.copy() if hasattr(value, "copy") else list(value),
                )
            ),
        ),
    ]
    if ATOM_AVAILABLE:
        cases.append(
            _make_case(
                group,
                "atom",
                _make_atom_container,
                lambda obj: (
                    lambda: setattr(
                        obj,
                        f"{group}_field",
                        value.copy() if hasattr(value, "copy") else list(value),
                    )
                ),
            )
        )
    return cases


def iter_assignment_cases() -> list[BenchmarkCase]:
    return [
        *_assignment_cases_for_group("list", LIST_VALUES),
        *_assignment_cases_for_group("set", SET_VALUES),
        *_assignment_cases_for_group("dict", DICT_VALUES),
    ]


def select_assignment_cases(
    *,
    families: set[str] | None = None,
    groups: set[str] | None = None,
    implementations: set[str] | None = None,
) -> list[BenchmarkCase]:
    cases = iter_assignment_cases()
    if families is not None:
        cases = [case for case in cases if case.family in families]
    if groups is not None:
        cases = [case for case in cases if case.group in groups]
    if implementations is not None:
        cases = [case for case in cases if case.implementation in implementations]
    return sorted(
        cases, key=lambda case: (case.family, case.group, case.implementation)
    )
