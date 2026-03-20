# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for dict[str, int] container field validation."""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


@pytest.mark.benchmark(group="validation_dict")
def test_benchmark_validation_dict_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline dict field (no validation)."""

    def dict_set_ops():
        py_slotted_typed._field = {"a": 1}

    benchmark(dict_set_ops)


@pytest.mark.benchmark(group="validation_dict")
def test_benchmark_validation_dict_ators(benchmark, ators_typed):
    """Benchmark Ators dict[str, int] field validation overhead."""

    def dict_set_ops():
        ators_typed.dict_field = {"a": 1}

    benchmark(dict_set_ops)


@pytest.mark.benchmark(group="validation_dict")
def test_benchmark_validation_dict_property(benchmark, property_typed):
    """Benchmark property-based dict[str, int] field validation overhead."""

    def dict_set_ops():
        property_typed.dict_field = {"a": 1}

    benchmark(dict_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_dict")
def test_benchmark_validation_dict_atom(benchmark, atom_typed):
    """Benchmark Atom dict field validation overhead."""

    def dict_set_ops():
        atom_typed.dict_field = {"a": 1}

    benchmark(dict_set_ops)
