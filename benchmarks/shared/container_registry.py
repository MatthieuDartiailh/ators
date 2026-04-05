# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Shared container benchmark case registry."""

import importlib.util
from typing import Any, Callable, cast

from ators import Ators, member
from benchmarks.shared.registry_types import BenchmarkCase

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))

if ATOM_AVAILABLE:
    import atom.api as atom_api


INITIAL_LIST = [1, 2, 3, 4]
INITIAL_SET = {1, 2, 3, 4}
INITIAL_DICT = {"a": 1, "b": 2, "c": 3, "d": 4}

def _ensure_int(value: Any) -> None:
    if not isinstance(value, int):
        raise TypeError(f"Expected int, got {type(value).__name__}")


def _ensure_key(key: Any) -> None:
    if not isinstance(key, str):
        raise TypeError(f"Expected str key, got {type(key).__name__}")


def _ensure_value(value: Any) -> None:
    if not isinstance(value, int):
        raise TypeError(f"Expected int value, got {type(value).__name__}")


class ValidatedIntList(list):
    def append(self, value):
        _ensure_int(value)
        return super().append(value)

    def insert(self, index, value):
        _ensure_int(value)
        return super().insert(index, value)

    def __setitem__(self, index, value):
        if isinstance(index, slice):
            values = list(value)
            for item in values:
                _ensure_int(item)
            return super().__setitem__(index, values)
        _ensure_int(value)
        return super().__setitem__(index, value)

    def extend(self, values):
        values = list(values)
        for item in values:
            _ensure_int(item)
        return super().extend(values)

    def __iadd__(self, values):
        self.extend(values)
        return self


class ValidatedIntSet(set):
    def add(self, value):
        _ensure_int(value)
        return super().add(value)

    def update(self, *values):
        validated = []
        for chunk in values:
            items = list(chunk)
            for item in items:
                _ensure_int(item)
            validated.append(items)
        return super().update(*validated)

    def __ior__(self, values):
        self.update(values)
        return self

    def __ixor__(self, values):
        values = list(values)
        for item in values:
            _ensure_int(item)
        super().__ixor__(set(values))
        return self

    def symmetric_difference_update(self, values):
        values = list(values)
        for item in values:
            _ensure_int(item)
        return super().symmetric_difference_update(values)


class _KeysProvider:
    def __init__(self):
        self._data = {"x": 11, "y": 12}

    def keys(self):
        return self._data.keys()

    def __getitem__(self, key):
        return self._data[key]


class ValidatedStrIntDict(dict):
    def __setitem__(self, key, value):
        _ensure_key(key)
        _ensure_value(value)
        return super().__setitem__(key, value)

    def update(self, other=(), **kwargs):
        if other:
            if hasattr(other, "keys"):
                for key in other.keys():
                    value = other[key]
                    _ensure_key(key)
                    _ensure_value(value)
            else:
                for key, value in other:
                    _ensure_key(key)
                    _ensure_value(value)
        for key, value in kwargs.items():
            _ensure_key(key)
            _ensure_value(value)
        return super().update(other, **kwargs)

    def setdefault(self, key, default=None):
        _ensure_key(key)
        if key in self:
            return self[key]
        _ensure_value(default)
        return super().setdefault(key, default)

    def __ior__(self, other):
        self.update(other)
        return self


class PyListContainer:
    def __init__(self):
        self.list_field = INITIAL_LIST.copy()


class PyValidatedListContainer:
    def __init__(self):
        self.list_field = ValidatedIntList(INITIAL_LIST)


class PySetContainer:
    def __init__(self):
        self.set_field = INITIAL_SET.copy()


class PyValidatedSetContainer:
    def __init__(self):
        self.set_field = ValidatedIntSet(INITIAL_SET)


class PyDictContainer:
    def __init__(self):
        self.dict_field = INITIAL_DICT.copy()


class PyValidatedDictContainer:
    def __init__(self):
        self.dict_field = ValidatedStrIntDict(INITIAL_DICT)


class AtorsListContainer(Ators):
    list_field: list[int] = member()


class AtorsSetContainer(Ators):
    set_field: set[int] = member()


