# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test type validation for ators object"""

from abc import ABC
from annotationlib import ForwardRef
from typing import TYPE_CHECKING, Any, Literal, TypeVar

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


class GenericBox[T](Ators):
    item: T = member()


class BoundGenericBox[T: int](Ators):
    item: T = member()


class GenericListBox[T](Ators):
    items: list[T] = member()


class GenericPair[T, U](Ators):
    first: T = member()
    second: U = member()


class BoundedPair[T: int, U: int](Ators):
    first: T = member()
    second: U = member()


class ForwardRefPartialHolder[T: int](Ators):
    pair: ForwardRef("GenericPair[int, T]") = member()


class DelayedForwardRefPartialHolder[T: int](Ators):
    pair: ForwardRef("DelayedGenericPair[int, T]") = member()


class DelayedGenericPair[T, U](Ators):
    first: T = member()
    second: U = member()


def test_generic_specialization_is_cached_class():
    int_box = GenericBox[int]
    assert int_box is GenericBox[int]
    assert int_box is not GenericBox[str]


def test_full_and_stepwise_specialization_are_identical():
    U = TypeVar("U")
    direct = GenericPair[int, str]
    stepwise = GenericPair[int, U][str]
    assert direct is stepwise


def test_unspecialized_typevar_uses_bound_when_available():
    box = BoundGenericBox()
    box.item = 1
    with pytest.raises(TypeError):
        box.item = "a"


def test_unspecialized_unbound_typevar_remains_broad():
    box = GenericBox()
    box.item = 1
    box.item = "a"


def test_specialized_typevar_narrows_validator():
    IntBox = GenericBox[int]
    box = IntBox()
    box.item = 1
    with pytest.raises(TypeError):
        box.item = "a"


def test_specialized_nested_typevar_narrows_validator():
    IntListBox = GenericListBox[int]
    box = IntListBox()
    box.items = [1, 2, 3]
    with pytest.raises(TypeError):
        box.items = ["a"]


def test_partial_specialization_keeps_generic_parameter():
    T2 = TypeVar("T2", bound=int)
    partial = GenericPair[int, T2]

    assert len(partial.__type_params__) == 1
    assert partial.__type_params__[0] is T2

    pair = partial()
    pair.first = 1
    pair.second = 2
    with pytest.raises(TypeError):
        pair.second = "a"


def test_partial_specialization_can_be_fully_specialized_later():
    T2 = TypeVar("T2", bound=int)
    partial = GenericPair[int, T2]
    final = partial[bool]

    pair = final()
    pair.first = 1
    pair.second = True
    with pytest.raises(TypeError):
        pair.second = "a"


def test_partial_specialization_typevar_bound_must_be_narrower():
    narrower = TypeVar("narrower", bound=bool)
    _ = BoundedPair[int, narrower]

    wider = TypeVar("wider", bound=str)
    with pytest.raises(TypeError, match="not narrower"):
        _ = BoundedPair[int, wider]


def test_partial_specialization_typevar_without_required_bound_is_rejected():
    unbounded = TypeVar("unbounded")
    with pytest.raises(TypeError, match="must define a bound"):
        _ = BoundedPair[int, unbounded]


def test_forward_ref_support_partial_specialization():
    T2 = TypeVar("T2", bound=int)
    holder = ForwardRefPartialHolder[T2]()

    holder.pair = GenericPair[int, T2]()
    with pytest.raises(TypeError):
        holder.pair = GenericPair[str, T2]()


def test_delayed_forward_ref_support_partial_specialization():
    holder = DelayedForwardRefPartialHolder[int]()
    holder.pair = DelayedGenericPair[int, int]()
    with pytest.raises(TypeError):
        holder.pair = DelayedGenericPair[str, int]()


# ---------------------------------------------------------------------------
# Constrained TypeVar tests
# ---------------------------------------------------------------------------

T_constrained = TypeVar("T_constrained", int, str)


class ConstrainedBox(Ators):
    item: T_constrained = member()


def test_constrained_typevar_accepts_first_constraint():
    box = ConstrainedBox()
    box.item = 1


def test_constrained_typevar_accepts_second_constraint():
    box = ConstrainedBox()
    box.item = "hello"


