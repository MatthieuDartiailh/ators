# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for generic-aware runtime subclass / instance checks."""

import sys
import typing
from types import ModuleType
from typing import Any

import pytest

from ators import Ators, member

# ---------------------------------------------------------------------------
# Generic Ators class fixture (PEP-695 syntax, Python 3.14+)
# ---------------------------------------------------------------------------


class G[T, U](Ators):
    """A two-parameter generic Ators base class."""


class BoundTypeVar[T: int](Ators):
    pass


class ConstrainedTypeVar[T: (int, float)](Ators):
    pass


T, U = G.__type_params__
TBound = BoundTypeVar.__type_params__[0]
TCon = ConstrainedTypeVar.__type_params__[0]


# ---------------------------------------------------------------------------
#  issubclass - positive cases
# ---------------------------------------------------------------------------


def test_subclass_exact_concrete():
    """issubclass(G[int, str], G[int, str]) is True (exact match)."""
    assert issubclass(G[int, str], G[int, str]) is True


def test_subclass_typevar_wildcard_first_arg():
    """issubclass(G[int, str], G[T, str]) is True (T is unconstrained wildcard)."""
    assert issubclass(G[int, str], G[T, str]) is True


def test_subclass_typevar_wildcard_both_args():
    """issubclass(G[int, str], G[T, U]) is True (both args are TypeVar wildcards)."""
    assert issubclass(G[int, str], G[T, U]) is True


def test_subclass_any_rhs():
    """issubclass(G[int, str], G[Any, Any]) is True."""
    assert issubclass(G[int, str], G[Any, Any]) is True


def test_subclass_any_rhs_mixed():
    """issubclass(G[int, str], G[int, Any]) is True."""
    assert issubclass(G[int, str], G[int, Any]) is True


def test_subclass_concrete_subtype():
    """issubclass(G[bool, str], G[int, str]) is True (bool is subclass of int)."""
    assert issubclass(G[bool, str], G[int, str]) is True


def test_subclass_typevar_bound_satisfied():
    """TypeVar with bound: concrete arg satisfies the bound."""
    assert issubclass(G[int, str], G[TBound, str]) is True


def test_subclass_typevar_constraint_satisfied():
    """TypeVar with constraints: concrete arg is one of them."""
    assert issubclass(G[int, str], G[TCon, str]) is True


# ---------------------------------------------------------------------------
#  issubclass - negative cases
# ---------------------------------------------------------------------------


def test_subclass_wrong_concrete_second_arg():
    """issubclass(G[int, bytes], G[T, str]) is False."""
    assert issubclass(G[int, bytes], G[T, str]) is False


def test_subclass_wrong_concrete_first_arg():
    """issubclass(G[str, str], G[int, str]) is False (str is not subclass of int)."""
    assert issubclass(G[str, str], G[int, str]) is False


def test_subclass_typevar_bound_violated():
    """TypeVar with bound: concrete arg does NOT satisfy the bound."""
    assert issubclass(G[str, str], G[TBound, str]) is False


def test_subclass_typevar_constraint_violated():
    """TypeVar with constraints: concrete arg is NOT one of them."""
    assert issubclass(G[str, str], G[TCon, str]) is False


def test_subclass_origin_mismatch():
    """Two unrelated generic classes are not compatible."""

    class H[T, U](Ators):
        pass

    h_T = H.__type_params__[0]
    assert issubclass(G[int, str], H[h_T, str]) is False


def test_subclass_non_specialized_lhs():
    """issubclass(G, G[T, str]) is False: plain class vs specialised."""
    assert issubclass(G, G[T, str]) is False


# ---------------------------------------------------------------------------
#  isinstance - positive cases
# ---------------------------------------------------------------------------


def test_isinstance_typevar_wildcard():
    """isinstance(obj, G[T, str]) is True when type(obj) is G[int, str]."""
    obj = G[int, str]()
    assert isinstance(obj, G[T, str]) is True


def test_isinstance_exact():
    """isinstance(obj, G[int, str]) is True when type(obj) is G[int, str]."""
    obj = G[int, str]()
    assert isinstance(obj, G[int, str]) is True


def test_isinstance_any_rhs():
    """isinstance(obj, G[Any, Any]) is True."""
    obj = G[int, str]()
    assert isinstance(obj, G[Any, Any]) is True


