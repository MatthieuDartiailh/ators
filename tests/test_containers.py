# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test ators object containers validation behavior."""

from contextlib import nullcontext
from types import MappingProxyType

import pytest


@pytest.fixture()
def ators_list_object():
    from ators import Ators

    class A(Ators):
        a: list[int]

    return A(a=[1, 2, 3])


@pytest.mark.parametrize(
    "operation, args, expected, exception",
    [
        ("append", (4,), [1, 2, 3, 4], None),
        ("append", ("e",), [1, 2, 3], TypeError),
        ("insert", (0, 0), [0, 1, 2, 3], None),
        ("insert", (0, "e"), [1, 2, 3], TypeError),
        ("__setitem__", (0, 10), [10, 2, 3], None),
        ("__setitem__", (0, "e"), [1, 2, 3], TypeError),
        ("__setitem__", (slice(0, 2), [10, 20]), [10, 20, 3], None),
        ("__setitem__", (slice(0, 2), [10, "e"]), [1, 2, 3], TypeError),
        ("extend", ([4, 5],), [1, 2, 3, 4, 5], None),
        ("extend", ([4, "5"],), [1, 2, 3], TypeError),
        ("__iadd__", ([4, 5],), [1, 2, 3, 4, 5], None),
        ("__iadd__", ([4, "5"],), [1, 2, 3], TypeError),
        # Operations that don't add items don't need validation
        ("pop", (), [1, 2], None),
        ("remove", (1,), [2, 3], None),
        ("__delitem__", (0,), [2, 3], None),
        ("reverse", (), [3, 2, 1], None),
        ("sort", (), [1, 2, 3], None),
    ],
)
def test_list_container_validation(
    ators_list_object, operation, args, expected, exception
):
    with pytest.raises(exception) if exception else nullcontext():
        getattr(ators_list_object.a, operation)(*args)
    assert list(ators_list_object.a) == expected


@pytest.fixture()
def ators_set_object():
    from ators import Ators

    class A(Ators):
        a: set[int]

    return A(a={1, 2, 3})


@pytest.mark.parametrize(
    "operation, value, expected, exception",
    [
        ("add", 4, {1, 2, 3, 4}, None),
        ("add", "e", {1, 2, 3}, TypeError),
        ("__ior__", {4, 5}, {1, 2, 3, 4, 5}, None),
        ("__ior__", {4, "5"}, {1, 2, 3}, TypeError),
        ("update", {4, 5}, {1, 2, 3, 4, 5}, None),
        ("update", {4, "5"}, {1, 2, 3}, TypeError),
        ("__isub__", {2}, {1, 3}, None),
        ("__isub__", {"2"}, {1, 2, 3}, None),
        ("difference_update", {2}, {1, 3}, None),
        ("difference_update", {"2"}, {1, 2, 3}, None),
        ("__iand__", {1, 2}, {1, 2}, None),
        ("__iand__", {1, "2"}, {1}, None),
        ("intersection_update", {1, 2}, {1, 2}, None),
        ("intersection_update", {1, "2"}, {1}, None),
        ["__ixor__", {1, 2, 5}, {3, 5}, None],
        ("__ixor__", {4, "5"}, {1, 2, 3}, TypeError),
        ["symmetric_difference_update", {1, 2, 5}, {3, 5}, None],
        ("symmetric_difference_update", {4, "5"}, {1, 2, 3}, TypeError),
    ],
)
def test_set_container_validation(
    ators_set_object, operation, value, expected, exception
):
    with pytest.raises(exception) if exception else nullcontext():
        getattr(ators_set_object.a, operation)(value)
    assert ators_set_object.a == expected


# def test_ators_set_pickling(ators_set_object):
#     dumped = pickle.dumps(ators_set_object.a)
#     loaded = pickle.loads(dumped)
#     assert loaded == ators_set_object.a
#     assert type(loaded) is set


@pytest.fixture()
def ators_dict_object():
    from ators import Ators

    class A(Ators):
        a: dict[str, int]

    return A(a={"a": 2})


