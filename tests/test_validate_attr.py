# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for the validate_attr metaclass option."""

from typing import ClassVar, Final

import pytest

from ators import Ators, member
from ators.behaviors import DelAttr, PreSetAttr

# ---------------------------------------------------------------------------
# Basic functionality with validate_attr=False
# ---------------------------------------------------------------------------


def test_validate_attr_false_basic():
    """Members are created without type validation when validate_attr=False."""

    class A(Ators, validate_attr=False):
        x: int

    a = A()
    a.x = "not an int"
    assert a.x == "not an int"

    a.x = 42
    assert a.x == 42


def test_validate_attr_false_default_value():
    """Default values still work when validate_attr=False."""

    class A(Ators, validate_attr=False):
        x: int = 1

    a = A()
    assert a.x == 1


def test_validate_attr_false_member_builder():
    """Explicit member builders still work when validate_attr=False."""

    class A(Ators, validate_attr=False):
        x: int = member()

    a = A()
    a.x = "hello"
    assert a.x == "hello"


# ---------------------------------------------------------------------------
# ClassVar handling with validate_attr=False
# ---------------------------------------------------------------------------


def test_validate_attr_false_classvar_parameterized():
    """ClassVar[T] annotations are still ignored as instance members."""

    class A(Ators, validate_attr=False):
        cls_attr: ClassVar[int] = 42
        x: str

    assert A.cls_attr == 42
    a = A()
    a.x = "hello"
    assert "cls_attr" not in dir(type(a).__ators_members__)


def test_validate_attr_false_classvar_bare():
    """Bare ClassVar annotations are still ignored as instance members."""

    class A(Ators, validate_attr=False):
        cls_attr: ClassVar = "hello"
        x: int

    assert A.cls_attr == "hello"
    a = A()
    a.x = 1


# ---------------------------------------------------------------------------
# Final handling with validate_attr=False
# ---------------------------------------------------------------------------


def test_validate_attr_false_final_parameterized():
    """Final[T] enforces read-only semantics even when validate_attr=False."""

    class A(Ators, validate_attr=False):
        x: Final[int] = 5

    a = A()
    assert a.x == 5

    assert isinstance(A.x.pre_setattr, PreSetAttr.ReadOnly)
    assert isinstance(A.x.delattr, DelAttr.Undeletable)

    with pytest.raises(AttributeError):
        a.x = 10

    with pytest.raises(AttributeError):
        del a.x


def test_validate_attr_false_final_bare():
    """Bare Final enforces read-only semantics even when validate_attr=False."""

    class A(Ators, validate_attr=False):
        x: Final = 5

    a = A()
    assert a.x == 5

    assert isinstance(A.x.pre_setattr, PreSetAttr.ReadOnly)
    assert isinstance(A.x.delattr, DelAttr.Undeletable)

    with pytest.raises(AttributeError):
        a.x = 10


# ---------------------------------------------------------------------------
# Coerce validation with validate_attr=False
# ---------------------------------------------------------------------------


def test_validate_attr_false_explicit_coerce_fails():
    """Explicit coerce on a member raises ValueError when validate_attr=False."""
    with pytest.raises(ValueError, match="Class creation failed: attribute 'x'"):

        class A(Ators, validate_attr=False):
            x: int = member().coerce()


def test_validate_attr_false_explicit_coerce_init_fails():
    """Explicit coerce_init on a member raises ValueError when validate_attr=False."""
    with pytest.raises(ValueError, match="Class creation failed: attribute 'x'"):

        class A(Ators, validate_attr=False):
            x: int = member().coerce_init()


def test_validate_attr_false_inherited_coerce_fails():
    """Inherited coercer from a base class raises ValueError when validate_attr=False."""

    class Base(Ators):
        x: int = member().coerce()

    with pytest.raises(ValueError, match="Class creation failed: attribute 'x'"):

        class Child(Base, validate_attr=False):
            pass


# ---------------------------------------------------------------------------
# validate_attr=True (default) still works as before
# ---------------------------------------------------------------------------


def test_validate_attr_true_default():
    """Default validate_attr=True still validates types."""

    class A(Ators):
        x: int

    a = A()
    with pytest.raises(TypeError):
        a.x = "not an int"


def test_validate_attr_true_explicit():
    """Explicit validate_attr=True still validates types."""

    class A(Ators, validate_attr=True):
        x: int

    a = A()
    with pytest.raises(TypeError):
        a.x = "not an int"


# ---------------------------------------------------------------------------
# Inheritance with validate_attr=False
# ---------------------------------------------------------------------------


def test_validate_attr_false_subclass():
    """Subclasses can also use validate_attr=False."""

    class Base(Ators, validate_attr=False):
        x: int

    class Child(Base, validate_attr=False):
        y: str

    a = Child()
    a.x = "not validated"
    a.y = 123
    assert a.x == "not validated"
    assert a.y == 123
