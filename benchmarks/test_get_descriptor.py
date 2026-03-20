"""Benchmark __get__ performance with untyped fields.

This module benchmarks attribute read performance across different frameworks:
- Python (slotted and non-slotted)
- Atom (if available)
- Ators (with Any annotation)
"""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


# ============================================================================
# __GET__ Benchmarks (descriptor access)
# ============================================================================


@pytest.mark.benchmark(group="get_descriptor")
def test_benchmark_get_ators(benchmark, ators_untyped):
    """Benchmark Ators __get__ performance with Any annotation."""

    cls = type(ators_untyped)

    def get_ops():
        _ = cls.field

    benchmark(get_ops)


@pytest.mark.benchmark(group="get_descriptor")
def test_benchmark_get_ators_frozen(benchmark, ators_frozen_untyped):
    """Benchmark frozen Ators __get__ performance with Any annotation."""

    cls = type(ators_frozen_untyped)

    def get_ops():
        _ = cls.field

    benchmark(get_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="get_descriptor")
def test_benchmark_get_atom(benchmark, atom_untyped):
    """Benchmark Atom __get__ performance with Value descriptor."""

    cls = type(atom_untyped)

    def get_ops():
        _ = cls.field

    benchmark(get_ops)


@pytest.mark.benchmark(group="get_descriptor")
def test_benchmark_get_property(benchmark, property_untyped):
    """Benchmark property __get__ performance (no validation)."""

    cls = type(property_untyped)

    def get_ops():
        _ = cls.field

    benchmark(get_ops)
