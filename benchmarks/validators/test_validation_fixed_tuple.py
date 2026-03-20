# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for fixed tuple[int, int, str] container field validation."""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


@pytest.mark.benchmark(group="validation_fixed_tuple")
def test_benchmark_validation_fixed_tuple_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline fixed tuple field (no validation)."""

    def fixed_tuple_set_ops():
        py_slotted_typed._field = (10, 20, "test")

    benchmark(fixed_tuple_set_ops)


@pytest.mark.benchmark(group="validation_fixed_tuple")
def test_benchmark_validation_fixed_tuple_ators(benchmark, ators_typed):
    """Benchmark Ators tuple[int, int, str] field validation overhead."""

    def fixed_tuple_set_ops():
        ators_typed.fixed_tuple_field = (10, 20, "test")

    benchmark(fixed_tuple_set_ops)


@pytest.mark.benchmark(group="validation_fixed_tuple")
def test_benchmark_validation_fixed_tuple_property(benchmark, property_typed):
    """Benchmark property-based tuple[int, int, str] field validation overhead."""

    def fixed_tuple_set_ops():
        property_typed.fixed_tuple_field = (10, 20, "test")

    benchmark(fixed_tuple_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_fixed_tuple")
def test_benchmark_validation_fixed_tuple_atom(benchmark, atom_typed):
    """Benchmark Atom fixed tuple field validation overhead."""

    def fixed_tuple_set_ops():
        atom_typed.fixed_tuple_field = (10, 20, "test")

    benchmark(fixed_tuple_set_ops)
