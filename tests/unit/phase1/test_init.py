# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Unit tests for Phase 1: init support in member and AtorsMeta."""

import pytest

from ators import Ators, member


def test_public_member_init_default_true():
    """Public members default to init=True."""

    class A(Ators):
        x: int

    assert A.__ators_members__["x"].init is True


def test_private_member_init_default_false():
    """Members whose names start with '_' default to init=False."""

    class A(Ators):
        _x: int

    assert A.__ators_members__["_x"].init is False


def test_explicit_init_false():
    """Explicit init=False marks the member as non-initializable."""

    class A(Ators):
        x: int = member(init=False)

    assert A.__ators_members__["x"].init is False


def test_explicit_init_true():
    """Explicit init=True is respected even for private-looking names."""

    class A(Ators):
        _x: int = member(init=True)

    assert A.__ators_members__["_x"].init is True


def test_init_false_raises_on_kwarg():
    """Passing a kwarg for a member with init=False raises TypeError."""

    class A(Ators):
        x: int = member(init=False)

    with pytest.raises(TypeError, match="unexpected keyword argument 'x'"):
        A(x=1)


def test_private_default_init_false_raises_on_kwarg():
    """Passing a kwarg for a private member (init=False by default) raises TypeError."""

    class A(Ators):
        _x: int

    with pytest.raises(TypeError, match="unexpected keyword argument '_x'"):
        A(_x=1)


def test_init_true_accepted_in_init():
    """Members with init=True (default for public names) can be passed to __init__."""

    class A(Ators):
        x: int

    a = A(x=42)
    assert a.x == 42


def test_ators_members_exposes_init_flag():
    """__ators_members__ exposes the init flag on every member."""

    class A(Ators):
        x: int
        y: int = member(init=False)
        _z: int

    assert A.__ators_members__["x"].init is True
    assert A.__ators_members__["y"].init is False
    assert A.__ators_members__["_z"].init is False


def test_inherited_member_init_preserved():
    """Init flags are inherited correctly in subclasses."""

    class Base(Ators):
        x: int
        y: int = member(init=False)

    class Child(Base):
        pass

    assert Child.__ators_members__["x"].init is True
    assert Child.__ators_members__["y"].init is False


def test_subclass_can_override_init_flag():
    """A subclass can override the init flag for an inherited member."""

    class Base(Ators):
        x: int  # init=True by default

    class Child(Base):
        x: int = member(init=False)

    assert Base.__ators_members__["x"].init is True
    assert Child.__ators_members__["x"].init is False
