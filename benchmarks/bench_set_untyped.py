"""Benchmark __set__ performance with untyped fields.

This module benchmarks attribute write performance across different frameworks:
- Python (slotted and non-slotted)
- Atom (if available)
- Ators (with Any annotation)

No validation is performed here, only the write operation.
"""

import pytest

try:
    from atom.api import Atom

    ATOM_AVAILABLE = True
except ImportError:
    ATOM_AVAILABLE = False


# ============================================================================
# __SET__ Benchmarks (Untyped - No Validation)
# ============================================================================


@pytest.mark.benchmark(group="set_untyped", disable_gc=True, min_rounds=100000)
def test_benchmark_set_py_slotted(benchmark, py_slotted_untyped):
    """Benchmark Python slotted __set__ performance."""

    def set_ops():
        py_slotted_untyped._field = 42

    benchmark(set_ops)


@pytest.mark.benchmark(group="set_untyped", disable_gc=True, min_rounds=100000)
def test_benchmark_set_py_plain(benchmark, py_plain_untyped):
    """Benchmark Python plain class __set__ performance (no __slots__)."""

    def set_ops():
        py_plain_untyped._field = 42

    benchmark(set_ops)


@pytest.mark.benchmark(group="set_untyped", disable_gc=True, min_rounds=100000)
def test_benchmark_set_ators(benchmark, ators_untyped):
    """Benchmark Ators __set__ performance with Any annotation (no validation)."""

    def set_ops():
        ators_untyped.field = 42

    benchmark(set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="set_untyped", disable_gc=True, min_rounds=100000)
def test_benchmark_set_atom(benchmark, atom_untyped):
    """Benchmark Atom __set__ performance with Value descriptor."""

    def set_ops():
        atom_untyped.field = 42

    benchmark(set_ops)
