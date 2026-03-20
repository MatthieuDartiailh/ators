# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for complex field validation."""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


@pytest.mark.benchmark(group="validation_complex")
def test_benchmark_validation_complex_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline complex field (no validation)."""

    def complex_set_ops():
        py_slotted_typed._field = 1 + 2j

    benchmark(complex_set_ops)


@pytest.mark.benchmark(group="validation_complex")
def test_benchmark_validation_complex_ators(benchmark, ators_typed):
    """Benchmark Ators complex field validation overhead."""

    def complex_set_ops():
        ators_typed.complex_field = 1 + 2j

    benchmark(complex_set_ops)


@pytest.mark.benchmark(group="validation_complex")
def test_benchmark_validation_complex_property(benchmark, property_typed):
    """Benchmark property-based complex field validation overhead."""

    def complex_set_ops():
        property_typed.complex_field = 1 + 2j

    benchmark(complex_set_ops)
