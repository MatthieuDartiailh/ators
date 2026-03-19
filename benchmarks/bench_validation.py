# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmark validation performance.

This module benchmarks the performance impact of type validation by comparing
__set__ operations on typed fields across different frameworks:
- Python (baseline with no validation)
- Atom (with type validation)
- Ators (with type validation)

Each benchmark measures the full set operation including any validation overhead.
"""

import importlib.util

import pytest

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))


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


@pytest.mark.benchmark(group="validation_int")
def test_benchmark_validation_int_property(benchmark, property_typed):
    """Benchmark property-based int field validation overhead."""

    def int_set_ops():
        property_typed.int_field = 42

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


@pytest.mark.benchmark(group="validation_float")
def test_benchmark_validation_float_property(benchmark, property_typed):
    """Benchmark property-based float field validation overhead."""

    def float_set_ops():
        property_typed.float_field = 3.14

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


@pytest.mark.benchmark(group="validation_bool")
def test_benchmark_validation_bool_property(benchmark, property_typed):
    """Benchmark property-based bool field validation overhead."""

    def bool_set_ops():
        property_typed.bool_field = True

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
def test_benchmark_validation_constrained_int_property(benchmark, property_typed):
    """Benchmark property-based constrained int field with custom validator."""

    def constrained_set_ops():
        property_typed.constrained_int_field = 50

    benchmark(constrained_set_ops)


@pytest.mark.benchmark(group="validation_constrained_int")
def test_benchmark_validation_constrained_int_ators(benchmark, ators_typed):
    """Benchmark Ators constrained int field validation overhead."""

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


@pytest.mark.benchmark(group="validation_tuple")
def test_benchmark_validation_tuple_property(benchmark, property_typed):
    """Benchmark property-based tuple[int, ...] field validation overhead."""

    def tuple_set_ops():
        property_typed.tuple_field = (1, 2, 3)

    benchmark(tuple_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_tuple")
def test_benchmark_validation_tuple_atom(benchmark, atom_typed):
    """Benchmark Atom tuple field validation overhead."""

    def tuple_set_ops():
        atom_typed.tuple_field = (1, 2, 3)

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
    """Benchmark Ators tuple[int, int, str] field validation overhead."""

    def fixed_tuple_set_ops():
        ators_typed.fixed_tuple_field = (10, 20, "test")

    benchmark(fixed_tuple_set_ops)


@pytest.mark.benchmark(group="validation_fixed_tuple")
def test_benchmark_validation_fixed_tuple_property(benchmark, property_typed):
    """Benchmark property-based tuple[int, int, str] field validation overhead."""

    def fixed_tuple_set_ops():
        property_typed.fixed_tuple_field = (10, 20, "test")

    benchmark(fixed_tuple_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_fixed_tuple")
def test_benchmark_validation_fixed_tuple_atom(benchmark, atom_typed):
    """Benchmark Atom fixed tuple field validation overhead."""

    def fixed_tuple_set_ops():
        atom_typed.fixed_tuple_field = (10, 20, "test")

    benchmark(fixed_tuple_set_ops)


# ============================================================================
# Complex Validation
# ============================================================================


@pytest.mark.benchmark(group="validation_complex")
def test_benchmark_validation_complex_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline complex field (no validation)."""

    def complex_set_ops():
        py_slotted_typed._field = 1 + 2j

    benchmark(complex_set_ops)


@pytest.mark.benchmark(group="validation_complex")
def test_benchmark_validation_complex_ators(benchmark, ators_typed):
    """Benchmark Ators complex field validation overhead."""

    def complex_set_ops():
        ators_typed.complex_field = 1 + 2j

    benchmark(complex_set_ops)


@pytest.mark.benchmark(group="validation_complex")
def test_benchmark_validation_complex_property(benchmark, property_typed):
    """Benchmark property-based complex field validation overhead."""

    def complex_set_ops():
        property_typed.complex_field = 1 + 2j

    benchmark(complex_set_ops)


# ============================================================================
# Bytes Validation
# ============================================================================


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


# ============================================================================
# FrozenSet Validation
# ============================================================================


@pytest.mark.benchmark(group="validation_frozenset")
def test_benchmark_validation_frozenset_py(benchmark, py_slotted_typed):
    """Benchmark Python baseline frozenset field (no validation)."""

    def frozenset_set_ops():
        py_slotted_typed._field = frozenset({1, 2, 3})

    benchmark(frozenset_set_ops)


@pytest.mark.benchmark(group="validation_frozenset")
def test_benchmark_validation_frozenset_ators(benchmark, ators_typed):
    """Benchmark Ators frozenset[int] field validation overhead."""

    def frozenset_set_ops():
        ators_typed.frozen_set_field = frozenset({1, 2, 3})

    benchmark(frozenset_set_ops)


@pytest.mark.benchmark(group="validation_frozenset")
def test_benchmark_validation_frozenset_property(benchmark, property_typed):
    """Benchmark property-based frozenset[int] field validation overhead."""

    def frozenset_set_ops():
        property_typed.frozen_set_field = frozenset({1, 2, 3})

    benchmark(frozenset_set_ops)


# ============================================================================
# Custom Class Validation
# ============================================================================


@pytest.mark.benchmark(group="validation_custom_class")
def test_benchmark_validation_custom_class_py(
    benchmark, py_slotted_typed, custom_class_instance
):
    """Benchmark Python baseline custom class field (no validation)."""

    def custom_class_set_ops():
        py_slotted_typed._field = custom_class_instance

    benchmark(custom_class_set_ops)


@pytest.mark.benchmark(group="validation_custom_class")
def test_benchmark_validation_custom_class_ators(
    benchmark, ators_typed, custom_class_instance
):
    """Benchmark Ators CustomClass field validation overhead."""

    def custom_class_set_ops():
        ators_typed.custom_class_field = custom_class_instance

    benchmark(custom_class_set_ops)


@pytest.mark.benchmark(group="validation_custom_class")
def test_benchmark_validation_custom_class_property(
    benchmark, property_typed, custom_class_instance
):
    """Benchmark property-based CustomClass field validation overhead."""

    def custom_class_set_ops():
        property_typed.custom_class_field = custom_class_instance

    benchmark(custom_class_set_ops)


@pytest.mark.skipif(not ATOM_AVAILABLE, reason="Atom not available")
@pytest.mark.benchmark(group="validation_custom_class")
def test_benchmark_validation_custom_class_atom(
    benchmark, atom_typed, custom_class_instance
):
    """Benchmark Atom custom class field validation overhead."""

    def custom_class_set_ops():
        atom_typed.custom_class_field = custom_class_instance

    benchmark(custom_class_set_ops)