def test_constrained_typevar_rejects_other_types():
    box = ConstrainedBox()
    with pytest.raises(TypeError):
        box.item = 1.5
    with pytest.raises(TypeError):
        box.item = []
    with pytest.raises(TypeError):
        box.item = {}


def test_constrained_typevar_matches_union_behavior():
    """Constrained TypeVar validation should match int | str union behavior."""

    class UnionBox(Ators):
        item: int | str = member(int | str)

    cbox = ConstrainedBox()
    ubox = UnionBox()

    for val in (1, "x"):
        cbox.item = val
        ubox.item = val

    for val in (1.5, [], {}):
        with pytest.raises(TypeError):
            cbox.item = val
        with pytest.raises(TypeError):
            ubox.item = val


# ---------------------------------------------------------------------------
# Constrained TypeVar generic class specialization tests
# ---------------------------------------------------------------------------

# PEP 695 syntax: [T: (int, str)] creates a constrained TypeVar
class ConstrainedGenericBox[T: (int, str)](Ators):
    item: T = member()


class ConstrainedGenericPair[T: (int, str), U](Ators):
    first: T = member()
    second: U = member()


def test_constrained_generic_unspecialized_accepts_constraints():
    box = ConstrainedGenericBox()
    box.item = 1
    box.item = "hello"


def test_constrained_generic_unspecialized_rejects_other_types():
    box = ConstrainedGenericBox()
    with pytest.raises(TypeError):
        box.item = 1.5


def test_constrained_generic_specialized_with_first_constraint():
    IntBox = ConstrainedGenericBox[int]
    box = IntBox()
    box.item = 1
    with pytest.raises(TypeError):
        box.item = "a"


def test_constrained_generic_specialized_with_second_constraint():
    StrBox = ConstrainedGenericBox[str]
    box = StrBox()
    box.item = "hello"
    with pytest.raises(TypeError):
        box.item = 1


def test_constrained_generic_specialized_with_subclass_of_constraint():
    # bool is a subclass of int, so it is within the constraints
    BoolBox = ConstrainedGenericBox[bool]
    box = BoolBox()
    box.item = True
    with pytest.raises(TypeError):
        box.item = "a"


def test_constrained_generic_specialization_rejects_outside_constraints():
    with pytest.raises(TypeError, match="not within the constraints"):
        _ = ConstrainedGenericBox[float]


def test_constrained_generic_specialization_rejects_list_type():
    with pytest.raises(TypeError, match="not within the constraints"):
        _ = ConstrainedGenericBox[list]


def test_constrained_generic_partial_specialization_with_subset_constraints():
    # T_sub has constraints (int, bool) — both are subtypes of int, which is in (int, str)
    T_sub = TypeVar("T_sub", int, bool)
    partial = ConstrainedGenericPair[T_sub, str]
    pair = partial()
    pair.first = 1
    pair.second = "x"
    with pytest.raises(TypeError):
        pair.first = "a"


def test_constrained_generic_partial_specialization_rejects_incompatible_constraints():
    # T_bad has float which is not within (int, str)
    T_bad = TypeVar("T_bad", int, float)
    with pytest.raises(TypeError, match="not within the constraints"):
        _ = ConstrainedGenericPair[T_bad, str]


def test_constrained_generic_partial_specialization_rejects_unconstrained_typevar():
    T_free = TypeVar("T_free")
    with pytest.raises(TypeError, match="compatible with the constraints"):
        _ = ConstrainedGenericPair[T_free, str]


def test_constrained_generic_partial_specialization_with_bound_within_constraints():
    # T_bound has bound=int, which is within (int, str) constraints
    T_bound = TypeVar("T_bound", bound=int)
    partial = ConstrainedGenericPair[T_bound, str]
    pair = partial()
    pair.first = 1
    pair.second = "x"
    with pytest.raises(TypeError):
        pair.first = "a"


def test_constrained_generic_partial_specialization_rejects_bound_outside_constraints():
    T_float_bound = TypeVar("T_float_bound", bound=float)
    with pytest.raises(TypeError, match="not within the constraints"):
        _ = ConstrainedGenericPair[T_float_bound, str]
