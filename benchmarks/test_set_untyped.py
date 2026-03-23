# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmark __set__ performance with untyped fields.

This module benchmarks attribute write performance across different frameworks:
- Python (slotted and non-slotted)
- Atom (if available)
- Ators (with Any annotation)

No validation is performed here, only the write operation.
"""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


# ============================================================================
# __SET__ Benchmarks (Untyped - No Validation)
# ============================================================================


@pytest.mark.benchmark(group="set_untyped")
def test_benchmark_set_py_slotted(benchmark, py_slotted_untyped):
    """Benchmark Python slotted __set__ performance."""

    def set_ops():
        py_slotted_untyped._field = 42

    benchmark(set_ops)


@pytest.mark.benchmark(group="set_untyped")
def test_benchmark_set_py_plain(benchmark, py_plain_untyped):
    """Benchmark Python plain class __set__ performance (no __slots__)."""

    def set_ops():
        py_plain_untyped._field = 42

    benchmark(set_ops)


@pytest.mark.benchmark(group="set_untyped")
def test_benchmark_set_ators(benchmark, ators_untyped):
    """Benchmark Ators __set__ performance with Any annotation (no validation)."""

    def set_ops():
        ators_untyped.field = 42

    benchmark(set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="set_untyped")
def test_benchmark_set_atom(benchmark, atom_untyped):
    """Benchmark Atom __set__ performance with Value descriptor."""

    def set_ops():
        atom_untyped.field = 42

    benchmark(set_ops)


@pytest.mark.benchmark(group="set_untyped")
def test_benchmark_set_property(benchmark, property_untyped):
    """Benchmark property __set__ performance (no validation)."""

    def set_ops():
        property_untyped.field = 42

    benchmark(set_ops)
