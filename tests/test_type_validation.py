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
from typing import TYPE_CHECKING, Any, Callable, Literal, TypeVar

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


def test_specialized_class_exposes_generic_metadata():
    import typing

    int_box = GenericBox[int]
    assert int_box.__origin__ is GenericBox
    assert int_box.__args__ == (int,)
    assert typing.get_origin(int_box) is GenericBox
    assert typing.get_args(int_box) == (int,)


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

# Callable validation tests
from typing import Callable


class CallableBox(Ators):
    """Test class with Callable type member"""
    callback: Callable[[int, str], bool] = member()


def test_callable_valid_with_correct_signature():
    """Test that a callable with matching signature is accepted"""
    def func(x: int, y: str) -> bool:
        return True

    box = CallableBox()
    box.callback = func  # Should not raise


def test_callable_valid_lambda_not_supported():
    """Test that unannotated lambdas are rejected"""
    box = CallableBox()
    # lambdas don't have annotations, should be rejected
    with pytest.raises(TypeError, match="annotated|annotation"):
        box.callback = lambda x, y: True


def test_callable_reject_non_callable():
    """Test that non-callable values are rejected"""
    box = CallableBox()
    with pytest.raises(TypeError, match="Callable"):
        box.callback = 42


def test_callable_reject_wrong_arity():
    """Test that callables with wrong parameter count are rejected"""
    def wrong_arity(x: int) -> bool:
        return True

    box = CallableBox()
    with pytest.raises(TypeError, match="parameter"):
        box.callback = wrong_arity


def test_callable_reject_missing_parameter_annotation():
    """Test that callables missing parameter annotations are rejected"""
    def unannotated_param(x, y: str) -> bool:  # x is unannotated
        return True

    box = CallableBox()
    with pytest.raises(TypeError, match="annotated|annotation"):
        box.callback = unannotated_param


def test_callable_reject_missing_return_annotation():
    """Test that callables missing return annotation are rejected"""
    def no_return_annotation(x: int, y: str):  # No return annotation
        return True

    box = CallableBox()
    with pytest.raises(TypeError, match="return|annotation"):
        box.callback = no_return_annotation


def test_callable_reject_wrong_parameter_type():
    """Test that callables with wrong parameter types are rejected"""
    def wrong_param_type(x: str, y: str) -> bool:  # x should be int
        return True

    box = CallableBox()
    with pytest.raises(TypeError, match="type|parameter"):
        box.callback = wrong_param_type


def test_callable_reject_wrong_return_type():
    """Test that callables with wrong return type are rejected"""
    def wrong_return_type(x: int, y: str) -> str:  # Should return bool
        return "true"

    box = CallableBox()
    with pytest.raises(TypeError, match="return|type"):
        box.callback = wrong_return_type


# Callable[..., ReturnType] tests
class VariadicCallableBox(Ators):
    """Test class with variadic Callable type"""
    callback: Callable[..., str] = member()


def test_callable_variadic_accepts_any_params():
    """Test that Callable[..., ReturnType] accepts any parameter count"""
    def any_params_func(x: int, y: str, z: float) -> str:
        return "ok"

    box = VariadicCallableBox()
    box.callback = any_params_func  # Should not raise


def test_callable_variadic_validates_return_type():
    """Test that Callable[..., ReturnType] still validates return type"""
    def wrong_return(x: int) -> int:  # Wrong return type
        return 42

    box = VariadicCallableBox()
    with pytest.raises(TypeError, match="return"):
        box.callback = wrong_return


def test_callable_empty_params():
    """Test Callable with no parameters"""
    class NoParamCallableBox(Ators):
        callback: Callable[[], str] = member()

    def no_params() -> str:
        return "ok"

    box = NoParamCallableBox()
    box.callback = no_params  # Should not raise


def test_callable_empty_params_rejects_if_not_match():
    """Test Callable with no parameters rejects functions with parameters"""
    class NoParamCallableBox(Ators):
        callback: Callable[[], str] = member()

    def has_params(x: int) -> str:
        return "ok"

    box = NoParamCallableBox()
    with pytest.raises(TypeError, match="parameter"):
        box.callback = has_params    with pytest.raises(TypeError):
        box.item = []
    with pytest.raises(TypeError):
        box.item = {}


def test_constrained_typevar_matches_union_behavior():
    """Constrained TypeVar validation should match int | str union behavior."""

    class UnionBox(Ators):
        item: int | str = member()

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


def test_fixed_tuple_validation_preserves_unchanged_items_after_transformation():
    class A(Ators):
        a: tuple[list[int], int] = member()

    a = A()
    a.a = ([1], 2)
    assert len(a.a) == 2
    assert a.a[0] == [1]
    assert a.a[1] == 2


def test_var_tuple_validation_preserves_unchanged_items_after_transformation():
    class A(Ators):
        a: tuple[list[int] | int, ...] = member()

    a = A()
    a.a = ([1], 2)
    assert len(a.a) == 2
    assert a.a[0] == [1]
    assert a.a[1] == 2