@pytest.mark.parametrize(
    "operation, value, kw, expected, exception",
    [
        ("__setitem__", ("b", 3), None, {"a": 2, "b": 3}, None),
        ("__setitem__", ("a", 3), None, {"a": 3}, None),
        ("__setitem__", (2, 3), None, {"a": 2}, TypeError),
        ("__setitem__", ("2", "1"), None, {"a": 2}, TypeError),
        ("setdefault", ("b", 3), None, {"a": 2, "b": 3}, None),
        ("setdefault", ("a", 3), None, {"a": 2}, None),
        # Value does not need to be validated
        ("setdefault", ("a", "3"), None, {"a": 2}, None),
        ("setdefault", (1, 3), None, {"a": 2}, TypeError),
        ("setdefault", ("b", "3"), None, {"a": 2}, TypeError),
        ("update", ({"b": 3},), {}, {"a": 2, "b": 3}, None),
        ("update", ({"a": 3},), {}, {"a": 3}, None),
        ("update", ({1: 3},), {}, {"a": 2}, TypeError),
        ("update", ({"a": "3"},), {}, {"a": 2}, TypeError),
        ("update", (MappingProxyType({"b": 3}),), {}, {"a": 2, "b": 3}, None),
        ("update", (MappingProxyType({"a": 3}),), {}, {"a": 3}, None),
        ("update", (MappingProxyType({1: 3}),), {}, {"a": 2}, TypeError),
        ("update", (MappingProxyType({"a": "3"}),), {}, {"a": 2}, TypeError),
        ("update", ([("b", 3)],), {}, {"a": 2, "b": 3}, None),
        ("update", ([("a", 3)],), {}, {"a": 3}, None),
        ("update", ([(1, 3)],), {}, {"a": 2}, TypeError),
        ("update", ([("a", "3")],), {}, {"a": 2}, TypeError),
        ("update", None, {"b": 3}, {"a": 2, "b": 3}, None),
        ("update", None, {"a": 3}, {"a": 3}, None),
        ("update", None, {"a": "3"}, {"a": 2}, TypeError),
        ("__ior__", ({"b": 3},), {}, {"a": 2, "b": 3}, None),
        ("__ior__", ({"a": 3},), {}, {"a": 3}, None),
        ("__ior__", ({1: 3},), {}, {"a": 2}, TypeError),
        ("__ior__", ({"a": "3"},), {}, {"a": 2}, TypeError),
    ],
)
def test_dict_container_validation(
    ators_dict_object, operation, value, kw, expected, exception
):
    with pytest.raises(exception) if exception else nullcontext():
        if value is not None:
            getattr(ators_dict_object.a, operation)(*value)
        else:
            getattr(ators_dict_object.a, operation)(**kw)
    assert ators_dict_object.a == expected


def test_list_same_owner_member_reassignment_copies_container():
    from ators import Ators

    class A(Ators):
        a: list[int]

    obj = A(a=[1, 2, 3])
    original = obj.a

    obj.a = original

    assert obj.a == [1, 2, 3]
    assert obj.a is not original
    assert type(obj.a) is type(original)


def test_set_same_owner_member_reassignment_copies_container():
    from ators import Ators

    class A(Ators):
        a: set[int]

    obj = A(a={1, 2, 3})
    original = obj.a

    obj.a = original

    assert obj.a == {1, 2, 3}
    assert obj.a is not original
    assert type(obj.a) is type(original)


def test_dict_same_owner_member_reassignment_copies_container():
    from ators import Ators

    class A(Ators):
        a: dict[str, int]

    obj = A(a={"a": 1, "b": 2})
    original = obj.a

    obj.a = original

    assert obj.a == {"a": 1, "b": 2}
    assert obj.a is not original
    assert type(obj.a) is type(original)


def test_list_reassignment_to_other_member_still_validates():
    from ators import Ators

    class A(Ators):
        ints: list[int]
        strs: list[str]

    obj = A(ints=[1, 2], strs=["x", "y"])

    with pytest.raises(TypeError):
        obj.strs = obj.ints


# def test_ators_dict_pickling(ators_dict_object):
#     dumped = pickle.dumps(ators_dict_object.a)
#     loaded = pickle.loads(dumped)
#     assert loaded == ators_dict_object.a
#     assert type(loaded) is dict


@pytest.fixture()
def ators_ordered_dict_object():
    from typing import OrderedDict

    from ators import Ators

    class A(Ators):
        a: OrderedDict[str, int]

    return A(a={"a": 2, "b": 3, "c": 4})