def test_isinstance_bound_satisfied():
    """isinstance(obj, G[TBound, str]) is True when first arg satisfies bound."""
    obj = G[int, str]()
    assert isinstance(obj, G[TBound, str]) is True


# ---------------------------------------------------------------------------
#  isinstance - negative cases
# ---------------------------------------------------------------------------


def test_isinstance_wrong_arg():
    """isinstance(obj, G[T, bytes]) is False when obj is G[int, str]."""
    obj = G[int, str]()
    assert isinstance(obj, G[T, bytes]) is False


def test_isinstance_bound_violated():
    """isinstance(obj, G[TBound, str]) is False when first arg violates bound."""
    obj = G[str, str]()
    assert isinstance(obj, G[TBound, str]) is False


# ---------------------------------------------------------------------------
#  Validation - ForwardRef forbidden at specialisation time
# ---------------------------------------------------------------------------


def test_forward_ref_raises_at_specialisation():
    """ForwardRef in specialisation args raises TypeError immediately."""
    fref = typing.ForwardRef("int")
    with pytest.raises(TypeError, match="ForwardRef"):
        G[fref, str]  # type: ignore


# ---------------------------------------------------------------------------
#  Determinism / regression
# ---------------------------------------------------------------------------


def test_repeated_subclass_returns_same_result():
    """Warm-cache path returns the same result as cold-cache path."""
    for _ in range(10):
        assert issubclass(G[int, str], G[T, str]) is True
        assert issubclass(G[int, bytes], G[T, str]) is False


def test_non_generic_issubclass_unaffected():
    """Plain (non-generic) Ators classes still work with issubclass."""

    class Base(Ators):
        pass

    class Child(Base):
        pass

    assert issubclass(Child, Base) is True
    assert issubclass(Base, Child) is False


def test_non_generic_isinstance_unaffected():
    """Plain (non-generic) Ators classes still work with isinstance."""

    class Plain(Ators):
        pass

    obj = Plain()
    assert isinstance(obj, Plain) is True


def test_arity_mismatch_raises_at_specialisation():
    """Passing the wrong number of type args raises TypeError at specialisation."""
    with pytest.raises(TypeError):
        G[int, str, float]  # 3 args for a 2-param class  # type: ignore


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
    pair: GenericPair[int, T] = member()


class DelayedForwardRefPartialHolder[T: int](Ators):
    pair: DelayedGenericPair[int, T] = member()


class DelayedGenericPair[T, U](Ators):
    first: T = member()
    second: U = member()


def test_generic_specialization_is_cached_class():
    int_box = GenericBox[int]
    assert int_box is GenericBox[int]
    assert int_box is not GenericBox[str]


def test_specialized_class_exposes_generic_metadata():
    int_box = GenericBox[int]
    assert int_box.__origin__ is GenericBox
    assert int_box.__args__ == (int,)
    assert typing.get_origin(int_box) is GenericBox
    assert typing.get_args(int_box) == (int,)


def test_specialized_alias_subclasscheck_rejects_mismatched_specialization():
    assert issubclass(GenericBox[int], GenericBox)
    assert issubclass(GenericBox[int], GenericBox[int])
    assert not issubclass(GenericBox[int], GenericBox[str])
    assert not issubclass(GenericBox[str], GenericBox[int])


def test_specialized_alias_metadata_stays_consistent_across_runtime_introspection():
    alias = GenericBox[int]
    assert alias.__origin__ is GenericBox
    assert alias.__args__ == (int,)
    assert alias.__type_params__ == ()
    assert alias.__annotations__["item"] is int
    assert "__annotations__" in alias.__dict__


def test_partial_specialization_preserves_unresolved_typevar_order_for_alias_metadata():
    class PartialBoundPair[T: int](GenericPair[int, T]):
        pass

    partial_t = PartialBoundPair.__type_params__[0]
    partial = PartialBoundPair[int]

    assert PartialBoundPair.__type_params__ == (partial_t,)
    assert partial.__type_params__ == ()
    assert partial.__args__ == (int, int)
    assert partial.__origin__ is GenericPair


def test_specialized_alias_exposes_runtime_metadata_and_alias_checks():
    alias = GenericBox[int]

    assert alias.__annotations__["item"] is int
    assert "__annotations__" in alias.__dict__
    assert isinstance(alias(), alias)
    assert issubclass(type(alias()), alias)
    assert issubclass(alias, GenericBox)


