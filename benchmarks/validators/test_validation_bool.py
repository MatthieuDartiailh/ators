# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for bool field validation."""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


@pytest.mark.benchmark(group="validation_bool")
def test_benchmark_validation_bool_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline bool field (no validation)."""

    def bool_set_ops():
        py_slotted_typed._field = True

    benchmark(bool_set_ops)


@pytest.mark.benchmark(group="validation_bool")
def test_benchmark_validation_bool_ators(benchmark, ators_typed):
    """Benchmark Ators bool field validation overhead."""

    def bool_set_ops():
        ators_typed.bool_field = True

    benchmark(bool_set_ops)


@pytest.mark.benchmark(group="validation_bool")
def test_benchmark_validation_bool_property(benchmark, property_typed):
    """Benchmark property-based bool field validation overhead."""

    def bool_set_ops():
        property_typed.bool_field = True

    benchmark(bool_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_bool")
def test_benchmark_validation_bool_atom(benchmark, atom_typed):
    """Benchmark Atom bool field validation overhead."""

    def bool_set_ops():
        atom_typed.bool_field = True

    benchmark(bool_set_ops)
