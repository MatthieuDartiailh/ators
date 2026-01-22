# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test type validation for ators object"""

from abc import ABC
from typing import Any, Literal, TYPE_CHECKING

import pytest

from ators import Ators, member, add_generic_type_attributes

if TYPE_CHECKING:
    from logging import Logger


class OB:
    pass


class CustomBase(ABC):
    pass


class CustomObj:
    pass


CustomBase.register(CustomObj)


class MyGen[T]:
    a: T

    def __init__(self, a: T):
        self.a = a


add_generic_type_attributes(MyGen, ("a",))


class UnknownGen[T]:
    a: T

    def __init__(self, a: T):
        self.a = a


type MyInt = int


# FIXME validate error messages
@pytest.mark.parametrize(
    "ann, goods, bads",
    [
        (object, [1, object()], []),
        (Any, [1, object()], []),
        (bool, [False, True], [""]),
        (int, [0, 1, -1], [1.0, ""]),
        (MyInt, [0, 1, -1], [1.0, ""]),
        (float, [0.0, 0.1], [1, ""]),
        (complex, [0.0 + 0j, 0.1j], [1, 1.0, ""]),
        (str, ["a"], [1]),
        (bytes, [b"a"], [""]),
        (OB, [OB()], [""]),
        (tuple, [()], [1, ""]),
        (tuple[int, ...], [(), (1,), (1, 2, 3)], [1, ("a",)]),
        (tuple[int, int], [(1, 2)], [1, (), (1,), (1, 2, 3), (1, "a")]),
        (frozenset, [frozenset(), frozenset((1,)), frozenset({1, "a"})], [1, ()]),
        (frozenset[int], [frozenset(), frozenset((1,))], [1, (), frozenset({1, "a"})]),
        (set, [set(), {1}, {1, "a"}], [1, ()]),
        (set[int], [set(), {1}], [1, (), {1, "a"}]),
        (dict, [{}, {1: 1}, {1: "a"}], [1, ()]),
        (dict[int, int], [{}, {1: 1}], [1, (), {1: "a"}, {"1": 1}, {"1": "a"}]),
        # NOTE Not a type validation
        (Literal[1, 2, 3], [1, 2, 3], [0, 4, "a"]),
        (CustomBase, [CustomObj()], ["", 1, object()]),
        (int | str, [1, "a"], [1.0, object()]),
        (int | str | None, [1, "a", None], [1.0, object()]),
        (int | tuple[int, int], [1, (1, 2)], [1.0, (1, 2, 3), "c", object()]),
        (int | Literal["a", "b"], [1, "a", "b"], [1.0, "c", object()]),
        (MyGen[int], [MyGen(1)], [MyGen("a"), MyGen(object()), 1, object()]),
        # Should warn about unknown generic type
        (
            UnknownGen[int],
            [UnknownGen(1), UnknownGen("a"), UnknownGen(object())],
            [1, object()],
        ),
    ],
)
def test_type_validators(ann, goods, bads):
    class A(Ators):
        a: ann = member()

    a = A()
    for good in goods:
        a.a = good
        assert a.a == good

    for bad in bads:
        with pytest.raises((TypeError, ValueError)):
            a.a = bad


def test_forward_ref_support_self_reference():
    class A(Ators):
        a: A = member()

    a1 = A()
    a2 = A()
    a1.a = a2
    assert a1.a is a2
    with pytest.raises(TypeError):
        a1.a = 5


@pytest.mark.parametrize(
    "resolver", [lambda: __import__("logging").__dict__, "logging", ["logging"]]
)
def test_forward_ref_support_callable_and_type_alias(resolver):
    type L = Logger

    class A(Ators):
        a: L = member().forward_ref_environment(resolver)
        b: Logger | int = member().forward_ref_environment(resolver)

    a1 = A()
    import logging

    a1.a = logging.getLogger("test")
    with pytest.raises(TypeError):
        a1.a = 5
    a1.b = logging.getLogger("test")
    a1.b = 5
    with pytest.raises(TypeError):
        a1.b = ""


def test_inherited_type_validator():
    class A(Ators):
        a: int

    class B(A):
        a = member().inherit()

    b = B()
    b.a = 5
    assert b.a == 5
    with pytest.raises(TypeError):
        b.a = ""