def test_specialized_alias_can_be_used_as_runtime_base_class():
    class Child(GenericBox[int]):
        pass

    child = Child()
    child.item = 1
    assert isinstance(child, GenericBox)
    assert not isinstance(child, GenericBox[int])
    assert issubclass(Child, GenericBox)

    with pytest.raises(TypeError):
        child.item = "not-int"


def test_legacy_generic_alias_uses_parameters_fallback_for_specialization():
    legacy_t = typing.TypeVar("legacy_t")

    class LegacyBox(Ators, typing.Generic[legacy_t]):
        item: legacy_t = member()

    alias = LegacyBox[int]
    assert LegacyBox.__parameters__ == (legacy_t,)
    assert alias.__annotations__["item"] is int
    assert isinstance(alias(), alias)


def test_full_and_stepwise_specialization_are_identical():
    class Partial[T](GenericPair[int, T]):
        pass

    direct = GenericPair[int, str]
    stepwise = Partial[str]
    assert direct is stepwise


def test_partial_specialization_typevar_bound_must_be_narrower():
    class NarrowerBoundPair[T: bool](BoundedPair[int, T]):
        pass

    _ = NarrowerBoundPair[bool]

    with pytest.raises(TypeError, match="not narrower"):

        class WiderBoundPair[T: str](BoundedPair[int, T]):  # type: ignore
            pass


def test_partial_specialization_typevar_without_required_bound_is_rejected():
    with pytest.raises(TypeError, match="must define a bound"):

        class UnboundedPair[T](BoundedPair[int, T]):  # type: ignore
            pass


def test_non_class_bounds_use_python_issubclass_fallback_for_narrower_typevar():
    with pytest.warns(UserWarning, match="No specific validation strategy recorded"):

        class SequenceHolder[T: typing.Iterable[int]](Ators):
            value: T = member()

    with pytest.warns(UserWarning, match="No specific validation strategy recorded"):

        class SameBoundSequence[T: typing.Iterable[int]](SequenceHolder[T]):
            pass

    with pytest.raises(TypeError, match="not narrower"):

        class WiderSequence[T: typing.Iterable[str]](SequenceHolder[T]):  # type: ignore
            pass


def test_non_class_constraints_use_python_issubclass_fallback_for_constraint_mismatch():
    with pytest.warns(UserWarning, match="No specific validation strategy recorded"):

        class ConstrainedSequenceHolder[
            T: (typing.Sequence[int], typing.Sequence[str])
        ](Ators):
            value: T = member()

    with pytest.warns(UserWarning, match="No specific validation strategy recorded"):

        class MatchingSequence[T: typing.Sequence[str]](ConstrainedSequenceHolder[T]):
            pass

    with pytest.raises(TypeError, match="not within the constraints"):

        class MismatchedSequence[T: typing.Sequence[float]](
            ConstrainedSequenceHolder[T]  # type: ignore
        ):
            pass


def test_unconstrained_typevar_without_bound_or_constraints_is_rejected():
    class ConstrainedHolder[T: (int, str)](Ators):
        value: T = member()

    with pytest.raises(
        TypeError, match="must define constraints or a bound compatible"
    ):

        class UnconstrainedHolder[T](ConstrainedHolder[T]):  # type: ignore
            pass


def test_same_named_typevars_create_distinct_specializations_without_slot_collision():
    class LeftTypeVarHolder[T: int](Ators):
        pass

    class RightTypeVarHolder[T: int](Ators):
        pass

    left_specialized = LeftTypeVarHolder[int]
    right_specialized = RightTypeVarHolder[int]

    left_slot = getattr(LeftTypeVarHolder.__type_params__[0], "__ators_typevar_slot__", None)
    right_slot = getattr(RightTypeVarHolder.__type_params__[0], "__ators_typevar_slot__", None)

    assert left_specialized is not right_specialized
    assert left_slot is not None
    assert right_slot is not None
    assert left_slot != right_slot


