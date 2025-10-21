# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test value value validation for ators object"""

import pytest

from ators import Ators, member, Member
from ators.behaviors import ValueValidator, append_value_validator


def test_enum_value_arg():
    assert ValueValidator.Enum(frozenset({1, 2, 3})).values == frozenset({1, 2, 3})
    assert ValueValidator.Enum({1, 2, 3}).values == frozenset({1, 2, 3})
    with pytest.raises(TypeError):
        ValueValidator.Enum([1, 2, 3])


def test_enumerated_value_validation():
    class A(Ators):
        a = member().append_value_validator(ValueValidator.Enum(frozenset({1, 2, 3})))

    a = A()
    a.a = 1
    assert a.a == 1
    a.a = 3
    assert a.a == 3

    with pytest.raises(ValueError) as e:
        a.a = -1
    assert "not in" in e.exconly()


def test_tuple_value_validation():
    class A(Ators):
        a = member().append_value_validator(
            ValueValidator.TupleItems(
                [
                    [ValueValidator.Enum(frozenset({1, 2, 3}))],
                    [ValueValidator.Enum(frozenset({4, 5, 6}))],
                ]
            )
        )

    a = A()
    a.a = (1, 4)
    assert a.a == (1, 4)
    a.a = (2, 5)
    assert a.a == (2, 5)

    with pytest.raises(ValueError) as e:
        a.a = (-1, 5)
    assert "Failed to validate item 0" in e.exconly()
    with pytest.raises(ValueError) as e:
        a.a = (2, -5)
    assert "Failed to validate item 1" in e.exconly()


def test_sequence_value_validation():
    class A(Ators):
        a = member().append_value_validator(
            ValueValidator.SequenceItems([ValueValidator.Enum(frozenset({1, 2, 3}))])
        )

    a = A()
    a.a = [1]
    assert a.a == [1]
    a.a = (3,)
    assert a.a == (3,)

    with pytest.raises(ValueError) as e:
        a.a = [-1]
    assert "Failed to validate item 0" in e.exconly()

    with pytest.raises(ValueError) as e:
        a.a = [1, -1]
    assert "Failed to validate item 1" in e.exconly()


def test_multiple_value_validators():
    class A(Ators):
        a = (
            member()
            .append_value_validator(ValueValidator.Enum(frozenset({1, 2, 3})))
            .append_value_validator(ValueValidator.Enum(frozenset({1, 4, 5})))
        )

    a = A()
    a.a = 1
    assert a.a == 1

    with pytest.raises(ValueError):
        a.a = 2


def test_call_value_value_validation():
    i = 0
    value = None

    def validate_value(v):
        nonlocal i, value
        i += 1
        value = v
        return 5

    class A(Ators):
        a: int = member().append_value_validator(
            ValueValidator.CallValue(validate_value)
        )

    a = A()
    a.a = 2
    assert a.a == 2
    assert i == 1
    assert value == 2

    a.a = 5
    assert a.a == 5
    assert i == 2
    assert value == 5


def test_call_member_object_value_value_validation():
    i = 0
    m = None
    obj = None
    value = None

    def validate_value(member, object, v):
        nonlocal i, m, obj, value
        i += 1
        m = member
        obj = object
        value = v
        return 5

    class A(Ators):
        a: int = member().append_value_validator(
            ValueValidator.CallMemberObjectValue(validate_value)
        )

    a = A()
    a.a = 2
    assert a.a == 2
    assert i == 1
    assert isinstance(m, Member)
    assert isinstance(obj, A)
    assert value == 2

    a.a = 5
    assert a.a == 5
    assert i == 2
    assert isinstance(m, Member)
    assert isinstance(obj, A)
    assert value == 5


def test_method_value_validation():
    i = 0
    me = None
    value = None

    class A(Ators):
        a: int = member()

        @append_value_validator(a)
        def _validate_a_value(self, m, v):
            nonlocal i, me, value
            me = m
            value = v
            i += 1
            return 8

    a = A()
    a.a = 2
    assert a.a == 2
    assert i == 1
    assert isinstance(me, Member)
    assert value == 2

    a.a = 4
    assert a.a == 4
    assert isinstance(me, Member)
    assert value == 4
    assert i == 2

    class B(A):
        def _validate_a_value(self, m, c):
            nonlocal i
            i += 2

    b = B()
    b.a = 5
    assert i == 4


def test_inherited_value_validation_behavior():
    i = 0

    def validate(member, object, v):
        nonlocal i
        i += 1
        if v != 5:
            raise ValueError

    class A(Ators):
        a: int = member().append_value_validator(
            ValueValidator.CallMemberObjectValue(validate)
        )

    class B(A):
        a = member().inherit()

    b = B()
    b.a = 5
    assert i == 1
    with pytest.raises(ValueError):
        b.a = 4
    assert i == 2


@pytest.mark.parametrize(
    "behavior, callable, expected, got",
    [
        (ValueValidator.CallValue, lambda: 1, 1, 0),
        (ValueValidator.CallMemberObjectValue, lambda: 1, 3, 0),
    ],
)
def test_bad_signature(behavior, callable, expected, got):
    with pytest.raises(ValueError) as e:

        class A(Ators):
            a: int = member().append_value_validator(behavior(callable))

    assert f"callable taking {expected}" in e.exconly()
    assert f"which takes {got}" in e.exconly()


def test_append_vv_not_as_decorator():
    with pytest.raises(RuntimeError) as e:

        class A(Ators):
            m = member()

            def f(self, m, c):
                pass

            append_value_validator(m)(f)

    assert "'append_value_validator' can only be used as a decorator" in e.exconly()


def test_append_vv_outside_class_body():
    with pytest.raises(RuntimeError) as e:
        m = member()

        @append_value_validator(m)
        def f(self, m, c):
            pass

    assert (
        "'append_value_validator' can only be used inside a class body" in e.exconly()
    )


def test_bad_signature_of_method():
    with pytest.raises(TypeError) as e:

        class A(Ators):
            m = member()

            @append_value_validator(m)
            def f(self):
                pass

    assert "Method signature for 'append_value_validator'" in e.exconly()
