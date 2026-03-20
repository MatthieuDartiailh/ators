# --------------------------------------------------------------------------------------
# Copyright (c) 2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for int field validation."""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


@pytest.mark.benchmark(group="validation_int", disable_gc=True, min_rounds=100000)
def test_benchmark_validation_int_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline int field (no validation)."""

    def int_set_ops():
        py_slotted_typed._field = 42

    benchmark(int_set_ops)


@pytest.mark.benchmark(group="validation_int", disable_gc=True, min_rounds=100000)
def test_benchmark_validation_int_ators(benchmark, ators_typed):
    """Benchmark Ators int field validation overhead."""

    def int_set_ops():
        ators_typed.int_field = 42

    benchmark(int_set_ops)


@pytest.mark.benchmark(group="validation_int", disable_gc=True, min_rounds=100000)
def test_benchmark_validation_int_property(benchmark, property_typed):
    """Benchmark property-based int field validation overhead."""

    def int_set_ops():
        property_typed.int_field = 42

    benchmark(int_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_int", disable_gc=True, min_rounds=100000)
def test_benchmark_validation_int_atom(benchmark, atom_typed):
    """Benchmark Atom int field validation overhead."""

    def int_set_ops():
        atom_typed.int_field = 42

    benchmark(int_set_ops)