@pytest.mark.parametrize(
    "operation, value, kw, expected, exception",
    [
        ("__setitem__", ("d", 5), None, {"a": 2, "b": 3, "c": 4, "d": 5}, None),
        ("__setitem__", ("a", 9), None, {"a": 9, "b": 3, "c": 4}, None),
        ("__setitem__", (2, 3), None, {"a": 2, "b": 3, "c": 4}, TypeError),
        ("__setitem__", ("d", "1"), None, {"a": 2, "b": 3, "c": 4}, TypeError),
        ("setdefault", ("d", 5), None, {"a": 2, "b": 3, "c": 4, "d": 5}, None),
        ("setdefault", ("a", 9), None, {"a": 2, "b": 3, "c": 4}, None),
        # Value does not need to be validated for existing keys
        ("setdefault", ("a", "9"), None, {"a": 2, "b": 3, "c": 4}, None),
        ("setdefault", (1, 5), None, {"a": 2, "b": 3, "c": 4}, TypeError),
        ("setdefault", ("d", "5"), None, {"a": 2, "b": 3, "c": 4}, TypeError),
        ("update", ({"d": 5},), {}, {"a": 2, "b": 3, "c": 4, "d": 5}, None),
        ("update", ({"a": 9},), {}, {"a": 9, "b": 3, "c": 4}, None),
        ("update", ({1: 5},), {}, {"a": 2, "b": 3, "c": 4}, TypeError),
        ("update", ({"a": "9"},), {}, {"a": 2, "b": 3, "c": 4}, TypeError),
        ("update", ([("d", 5)],), {}, {"a": 2, "b": 3, "c": 4, "d": 5}, None),
        ("update", ([(1, 5)],), {}, {"a": 2, "b": 3, "c": 4}, TypeError),
        ("update", None, {"d": 5}, {"a": 2, "b": 3, "c": 4, "d": 5}, None),
        ("update", None, {"a": "9"}, {"a": 2, "b": 3, "c": 4}, TypeError),
        ("__ior__", ({"d": 5},), {}, {"a": 2, "b": 3, "c": 4, "d": 5}, None),
        ("__ior__", ({1: 5},), {}, {"a": 2, "b": 3, "c": 4}, TypeError),
        ("__ior__", ({"a": "9"},), {}, {"a": 2, "b": 3, "c": 4}, TypeError),
    ],
)
def test_ordered_dict_container_validation(
    ators_ordered_dict_object, operation, value, kw, expected, exception
):
    with pytest.raises(exception) if exception else nullcontext():
        if value is not None:
            getattr(ators_ordered_dict_object.a, operation)(*value)
        else:
            getattr(ators_ordered_dict_object.a, operation)(**kw)
    assert ators_ordered_dict_object.a == expected


def test_ordered_dict_insertion_order_preserved(ators_ordered_dict_object):
    """Insertion order must be preserved across operations."""
    obj = ators_ordered_dict_object
    assert list(obj.a.keys()) == ["a", "b", "c"]
    obj.a["d"] = 5
    assert list(obj.a.keys()) == ["a", "b", "c", "d"]
    obj.a.update({"e": 6, "f": 7})
    assert list(obj.a.keys()) == ["a", "b", "c", "d", "e", "f"]


def test_ordered_dict_move_to_end_last_true(ators_ordered_dict_object):
    """move_to_end(key) moves key to the back."""
    obj = ators_ordered_dict_object
    obj.a.move_to_end("a")
    assert list(obj.a.keys()) == ["b", "c", "a"]
    assert obj.a["a"] == 2


def test_ordered_dict_move_to_end_last_false(ators_ordered_dict_object):
    """move_to_end(key, last=False) moves key to the front."""
    obj = ators_ordered_dict_object
    obj.a.move_to_end("c", last=False)
    assert list(obj.a.keys()) == ["c", "a", "b"]
    assert obj.a["c"] == 4


def test_ordered_dict_move_to_end_nonexistent_key(ators_ordered_dict_object):
    """move_to_end raises KeyError for a missing key."""
    with pytest.raises(KeyError):
        ators_ordered_dict_object.a.move_to_end("nonexistent")


def test_ordered_dict_popitem_last_true(ators_ordered_dict_object):
    """popitem(last=True) removes and returns the last inserted item."""
    obj = ators_ordered_dict_object
    k, v = obj.a.popitem()
    assert k == "c"
    assert v == 4
    assert list(obj.a.keys()) == ["a", "b"]


def test_ordered_dict_popitem_last_false(ators_ordered_dict_object):
    """popitem(last=False) removes and returns the first inserted item."""
    obj = ators_ordered_dict_object
    k, v = obj.a.popitem(last=False)
    assert k == "a"
    assert v == 2
    assert list(obj.a.keys()) == ["b", "c"]


def test_ordered_dict_popitem_empty():
    """popitem on an empty AtorsOrderedDict raises KeyError."""
    from typing import OrderedDict

    from ators import Ators

    class A(Ators):
        a: OrderedDict[str, int]

    obj = A(a={})
    with pytest.raises(KeyError):
        obj.a.popitem()


def test_ordered_dict_same_owner_member_reassignment_copies_container():
    from typing import OrderedDict

    from ators import Ators

    class A(Ators):
        a: OrderedDict[str, int]

    obj = A(a={"x": 1, "y": 2})
    original = obj.a

    obj.a = original

    assert obj.a == {"x": 1, "y": 2}
    assert obj.a is not original
    assert type(obj.a) is type(original)


def test_ordered_dict_container_type():
    """AtorsOrderedDict is a subclass of dict."""
    from typing import OrderedDict

    from ators import Ators
    from ators._ators import AtorsOrderedDict

    class A(Ators):
        a: OrderedDict[str, int]

    obj = A(a={"x": 1})
    assert isinstance(obj.a, dict)
    assert isinstance(obj.a, AtorsOrderedDict)
