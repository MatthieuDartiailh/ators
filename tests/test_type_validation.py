# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test type validation for ators object"""

from abc import ABC
from typing import TYPE_CHECKING, Any, Literal

import pytest

from ators import Ators, add_generic_type_attributes, member

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
    "ann, goods, bads, warn",
    [
        (object, [1, object()], [], False),
        (Any, [1, object()], [], False),
        (bool, [False, True], [""], False),
        (int, [0, 1, -1], [1.0, ""], False),
        (MyInt, [0, 1, -1], [1.0, ""], False),
        (float, [0.0, 0.1], [1, ""], False),
        (complex, [0.0 + 0j, 0.1j], [1, 1.0, ""], False),
        (str, ["a"], [1], False),
        (bytes, [b"a"], [""], False),
        (OB, [OB()], [""], False),
        (tuple, [()], [1, ""], False),
        (tuple[int, ...], [(), (1,), (1, 2, 3)], [1, ("a",)], False),
        (tuple[int, int], [(1, 2)], [1, (), (1,), (1, 2, 3), (1, "a")], False),
        (
            frozenset,
            [frozenset(), frozenset((1,)), frozenset({1, "a"})],
            [1, ()],
            False,
        ),
        (
            frozenset[int],
            [frozenset(), frozenset((1,))],
            [1, (), frozenset({1, "a"})],
            False,
        ),
        (set, [set(), {1}, {1, "a"}], [1, ()], False),
        (set[int], [set(), {1}], [1, (), {1, "a"}], False),
        (dict, [{}, {1: 1}, {1: "a"}], [1, ()], False),
        (dict[int, int], [{}, {1: 1}], [1, (), {1: "a"}, {"1": 1}, {"1": "a"}], False),
        # NOTE Not a type validation
        (Literal[1, 2, 3], [1, 2, 3], [0, 4, "a"], False),
        (CustomBase, [CustomObj()], ["", 1, object()], False),
        (int | str, [1, "a"], [1.0, object()], False),
        (int | str | None, [1, "a", None], [1.0, object()], False),
        (int | tuple[int, int], [1, (1, 2)], [1.0, (1, 2, 3), "c", object()], False),
        (int | Literal["a", "b"], [1, "a", "b"], [1.0, "c", object()], False),
        (MyGen[int], [MyGen(1)], [MyGen("a"), MyGen(object()), 1, object()], False),
        # Should warn about unknown generic type
        (
            UnknownGen[int],
            [UnknownGen(1), UnknownGen("a"), UnknownGen(object())],
            [1, object()],
            True,
        ),
    ],
)
def test_type_validators(ann, goods, bads, warn):

    if warn:
        with pytest.warns(UserWarning, match="No specific validation strategy"):

            class A(Ators):
                a: ann = member()
    else:

        class A(Ators):
            a: ann = member()

    a = A()
    for good in goods:
        a.a = good
        assert a.a == good

    for bad in bads:
        with pytest.raises((TypeError, ValueError)):
            a.a = bad


class SelfRefA(Ators):
    a: SelfRefA = member()


def test_forward_ref_support_self_reference():

    a1 = SelfRefA()
    a2 = SelfRefA()
    a1.a = a2
    assert a1.a is a2
    with pytest.raises(TypeError):
        a1.a = 5


class OutOfOrderA(Ators):
    a: OutOfOrderB
    b: tuple[OutOfOrderB, ...]


class OutOfOrderB(Ators):
    pass


@pytest.mark.parametrize(
    "attr, good, bad", [("a", OutOfOrderB(), 5), ("b", (OutOfOrderB(),), (5,))]
)
def test_forward_ref_support_out_of_order(attr, good, bad):

    a1 = OutOfOrderA()
    setattr(a1, attr, good)
    assert getattr(a1, attr) is good
    with pytest.raises(TypeError):
        setattr(a1, attr, bad)


def test_forward_ref_preserve_owner_in_subclasses():
    class NSRA(SelfRefA):
        pass

    class NOOA(OutOfOrderA):
        pass

    a1 = NSRA()
    a2 = NSRA()
    a1.a = a2
    assert a1.a is a2
    with pytest.raises(TypeError):
        a1.a = 5

    a1 = NOOA()
    b1 = OutOfOrderB()
    a1.a = b1
    assert a1.a is b1
    with pytest.raises(TypeError):
        a1.a = 5


def test_forward_ref_failed_to_resolve():
    class A(Ators):
        a: NonExistent = member()  # noqa : F821

    a1 = A()
    with pytest.raises(NameError) as e:
        a1.a = 5
    assert (
        "Failed to resolve forward reference for a: ForwardRef('NonExistent')"
        in str(e.value.__cause__)
    )


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
