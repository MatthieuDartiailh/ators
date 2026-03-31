# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmark __set__ performance with untyped fields using pyperf.

This module benchmarks attribute write performance across different frameworks:
- Python (slotted and non-slotted)
- Atom (if available)
- Ators (with Any annotation)

No validation is performed here, only the write operation.

Run with: python bench_set_untyped.py [--help for options]
"""

import importlib.util
from typing import Any

import pyperf

from ators import Ators, member

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


def bench_set_py_slotted(runner: pyperf.Runner):
    """Benchmark Python slotted __set__ performance."""
    obj = PySlottedClass()

    def set_ops():
        obj._field = 42

    runner.bench_func("set_py_slotted", set_ops)


def bench_set_py_plain(runner: pyperf.Runner):
    """Benchmark Python plain class __set__ performance (no __slots__)."""
    obj = PyPlainClass()

    def set_ops():
        obj._field = 42

    runner.bench_func("set_py_plain", set_ops)


def bench_set_ators(runner: pyperf.Runner):
    """Benchmark Ators __set__ performance with Any annotation."""
    obj = AtorsUntypedClass()

    def set_ops():
        obj.field = 42

    runner.bench_func("set_ators", set_ops)


def bench_set_atom(runner: pyperf.Runner):
    """Benchmark Atom __set__ performance with Value descriptor."""
    if not ATOM_AVAILABLE:
        return

    obj = AtomUntypedClass()

    def set_ops():
        obj.field = 42

    runner.bench_func("set_atom", set_ops)


def bench_set_property(runner: pyperf.Runner):
    """Benchmark property __set__ performance (no validation)."""
    obj = PropertyUntypedClass()

    def set_ops():
        obj.field = 42

    runner.bench_func("set_property", set_ops)


if __name__ == "__main__":
    runner = pyperf.Runner()
    bench_set_py_slotted(runner)
    bench_set_py_plain(runner)
    bench_set_ators(runner)
    bench_set_atom(runner)
    bench_set_property(runner)
