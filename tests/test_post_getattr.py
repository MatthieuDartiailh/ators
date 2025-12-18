# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test post-getattr behavior for ators object"""

import pytest

from ators import Ators, member, Member
from ators.behaviors import PostGetAttr, postget


def test_call_name_object_value_postget():
    i = 0
    m = None
    obj = None
    value = None

    def post_get(name, object, v):
        nonlocal i, m, obj, value
        i += 1
        m = name
        obj = object
        value = v
        return 5

    class A(Ators):
        a: int = member().postget(PostGetAttr.CallNameObjectValue(post_get))

    a = A()
    a.a = 2
    assert a.a == 2
    assert i == 1
    assert isinstance(m, str)
    assert isinstance(obj, A)
    assert value == 2
    assert a.a == 2
    assert i == 2


def test_method_postget():
    i = 0
    me = None
    value = None

    class A(Ators):
        a: int = member()

        @postget(a)
        def _postget_a(self, m, v):
            nonlocal i, me, value
            me = m
            value = v
            i += 1
            return 8

    a = A()
    a.a = 2
    assert a.a == 2
    assert isinstance(me, str)
    assert value == 2
    assert i == 1
    assert a.a == 2
    assert i == 2

    class B(A):
        def _postget_a(self, m, v):
            nonlocal i
            i += 2
            return 9

    b = B()
    b.a = 5
    assert b.a == 5
    assert i == 4


def test_inherited_postget_behavior():
    i = 0

    def post_get(member, object, v):
        nonlocal i
        i += 1

    class A(Ators):
        a: int = member().postget(PostGetAttr.CallNameObjectValue(post_get))

    class B(A):
        a = member().inherit()

    b = B()
    b.a = 5
    assert b.a == 5
    assert i == 1


@pytest.mark.parametrize(
    "behavior, callable, expected, got",
    [(PostGetAttr.CallNameObjectValue, lambda: 1, 3, 0)],
)
def test_bad_signature(behavior, callable, expected, got):
    with pytest.raises(ValueError) as e:

        class A(Ators):
            a: int = member().postget(behavior(callable))

    assert f"callable taking {expected}" in e.exconly()
    assert f"which takes {got}" in e.exconly()


def test_postget_not_as_decorator():
    with pytest.raises(RuntimeError) as e:

        class A(Ators):
            m = member()

            def f(self, m, v):
                pass

            postget(m)(f)

    assert "'postget' can only be used as a decorator" in e.exconly()


def test_postget_outside_class_body():
    with pytest.raises(RuntimeError) as e:
        m = member()

        @postget(m)
        def f(self, m, v):
            pass

    assert "'postget' can only be used inside a class body" in e.exconly()


def test_bad_signature_of_method():
    with pytest.raises(TypeError) as e:

        class A(Ators):
            m = member()

            @postget(m)
            def f(self):
                pass

    assert "Method signature for 'postget'" in e.exconly()


def test_warn_on_multiple_setting_of_postget():
    with pytest.warns(UserWarning):

        class A(Ators):
            a: int = (
                member()
                .postget(PostGetAttr.CallNameObjectValue(lambda m, o, v: 1))
                .postget(PostGetAttr.NoOp())
            )
