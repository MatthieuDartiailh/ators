# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmark descriptor __get__ performance using pyperf.

This module benchmarks descriptor access (via class, not instance) performance
across different frameworks:
- Ators (with Any annotation)
- Atom (if available)
- Properties

Run with: python bench_get_descriptor.py [--help for options]

"""

import importlib.util
from typing import Any

import pyperf

from ators import Ators, freeze, member

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))

if ATOM_AVAILABLE:
    from atom.api import Atom, Value


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


def bench_get_ators(runner: pyperf.Runner):
    """Benchmark Ators descriptor __get__ performance with Any annotation."""
    cls = AtorsUntypedClass

    def get_ops():
        _ = cls.field

    runner.bench_func("get_ators_descriptor", get_ops)


def bench_get_ators_frozen(runner: pyperf.Runner):
    """Benchmark frozen Ators descriptor __get__ performance with Any annotation."""
    obj = AtorsUntypedClass(field=42)
    freeze(obj)
    cls = type(obj)

    def get_ops():
        _ = cls.field

    runner.bench_func("get_ators_frozen_descriptor", get_ops)


def bench_get_atom(runner: pyperf.Runner):
    """Benchmark Atom descriptor __get__ performance with Value descriptor."""
    if not ATOM_AVAILABLE:
        return

    cls = AtomUntypedClass

    def get_ops():
        _ = cls.field

    runner.bench_func("get_atom_descriptor", get_ops)


def bench_get_property(runner: pyperf.Runner):
    """Benchmark property descriptor __get__ performance (no validation)."""
    cls = PropertyUntypedClass

    def get_ops():
        _ = cls.field

    runner.bench_func("get_property_descriptor", get_ops)


if __name__ == "__main__":
    runner = pyperf.Runner()
    bench_get_ators(runner)
    bench_get_ators_frozen(runner)
    bench_get_atom(runner)
    bench_get_property(runner)