def test_module_shadowed_typevar_rebuilds_with_nested_type_alias_resolution():
    module_name = "shadowed_typevar_typealias_module"
    module = ModuleType(module_name)
    sys.modules[module_name] = module
    try:
        exec(
            """
from ators import Ators, member

class BoundAliasT[T: int]:
    pass

T = BoundAliasT.__type_params__[0]
type AliasT = tuple[int, T]

class Pair[T, U](Ators):
    left: T = member()
    right: U = member()

class Holder[T](Ators):
    alias: AliasT = member()
    pair: Pair[int, T] = member()
""",
            module.__dict__,
        )

        holder = module.Holder[int]()
        holder.alias = (1, 2)
        holder.pair = module.Pair[int, int]()

        with pytest.raises(TypeError):
            holder.alias = ("bad", 2)

        with pytest.raises(TypeError):
            holder.pair = module.Pair[str, int]()
    finally:
        sys.modules.pop(module_name, None)


def test_eager_partial_specialization_keeps_owner_local_typevar_context():
    class Holder[T: int](Ators):
        pair: GenericPair[int, T] = member()

    class PartialHolder[T: int](Holder[T]):
        pass

    specialized = PartialHolder[int]
    assert specialized.__origin__ is Holder
    assert specialized.__args__ == (int,)
    assert specialized.__type_params__ == ()


def test_owner_local_typevar_context_survives_inner_generic_respecialization():
    holder = DelayedForwardRefPartialHolder[int]()
    holder.pair = DelayedGenericPair[int, int]()
    with pytest.raises(TypeError):
        holder.pair = DelayedGenericPair[str, int]()  # type: ignore

    class ReboundHolder[T: int](Ators):
        pair: DelayedGenericPair[int, T] = member()

    rebound = ReboundHolder[int]()
    rebound.pair = DelayedGenericPair[int, int]()
    with pytest.raises(TypeError):
        rebound.pair = DelayedGenericPair[str, int]()  # type: ignore


def test_partial_specialization_keeps_owner_local_typevar_context():
    class Holder[T: int](Ators):
        pair: GenericPair[int, T] = member()

    class NestedHolder[T: int](Holder[T]):
        pass

    expected = Holder[int]
    nested = NestedHolder[int]
    assert expected is nested
    assert nested.__origin__ is Holder
    assert nested.__args__ == (int,)
    assert nested.__type_params__ == ()


def test_same_name_typevars_keep_distinct_owner_local_slots():
    class LeftTypeVarHolder[T: int](Ators):
        pass

    class RightTypeVarHolder[T: int](Ators):
        pass

    left_slot = getattr(LeftTypeVarHolder.__type_params__[0], "__ators_typevar_slot__", None)
    right_slot = getattr(RightTypeVarHolder.__type_params__[0], "__ators_typevar_slot__", None)

    assert left_slot is not None
    assert right_slot is not None
    assert left_slot != right_slot
    assert getattr(LeftTypeVarHolder.__type_params__[0], "__name__", None) == "T"
    assert getattr(RightTypeVarHolder.__type_params__[0], "__name__", None) == "T"


def test_specialization_propagates_slot_when_only_one_side_is_initialized():
    class LocalTypeVarHolder[T: int](Ators):
        pass

    class Box[T](Ators):
        value: T = member()

    class Holder[T: int](Ators):
        boxed: Box[T] = member()

    local_slot = getattr(LocalTypeVarHolder.__type_params__[0], "__ators_typevar_slot__", None)
    holder_slot = getattr(Holder.__type_params__[0], "__ators_typevar_slot__", None)

    assert local_slot is not None
    assert holder_slot is not None
    assert holder_slot != local_slot
    assert getattr(Holder.__type_params__[0], "__name__", None) == "T"

    specialized = Holder[int]
    assert specialized.__origin__ is Holder
    assert specialized.__args__ == (int,)


def test_module_shadowed_typevar_bound_is_used_in_generic_class_resolution():
    class BoundShadowTypeVar[T: int](Ators):
        pass

    global T
    T = BoundShadowTypeVar.__type_params__[0]

    class ShadowBoundHolder[T](Ators):
        value: T = member()

    holder = ShadowBoundHolder[int]()
    holder.value = 1
    with pytest.raises(TypeError):
        holder.value = "a"  # type: ignore


def test_module_shadowed_typevar_constraints_are_used_in_generic_class_resolution():
    class ConstrainedShadowTypeVar[T: (int, str)](Ators):
        pass

    global T
    T = ConstrainedShadowTypeVar.__type_params__[0]

    class ShadowConstrainedHolder[T](Ators):
        value: T = member()

    holder = ShadowConstrainedHolder[int]()
    holder.value = 1
    with pytest.raises(TypeError):
        holder.value = 1.5  # type: ignore


