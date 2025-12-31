# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test default behavior for ators object"""

import pytest

from ators import (
    Ators,
    member,
    get_member,
    get_members,
    get_members_by_tag,
    get_members_by_tag_and_value,
    get_member_customization_tool,
)
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


def test_member_access_fucntions():
    class A(Ators):
        a = member().tag(t=1)
        b = member().tag(t=2)
        c = member().tag(u=1)
        d = member().tag()

    for obj in (A, A()):
        assert get_member(obj, "d").name == "d"
        assert list(sorted(get_members(obj))) == ["a", "b", "c", "d"]
        for k, (m, v) in get_members_by_tag(obj, "t").items():
            assert m.name == k
            assert v == {"a": 1, "b": 2}[k]
        assert list(get_members_by_tag_and_value(obj, "t", 1)) == ["a"]


def test_member_init_subclass():
    class A(Ators):
        a = member().constant()

        def __init_subclass__(cls):
            t = get_member_customization_tool(cls)
            for m in get_members(cls):
                t[m].tag(a=1)

    class B(A):
        b = member().constant()

    assert isinstance(B.a.pre_setattr, PreSetAttr.Constant)
    assert isinstance(B.a.delattr, DelAttr.Undeletable)
    assert B.a.metadata == {"a": 1}
    assert isinstance(B.b.pre_setattr, PreSetAttr.Constant)
    assert isinstance(B.b.delattr, DelAttr.Undeletable)
    assert B.b.metadata == {"a": 1}
    assert B.__ators_members__["a"] is B.a
    assert B.__ators_members__["b"] is B.b
    assert B.a in B.__ators_specific_members__
    assert B.b in B.__ators_specific_members__

    with pytest.raises(RuntimeError):
        get_member_customization_tool(B)
