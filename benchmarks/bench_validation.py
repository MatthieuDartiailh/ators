"""Benchmark validation performance.

This module benchmarks the performance impact of type validation by comparing
__set__ operations on typed fields across different frameworks:
- Python (baseline with no validation)
- Atom (with type validation)
- Ators (with type validation)

Each benchmark measures the full set operation including any validation overhead.
"""

import pytest

try:
    from atom.api import Atom

    ATOM_AVAILABLE = True
except ImportError:
    ATOM_AVAILABLE = False


# ============================================================================
# Int Validation
# ============================================================================


@pytest.mark.benchmark(group="validation_int")
def test_benchmark_validation_int_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline int field (no validation)."""

    def int_set_ops():
        py_slotted_typed._field = 42

    benchmark(int_set_ops)


@pytest.mark.benchmark(group="validation_int")
def test_benchmark_validation_int_ators(benchmark, ators_typed):
    """Benchmark Ators int field validation overhead."""

    def int_set_ops():
        ators_typed.int_field = 42

    benchmark(int_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_int")
def test_benchmark_validation_int_atom(benchmark, atom_typed):
    """Benchmark Atom int field validation overhead."""

    def int_set_ops():
        atom_typed.int_field = 42

    benchmark(int_set_ops)


# ============================================================================
# Float Validation
# ============================================================================


@pytest.mark.benchmark(group="validation_float")
def test_benchmark_validation_float_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline float field (no validation)."""

    def float_set_ops():
        py_slotted_typed._field = 3.14

    benchmark(float_set_ops)


@pytest.mark.benchmark(group="validation_float")
def test_benchmark_validation_float_ators(benchmark, ators_typed):
    """Benchmark Ators float field validation overhead."""

    def float_set_ops():
        ators_typed.float_field = 3.14

    benchmark(float_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_float")
def test_benchmark_validation_float_atom(benchmark, atom_typed):
    """Benchmark Atom float field validation overhead."""

    def float_set_ops():
        atom_typed.float_field = 3.14

    benchmark(float_set_ops)


# ============================================================================
# String Validation
# ============================================================================


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


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_str")
def test_benchmark_validation_str_atom(benchmark, atom_typed):
    """Benchmark Atom str field validation overhead."""

    def str_set_ops():
        atom_typed.str_field = "test"

    benchmark(str_set_ops)


# ============================================================================
# Bool Validation
# ============================================================================


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


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_bool")
def test_benchmark_validation_bool_atom(benchmark, atom_typed):
    """Benchmark Atom bool field validation overhead."""

    def bool_set_ops():
        atom_typed.bool_field = True

    benchmark(bool_set_ops)


# ============================================================================
# Optional[int] Validation
# ============================================================================


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


# ============================================================================
# Literal Validation (Constrained Values)
# ============================================================================


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


# ============================================================================
# Container Validation (set[int])
# ============================================================================


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


# ============================================================================
# Container Validation (dict[str, int])
# ============================================================================


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


# ============================================================================
# Constrained Int Validation
# ============================================================================


@pytest.mark.benchmark(group="validation_constrained_int")
def test_benchmark_validation_constrained_int_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline constrained int field (no validation)."""

    def constrained_set_ops():
        py_slotted_typed._field = 50

    benchmark(constrained_set_ops)


@pytest.mark.benchmark(group="validation_constrained_int")
def test_benchmark_validation_constrained_int_ators(benchmark, ators_typed):
    """Benchmark Ators constrained int field with custom validator."""

    def constrained_set_ops():
        ators_typed.constrained_int_field = 50

    benchmark(constrained_set_ops)


# ============================================================================
# Tuple Validation
# ============================================================================


@pytest.mark.benchmark(group="validation_tuple")
def test_benchmark_validation_tuple_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline tuple field (no validation)."""

    def tuple_set_ops():
        py_slotted_typed._field = (1, 2, 3)

    benchmark(tuple_set_ops)


@pytest.mark.benchmark(group="validation_tuple")
def test_benchmark_validation_tuple_ators(benchmark, ators_typed):
    """Benchmark Ators tuple[int, ...] field validation overhead."""

    def tuple_set_ops():
        ators_typed.tuple_field = (1, 2, 3)

    benchmark(tuple_set_ops)


# ============================================================================
# Fixed Tuple Validation
# ============================================================================


@pytest.mark.benchmark(group="validation_fixed_tuple")
def test_benchmark_validation_fixed_tuple_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline fixed tuple field (no validation)."""

    def fixed_tuple_set_ops():
        py_slotted_typed._field = (10, 20, "test")

    benchmark(fixed_tuple_set_ops)


@pytest.mark.benchmark(group="validation_fixed_tuple")
def test_benchmark_validation_fixed_tuple_ators(benchmark, ators_typed):
    """Benchmark Ators fixed tuple field validation overhead."""

    def fixed_tuple_set_ops():
        ators_typed.fixed_tuple_field = (10, 20, "test")

    benchmark(fixed_tuple_set_ops)
