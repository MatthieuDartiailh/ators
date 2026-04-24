# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for Sequence, Collection, and Mapping protocol validators (Blocks A, C, G)."""

import warnings
from collections import UserDict, UserList
from collections.abc import (
    Collection,
    Mapping,
    Sequence,
)
from types import MappingProxyType

import pytest

from ators import Ators, member

# -------------------------------------------------------------------------------------
# Block A: Sequence[T]
# -------------------------------------------------------------------------------------


class SequenceHolder(Ators):
    s: Sequence[int] = member()
    s_bare: Sequence = member()


@pytest.fixture()
def seq_holder():
    return SequenceHolder(s=[1, 2, 3], s_bare=[1, 2, 3])


def test_sequence_list_ok(seq_holder):
    seq_holder.s = [4, 5, 6]
    assert seq_holder.s == [4, 5, 6]


def test_sequence_tuple_ok(seq_holder):
    seq_holder.s = (1, 2, 3)
    assert seq_holder.s == (1, 2, 3)


def test_sequence_range_ok(seq_holder):
    seq_holder.s = range(5)


def test_sequence_item_validation_error(seq_holder):
    with pytest.raises(TypeError):
        seq_holder.s = [1, "x"]


def test_sequence_set_is_not_sequence(seq_holder):
    with pytest.raises(TypeError):
        seq_holder.s = {1, 2}


def test_sequence_int_is_not_sequence(seq_holder):
    with pytest.raises(TypeError):
        seq_holder.s = 42


def test_sequence_bare_no_item_validation(seq_holder):
    # bare Sequence has no item validator — accepts any sequence regardless of item types
    seq_holder.s_bare = [1, "x", None]
    seq_holder.s_bare = (1, 2, 3)


def test_sequence_bare_set_rejected(seq_holder):
    with pytest.raises(TypeError):
        seq_holder.s_bare = {1, 2}


def test_sequence_returns_original_value(seq_holder):
    original = [1, 2, 3]
    seq_holder.s = original
    # read-only protocol: value stored is the original object (no wrapper)
    assert seq_holder.s is original


# -------------------------------------------------------------------------------------
# Block A: Collection[T]
# -------------------------------------------------------------------------------------


class CollectionHolder(Ators):
    c: Collection[int] = member()
    c_bare: Collection = member()


@pytest.fixture()
def coll_holder():
    return CollectionHolder(c=[1, 2, 3], c_bare=[1, 2])


def test_collection_list_ok(coll_holder):
    coll_holder.c = [1, 2, 3]


def test_collection_set_ok(coll_holder):
    # set is a Collection (but not a Sequence)
    coll_holder.c = {1, 2, 3}


def test_collection_tuple_ok(coll_holder):
    coll_holder.c = (1, 2, 3)


def test_collection_item_validation_error(coll_holder):
    with pytest.raises(TypeError):
        coll_holder.c = [1, "x"]


def test_collection_set_item_validation_error(coll_holder):
    with pytest.raises(TypeError):
        coll_holder.c = {1, "x"}


def test_collection_int_rejected(coll_holder):
    with pytest.raises(TypeError):
        coll_holder.c = 42


def test_collection_bare_no_item_validation(coll_holder):
    coll_holder.c_bare = [1, "x"]
    coll_holder.c_bare = {1, "x"}


def test_collection_returns_original_value(coll_holder):
    original = [1, 2, 3]
    coll_holder.c = original
    assert coll_holder.c is original


# -------------------------------------------------------------------------------------
# Block C: Mapping[K, V]
# -------------------------------------------------------------------------------------


class MappingHolder(Ators):
    m: Mapping[str, int] = member()
    m_bare: Mapping = member()


@pytest.fixture()
def mapping_holder():
    return MappingHolder(m={"a": 1}, m_bare={"x": 1})


def test_mapping_dict_ok(mapping_holder):
    mapping_holder.m = {"a": 1, "b": 2}


def test_mapping_proxy_ok(mapping_holder):
    mapping_holder.m = MappingProxyType({"a": 1})


def test_mapping_value_type_error(mapping_holder):
    with pytest.raises(TypeError):
        mapping_holder.m = {"a": "x"}


def test_mapping_key_type_error(mapping_holder):
    with pytest.raises(TypeError):
        mapping_holder.m = {1: 1}


def test_mapping_list_rejected(mapping_holder):
    with pytest.raises(TypeError):
        mapping_holder.m = [1, 2]


def test_mapping_int_rejected(mapping_holder):
    with pytest.raises(TypeError):
        mapping_holder.m = 42


