# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for Python 3.15 builtin frozendict support."""

import builtins
import pickle
from collections.abc import Mapping
from typing import Any

import pytest

from ators import Ators, Member, is_frozen, member

FROZENDICT = getattr(builtins, "frozendict", None)

pytestmark = pytest.mark.skipif(
    FROZENDICT is None, reason="requires Python 3.15 builtin frozendict"
)


class MappingLike(Mapping):
    def __init__(self, data):
        self._data = data

    def __getitem__(self, key):
        return self._data[key]

    def __iter__(self):
        return iter(self._data)

    def __len__(self):
        return len(self._data)


if FROZENDICT is not None:

    class _PickleFrozendictClass(Ators):
        mapping: FROZENDICT[str, int]


def test_frozendict_annotation_support_and_valid_assignment():
    class A(Ators):
        mapping: FROZENDICT[str, int] = member()

    value = FROZENDICT({"a": 1})
    a = A()
    a.mapping = value

    assert a.mapping == value
    assert type(a.mapping) is FROZENDICT


@pytest.mark.parametrize("bad_value", [{1: 1}, {"a": "1"}], ids=["bad-key", "bad-value"])
def test_frozendict_rejects_invalid_key_or_value_types(bad_value):
    class A(Ators):
        mapping: FROZENDICT[str, int] = member()

    a = A()
    with pytest.raises(TypeError):
        a.mapping = FROZENDICT(bad_value)


def test_frozendict_rejects_plain_dict_without_coercion():
    class A(Ators):
        mapping: FROZENDICT[str, int] = member()

    a = A()
    with pytest.raises(TypeError, match="frozendict"):
        a.mapping = {"a": 1}


@pytest.mark.parametrize(
    "value",
    [
        {1: "2", "3": 4},
        MappingLike({1: "2", "3": 4}),
        [(1, "2"), ("3", 4)],
    ],
)
def test_frozendict_coercion_accepts_mapping_like_inputs(value):
    class A(Ators):
        mapping: Member[FROZENDICT[str, int], Any] = member().coerce()

    a = A(mapping=value)

    assert type(a.mapping) is FROZENDICT
    assert a.mapping == FROZENDICT({"1": 2, "3": 4})


def test_frozendict_nested_coercion_recursively_applies_validators():
    class A(Ators):
        mapping: Member[FROZENDICT[str, tuple[int, ...]], Any] = member().coerce()

    a = A(mapping={"items": ["1", "2"]})

    assert type(a.mapping) is FROZENDICT
    assert a.mapping == FROZENDICT({"items": (1, 2)})


def test_frozendict_validation_rebuilds_when_nested_values_are_copied():
    class A(Ators):
        mapping: FROZENDICT[str, list[int]] = member()

    items = [1, 2]
    value = FROZENDICT({"items": items})
    a = A(mapping=value)

    assert type(a.mapping) is FROZENDICT
    assert a.mapping == value
    assert a.mapping is not value
    assert a.mapping["items"] == items
    assert a.mapping["items"] is not items


def test_frozendict_is_compatible_with_frozen_classes():
    class TypedFrozen(Ators, frozen=True):
        mapping: FROZENDICT[str, int]

    class RawFrozen(Ators, frozen=True):
        mapping: FROZENDICT

    typed = TypedFrozen(mapping=FROZENDICT({"a": 1}))
    raw = RawFrozen(mapping=FROZENDICT({"a": 1}))

    assert is_frozen(typed)
    assert is_frozen(raw)


def test_frozendict_pickling_round_trip():
    a = _PickleFrozendictClass(mapping=FROZENDICT({"a": 1}))
    restored = pickle.loads(pickle.dumps(a))

    assert type(restored.mapping) is FROZENDICT
    assert restored.mapping == FROZENDICT({"a": 1})
