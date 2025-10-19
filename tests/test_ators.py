# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test default behavior for ators object"""

import pytest

from ators import Ators, member
from ators.behaviors import PreSetAttr, DelAttr


def test_member_slot_do_not_overlap():
    class A(Ators):
        a = member()
        b = member()

    a = A()
    a.a = 1
    a.b = 2
    assert a.a == 1
    assert a.b == 2


def test_dual_use_is_forbidden():
    with pytest.raises(RuntimeError) as e:

        class A(Ators):
            a = b = member()

    assert "assigned the same member" in e.exconly()


def test_member_constant():
    class A(Ators):
        a = member().constant()

    assert isinstance(A.a.pre_setattr, PreSetAttr.Constant)
    assert isinstance(A.a.delattr, DelAttr.Undeletable)


@pytest.mark.parametrize("kwargs", [{}, {"a": 2}, {"b": 2}, {"a": 3, "b": 4}])
def test_ators_init(kwargs):
    class A(Ators):
        a: int
        b: int = 1

    a = A(**kwargs)
    if "a" in kwargs:
        assert a.a == kwargs["a"]
    else:
        with pytest.raises(TypeError):
            a.a

    assert a.b == kwargs.get("b", 1)
