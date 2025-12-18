# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test pre-getattr behavior for ators object"""

import pytest

from ators import Ators, member, Member
from ators.behaviors import PreGetAttr, preget


def test_call_name_object_preget():
    i = 0
    m = None
    obj = None

    def pre_get(member, object):
        nonlocal i, m, obj
        i += 1
        m = member
        obj = object
        return 5

    class A(Ators):
        a: int = member().preget(PreGetAttr.CallNameObject(pre_get))

    a = A()
    a.a = 2
    assert a.a == 2
    assert i == 1
    assert isinstance(m, str)
    assert isinstance(obj, A)
    assert a.a == 2
    assert i == 2


def test_method_preget():
    i = 0
    me = None

    class A(Ators):
        a: int = member()

        @preget(a)
        def _preget_a(self, m):
            nonlocal i, me
            me = m
            i += 1
            return 8

    a = A()
    a.a = 2
    assert a.a == 2
    assert isinstance(me, str)
    assert i == 1
    assert a.a == 2
    assert i == 2

    class B(A):
        def _preget_a(self, m):
            nonlocal i
            i += 2
            return 9

    b = B()
    b.a = 5
    assert b.a == 5
    assert i == 4


def test_inherited_preget_behavior():
    i = 0

    def pre_get(name, object):
        nonlocal i
        i += 1

    class A(Ators):
        a: int = member().preget(PreGetAttr.CallNameObject(pre_get))

    class B(A):
        a = member().inherit()

    b = B()
    b.a = 5
    assert b.a == 5
    assert i == 1


@pytest.mark.parametrize(
    "behavior, callable, expected, got",
    [(PreGetAttr.CallNameObject, lambda: 1, 2, 0)],
)
def test_bad_signature(behavior, callable, expected, got):
    with pytest.raises(ValueError) as e:

        class A(Ators):
            a: int = member().preget(behavior(callable))

    assert f"callable taking {expected}" in e.exconly()
    assert f"which takes {got}" in e.exconly()


def test_preget_not_as_decorator():
    with pytest.raises(RuntimeError) as e:

        class A(Ators):
            m = member()

            def f(self, m):
                pass

            preget(m)(f)

    assert "'preget' can only be used as a decorator" in e.exconly()


def test_preget_outside_class_body():
    with pytest.raises(RuntimeError) as e:
        m = member()

        @preget(m)
        def f(self, m):
            pass

    assert "'preget' can only be used inside a class body" in e.exconly()


def test_bad_signature_of_method():
    with pytest.raises(TypeError) as e:

        class A(Ators):
            m = member()

            @preget(m)
            def f(self):
                pass

    assert "Method signature for 'preget'" in e.exconly()


def test_warn_on_multiple_setting_of_preget():
    with pytest.warns(UserWarning):

        class A(Ators):
            a: int = (
                member()
                .preget(PreGetAttr.CallNameObject(lambda m, o: 1))
                .preget(PreGetAttr.NoOp())
            )
