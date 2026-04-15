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
        ("__setitem__", (-1, 10), [1, 2, 10], None),
        ("__setitem__", (3, 10), [1, 2, 3], IndexError),
        ("__setitem__", (slice(0, 2), [10, 20]), [10, 20, 3], None),
        ("__setitem__", (slice(0, 2), [10, "e"]), [1, 2, 3], TypeError),
        # Extended slice (step != 1)
        ("__setitem__", (slice(0, 3, 2), [9, 10]), [9, 2, 10], None),
        ("__setitem__", (slice(0, 3, 2), [9]), [1, 2, 3], ValueError),
        ("__setitem__", (slice(0, 3, 2), [9, "e"]), [1, 2, 3], TypeError),
        # Negative step
        ("__setitem__", (slice(2, None, -2), [9, 10]), [10, 2, 9], None),
        ("extend", ([4, 5],), [1, 2, 3, 4, 5], None),
        ("extend", ([4, "5"],), [1, 2, 3], TypeError),
        ("__iadd__", ([4, 5],), [1, 2, 3, 4, 5], None),
        ("__iadd__", ([4, "5"],), [1, 2, 3], TypeError),
        # Operations that don't add items don't need validation
        ("pop", (), [1, 2], None),
        ("remove", (1,), [2, 3], None),
        # ("__delitem__", (0,), [2, 3], None),
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
        ("__delitem__", ("a",), None, {}, None),
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
        obj.strs = obj.ints  # type: ignore


# def test_ators_dict_pickling(ators_dict_object):
#     dumped = pickle.dumps(ators_dict_object.a)
#     loaded = pickle.loads(dumped)
#     assert loaded == ators_dict_object.a
#     assert type(loaded) is dict


# @pytest.mark.parametrize(
#     "initial, delete_index, expected, expected_exc",
#     [
#         ([1, 2, 3], 0, [2, 3], None),
#         ([1, 2, 3], -1, [1, 2], None),
#         ([1, 2, 3], 3, None, IndexError),
#         ([1, 2, 3], -4, None, IndexError),
#         ([0, 1, 2, 3, 4, 5], slice(1, 4), [0, 4, 5], None),  # contiguous slice
#         (
#             [0, 1, 2, 3, 4, 5],
#             slice(None, None, 2),
#             [1, 3, 5],
#             None,
#         ),  # extended slice step=2
#         ([10, 11, 12, 13, 14], slice(3, None, -2), [10, 12, 14], None),  # negative step
#         ([0, 1, 2, 3, 4], slice(4, 0, -2), [0, 1, 3], None),
#         ([1, 2, 3], slice(1, 1), [1, 2, 3], None),  # empty slice no-op
#         ([1, 2, 3], "0", None, TypeError),  # invalid index type
#     ],
# )
# def test_list_delitem_parametrized(initial, delete_index, expected, expected_exc):
#     """Parametrized tests for list __delitem__ behaviour on Ators list members.

#     This uses the existing A(Ators) class in this module which declares a slot for
#     list_field (the file already uses list-related tests earlier). Each case
#     verifies semantics for single index, negative index, out-of-range, contiguous
#     slice, extended-slice (step != 1), negative-step slices and invalid index
#     types.
#     """
#     from ators import Ators

#     class A(Ators):
#         list_field: list[int]

#     a = A(list_field=initial.copy())
#     if expected_exc is not None:
#         with pytest.raises(expected_exc):
#             del a.list_field[delete_index]  # type: ignore[arg-type]
#     else:
#         del a.list_field[delete_index]
#         assert list(a.list_field) == expected
