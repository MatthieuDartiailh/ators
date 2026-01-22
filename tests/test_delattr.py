# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test delattr behavior for ators object"""

import pytest

from ators import Ators, member
from ators.behaviors import DelAttr


def test_delattr():
    class A(Ators):
        a: int = 5

    a = A()
    a.a = 1
    assert a.a == 1
    del a.a
    assert a.a == 5


def test_forbidden_delattr():
    class A(Ators):
        a: int = member().del_(DelAttr.Undeletable())

    a = A()
    a.a = 1
    with pytest.raises(TypeError) as e:
        del a.a

    assert "cannot be deleted" in e.exconly()


def test_inherited_delattr_behavior():
    class A(Ators):
        a: int = member().del_(DelAttr.Undeletable())

    class B(A):
        a = member().inherit()

    b = B()
    b.a = 1
    with pytest.raises(TypeError) as e:
        del b.a
    assert "cannot be deleted" in e.exconly()


def test_warn_on_multiple_setting_of_del_():
    with pytest.warns(UserWarning):

        class A(Ators):
            a: int = member().del_(DelAttr.Slot()).del_(DelAttr.Undeletable())
