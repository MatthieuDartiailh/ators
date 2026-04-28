# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for generic-aware runtime subclass / instance checks."""

import typing
from typing import Any, TypeVar

import pytest

from ators import Ators

# ---------------------------------------------------------------------------
# Generic Ators class fixture (PEP-695 syntax, Python 3.14+)
# ---------------------------------------------------------------------------


class G[T, U](Ators):
    """A two-parameter generic Ators base class."""


T, U = G.__type_params__
TBound = TypeVar("TBound", bound=int)
TCon = TypeVar("TCon", int, float)


# ---------------------------------------------------------------------------
# A.1  issubclass - positive cases
# ---------------------------------------------------------------------------


def test_subclass_exact_concrete():
    """issubclass(G[int, str], G[int, str]) is True (exact match)."""
    assert issubclass(G[int, str], G[int, str]) is True


def test_subclass_typevar_wildcard_first_arg():
    """issubclass(G[int, str], G[T, str]) is True (T is unconstrained wildcard)."""
    assert issubclass(G[int, str], G[T, str]) is True


def test_subclass_typevar_wildcard_both_args():
    """issubclass(G[int, str], G[T, U]) is True (both args are TypeVar wildcards)."""
    assert issubclass(G[int, str], G[T, U]) is True


def test_subclass_any_rhs():
    """issubclass(G[int, str], G[Any, Any]) is True."""
    assert issubclass(G[int, str], G[Any, Any]) is True


def test_subclass_any_rhs_mixed():
    """issubclass(G[int, str], G[int, Any]) is True."""
    assert issubclass(G[int, str], G[int, Any]) is True


def test_subclass_concrete_subtype():
    """issubclass(G[bool, str], G[int, str]) is True (bool is subclass of int)."""
    assert issubclass(G[bool, str], G[int, str]) is True


def test_subclass_typevar_bound_satisfied():
    """TypeVar with bound: concrete arg satisfies the bound."""
    assert issubclass(G[int, str], G[TBound, str]) is True


def test_subclass_typevar_constraint_satisfied():
    """TypeVar with constraints: concrete arg is one of them."""
    assert issubclass(G[int, str], G[TCon, str]) is True


# ---------------------------------------------------------------------------
# A.2  issubclass - negative cases
# ---------------------------------------------------------------------------


def test_subclass_wrong_concrete_second_arg():
    """issubclass(G[int, bytes], G[T, str]) is False."""
    assert issubclass(G[int, bytes], G[T, str]) is False


def test_subclass_wrong_concrete_first_arg():
    """issubclass(G[str, str], G[int, str]) is False (str is not subclass of int)."""
    assert issubclass(G[str, str], G[int, str]) is False


def test_subclass_typevar_bound_violated():
    """TypeVar with bound: concrete arg does NOT satisfy the bound."""
    assert issubclass(G[str, str], G[TBound, str]) is False


def test_subclass_typevar_constraint_violated():
    """TypeVar with constraints: concrete arg is NOT one of them."""
    assert issubclass(G[str, str], G[TCon, str]) is False


def test_subclass_origin_mismatch():
    """Two unrelated generic classes are not compatible."""

    class H[T, U](Ators):
        pass

    h_T = H.__type_params__[0]
    assert issubclass(G[int, str], H[h_T, str]) is False


def test_subclass_non_specialized_lhs():
    """issubclass(G, G[T, str]) is False: plain class vs specialised."""
    assert issubclass(G, G[T, str]) is False


# ---------------------------------------------------------------------------
# B.1  isinstance - positive cases
# ---------------------------------------------------------------------------


def test_isinstance_typevar_wildcard():
    """isinstance(obj, G[T, str]) is True when type(obj) is G[int, str]."""
    obj = G[int, str]()
    assert isinstance(obj, G[T, str]) is True


def test_isinstance_exact():
    """isinstance(obj, G[int, str]) is True when type(obj) is G[int, str]."""
    obj = G[int, str]()
    assert isinstance(obj, G[int, str]) is True


def test_isinstance_any_rhs():
    """isinstance(obj, G[Any, Any]) is True."""
    obj = G[int, str]()
    assert isinstance(obj, G[Any, Any]) is True


def test_isinstance_bound_satisfied():
    """isinstance(obj, G[TBound, str]) is True when first arg satisfies bound."""
    obj = G[int, str]()
    assert isinstance(obj, G[TBound, str]) is True


# ---------------------------------------------------------------------------
# B.2  isinstance - negative cases
# ---------------------------------------------------------------------------


def test_isinstance_wrong_arg():
    """isinstance(obj, G[T, bytes]) is False when obj is G[int, str]."""
    obj = G[int, str]()
    assert isinstance(obj, G[T, bytes]) is False


def test_isinstance_bound_violated():
    """isinstance(obj, G[TBound, str]) is False when first arg violates bound."""
    obj = G[str, str]()
    assert isinstance(obj, G[TBound, str]) is False


# ---------------------------------------------------------------------------
# C.  Validation - ForwardRef forbidden at specialisation time
# ---------------------------------------------------------------------------


def test_forward_ref_raises_at_specialisation():
    """ForwardRef in specialisation args raises TypeError immediately."""
    fref = typing.ForwardRef("int")
    with pytest.raises(TypeError, match="ForwardRef"):
        G[fref, str]


# ---------------------------------------------------------------------------
# D.  Rust-only path enforcement
# ---------------------------------------------------------------------------


def test_rust_subclasscheck_importable():
    """rust_subclasscheck is directly callable from _ators."""
    from ators._ators import rust_subclasscheck

    assert callable(rust_subclasscheck)


def test_rust_instancecheck_importable():
    """rust_instancecheck is directly callable from _ators."""
    from ators._ators import rust_instancecheck

    assert callable(rust_instancecheck)


# ---------------------------------------------------------------------------
# E.  Determinism / regression
# ---------------------------------------------------------------------------


def test_repeated_subclass_returns_same_result():
    """Warm-cache path returns the same result as cold-cache path."""
    for _ in range(10):
        assert issubclass(G[int, str], G[T, str]) is True
        assert issubclass(G[int, bytes], G[T, str]) is False


def test_non_generic_issubclass_unaffected():
    """Plain (non-generic) Ators classes still work with issubclass."""

    class Base(Ators):
        pass

    class Child(Base):
        pass

    assert issubclass(Child, Base) is True
    assert issubclass(Base, Child) is False


def test_non_generic_isinstance_unaffected():
    """Plain (non-generic) Ators classes still work with isinstance."""

    class Plain(Ators):
        pass

    obj = Plain()
    assert isinstance(obj, Plain) is True


def test_arity_mismatch_raises_at_specialisation():
    """Passing the wrong number of type args raises TypeError at specialisation."""
    with pytest.raises(TypeError):
        G[int, str, float]  # 3 args for a 2-param class
