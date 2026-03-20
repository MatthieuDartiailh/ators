# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for float field validation."""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


@pytest.mark.benchmark(group="validation_float")
def test_benchmark_validation_float_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline float field (no validation)."""

    def float_set_ops():
        py_slotted_typed._field = 3.14

    benchmark(float_set_ops)


@pytest.mark.benchmark(group="validation_float")
def test_benchmark_validation_float_ators(benchmark, ators_typed):
    """Benchmark Ators float field validation overhead."""

    def float_set_ops():
        ators_typed.float_field = 3.14

    benchmark(float_set_ops)


@pytest.mark.benchmark(group="validation_float")
def test_benchmark_validation_float_property(benchmark, property_typed):
    """Benchmark property-based float field validation overhead."""

    def float_set_ops():
        property_typed.float_field = 3.14

    benchmark(float_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_float")
def test_benchmark_validation_float_atom(benchmark, atom_typed):
    """Benchmark Atom float field validation overhead."""

    def float_set_ops():
        atom_typed.float_field = 3.14

    benchmark(float_set_ops)
