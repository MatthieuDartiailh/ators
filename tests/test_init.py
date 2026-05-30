# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Unit tests for init support in member and AtorsMeta."""

import pytest

from ators import Ators, get_members, member


def test_public_member_init_default_true():
    """Public members default to init=True."""

    class A(Ators):
        x: int

    a = A(x=1)
    assert a.x == 1


def test_private_member_init_default_false():
    """Members whose names start with '_' default to init=False."""

    class A(Ators):
        _x: int

    with pytest.raises(TypeError, match="non-init member"):
        A(_x=1)


def test_explicit_init_false():
    """Explicit init=False marks the member as non-initializable."""

    class A(Ators):
        x: int = member(init=False)

    with pytest.raises(TypeError, match="non-init member"):
        A(x=1)


def test_explicit_init_true():
    """Explicit init=True is respected even for private-looking names."""

    class A(Ators):
        _x: int = member(init=True)

    a = A(_x=42)
    assert a._x == 42


def test_init_true_accepted_in_init():
    """Members with init=True (default for public names) can be passed to __init__."""

    class A(Ators):
        x: int

    a = A(x=42)
    assert a.x == 42


def test_get_members_exposes_init_flag():
    """get_members exposes the init flag on every member."""

    class A(Ators):
        x: int
        y: int = member(init=False)
        _z: int

    members = get_members(A)
    assert members["x"].init is True
    assert members["y"].init is False
    assert members["_z"].init is False


def test_inherited_member_init_preserved():
    """Init flags are inherited correctly in subclasses."""

    class Base(Ators):
        x: int
        y: int = member(init=False)

    class Child(Base):
        pass

    a = Child(x=10)
    assert a.x == 10
    with pytest.raises(TypeError, match="non-init member"):
        Child(x=10, y=5)


def test_subclass_can_override_init_flag():
    """A subclass can override the init flag for an inherited member."""

    class Base(Ators):
        x: int  # init=True by default

    class Child(Base):
        x: int = member(init=False)

    b = Base(x=1)
    assert b.x == 1
    with pytest.raises(TypeError, match="non-init member"):
        Child(x=1)