# ---------------------------------------------------------------------------
# Callable variance tests (Phase 2)
# ---------------------------------------------------------------------------
# These tests validate Liskov Substitution Principle (LSP) for callables:
# - Contravariance for parameters: callable accepting more types can substitute
# - Covariance for return types: callable returning more specific types can substitute


class Animal:
    pass


class Dog(Animal):
    pass


class Cat(Animal):
    pass


# Test fixtures for variance
def callable_animal_to_animal(x: Animal) -> Animal:
    """Callable that accepts Animal and returns Animal"""
    return Animal()


def callable_object_to_dog(x: object) -> Dog:
    """Callable that accepts object (supertype) and returns Dog (subtype) - MOST GENERAL"""
    return Dog()


def callable_dog_to_animal(x: Dog) -> Animal:
    """Callable that accepts Dog and returns Animal"""
    return Animal()


def callable_animal_to_dog(x: Animal) -> Dog:
    """Callable that accepts Animal and returns Dog"""
    return Dog()


def callable_object_to_animal(x: object) -> Animal:
    """Callable that accepts object and returns Animal"""
    return Animal()


def callable_dog_to_dog(x: Dog) -> Dog:
    """Callable that accepts Dog and returns Dog"""
    return Dog()


# =========================================================================
# Contravariance Tests (Parameter Acceptance)
# =========================================================================

def test_callable_contravariance_supertype_params_accepted():
    """Callable accepting supertype (object) should substitute for Animal"""
    class AnimalHandler(Ators):
        handler: Callable[[Animal], Animal] = member()

    obj = AnimalHandler()
    # callable_object_to_dog accepts object (more general) - should be accepted
    obj.handler = callable_object_to_dog


def test_callable_contravariance_subtype_params_rejected():
    """Callable accepting subtype (Dog) should NOT substitute for Animal"""
    class AnimalHandler(Ators):
        handler: Callable[[Animal], Animal] = member()

    obj = AnimalHandler()
    # callable_dog_to_animal accepts only Dog (more specific) - should be rejected
    with pytest.raises(TypeError, match="contravariance"):
        obj.handler = callable_dog_to_animal


def test_callable_contravariance_exact_match_still_works():
    """Exact parameter match should still be accepted (backward compat)"""
    class AnimalHandler(Ators):
        handler: Callable[[Animal], Animal] = member()

    obj = AnimalHandler()
    # callable_animal_to_animal accepts Animal - should be accepted
    obj.handler = callable_animal_to_animal


def test_callable_contravariance_multiple_params():
    """Test contravariance with multiple parameters"""
    class MultiParamHandler(Ators):
        handler: Callable[[Animal, Animal], Animal] = member()

    def handler_with_supertypes(x: object, y: object) -> Animal:
        return Animal()

    def handler_with_subtypes(x: Dog, y: Dog) -> Animal:
        return Animal()

    obj = MultiParamHandler()
    # Supertypes should be accepted (contravariance)
    obj.handler = handler_with_supertypes
    # Subtypes should be rejected
    with pytest.raises(TypeError, match="contravariance"):
        obj.handler = handler_with_subtypes


def test_callable_contravariance_first_param_fails():
    """Test error when first parameter violates contravariance"""
    class TwoParamHandler(Ators):
        handler: Callable[[Animal, Animal], Animal] = member()

    def handler_first_narrow(x: Dog, y: object) -> Animal:
        return Animal()

    obj = TwoParamHandler()
    with pytest.raises(TypeError, match="Parameter 0|contravariance"):
        obj.handler = handler_first_narrow


def test_callable_contravariance_second_param_fails():
    """Test error when second parameter violates contravariance"""
    class TwoParamHandler(Ators):
        handler: Callable[[Animal, Animal], Animal] = member()

    def handler_second_narrow(x: object, y: Dog) -> Animal:
        return Animal()

    obj = TwoParamHandler()
    with pytest.raises(TypeError, match="Parameter 1|contravariance"):
        obj.handler = handler_second_narrow


# =========================================================================
# Covariance Tests (Return Type Acceptance)
# =========================================================================

def test_callable_covariance_subtype_return_accepted():
    """Callable returning subtype (Dog) should substitute for Animal"""
    class AnimalProvider(Ators):
        provider: Callable[[int], Animal] = member()

    def provider_returns_dog(x: int) -> Dog:
        return Dog()

    obj = AnimalProvider()
    obj.provider = provider_returns_dog


def test_callable_covariance_supertype_return_rejected():
    """Callable returning supertype (object) should NOT substitute for Animal"""
    class AnimalProvider(Ators):
        provider: Callable[[int], Animal] = member()

    def provider_returns_object(x: int) -> object:
        return object()

    obj = AnimalProvider()
    with pytest.raises(TypeError, match="covariance"):
        obj.provider = provider_returns_object


def test_callable_covariance_exact_match_still_works():
    """Exact return type match should still be accepted (backward compat)"""
    class AnimalProvider(Ators):
        provider: Callable[[int], Animal] = member()

    def provider_returns_animal(x: int) -> Animal:
        return Animal()

    obj = AnimalProvider()
    obj.provider = provider_returns_animal


