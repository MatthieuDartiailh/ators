# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test ators object containers validation behavior."""

import pickle
from contextlib import nullcontext
from types import MappingProxyType

import pytest


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


# XXX bad module
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


# XXX bad module
# def test_ators_dict_pickling(ators_dict_object):
#     dumped = pickle.dumps(ators_dict_object.a)
#     loaded = pickle.loads(dumped)
#     assert loaded == ators_dict_object.a
#     assert type(loaded) is dict
