# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for str field validation."""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


@pytest.mark.benchmark(group="validation_str")
def test_benchmark_validation_str_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline str field (no validation)."""

    def str_set_ops():
        py_slotted_typed._field = "test"

    benchmark(str_set_ops)


@pytest.mark.benchmark(group="validation_str")
def test_benchmark_validation_str_ators(benchmark, ators_typed):
    """Benchmark Ators str field validation overhead."""

    def str_set_ops():
        ators_typed.str_field = "test"

    benchmark(str_set_ops)


@pytest.mark.benchmark(group="validation_str")
def test_benchmark_validation_str_property(benchmark, property_typed):
    """Benchmark property-based str field validation overhead."""

    def str_set_ops():
        property_typed.str_field = "test"

    benchmark(str_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_str")
def test_benchmark_validation_str_atom(benchmark, atom_typed):
    """Benchmark Atom str field validation overhead."""

    def str_set_ops():
        atom_typed.str_field = "test"

    benchmark(str_set_ops)
