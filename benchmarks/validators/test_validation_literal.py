# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for Literal constrained value field validation."""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


@pytest.mark.benchmark(group="validation_literal")
def test_benchmark_validation_literal_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline literal field (no validation)."""

    def literal_set_ops():
        py_slotted_typed._field = 2

    benchmark(literal_set_ops)


@pytest.mark.benchmark(group="validation_literal")
def test_benchmark_validation_literal_ators(benchmark, ators_typed):
    """Benchmark Ators Literal field validation overhead."""

    def literal_set_ops():
        ators_typed.enum_like_field = 2

    benchmark(literal_set_ops)


@pytest.mark.benchmark(group="validation_literal")
def test_benchmark_validation_literal_property(benchmark, property_typed):
    """Benchmark property-based Literal field validation overhead."""

    def literal_set_ops():
        property_typed.enum_like_field = 2

    benchmark(literal_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_literal")
def test_benchmark_validation_literal_atom(benchmark, atom_typed):
    """Benchmark Atom Enum field validation overhead."""

    def literal_set_ops():
        atom_typed.enum_like_field = 2

    benchmark(literal_set_ops)
