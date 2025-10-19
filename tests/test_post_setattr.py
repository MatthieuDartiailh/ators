# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test post-setattr behavior for ators object"""

import pytest

from ators import Ators, member, Member
from ators.behaviors import PostSetAttr, postset


def test_call_member_object_old_new_postset():
    i = 0
    m = None
    obj = None
    old = None
    new = None

    def post_set(member, object, o, n):
        nonlocal i, m, obj, old, new
        i += 1
        m = member
        obj = object
        old = o
        new = n
        return 5

    class A(Ators):
        a: int = member().postset(PostSetAttr.CallMemberObjectOldNew(post_set))

    a = A()
    a.a = 2
    assert a.a == 2
    assert i == 1
    assert isinstance(m, Member)
    assert isinstance(obj, A)
    assert old is None
    assert new == 2

    a.a = 5
    assert a.a == 5
    assert i == 2
    assert isinstance(m, Member)
    assert isinstance(obj, A)
    assert old == 2
    assert new == 5


def test_method_postset():
    i = 0
    me = None
    old = None
    new = None

    class A(Ators):
        a: int = member()

        @postset(a)
        def _postset_a(self, m, o, n):
            nonlocal i, me, old, new
            me = m
            old = o
            new = n
            i += 1
            return 8

    a = A()
    a.a = 2
    assert a.a == 2
    assert isinstance(me, Member)
    assert old is None
    assert new == 2
    assert i == 1

    a.a = 4
    assert a.a == 4
    assert isinstance(me, Member)
    assert old == 2
    assert new == 4
    assert i == 2

    class B(A):
        def _postset_a(self, m, o, n):
            nonlocal i
            i += 2
            return 9

    b = B()
    b.a = 5
    assert i == 4


def test_inherited_postset_behavior():
    i = 0

    def post_set(member, object, o, n):
        nonlocal i
        i += 1

    class A(Ators):
        a: int = member().postset(PostSetAttr.CallMemberObjectOldNew(post_set))

    class B(A):
        a = member().inherit()

    b = B()
    b.a = 5
    assert i == 1


@pytest.mark.parametrize(
    "behavior, callable, expected, got",
    [(PostSetAttr.CallMemberObjectOldNew, lambda: 1, 4, 0)],
)
def test_bad_signature(behavior, callable, expected, got):
    with pytest.raises(ValueError) as e:

        class A(Ators):
            a: int = member().postset(behavior(callable))

    assert f"callable taking {expected}" in e.exconly()
    assert f"which takes {got}" in e.exconly()


def test_postset_not_as_decorator():
    with pytest.raises(RuntimeError) as e:

        class A(Ators):
            m = member()

            def f(self, m, o, n):
                pass

            postset(m)(f)

    assert "'postset' can only be used as a decorator" in e.exconly()


def test_postset_outside_class_body():
    with pytest.raises(RuntimeError) as e:
        m = member()

        @postset(m)
        def f(self, m, o, s):
            pass

    assert "'postset' can only be used inside a class body" in e.exconly()


def test_bad_signature_of_method():
    with pytest.raises(TypeError) as e:

        class A(Ators):
            m = member()

            @postset(m)
            def f(self):
                pass

    assert "Method signature for 'postset'" in e.exconly()


def test_warn_on_multiple_setting_of_postget():
    with pytest.warns(UserWarning):

        class A(Ators):
            a: int = (
                member()
                .postset(PostSetAttr.CallMemberObjectOldNew(lambda m, o, ol, n: 1))
                .postset(PostSetAttr.NoOp())
            )