def test_callable_covariance_deep_hierarchy():
    """Test covariance with deeper inheritance hierarchies"""
    class AnimalProvider(Ators):
        provider: Callable[[int], Animal] = member()

    def provider_returns_dog(x: int) -> Dog:
        return Dog()

    obj = AnimalProvider()
    obj.provider = provider_returns_dog


# =========================================================================
# Combined Variance Tests
# =========================================================================

def test_callable_both_variances_correct():
    """Test that contravariant params + covariant return both work together"""
    class CallableBox(Ators):
        callback: Callable[[Animal], Animal] = member()

    # object -> Dog: contravariant params (object > Animal) + covariant return (Dog < Animal)
    obj = CallableBox()
    obj.callback = callable_object_to_dog


def test_callable_param_contravariance_fail_return_pass():
    """Test param fails, return passes -> should still reject"""
    class CallableBox(Ators):
        callback: Callable[[Animal], Animal] = member()

    def narrow_param_good_return(x: Dog) -> Dog:
        return Dog()

    obj = CallableBox()
    with pytest.raises(TypeError, match="contravariance"):
        obj.callback = narrow_param_good_return


def test_callable_param_pass_return_covariance_fail():
    """Test param passes, return fails -> should still reject"""
    class CallableBox(Ators):
        callback: Callable[[Animal], Animal] = member()

    def good_param_bad_return(x: object) -> object:
        return object()

    obj = CallableBox()
    with pytest.raises(TypeError, match="covariance"):
        obj.callback = good_param_bad_return


def test_callable_both_variances_fail():
    """Test both contravariance and covariance fail"""
    class CallableBox(Ators):
        callback: Callable[[Animal], Animal] = member()

    def both_wrong(x: Dog) -> object:
        return object()

    obj = CallableBox()
    with pytest.raises(TypeError):  # Should match one of the errors
        obj.callback = both_wrong


# =========================================================================
# Edge Cases
# =========================================================================

def test_callable_variance_with_object_param():
    """Test that object as parameter type follows contravariance"""
    class ObjectParamHandler(Ators):
        handler: Callable[[object], str] = member()

    def handler_returns_str(x: object) -> str:
        return "ok"

    obj = ObjectParamHandler()
    obj.handler = handler_returns_str


def test_callable_variance_with_object_return():
    """Test that object as return type follows covariance"""
    class ObjectReturnProvider(Ators):
        provider: Callable[[int], object] = member()

    def provider_returns_str(x: int) -> str:
        return "ok"

    obj = ObjectReturnProvider()
    obj.provider = provider_returns_str


def test_callable_variance_empty_params_ignores_contravariance():
    """Callable[..., X] should not check parameter contravariance"""
    class VariadicHandler(Ators):
        handler: Callable[..., Animal] = member()

    def any_params_to_animal(*args, **kwargs) -> Animal:
        return Animal()

    obj = VariadicHandler()
    obj.handler = any_params_to_animal


def test_callable_variance_none_type():
    """Test variance with None type in hierarchy"""
    class NoneProvider(Ators):
        provider: Callable[[int], object] = member()

    def provider_returns_none(x: int) -> None:
        return None

    obj = NoneProvider()
    # None is a subtype of object, should be accepted
    obj.provider = provider_returns_none


def test_callable_variance_builtin_types():
    """Test variance with builtin types (bool is subclass of int)"""
    class NumberHandler(Ators):
        handler: Callable[[int], int] = member()

    def handler_bool_to_int(x: int) -> bool:
        return True

    obj = NumberHandler()
    obj.handler = handler_bool_to_int


# =========================================================================
# Regression Tests (Phase 1 Still Works)
# =========================================================================

def test_callable_variance_exact_match_single_param():
    """Exact matching for single parameter should still work"""
    class SingleParamBox(Ators):
        callback: Callable[[int], str] = member()

    def exact_match(x: int) -> str:
        return "ok"

    obj = SingleParamBox()
    obj.callback = exact_match


def test_callable_variance_strict_unannotated_still_rejected():
    """Unannotated callables should still be rejected"""
    class AnnotatedCallableBox(Ators):
        callback: Callable[[int], str] = member()

    def unannotated(x):  # No annotation!
        return "ok"

    obj = AnnotatedCallableBox()
    with pytest.raises(TypeError, match="annotated"):
        obj.callback = unannotated


def test_callable_variance_arity_still_checked():
    """Wrong arity should still be rejected"""
    class TwoParamBox(Ators):
        callback: Callable[[int, str], int] = member()

    def wrong_arity(x: int) -> int:
        return 42

    obj = TwoParamBox()
    with pytest.raises(TypeError, match="parameter"):
        obj.callback = wrong_arity


def test_callable_variance_non_callable_still_rejected():
    """Non-callable values should still be rejected"""
    class CallableBox(Ators):
        callback: Callable[[int], str] = member()

    obj = CallableBox()
    with pytest.raises(TypeError, match="Callable"):
        obj.callback = 42
