# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for Member.__class_getitem__."""

from types import GenericAlias

import pytest

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
# Negative cases: wrong arity
# ---------------------------------------------------------------------------


def test_class_getitem_single_arg_raises_type_error():
    """Member[int] raises TypeError (too few arguments)."""
    with pytest.raises(TypeError, match="exactly 2 arguments, got 1"):
        Member[int]


def test_class_getitem_three_args_raises_type_error():
    """Member[int, str, float] raises TypeError (too many arguments)."""
    with pytest.raises(TypeError, match="exactly 2 arguments, got 3"):
        Member[int, str, float]


def test_class_getitem_zero_args_raises_type_error():
    """Member[()] raises TypeError (zero arguments)."""
    with pytest.raises(TypeError, match="exactly 2 arguments, got 0"):
        Member[()]
