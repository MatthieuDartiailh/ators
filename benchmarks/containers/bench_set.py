# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmark set methods implemented in Rust for Ators containers.

Compared implementations:
- Ators typed set member
- Atom typed set member (if available)
- Pure Python set (no validation)
- Pure Python set with runtime int validation

Run with: python benchmarks/containers/bench_set.py
"""

import importlib.util
from typing import Any, cast

import pyperf

from ators import Ators, member

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))

if ATOM_AVAILABLE:
    import atom.api as atom_api


INITIAL = {1, 2, 3, 4}


def _ensure_int(value):
    if not isinstance(value, int):
        raise TypeError(f"Expected int, got {type(value).__name__}")


class ValidatedIntSet(set):
    """Python set with int validation for Ators-equivalent mutating methods."""

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


class PySetContainer:
    """Pure Python container without validation."""

    def __init__(self):
        self.set_field = INITIAL.copy()


class PyValidatedSetContainer:
    """Pure Python container with validation."""

    def __init__(self):
        self.set_field = ValidatedIntSet(INITIAL)


class AtorsSetContainer(Ators):
    """Ators container for set benchmarks."""

    set_field: set[int] = member()


if ATOM_AVAILABLE:

    class AtomSetContainer(atom_api.Atom):
        """Atom container for set benchmarks."""

        set_field = cast(Any, atom_api.Set)(cast(Any, atom_api.Int)())


def _build_ators():
    return AtorsSetContainer(set_field=INITIAL.copy())


def _build_atom():
    return AtomSetContainer(set_field=INITIAL.copy())


def _implementations():
    impls = {
        "py": PySetContainer,
        "py_typed": PyValidatedSetContainer,
        "ators": _build_ators,
    }
    if ATOM_AVAILABLE:
        impls["atom"] = _build_atom
    return impls


def _bench_all(runner: pyperf.Runner, method: str, op_builder):
    for name, factory in _implementations().items():
        obj = factory()
        runner.bench_func(f"set_{method}_{name}", op_builder(obj))


def bench_add(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            obj.set_field.add(9)
            obj.set_field.discard(9)

        return op

    _bench_all(runner, "add", op_builder)


def bench_ior(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            obj.set_field |= {9, 10}
            obj.set_field.difference_update({9, 10})

        return op

    _bench_all(runner, "ior", op_builder)


def bench_update(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            obj.set_field.update({9, 10})
            obj.set_field.difference_update({9, 10})

        return op

    _bench_all(runner, "update", op_builder)


def bench_ixor(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            obj.set_field ^= {9, 10}
            obj.set_field ^= {9, 10}

        return op

    _bench_all(runner, "ixor", op_builder)


def bench_symmetric_difference_update(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            obj.set_field.symmetric_difference_update({11, 12})
            obj.set_field.symmetric_difference_update({11, 12})

        return op

    _bench_all(runner, "symmetric_difference_update", op_builder)


if __name__ == "__main__":
    runner = pyperf.Runner()
    bench_add(runner)
    bench_ior(runner)
    bench_update(runner)
    bench_ixor(runner)
    bench_symmetric_difference_update(runner)