# Generic specialization edge cases for Rust error paths
def test_generic_specialization_with_union_type_parameters() -> None:
    """Test generic specialization with union type parameters."""

    class Container[T](Ators):
        value: T = member()

    # Test specialization with union type
    cont = Container[int | str]()
    cont.value = 42
    assert cont.value == 42
    cont.value = "hello"
    assert cont.value == "hello"
    with pytest.raises((TypeError, ValueError)):
        cont.value = []  # type: ignore


def test_generic_specialization_with_nested_container_types() -> None:
    """Test generic specialization with complex nested container types."""

    class Storage[T](Ators):
        data: list[T] = member()

    # Specialize with list type
    store = Storage[list[int]]()
    store.data = [[], [1, 2, 3]]
    assert store.data == [[], [1, 2, 3]]

    # Should reject non-list values
    with pytest.raises((TypeError, ValueError)):
        store.data = [[1], "invalid"]  # type: ignore


def test_generic_with_multiple_constraints_in_member() -> None:
    """Test generic type with constrained TypeVar in member field."""

    class Holder[TCons: (int, str)](Ators):
        value: TCons = member()

    holder = Holder()
    holder.value = 42  # type: ignore
    assert holder.value == 42
    holder.value = "text"  # type: ignore
    assert holder.value == "text"

    with pytest.raises((TypeError, ValueError)):
        holder.value = []  # type: ignore


def test_generic_specialization_caching_with_same_named_types() -> None:
    """Test that generic specialization cache doesn't collide with identically-named types."""

    class GenericBox[T](Ators):
        item: T = member()

    # Create specializations with same name but different instances
    box1 = GenericBox[int]()
    box1.item = 42

    box2 = GenericBox[str]()
    box2.item = "text"

    # Verify they remain independent
    assert box1.item == 42
    assert box2.item == "text"

    with pytest.raises((TypeError, ValueError)):
        box1.item = "wrong"  # type: ignore

    with pytest.raises((TypeError, ValueError)):
        box2.item = 123  # type: ignore


def test_generic_with_optional_type_parameter() -> None:
    """Test generic specialization with Optional type parameter."""

    class MaybeBox[T](Ators):
        value: T | None = member()

    # Test with int | None
    box = MaybeBox[int]()
    box.value = None
    assert box.value is None
    box.value = 42
    assert box.value == 42

    with pytest.raises((TypeError, ValueError)):
        box.value = "invalid"  # type: ignore


def test_nested_generic_alias_rebuilds_shadowed_module_typevar_metadata():
    module_name = "shadowed_typevar_module"
    module = ModuleType(module_name)
    sys.modules[module_name] = module
    try:
        exec(
            """
from ators import Ators, member

class BoundShadowTypeVar[T: int]:
    pass

T = BoundShadowTypeVar.__type_params__[0]

class ShadowPair[T, U](Ators):
    first: T = member()
    second: U = member()

class ShadowPairHolder[T](Ators):
    pair: ShadowPair[int, T] = member()
""",
            module.__dict__,
        )

        holder = module.ShadowPairHolder[int]()
        holder.pair = module.ShadowPair[int, int]()
        with pytest.raises(TypeError):
            holder.pair = module.ShadowPair[str, int]()
    finally:
        sys.modules.pop(module_name, None)


def test_forward_ref_with_generic_typevar_resolution():
    """Forward refs in generics resolve with TypeVar bindings, not just module scope."""

    # Generic class with member using TypeVar
    class Holder[T](Ators):
        value: T

    # Specialize with int
    IntHolder = Holder[int]
    ih = IntHolder()

    # Validate int path works
    ih.value = 42
    assert ih.value == 42

    # Validate rejection of non-int
    with pytest.raises(TypeError) as exc_info:
        ih.value = "string"
    assert "int" in str(exc_info.value)

    # Specialize with str - different type validation
    StrHolder = Holder[str]
    sh = StrHolder()
    sh.value = "hello"
    assert sh.value == "hello"

    with pytest.raises(TypeError):
        sh.value = 42

    # This exercises LateResolvedValidator::validate() with TypeVar binding
    # Rust: types.rs lines 135-150 (TypeVar resolution in forward refs)
