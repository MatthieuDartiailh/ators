# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmark list methods implemented in Rust for Ators containers.

Compared implementations:
- Ators typed list member
- Atom typed list member (if available)
- Pure Python list (no validation)
- Pure Python list with runtime int validation

Run with: python benchmarks/containers/bench_list.py
"""

import importlib.util
from typing import Any, cast

import pyperf

from ators import Ators, member

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))

if ATOM_AVAILABLE:
    import atom.api as atom_api


INITIAL = [1, 2, 3, 4]


def _ensure_int(value):
    if not isinstance(value, int):
        raise TypeError(f"Expected int, got {type(value).__name__}")


class ValidatedIntList(list):
    """Python list with int validation for Ators-equivalent mutating methods."""

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


class PyListContainer:
    """Pure Python container without validation."""

    def __init__(self):
        self.list_field = INITIAL.copy()


class PyValidatedListContainer:
    """Pure Python container with validation."""

    def __init__(self):
        self.list_field = ValidatedIntList(INITIAL)


class AtorsListContainer(Ators):
    """Ators container for list benchmarks."""

    list_field: list[int] = member()


if ATOM_AVAILABLE:

    class AtomListContainer(atom_api.Atom):
        """Atom container for list benchmarks."""

        list_field = cast(Any, atom_api.List)(cast(Any, atom_api.Int)())


def _build_ators():
    return AtorsListContainer(list_field=INITIAL.copy())


def _build_atom():
    return AtomListContainer(list_field=INITIAL.copy())


def _implementations():
    impls = {
        "py": PyListContainer,
        "py_typed": PyValidatedListContainer,
        "ators": _build_ators,
    }
    if ATOM_AVAILABLE:
        impls["atom"] = _build_atom
    return impls


def _bench_all(runner: pyperf.Runner, method: str, op_builder):
    for name, factory in _implementations().items():
        obj = factory()
        runner.bench_func(f"list_{method}_{name}", op_builder(obj))


def bench_append(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            obj.list_field.append(9)
            obj.list_field.pop()

        return op

    _bench_all(runner, "append", op_builder)


def bench_insert(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            obj.list_field.insert(0, 9)
            del obj.list_field[0]

        return op

    _bench_all(runner, "insert", op_builder)


def bench_setitem(runner: pyperf.Runner):
    def op_builder(obj):
        next_value = [9]

        def op():
            obj.list_field[0] = next_value[0]
            next_value[0] = 1 if next_value[0] == 9 else 9

        return op

    _bench_all(runner, "setitem", op_builder)


def bench_extend(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            obj.list_field.extend([9, 10])
            del obj.list_field[-2:]

        return op

    _bench_all(runner, "extend", op_builder)


def bench_iadd(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            obj.list_field += [9, 10]
            del obj.list_field[-2:]

        return op

    _bench_all(runner, "iadd", op_builder)


def bench_delitem(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            del obj.list_field[0]
            obj.list_field.insert(0, 1)

        return op

    _bench_all(runner, "delitem", op_builder)


if __name__ == "__main__":
    runner = pyperf.Runner()
    bench_append(runner)
    bench_insert(runner)
    bench_setitem(runner)
    bench_extend(runner)
    bench_iadd(runner)
    bench_delitem(runner)
