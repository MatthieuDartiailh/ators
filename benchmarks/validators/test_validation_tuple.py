# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for tuple[int, ...] container field validation."""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


@pytest.mark.benchmark(group="validation_tuple")
def test_benchmark_validation_tuple_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline tuple field (no validation)."""

    def tuple_set_ops():
        py_slotted_typed._field = (1, 2, 3)

    benchmark(tuple_set_ops)


@pytest.mark.benchmark(group="validation_tuple")
def test_benchmark_validation_tuple_ators(benchmark, ators_typed):
    """Benchmark Ators tuple[int, ...] field validation overhead."""

    def tuple_set_ops():
        ators_typed.tuple_field = (1, 2, 3)

    benchmark(tuple_set_ops)


@pytest.mark.benchmark(group="validation_tuple")
def test_benchmark_validation_tuple_property(benchmark, property_typed):
    """Benchmark property-based tuple[int, ...] field validation overhead."""

    def tuple_set_ops():
        property_typed.tuple_field = (1, 2, 3)

    benchmark(tuple_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_tuple")
def test_benchmark_validation_tuple_atom(benchmark, atom_typed):
    """Benchmark Atom tuple field validation overhead."""

    def tuple_set_ops():
        atom_typed.tuple_field = (1, 2, 3)

    benchmark(tuple_set_ops)
