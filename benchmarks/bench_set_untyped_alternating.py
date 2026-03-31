# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmark __set__ performance with untyped fields using alternating values.

This benchmark alternates assigned values (41, 42, 41, 42, ...) to avoid
same-object fast-path effects that can appear when repeatedly assigning the
same singleton object.

Run with: python bench_set_untyped_alternating.py [--help for options]
"""

import importlib.util
from typing import Any, Callable

import pyperf

from ators import Ators, member

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))

if ATOM_AVAILABLE:
    from atom.api import Atom, Value


class PySlottedClass:
    """Reference implementation using Python __slots__."""

    __slots__ = ("_field",)

    def __init__(self):
        self._field = 0


class PyPlainClass:
    """Python class without __slots__ for comparison."""

    def __init__(self):
        self._field = 0


class AtorsUntypedClass(Ators):
    """Ators class with untyped field (Any annotation)."""

    field: Any = member()


if ATOM_AVAILABLE:

    class AtomUntypedClass(Atom):
        """Atom class with untyped field (Value descriptor)."""

        field = Value()


class PropertyUntypedClass:
    """Python class with simple properties (no validation)."""

    def __init__(self):
        self._field = 0

    @property
    def field(self):
        return self._field

    @field.setter
    def field(self, value):
        self._field = value


def make_alternating_setter(setter: Callable[[int], None]) -> Callable[[], None]:
    """Create a setter that alternates between 41 and 42 on each call."""

    value = 41

    def set_ops():
        nonlocal value
        setter(value)
        value = 42 if value == 41 else 41

    return set_ops


def bench_set_py_slotted_alt(runner: pyperf.Runner):
    obj = PySlottedClass()
    runner.bench_func(
        "set_py_slotted_alt",
        make_alternating_setter(lambda v: setattr(obj, "_field", v)),
    )


def bench_set_py_plain_alt(runner: pyperf.Runner):
    obj = PyPlainClass()
    runner.bench_func(
        "set_py_plain_alt", make_alternating_setter(lambda v: setattr(obj, "_field", v))
    )


def bench_set_ators_alt(runner: pyperf.Runner):
    obj = AtorsUntypedClass()
    runner.bench_func(
        "set_ators_alt", make_alternating_setter(lambda v: setattr(obj, "field", v))
    )


def bench_set_atom_alt(runner: pyperf.Runner):
    if not ATOM_AVAILABLE:
        return

    obj = AtomUntypedClass()
    runner.bench_func(
        "set_atom_alt", make_alternating_setter(lambda v: setattr(obj, "field", v))
    )


def bench_set_property_alt(runner: pyperf.Runner):
    obj = PropertyUntypedClass()
    runner.bench_func(
        "set_property_alt", make_alternating_setter(lambda v: setattr(obj, "field", v))
    )


if __name__ == "__main__":
    runner = pyperf.Runner()
    bench_set_py_slotted_alt(runner)
    bench_set_py_plain_alt(runner)
    bench_set_ators_alt(runner)
    bench_set_atom_alt(runner)
    bench_set_property_alt(runner)
