# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test pre-setattr behavior for ators object"""

from typing import Final

import pytest

from ators import Ators, member, Member
from ators.behaviors import PreSetAttr, preset


def test_constant_preset():
    class A(Ators):
        a: Final[int] = member().preset(PreSetAttr.Constant()).default(1)

    a = A()
    assert a.a == 1
    with pytest.raises(TypeError) as e:
        a.a = 1

    assert "constant" in e.exconly()


def test_constant_preset_bad_annotation():
    with pytest.raises(TypeError) as e:

        class A(Ators):
            a: int = member().preset(PreSetAttr.Constant()).default(1)

    assert "Failed to configure" in e.exconly()


def test_read_only_preset():
    class A(Ators):
        a: Final[int] = member().default(1)

    a = A()
    assert a.a == 1
    with pytest.raises(TypeError) as e:
        a.a = 1

    assert "read only" in e.exconly()

    a = A()
    a.a = 2
    assert a.a == 2
    with pytest.raises(TypeError) as e:
        a.a = 1

    assert "read only" in e.exconly()


def test_read_only_preset_bad_annotation():
    with pytest.raises(TypeError) as e:

        class A(Ators):
            a: int = member().preset(PreSetAttr.ReadOnly()).default(1)

    assert "Failed to configure" in e.exconly()


def test_call_member_object_value_preset():
    i = 0
    m = None
    obj = None
    current = None

    def pre_set(member, object, c):
        nonlocal i, m, obj, current
        i += 1
        m = member
        obj = object
        current = c
        return 5

    class A(Ators):
        a: int = member().preset(PreSetAttr.CallMemberObjectValue(pre_set))

    a = A()
    a.a = 2
    assert a.a == 2
    assert i == 1
    assert isinstance(m, Member)
    assert isinstance(obj, A)
    assert current is None

    a.a = 5
    assert a.a == 5
    assert i == 2
    assert isinstance(m, Member)
    assert isinstance(obj, A)
    assert current == 2


def test_method_preset():
    i = 0
    me = None
    current = None

    class A(Ators):
        a: int = member()

        @preset(a)
        def _preset_a(self, m, c):
            nonlocal i, me, current
            me = m
            current = c
            i += 1
            return 8

    a = A()
    a.a = 2
    assert a.a == 2
    assert i == 1
    assert isinstance(me, Member)
    assert current is None

    a.a = 4
    assert a.a == 4
    assert isinstance(me, Member)
    assert current == 2
    assert i == 2

    class B(A):
        def _preset_a(self, m, c):
            nonlocal i
            i += 2
            return 9

    b = B()
    b.a = 5
    assert i == 4


def test_inherited_preset_behavior():
    i = 0

    def pre_set(member, object, c):
        nonlocal i
        i += 1

    class A(Ators):
        a: int = member().preset(PreSetAttr.CallMemberObjectValue(pre_set))

    class B(A):
        a = member().inherit()

    b = B()
    b.a = 5
    assert i == 1


@pytest.mark.parametrize(
    "behavior, callable, expected, got",
    [(PreSetAttr.CallMemberObjectValue, lambda: 1, 3, 0)],
)
def test_bad_signature(behavior, callable, expected, got):
    with pytest.raises(ValueError) as e:

        class A(Ators):
            a: int = member().preset(behavior(callable))

    assert f"callable taking {expected}" in e.exconly()
    assert f"which takes {got}" in e.exconly()


def test_preset_not_as_decorator():
    with pytest.raises(RuntimeError) as e:

        class A(Ators):
            m = member()

            def f(self, m, c):
                pass

            preset(m)(f)

    assert "'preset' can only be used as a decorator" in e.exconly()


def test_preset_outside_class_body():
    with pytest.raises(RuntimeError) as e:
        m = member()

        @preset(m)
        def f(self, m, c):
            pass

    assert "'preset' can only be used inside a class body" in e.exconly()


def test_bad_signature_of_method():
    with pytest.raises(TypeError) as e:

        class A(Ators):
            m = member()

            @preset(m)
            def f(self):
                pass

    assert "Method signature for 'preset'" in e.exconly()


def test_warn_on_multiple_setting_of_postget():
    with pytest.warns(UserWarning):

        class A(Ators):
            a: int = (
                member()
                .preset(PreSetAttr.CallMemberObjectValue(lambda m, o, c: 1))
                .preset(PreSetAttr.NoOp())
            )
