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
from typing import Any, TypeVar

import pytest

from ators import Ators, member

# ---------------------------------------------------------------------------
# Generic Ators class fixture (PEP-695 syntax, Python 3.14+)
# ---------------------------------------------------------------------------


class G[T, U](Ators):
    """A two-parameter generic Ators base class."""


T, U = G.__type_params__
TBound = TypeVar("TBound", bound=int)
TCon = TypeVar("TCon", int, float)


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
    T = TypeVar("T", bound=int)
    partial = GenericPair[int, T]

    assert partial.__type_params__ == (T,)
    assert partial.__args__ == (int, T)
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
    T = TypeVar("T")

    class LegacyBox(Ators, typing.Generic[T]):
        item: T = member()

    alias = LegacyBox[int]
    assert LegacyBox.__parameters__ == (T,)
    assert alias.__annotations__["item"] is int
    assert isinstance(alias(), alias)


def test_full_and_stepwise_specialization_are_identical():
    U = TypeVar("U")
    direct = GenericPair[int, str]
    stepwise = GenericPair[int, U][str]
    assert direct is stepwise


def test_partial_specialization_typevar_bound_must_be_narrower():
    narrower = TypeVar("narrower", bound=bool)
    _ = BoundedPair[int, narrower]

    wider = TypeVar("wider", bound=str)
    with pytest.raises(TypeError, match="not narrower"):
        _ = BoundedPair[int, wider]  # type: ignore


def test_partial_specialization_typevar_without_required_bound_is_rejected():
    unbounded = TypeVar("unbounded")
    with pytest.raises(TypeError, match="must define a bound"):
        _ = BoundedPair[int, unbounded]  # type: ignore


def test_non_class_bounds_use_python_issubclass_fallback_for_narrower_typevar():
    with pytest.warns(UserWarning, match="No specific validation strategy recorded"):

        class SequenceHolder[T: typing.Sequence[int]](Ators):
            value: T = member()

    same = TypeVar("same", bound=typing.Sequence[int])
    with pytest.warns(UserWarning, match="No specific validation strategy recorded"):
        _ = SequenceHolder[same]

    wider = TypeVar("wider", bound=typing.Sequence[str])
    with pytest.raises(TypeError, match="not narrower"):
        _ = SequenceHolder[wider]  # type: ignore


def test_non_class_constraints_use_python_issubclass_fallback_for_constraint_mismatch():
    with pytest.warns(UserWarning, match="No specific validation strategy recorded"):

        class ConstrainedSequenceHolder[
            T: (typing.Sequence[int], typing.Sequence[str])
        ](Ators):
            value: T = member()

    matching = TypeVar("matching", bound=typing.Sequence[str])
    with pytest.warns(UserWarning, match="No specific validation strategy recorded"):
        _ = ConstrainedSequenceHolder[matching]

    mismatched = TypeVar("mismatched", bound=typing.Sequence[float])
    with pytest.raises(TypeError, match="not within the constraints"):
        _ = ConstrainedSequenceHolder[mismatched]  # type: ignore


def test_module_shadowed_typevar_rebuilds_with_nested_type_alias_resolution():
    module_name = "shadowed_typevar_typealias_module"
    module = ModuleType(module_name)
    sys.modules[module_name] = module
    try:
        exec(
            """
from typing import TypeVar
from ators import Ators, member

T = TypeVar('T', bound=int)

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
    T2 = TypeVar("T2", bound=int)
    holder = ForwardRefPartialHolder[T2]()  # type: ignore

    holder.pair = GenericPair[int, T2]()  # type: ignore
    with pytest.raises(TypeError):
        holder.pair = GenericPair[str, T2]()  # type: ignore


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

    holder = Holder[int]()
    holder.pair = GenericPair[int, int]()
    with pytest.raises(TypeError):
        holder.pair = GenericPair[str, int]()  # type: ignore

    other = TypeVar("other", bound=int)
    holder2 = Holder[other]()  # type: ignore
    holder2.pair = GenericPair[int, other]()  # type: ignore
    with pytest.raises(TypeError):
        holder2.pair = GenericPair[str, other]()  # type: ignore


def test_same_name_typevars_keep_distinct_owner_local_slots():
    T_left = TypeVar("T", bound=int)  # type: ignore
    T_right = TypeVar("T", bound=int)  # type: ignore

    class Box[T](Ators):
        value: T = member()

    class Holder[T: int](Ators):
        boxed: Box[T] = member()

    left_specialized = Holder[T_left]
    right_specialized = Holder[T_right]

    left_slot = getattr(T_left, "__ators_typevar_slot__", None)
    right_slot = getattr(T_right, "__ators_typevar_slot__", None)

    assert left_slot is not None
    assert right_slot is not None
    assert left_slot != right_slot
    assert (
        getattr(left_specialized.__type_params__[0], "__ators_typevar_slot__", None)
        == left_slot
    )
    assert (
        getattr(right_specialized.__type_params__[0], "__ators_typevar_slot__", None)
        == right_slot
    )


def test_specialization_propagates_slot_when_only_one_side_is_initialized():
    T_local = TypeVar("T", bound=int)  # type: ignore

    class Box[T](Ators):
        value: T = member()

    class Holder[T: int](Ators):
        boxed: Box[T] = member()

    specialized = Holder[T_local]
    local_slot = getattr(T_local, "__ators_typevar_slot__", None)

    assert local_slot is not None
    assert (
        getattr(specialized.__type_params__[0], "__ators_typevar_slot__", None)
        == local_slot
    )
    assert getattr(specialized.__type_params__[0], "__name__", None) == "T"


def test_module_shadowed_typevar_bound_is_used_in_generic_class_resolution():
    global T
    T = TypeVar("T", bound=int)  # type: ignore

    class ShadowBoundHolder[T](Ators):
        value: T = member()

    holder = ShadowBoundHolder[int]()
    holder.value = 1
    with pytest.raises(TypeError):
        holder.value = "a"  # type: ignore


def test_module_shadowed_typevar_constraints_are_used_in_generic_class_resolution():
    global T
    T = TypeVar("T", int, str)  # type: ignore

    class ShadowConstrainedHolder[T](Ators):
        value: T = member()

    holder = ShadowConstrainedHolder[int]()
    holder.value = 1
    with pytest.raises(TypeError):
        holder.value = 1.5  # type: ignore


def test_nested_generic_alias_rebuilds_shadowed_module_typevar_metadata():
    module_name = "shadowed_typevar_module"
    module = ModuleType(module_name)
    sys.modules[module_name] = module
    try:
        exec(
            """
from typing import TypeVar
from ators import Ators, member

T = TypeVar('T', bound=int)

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