class AtorsDictContainer(Ators):
    dict_field: dict[str, int] = member()


if ATOM_AVAILABLE:

    class AtomListContainer(atom_api.Atom):
        list_field = cast(Any, atom_api.List)(cast(Any, atom_api.Int)())

    class AtomSetContainer(atom_api.Atom):
        set_field = cast(Any, atom_api.Set)(cast(Any, atom_api.Int)())

    class AtomDictContainer(atom_api.Atom):
        dict_field = cast(Any, atom_api.Dict)(
            cast(Any, atom_api.Str)(),
            cast(Any, atom_api.Int)(),
        )


def _list_implementations() -> dict[str, Callable[[], Any]]:
    implementations: dict[str, Callable[[], Any]] = {
        "py": PyListContainer,
        "py_typed": PyValidatedListContainer,
        "ators": lambda: AtorsListContainer(list_field=INITIAL_LIST.copy()),
    }
    if ATOM_AVAILABLE:
        implementations["atom"] = lambda: AtomListContainer(list_field=INITIAL_LIST.copy())
    return implementations


def _set_implementations() -> dict[str, Callable[[], Any]]:
    implementations: dict[str, Callable[[], Any]] = {
        "py": PySetContainer,
        "py_typed": PyValidatedSetContainer,
        "ators": lambda: AtorsSetContainer(set_field=INITIAL_SET.copy()),
    }
    if ATOM_AVAILABLE:
        implementations["atom"] = lambda: AtomSetContainer(set_field=INITIAL_SET.copy())
    return implementations


def _dict_implementations() -> dict[str, Callable[[], Any]]:
    implementations: dict[str, Callable[[], Any]] = {
        "py": PyDictContainer,
        "py_typed": PyValidatedDictContainer,
        "ators": lambda: AtorsDictContainer(dict_field=INITIAL_DICT.copy()),
    }
    if ATOM_AVAILABLE:
        implementations["atom"] = lambda: AtomDictContainer(dict_field=INITIAL_DICT.copy())
    return implementations


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
        benchmark_name=f"containers.{family}.{group}.{implementation}",
        operation_factory=lambda: op_builder(factory()),
    )


def _list_cases() -> list[BenchmarkCase]:
    cases = []
    for implementation, factory in _list_implementations().items():
        cases.extend(
            [
                _make_case(
                    "list",
                    "append",
                    implementation,
                    factory,
                    lambda obj: lambda: (obj.list_field.append(9), obj.list_field.pop()),
                ),
                _make_case(
                    "list",
                    "insert",
                    implementation,
                    factory,
                    lambda obj: lambda: (obj.list_field.insert(0, 9), obj.list_field.__delitem__(0)),
                ),
                _make_case(
                    "list",
                    "setitem",
                    implementation,
                    factory,
                    _build_list_setitem_op,
                ),
                _make_case(
                    "list",
                    "extend",
                    implementation,
                    factory,
                    lambda obj: lambda: (obj.list_field.extend([9, 10]), obj.list_field.__delitem__(slice(-2, None))),
                ),
                _make_case(
                    "list",
                    "iadd",
                    implementation,
                    factory,
                    _build_list_iadd_op,
                ),
                _make_case(
                    "list",
                    "delitem",
                    implementation,
                    factory,
                    lambda obj: lambda: (obj.list_field.__delitem__(0), obj.list_field.insert(0, 1)),
                ),
            ]
        )
    return cases


def _build_list_setitem_op(obj: Any) -> Callable[[], None]:
    next_value = [9]

    def op() -> None:
        obj.list_field[0] = next_value[0]
        next_value[0] = 1 if next_value[0] == 9 else 9

    return op


def _build_list_iadd_op(obj: Any) -> Callable[[], None]:
    def op() -> None:
        obj.list_field += [9, 10]
        del obj.list_field[-2:]

    return op


