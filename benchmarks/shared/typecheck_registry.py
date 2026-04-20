# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Shared benchmark case registry for generic-aware type-check operations.

Families
--------
``typecheck``
    Measures ``issubclass`` and ``isinstance`` performance for:

    * ``py`` - standard Python classes (baseline).
    * ``ators`` - non-generic Ators classes (metaclass overhead).
    * ``ators_generic_concrete`` - fully-concrete specialisation
      (e.g. ``issubclass(A[int, str], A[int, str])``).
    * ``ators_generic_typevar`` - TypeVar-pattern match
      (e.g. ``issubclass(A[int, str], A[T, str])``).
"""

from __future__ import annotations

from collections.abc import Callable

from ators import Ators
from benchmarks.shared.registry_types import BenchmarkCase


# ---------------------------------------------------------------------------
# Standard Python baseline classes
# ---------------------------------------------------------------------------
class _PyBase:
    pass


class _PyChild(_PyBase):
    pass


_py_child_inst = _PyChild()


# ---------------------------------------------------------------------------
# Non-generic Ators classes
# ---------------------------------------------------------------------------
class _AtorsBase(Ators):
    pass


class _AtorsChild(_AtorsBase):
    pass


_ators_child_inst = _AtorsChild()


# ---------------------------------------------------------------------------
# Generic Ators class (two type params, PEP-695 syntax)
# ---------------------------------------------------------------------------
class _AtorsG[T, U](Ators):
    pass


# Extract the TypeVar objects created by the class definition so that
# partial-specialisation patterns like ``_AtorsG[_T, str]`` use the same
# TypeVar identities that the Rust engine records.
_T, _U = _AtorsG.__type_params__

# Concrete and partial specialisations (created once at module level).
_G_int_str = _AtorsG[int, str]
_G_T_str = _AtorsG[_T, str]
_G_T_U = _AtorsG[_T, _U]
_G_int_int = _AtorsG[int, int]

_g_int_str_inst = _G_int_str()
_g_int_int_inst = _G_int_int()


# ---------------------------------------------------------------------------
# Case factories
# ---------------------------------------------------------------------------


def _make_py_issubclass() -> Callable[[], None]:
    child, base = _PyChild, _PyBase

    def _op() -> None:
        issubclass(child, base)

    return _op


def _make_py_isinstance() -> Callable[[], None]:
    inst, base = _py_child_inst, _PyBase

    def _op() -> None:
        isinstance(inst, base)

    return _op


def _make_ators_issubclass() -> Callable[[], None]:
    child, base = _AtorsChild, _AtorsBase

    def _op() -> None:
        issubclass(child, base)

    return _op


def _make_ators_isinstance() -> Callable[[], None]:
    inst, base = _ators_child_inst, _AtorsBase

    def _op() -> None:
        isinstance(inst, base)

    return _op


def _make_generic_concrete_issubclass() -> Callable[[], None]:
    lhs, rhs = _G_int_str, _G_int_str

    def _op() -> None:
        issubclass(lhs, rhs)

    return _op


def _make_generic_concrete_isinstance() -> Callable[[], None]:
    inst, rhs = _g_int_str_inst, _G_int_str

    def _op() -> None:
        isinstance(inst, rhs)

    return _op


def _make_generic_typevar_issubclass() -> Callable[[], None]:
    lhs, rhs = _G_int_str, _G_T_str

    def _op() -> None:
        issubclass(lhs, rhs)

    return _op


def _make_generic_typevar_isinstance() -> Callable[[], None]:
    inst, rhs = _g_int_str_inst, _G_T_str

    def _op() -> None:
        isinstance(inst, rhs)

    return _op


def _make_py_issubclass_negative() -> Callable[[], None]:
    child, base = _PyBase, _PyChild  # reversed — False

    def _op() -> None:
        issubclass(child, base)

    return _op


def _make_py_isinstance_negative() -> Callable[[], None]:
    inst, base = _py_child_inst, _PyChild.__mro__[-1]  # object — True? No, use unrelated

    def _op() -> None:
        isinstance(inst, int)

    return _op


def _make_ators_issubclass_negative() -> Callable[[], None]:
    child, base = _AtorsBase, _AtorsChild  # reversed — False

    def _op() -> None:
        issubclass(child, base)

    return _op


def _make_ators_isinstance_negative() -> Callable[[], None]:
    inst, base = _ators_child_inst, int  # different type — False

    def _op() -> None:
        isinstance(inst, base)

    return _op


def _make_generic_concrete_issubclass_negative() -> Callable[[], None]:
    lhs, rhs = _G_int_str, _G_int_int  # arg mismatch — False

    def _op() -> None:
        issubclass(lhs, rhs)

    return _op


def _make_generic_concrete_isinstance_negative() -> Callable[[], None]:
    inst, rhs = _g_int_int_inst, _G_int_str  # arg mismatch — False

    def _op() -> None:
        isinstance(inst, rhs)

    return _op


def _make_generic_typevar_issubclass_negative() -> Callable[[], None]:
    lhs, rhs = _G_int_str, _G_T_str  # rhs is _AtorsG[T, str]; lhs matches → test origin mismatch

    # Use a non-matching lhs: _G_int_int vs _G_T_str (str != int) — False
    lhs2, rhs2 = _G_int_int, _G_T_str

    def _op() -> None:
        issubclass(lhs2, rhs2)

    return _op


def _make_generic_typevar_isinstance_negative() -> Callable[[], None]:
    inst, rhs = _g_int_int_inst, _G_T_str  # int != str constraint — False

    def _op() -> None:
        isinstance(inst, rhs)

    return _op


def _make_generic_typevar_both_issubclass() -> Callable[[], None]:
    lhs, rhs = _G_int_str, _G_T_U

    def _op() -> None:
        issubclass(lhs, rhs)

    return _op


# ---------------------------------------------------------------------------
# Case list
# ---------------------------------------------------------------------------
_CASES: list[BenchmarkCase] = [
    # --- py baseline ---
    BenchmarkCase(
        family="typecheck",
        group="issubclass",
        implementation="py",
        benchmark_name="typecheck/issubclass/py",
        operation_factory=_make_py_issubclass,
    ),
    BenchmarkCase(
        family="typecheck",
        group="isinstance",
        implementation="py",
        benchmark_name="typecheck/isinstance/py",
        operation_factory=_make_py_isinstance,
    ),
    # --- ators non-generic ---
    BenchmarkCase(
        family="typecheck",
        group="issubclass",
        implementation="ators",
        benchmark_name="typecheck/issubclass/ators",
        operation_factory=_make_ators_issubclass,
    ),
    BenchmarkCase(
        family="typecheck",
        group="isinstance",
        implementation="ators",
        benchmark_name="typecheck/isinstance/ators",
        operation_factory=_make_ators_isinstance,
    ),
    # --- ators generic concrete ---
    BenchmarkCase(
        family="typecheck",
        group="issubclass",
        implementation="ators_generic_concrete",
        benchmark_name="typecheck/issubclass/ators_generic_concrete",
        operation_factory=_make_generic_concrete_issubclass,
    ),
    BenchmarkCase(
        family="typecheck",
        group="isinstance",
        implementation="ators_generic_concrete",
        benchmark_name="typecheck/isinstance/ators_generic_concrete",
        operation_factory=_make_generic_concrete_isinstance,
    ),
    # --- ators generic TypeVar-pattern ---
    BenchmarkCase(
        family="typecheck",
        group="issubclass",
        implementation="ators_generic_typevar",
        benchmark_name="typecheck/issubclass/ators_generic_typevar",
        operation_factory=_make_generic_typevar_issubclass,
    ),
    BenchmarkCase(
        family="typecheck",
        group="isinstance",
        implementation="ators_generic_typevar",
        benchmark_name="typecheck/isinstance/ators_generic_typevar",
        operation_factory=_make_generic_typevar_isinstance,
    ),
    BenchmarkCase(
        family="typecheck",
        group="issubclass",
        implementation="ators_generic_typevar_both",
        benchmark_name="typecheck/issubclass/ators_generic_typevar_both",
        operation_factory=_make_generic_typevar_both_issubclass,
    ),
    # --- negative (False) outcomes ---
    BenchmarkCase(
        family="typecheck",
        group="issubclass",
        implementation="py_negative",
        benchmark_name="typecheck/issubclass/py_negative",
        operation_factory=_make_py_issubclass_negative,
    ),
    BenchmarkCase(
        family="typecheck",
        group="isinstance",
        implementation="py_negative",
        benchmark_name="typecheck/isinstance/py_negative",
        operation_factory=_make_py_isinstance_negative,
    ),
    BenchmarkCase(
        family="typecheck",
        group="issubclass",
        implementation="ators_negative",
        benchmark_name="typecheck/issubclass/ators_negative",
        operation_factory=_make_ators_issubclass_negative,
    ),
    BenchmarkCase(
        family="typecheck",
        group="isinstance",
        implementation="ators_negative",
        benchmark_name="typecheck/isinstance/ators_negative",
        operation_factory=_make_ators_isinstance_negative,
    ),
    BenchmarkCase(
        family="typecheck",
        group="issubclass",
        implementation="ators_generic_concrete_negative",
        benchmark_name="typecheck/issubclass/ators_generic_concrete_negative",
        operation_factory=_make_generic_concrete_issubclass_negative,
    ),
    BenchmarkCase(
        family="typecheck",
        group="isinstance",
        implementation="ators_generic_concrete_negative",
        benchmark_name="typecheck/isinstance/ators_generic_concrete_negative",
        operation_factory=_make_generic_concrete_isinstance_negative,
    ),
    BenchmarkCase(
        family="typecheck",
        group="issubclass",
        implementation="ators_generic_typevar_negative",
        benchmark_name="typecheck/issubclass/ators_generic_typevar_negative",
        operation_factory=_make_generic_typevar_issubclass_negative,
    ),
    BenchmarkCase(
        family="typecheck",
        group="isinstance",
        implementation="ators_generic_typevar_negative",
        benchmark_name="typecheck/isinstance/ators_generic_typevar_negative",
        operation_factory=_make_generic_typevar_isinstance_negative,
    ),
]


def select_typecheck_cases(
    families: set[str] | None,
    groups: set[str] | None,
    implementations: set[str] | None,
) -> list[BenchmarkCase]:
    """Return typecheck benchmark cases filtered by the given criteria."""
    result = []
    for case in _CASES:
        if families is not None and case.family not in families:
            continue
        if groups is not None and case.group not in groups:
            continue
        if implementations is not None and case.implementation not in implementations:
            continue
        result.append(case)
    return result