def test_mapping_bare_no_validation(mapping_holder):
    mapping_holder.m_bare = {"a": 1}
    mapping_holder.m_bare = {1: "x"}


def test_mapping_bare_non_mapping_rejected(mapping_holder):
    with pytest.raises(TypeError):
        mapping_holder.m_bare = [1, 2]


def test_mapping_returns_original_value(mapping_holder):
    original = {"a": 1}
    mapping_holder.m = original
    assert mapping_holder.m is original


# -------------------------------------------------------------------------------------
# Block G: Subclass auto-detection (bare types)
# -------------------------------------------------------------------------------------


class SubclassHolder(Ators):
    # UserList subclasses MutableSequence which subclasses Sequence
    ul: UserList = member()
    # UserDict subclasses MutableMapping which subclasses Mapping
    ud: UserDict = member()


@pytest.fixture()
def subclass_holder():
    return SubclassHolder(ul=UserList([1, 2]), ud=UserDict({"a": 1}))


def test_subclass_userlist_detected_as_sequence(subclass_holder):
    # UserList is a Sequence subclass → uses Sequence validator (no item validator)
    subclass_holder.ul = UserList([3, 4])


def test_subclass_userlist_accepts_any_sequence(subclass_holder):
    # Sequence validator: accepts any Sequence (bare — no item type constraint)
    subclass_holder.ul = [1, 2, 3]
    subclass_holder.ul = (1, 2, 3)


def test_subclass_userlist_rejects_non_sequence(subclass_holder):
    with pytest.raises(TypeError):
        subclass_holder.ul = {1, 2}  # set is not a Sequence


def test_subclass_userdict_detected_as_mapping(subclass_holder):
    subclass_holder.ud = UserDict({"b": 2})


def test_subclass_userdict_accepts_any_mapping(subclass_holder):
    subclass_holder.ud = {"c": 3}
    subclass_holder.ud = MappingProxyType({"d": 4})


def test_subclass_userdict_rejects_non_mapping(subclass_holder):
    with pytest.raises(TypeError):
        subclass_holder.ud = [1, 2]


def test_subclass_no_userwarning_emitted():
    """No UserWarning should be emitted when annotating with known ABC subclasses."""
    with warnings.catch_warnings():
        warnings.simplefilter("error", UserWarning)

        class A(Ators):
            ul: UserList = member()
            ud: UserDict = member()


# -------------------------------------------------------------------------------------
# Block G: Subclass auto-detection (generic parameterized types)
# -------------------------------------------------------------------------------------


class MySeq[T](Sequence):
    """Minimal concrete Sequence subclass."""

    def __init__(self, data: list[T]):
        self._data = list(data)

    def __getitem__(self, index):
        return self._data[index]

    def __len__(self):
        return len(self._data)


class MyMap[K, V](Mapping):
    """Minimal concrete Mapping subclass."""

    def __init__(self, data: dict[K, V]):
        self._data = dict(data)

    def __getitem__(self, key):
        return self._data[key]

    def __iter__(self):
        return iter(self._data)

    def __len__(self):
        return len(self._data)


class GenericSubclassHolder(Ators):
    # MySeq[int] — origin is MySeq which is a Sequence subclass
    seq: MySeq[int] = member()
    # MyMap[str, int] — origin is MyMap which is a Mapping subclass
    mp: MyMap[str, int] = member()


@pytest.fixture()
def generic_subclass_holder():
    return GenericSubclassHolder(
        seq=MySeq([1, 2, 3]),
        mp=MyMap({"a": 1}),
    )


def test_generic_subclass_sequence_ok(generic_subclass_holder):
    generic_subclass_holder.seq = MySeq([4, 5, 6])


def test_generic_subclass_sequence_item_validation(generic_subclass_holder):
    with pytest.raises(TypeError):
        generic_subclass_holder.seq = MySeq([1, "x"])


def test_generic_subclass_mapping_ok(generic_subclass_holder):
    generic_subclass_holder.mp = MyMap({"b": 2})


def test_generic_subclass_mapping_item_validation_key(generic_subclass_holder):
    with pytest.raises(TypeError):
        generic_subclass_holder.mp = MyMap({1: 2})  # key should be str


def test_generic_subclass_mapping_item_validation_value(generic_subclass_holder):
    with pytest.raises(TypeError):
        generic_subclass_holder.mp = MyMap({"a": "x"})  # value should be int


def test_generic_subclass_no_userwarning_emitted():
    """Parameterized generic ABC subclasses should not emit UserWarning."""
    with warnings.catch_warnings():
        warnings.simplefilter("error", UserWarning)

        class A(Ators):
            seq: MySeq[int] = member()
            mp: MyMap[str, int] = member()
