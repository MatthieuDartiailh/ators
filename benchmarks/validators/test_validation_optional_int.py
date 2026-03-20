# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for Optional[int] field validation."""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


@pytest.mark.benchmark(group="validation_optional_int")
def test_benchmark_validation_optional_int_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline optional int field (no validation)."""

    def optional_set_ops():
        py_slotted_typed._field = 100

    benchmark(optional_set_ops)


@pytest.mark.benchmark(group="validation_optional_int")
def test_benchmark_validation_optional_int_ators(benchmark, ators_typed):
    """Benchmark Ators Optional[int] field validation overhead."""

    def optional_set_ops():
        ators_typed.optional_int_field = 100

    benchmark(optional_set_ops)


@pytest.mark.benchmark(group="validation_optional_int")
def test_benchmark_validation_optional_int_property(benchmark, property_typed):
    """Benchmark property-based Optional[int] field validation overhead."""

    def optional_set_ops():
        property_typed.optional_int_field = 100

    benchmark(optional_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_optional_int")
def test_benchmark_validation_optional_int_atom(benchmark, atom_typed):
    """Benchmark Atom optional int field validation overhead."""

    def optional_set_ops():
        atom_typed.optional_int_field = 100

    benchmark(optional_set_ops)
