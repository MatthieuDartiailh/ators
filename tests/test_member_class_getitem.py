# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for Member.__class_getitem__."""

from types import GenericAlias

from ators import Member

# ---------------------------------------------------------------------------
# Positive cases
# ---------------------------------------------------------------------------


def test_class_getitem_returns_generic_alias():
    """Member[int, str] returns a types.GenericAlias."""
    ga = Member[int, str]
    assert isinstance(ga, GenericAlias)


def test_class_getitem_origin_is_member():
    """The __origin__ of Member[int, str] is Member."""
    ga = Member[int, str]
    assert ga.__origin__ is Member


def test_class_getitem_args_are_correct():
    """The __args__ of Member[int, str] is (int, str)."""
    ga = Member[int, str]
    assert ga.__args__ == (int, str)


def test_class_getitem_different_types():
    """Member[float, bytes] produces the expected alias."""
    ga = Member[float, bytes]
    assert isinstance(ga, GenericAlias)
    assert ga.__origin__ is Member
    assert ga.__args__ == (float, bytes)


# ---------------------------------------------------------------------------
# Arity: wrong-arity subscriptions create a GenericAlias; the error is only
# raised later by the metaclass pairing check (see test_coercion.py).
# ---------------------------------------------------------------------------


def test_class_getitem_single_arg_creates_alias():
    """Member[int] creates a GenericAlias with one arg (arity checked by metaclass)."""
    ga = Member[int]  # type: ignore[type-arg]
    assert isinstance(ga, GenericAlias)
    assert ga.__origin__ is Member
    assert ga.__args__ == (int,)


def test_class_getitem_three_args_creates_alias():
    """Member[int, str, float] creates a GenericAlias (arity checked by metaclass)."""
    ga = Member[int, str, float]  # type: ignore[type-arg]
    assert isinstance(ga, GenericAlias)
    assert ga.__origin__ is Member
    assert ga.__args__ == (int, str, float)


def test_class_getitem_zero_args_creates_alias():
    """Member[()] creates a GenericAlias with zero args (arity checked by metaclass)."""
    ga = Member[()]  # type: ignore[type-arg]
    assert isinstance(ga, GenericAlias)
    assert ga.__origin__ is Member
    assert ga.__args__ == ()
