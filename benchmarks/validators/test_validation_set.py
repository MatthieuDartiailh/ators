# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for set[int] container field validation."""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


@pytest.mark.benchmark(group="validation_set")
def test_benchmark_validation_set_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline set field (no validation)."""

    def set_set_ops():
        py_slotted_typed._field = {1, 2, 3}

    benchmark(set_set_ops)


@pytest.mark.benchmark(group="validation_set")
def test_benchmark_validation_set_ators(benchmark, ators_typed):
    """Benchmark Ators set[int] field validation overhead."""

    def set_set_ops():
        ators_typed.set_field = {1, 2, 3}

    benchmark(set_set_ops)


@pytest.mark.benchmark(group="validation_set")
def test_benchmark_validation_set_property(benchmark, property_typed):
    """Benchmark property-based set[int] field validation overhead."""

    def set_set_ops():
        property_typed.set_field = {1, 2, 3}

    benchmark(set_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_set")
def test_benchmark_validation_set_atom(benchmark, atom_typed):
    """Benchmark Atom set field validation overhead."""

    def set_set_ops():
        atom_typed.set_field = {1, 2, 3}

    benchmark(set_set_ops)
