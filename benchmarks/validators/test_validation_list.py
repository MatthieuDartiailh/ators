# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for list[int] container field validation."""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


@pytest.mark.benchmark(group="validation_list")
def test_benchmark_validation_list_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline list field (no validation)."""

    def set_list_ops():
        py_slotted_typed._field = [1, 2, 3]

    benchmark(set_list_ops)


@pytest.mark.benchmark(group="validation_list")
def test_benchmark_validation_list_ators(benchmark, ators_typed):
    """Benchmark Ators list[int] field validation overhead."""

    def set_list_ops():
        ators_typed.list_field = [1, 2, 3]

    benchmark(set_list_ops)


@pytest.mark.benchmark(group="validation_list")
def test_benchmark_validation_list_property(benchmark, property_typed):
    """Benchmark property-based list[int] field validation overhead."""

    def set_list_ops():
        property_typed.list_field = [1, 2, 3]

    benchmark(set_list_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_list")
def test_benchmark_validation_list_atom(benchmark, atom_typed):
    """Benchmark Atom list[int] field validation overhead."""

    def set_list_ops():
        atom_typed.list_field = [1, 2, 3]

    benchmark(set_list_ops)
