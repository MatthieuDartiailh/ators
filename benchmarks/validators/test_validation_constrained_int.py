# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for constrained int field validation."""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


@pytest.mark.benchmark(group="validation_constrained_int")
def test_benchmark_validation_constrained_int_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline constrained int field (no validation)."""

    def constrained_set_ops():
        py_slotted_typed._field = 50

    benchmark(constrained_set_ops)


@pytest.mark.benchmark(group="validation_constrained_int")
def test_benchmark_validation_constrained_int_property(benchmark, property_typed):
    """Benchmark property-based constrained int field with custom validator."""

    def constrained_set_ops():
        property_typed.constrained_int_field = 50

    benchmark(constrained_set_ops)


@pytest.mark.benchmark(group="validation_constrained_int")
def test_benchmark_validation_constrained_int_ators(benchmark, ators_typed):
    """Benchmark Ators constrained int field validation overhead."""

    def constrained_set_ops():
        ators_typed.constrained_int_field = 50

    benchmark(constrained_set_ops)
