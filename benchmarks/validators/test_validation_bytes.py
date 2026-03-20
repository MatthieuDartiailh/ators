# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for bytes field validation."""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


@pytest.mark.benchmark(group="validation_bytes")
def test_benchmark_validation_bytes_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline bytes field (no validation)."""

    def bytes_set_ops():
        py_slotted_typed._field = b"test"

    benchmark(bytes_set_ops)


@pytest.mark.benchmark(group="validation_bytes")
def test_benchmark_validation_bytes_ators(benchmark, ators_typed):
    """Benchmark Ators bytes field validation overhead."""

    def bytes_set_ops():
        ators_typed.bytes_field = b"test"

    benchmark(bytes_set_ops)


@pytest.mark.benchmark(group="validation_bytes")
def test_benchmark_validation_bytes_property(benchmark, property_typed):
    """Benchmark property-based bytes field validation overhead."""

    def bytes_set_ops():
        property_typed.bytes_field = b"test"

    benchmark(bytes_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_bytes")
def test_benchmark_validation_bytes_atom(benchmark, atom_typed):
    """Benchmark Atom bytes field validation overhead."""

    def bytes_set_ops():
        atom_typed.bytes_field = b"test"

    benchmark(bytes_set_ops)
