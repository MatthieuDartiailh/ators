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
from ators.member import Member
from ators.member.behaviors import Default, default


def test_no_default():
    class A(Ators):
        a: int

    a = A()
    with pytest.raises(TypeError) as e:
        a.a

    assert "value is unset and has no default" in e.exconly()


def test_static_default():
    class A(Ators):
        a: int = 2

    a = A()
    assert a.a == 2


def test_call_default():
    i = 0

    def make_default():
        nonlocal i
        i += 1
        return 5

    class A(Ators):
        a: int = member().default(Default.Call(make_default))

    a = A()
    assert a.a == 5
    assert i == 1
    assert a.a == 5
    assert i == 1


def test_call_member_object_default():
    i = 0
    m = None
    obj = None

    def make_default(member, object):
        nonlocal i, m, obj
        i += 1
        m = member
        obj = object
        return 5

    class A(Ators):
        a: int = member().default(Default.CallMemberObject(make_default))

    a = A()
    assert a.a == 5
    assert i == 1
    assert isinstance(m, Member)
    assert isinstance(obj, A)
    assert a.a == 5
    assert i == 1


def test_method_default():
    i = 0

    class A(Ators):
        a: int = member()

        @default(a)
        def _default_a(self, m):
            nonlocal i
            i += 1
            return 8

    a = A()
    assert a.a == 8
    assert i == 1
    assert a.a == 8
    assert i == 1

    class B(A):
        def _default_a(self, m):
            return 9

    assert B().a == 9


# XXX test bad usage of default decorator
# XXX test bad signature of function used in call and callmemberobject
# XXX test multiple setting of default
