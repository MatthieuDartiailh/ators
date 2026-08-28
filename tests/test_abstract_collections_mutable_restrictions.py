# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for mutable container restrictions in abstract collections.

Mutable containers (list, dict, set) cannot be used as item types in abstract
collections (Sequence, Collection, Mapping) because ators cannot insert wrapped
versions of those mutable containers inside them.
"""

from collections.abc import Collection, Mapping, Sequence
import pytest
from ators import Ators, member


# -------------------------------------------------------------------------------------
# Sequence[T] - Forbid mutable containers
# -------------------------------------------------------------------------------------


def test_sequence_of_list_forbidden():
    """Sequence[list] should raise TypeError during class definition."""
    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadSeqList(Ators):
            s: Sequence[list] = member()


def test_sequence_of_dict_forbidden():
    """Sequence[dict] should raise TypeError during class definition."""
    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadSeqDict(Ators):
            s: Sequence[dict] = member()


def test_sequence_of_set_forbidden():
    """Sequence[set] should raise TypeError during class definition."""
    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadSeqSet(Ators):
            s: Sequence[set] = member()


def test_sequence_of_frozenset_allowed():
    """Sequence[frozenset] is allowed (frozenset is immutable)."""
    class GoodSeqFrozenSet(Ators):
        s: Sequence[frozenset] = member()

    obj = GoodSeqFrozenSet(s=[frozenset([1, 2]), frozenset([3, 4])])
    assert len(obj.s) == 2


def test_sequence_of_tuple_allowed():
    """Sequence[tuple] is allowed (tuple is immutable)."""
    class GoodSeqTuple(Ators):
        s: Sequence[tuple] = member()

    obj = GoodSeqTuple(s=[(1, 2), (3, 4)])
    assert len(obj.s) == 2


def test_sequence_of_str_allowed():
    """Sequence[str] is allowed."""
    class GoodSeqStr(Ators):
        s: Sequence[str] = member()

    obj = GoodSeqStr(s=["hello", "world"])
    assert obj.s == ["hello", "world"]


# -------------------------------------------------------------------------------------
# Collection[T] - Forbid mutable containers
# -------------------------------------------------------------------------------------


def test_collection_of_list_forbidden():
    """Collection[list] should raise TypeError during class definition."""
    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadCollList(Ators):
            c: Collection[list] = member()


def test_collection_of_dict_forbidden():
    """Collection[dict] should raise TypeError during class definition."""
    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadCollDict(Ators):
            c: Collection[dict] = member()


def test_collection_of_set_forbidden():
    """Collection[set] should raise TypeError during class definition."""
    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadCollSet(Ators):
            c: Collection[set] = member()


def test_collection_of_frozenset_allowed():
    """Collection[frozenset] is allowed."""
    class GoodCollFrozenSet(Ators):
        c: Collection[frozenset] = member()

    obj = GoodCollFrozenSet(c=[frozenset([1])])
    assert len(obj.c) == 1


def test_collection_of_int_allowed():
    """Collection[int] is allowed."""
    class GoodCollInt(Ators):
        c: Collection[int] = member()

    obj = GoodCollInt(c=[1, 2, 3])
    assert obj.c == [1, 2, 3]


# -------------------------------------------------------------------------------------
# Mapping[K, V] - Forbid mutable containers for keys and values
# -------------------------------------------------------------------------------------


def test_mapping_list_keys_forbidden():
    """Mapping[list, int] should raise TypeError (list keys)."""
    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadMapListKey(Ators):
            m: Mapping[list, int] = member()


def test_mapping_dict_keys_forbidden():
    """Mapping[dict, int] should raise TypeError (dict keys)."""
    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadMapDictKey(Ators):
            m: Mapping[dict, int] = member()


def test_mapping_set_keys_forbidden():
    """Mapping[set, int] should raise TypeError (set keys)."""
    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadMapSetKey(Ators):
            m: Mapping[set, int] = member()


def test_mapping_str_list_values_forbidden():
    """Mapping[str, list] should raise TypeError (list values)."""
    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadMapListValue(Ators):
            m: Mapping[str, list] = member()


def test_mapping_str_dict_values_forbidden():
    """Mapping[str, dict] should raise TypeError (dict values)."""
    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadMapDictValue(Ators):
            m: Mapping[str, dict] = member()


def test_mapping_str_set_values_forbidden():
    """Mapping[str, set] should raise TypeError (set values)."""
    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadMapSetValue(Ators):
            m: Mapping[str, set] = member()


def test_mapping_str_tuple_values_allowed():
    """Mapping[str, tuple] is allowed (tuple is immutable)."""
    class GoodMapTuple(Ators):
        m: Mapping[str, tuple] = member()

    obj = GoodMapTuple(m={"a": (1, 2)})
    assert obj.m == {"a": (1, 2)}


def test_mapping_str_frozenset_values_allowed():
    """Mapping[str, frozenset] is allowed (frozenset is immutable)."""
    class GoodMapFrozenSet(Ators):
        m: Mapping[str, frozenset] = member()

    obj = GoodMapFrozenSet(m={"a": frozenset([1, 2])})
    assert "a" in obj.m


def test_mapping_tuple_keys_allowed():
    """Mapping[tuple, int] is allowed (tuple is immutable)."""
    class GoodMapTupleKey(Ators):
        m: Mapping[tuple, int] = member()

    obj = GoodMapTupleKey(m={(1, 2): 3})
    assert obj.m[(1, 2)] == 3


def test_mapping_frozenset_keys_allowed():
    """Mapping[frozenset, int] is allowed (frozenset is immutable)."""
    class GoodMapFrozenSetKey(Ators):
        m: Mapping[frozenset, int] = member()

    obj = GoodMapFrozenSetKey(m={frozenset([1]): 2})
    assert obj.m[frozenset([1])] == 2


# -------------------------------------------------------------------------------------
# Complex nested cases
# -------------------------------------------------------------------------------------


def test_sequence_of_sequence_of_list_forbidden():
    """Sequence[Sequence[list]] should raise TypeError (nested mutable list)."""
    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadNestedSeqList(Ators):
            s: Sequence[Sequence[list]] = member()


def test_sequence_of_sequence_of_int_allowed():
    """Sequence[Sequence[int]] is allowed."""
    class GoodNestedSeq(Ators):
        s: Sequence[Sequence[int]] = member()

    obj = GoodNestedSeq(s=[[1, 2], [3, 4]])
    assert len(obj.s) == 2


def test_mapping_str_mapping_str_list_forbidden():
    """Mapping[str, Mapping[str, list]] should raise TypeError (nested Mapping with list values)."""
    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadNestedMapList(Ators):
            m: Mapping[str, Mapping[str, list]] = member()


def test_mapping_str_mapping_str_int_allowed():
    """Mapping[str, Mapping[str, int]] is allowed."""
    class GoodNestedMap(Ators):
        m: Mapping[str, Mapping[str, int]] = member()

    obj = GoodNestedMap(m={"a": {"b": 1}})
    assert obj.m == {"a": {"b": 1}}


# -------------------------------------------------------------------------------------
# Union types with mutable containers
# -------------------------------------------------------------------------------------


def test_union_with_sequence_of_list_forbidden():
    """Union containing Sequence[list] should raise TypeError."""
    from typing import Union

    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadUnionSeqList(Ators):
            val: Union[Sequence[list], int] = member()


def test_union_with_mapping_str_dict_forbidden():
    """Union containing Mapping[str, dict] should raise TypeError."""
    from typing import Union

    with pytest.raises(TypeError, match="Cannot use mutable container|Failed to configure Member"):
        class BadUnionMapDict(Ators):
            val: Union[Mapping[str, dict], None] = member()


def test_union_with_sequence_of_int_allowed():
    """Union[Sequence[int], int] is allowed."""
    from typing import Union

    class GoodUnionSeq(Ators):
        val: Union[Sequence[int], int] = member()

    obj1 = GoodUnionSeq(val=[1, 2, 3])
    obj2 = GoodUnionSeq(val=42)
    assert obj1.val == [1, 2, 3]
    assert obj2.val == 42