def _set_cases() -> list[BenchmarkCase]:
    cases = []
    for implementation, factory in _set_implementations().items():
        cases.extend(
            [
                _make_case(
                    "set",
                    "add",
                    implementation,
                    factory,
                    lambda obj: lambda: (obj.set_field.add(9), obj.set_field.discard(9)),
                ),
                _make_case(
                    "set",
                    "ior",
                    implementation,
                    factory,
                    _build_set_ior_op,
                ),
                _make_case(
                    "set",
                    "update",
                    implementation,
                    factory,
                    lambda obj: lambda: (obj.set_field.update({9, 10}), obj.set_field.difference_update({9, 10})),
                ),
                _make_case(
                    "set",
                    "ixor",
                    implementation,
                    factory,
                    lambda obj: lambda: (obj.set_field.__ixor__({9, 10}), obj.set_field.__ixor__({9, 10})),
                ),
                _make_case(
                    "set",
                    "symmetric_difference_update",
                    implementation,
                    factory,
                    lambda obj: lambda: (
                        obj.set_field.symmetric_difference_update({11, 12}),
                        obj.set_field.symmetric_difference_update({11, 12}),
                    ),
                ),
            ]
        )
    return cases


def _build_set_ior_op(obj: Any) -> Callable[[], None]:
    def op() -> None:
        obj.set_field |= {9, 10}
        obj.set_field.difference_update({9, 10})

    return op


def _dict_cases() -> list[BenchmarkCase]:
    cases = []
    for implementation, factory in _dict_implementations().items():
        cases.extend(
            [
                _make_case(
                    "dict",
                    "setitem",
                    implementation,
                    factory,
                    _build_dict_setitem_op,
                ),
                _make_case(
                    "dict",
                    "update_dict",
                    implementation,
                    factory,
                    lambda obj: lambda: (
                        obj.dict_field.update({"x": 11, "y": 12}),
                        obj.dict_field.pop("x"),
                        obj.dict_field.pop("y"),
                    ),
                ),
                _make_case(
                    "dict",
                    "update_keys_provider",
                    implementation,
                    factory,
                    _build_dict_update_keys_op,
                ),
                _make_case(
                    "dict",
                    "update_pairs",
                    implementation,
                    factory,
                    lambda obj: lambda: (
                        obj.dict_field.update([("x", 11), ("y", 12)]),
                        obj.dict_field.pop("x"),
                        obj.dict_field.pop("y"),
                    ),
                ),
                _make_case(
                    "dict",
                    "setdefault_existing",
                    implementation,
                    factory,
                    lambda obj: lambda: obj.dict_field.setdefault("a", 9),
                ),
                _make_case(
                    "dict",
                    "setdefault_missing",
                    implementation,
                    factory,
                    lambda obj: lambda: (obj.dict_field.setdefault("z", 26), obj.dict_field.pop("z")),
                ),
                _make_case(
                    "dict",
                    "ior",
                    implementation,
                    factory,
                    _build_dict_ior_op,
                ),
            ]
        )
    return cases


def _build_dict_setitem_op(obj: Any) -> Callable[[], None]:
    next_value = [9]

    def op() -> None:
        obj.dict_field["a"] = next_value[0]
        next_value[0] = 1 if next_value[0] == 9 else 9

    return op


def _build_dict_update_keys_op(obj: Any) -> Callable[[], None]:
    provider = _KeysProvider()

    def op() -> None:
        obj.dict_field.update(provider)
        obj.dict_field.pop("x")
        obj.dict_field.pop("y")

    return op


def _build_dict_ior_op(obj: Any) -> Callable[[], None]:
    def op() -> None:
        obj.dict_field |= {"x": 11, "y": 12}
        obj.dict_field.pop("x")
        obj.dict_field.pop("y")

    return op


def iter_container_cases() -> list[BenchmarkCase]:
    return [*_list_cases(), *_set_cases(), *_dict_cases()]


def select_container_cases(
    *,
    families: set[str] | None = None,
    groups: set[str] | None = None,
    implementations: set[str] | None = None,
) -> list[BenchmarkCase]:
    cases = iter_container_cases()
    if families is not None:
        cases = [case for case in cases if case.family in families]
    if groups is not None:
        cases = [case for case in cases if case.group in groups]
    if implementations is not None:
        cases = [case for case in cases if case.implementation in implementations]
    return sorted(cases, key=lambda case: (case.family, case.group, case.implementation))
