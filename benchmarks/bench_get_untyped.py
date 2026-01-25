"""Benchmark __get__ performance with untyped fields.

This module benchmarks attribute read performance across different frameworks:
- Python (slotted and non-slotted)
- Atom (if available)
- Ators (with Any annotation)
"""

import pytest

try:
    from atom.api import Atom

    ATOM_AVAILABLE = True
except ImportError:
    ATOM_AVAILABLE = False


# ============================================================================
# __GET__ Benchmarks (Untyped)
# ============================================================================


@pytest.mark.benchmark(group="get_untyped")
def test_benchmark_get_py_slotted(benchmark, py_slotted_untyped):
    """Benchmark Python slotted __get__ performance."""

    def get_ops():
        _ = py_slotted_untyped._field

    benchmark(get_ops)


@pytest.mark.benchmark(group="get_untyped")
def test_benchmark_get_py_plain(benchmark, py_plain_untyped):
    """Benchmark Python plain class __get__ performance (no __slots__)."""

    def get_ops():
        _ = py_plain_untyped._field

    benchmark(get_ops)


@pytest.mark.benchmark(group="get_untyped")
def test_benchmark_get_ators(benchmark, ators_untyped):
    """Benchmark Ators __get__ performance with Any annotation."""

    def get_ops():
        _ = ators_untyped.field


    benchmark(get_ops)


@pytest.mark.benchmark(group="get_untyped")
def test_benchmark_get_ators_frozen(benchmark, ators_frozen_untyped):
    """Benchmark frozen Ators __get__ performance with Any annotation."""

    def get_ops():
        _ = ators_frozen_untyped.field

    benchmark(get_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="get_untyped")
def test_benchmark_get_atom(benchmark, atom_untyped):
    """Benchmark Atom __get__ performance with Value descriptor."""

    def get_ops():
        _ = atom_untyped.field

    benchmark(get_ops)
