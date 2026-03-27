# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmark __get__ performance with untyped fields using pyperf.

This module benchmarks attribute read performance across different frameworks:
- Python (slotted and non-slotted)
- Atom (if available)
- Ators (with Any annotation)

Run with: python bench_get_untyped.py [--help for options]
"""

import importlib.util
from typing import Any

import pyperf

from ators import Ators, freeze, member

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))

if ATOM_AVAILABLE:
    from atom.api import Atom, Value


# ============================================================================
# Python Reference Implementations
# ============================================================================


class PySlottedClass:
    """Reference implementation using Python __slots__."""

    __slots__ = ("_field",)

    def __init__(self):
        self._field = 0


class PyPlainClass:
    """Python class without __slots__ for comparison."""

    def __init__(self):
        self._field = 0


# ============================================================================
# Untyped Implementations
# ============================================================================


class AtorsUntypedClass(Ators):
    """Ators class with untyped field (Any annotation)."""

    field: Any = member()


# ============================================================================
# Atom Implementations (if available)
# ============================================================================


if ATOM_AVAILABLE:

    class AtomUntypedClass(Atom):
        """Atom class with untyped field (Value descriptor)."""

        field = Value()


# ============================================================================
# Property-based Implementations
# ============================================================================


class PropertyUntypedClass:
    """Python class with simple properties (no validation) for untyped benchmarks."""

    def __init__(self):
        self._field = 0

    @property
    def field(self):
        return self._field

    @field.setter
    def field(self, value):
        self._field = value


# ============================================================================
# Benchmark Functions
# ============================================================================


def bench_get_py_slotted(runner: pyperf.Runner):
    """Benchmark Python slotted __get__ performance."""
    obj = PySlottedClass()
    obj._field = 42

    def get_ops():
        _ = obj._field

    runner.bench_func("get_py_slotted", get_ops)


def bench_get_py_plain(runner: pyperf.Runner):
    """Benchmark Python plain class __get__ performance (no __slots__)."""
    obj = PyPlainClass()
    obj._field = 42

    def get_ops():
        _ = obj._field

    runner.bench_func("get_py_plain", get_ops)


def bench_get_ators(runner: pyperf.Runner):
    """Benchmark Ators __get__ performance with Any annotation."""
    obj = AtorsUntypedClass(field=42)

    def get_ops():
        _ = obj.field

    runner.bench_func("get_ators", get_ops)


def bench_get_ators_frozen(runner: pyperf.Runner):
    """Benchmark frozen Ators __get__ performance with Any annotation."""
    obj = AtorsUntypedClass(field=42)
    freeze(obj)

    def get_ops():
        _ = obj.field

    runner.bench_func("get_ators_frozen", get_ops)


def bench_get_atom(runner: pyperf.Runner):
    """Benchmark Atom __get__ performance with Value descriptor."""
    if not ATOM_AVAILABLE:
        return

    obj = AtomUntypedClass(field=42)

    def get_ops():
        _ = obj.field

    runner.bench_func("get_atom", get_ops)


def bench_get_property(runner: pyperf.Runner):
    """Benchmark property __get__ performance (no validation)."""
    obj = PropertyUntypedClass()
    obj.field = 42

    def get_ops():
        _ = obj.field

    runner.bench_func("get_property", get_ops)


if __name__ == "__main__":
    runner = pyperf.Runner()
    bench_get_py_slotted(runner)
    bench_get_py_plain(runner)
    bench_get_ators(runner)
    bench_get_ators_frozen(runner)
    bench_get_atom(runner)
    bench_get_property(runner)
