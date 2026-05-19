# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test ators object containers validation behavior."""

from collections import defaultdict
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
        # __delitem__: positive index
        ("__delitem__", (0,), [2, 3], None),
        # __delitem__: negative index
        ("__delitem__", (-1,), [1, 2], None),
        # __delitem__: out-of-range index
        ("__delitem__", (3,), [1, 2, 3], IndexError),
        # __delitem__: out-of-range negative index
        ("__delitem__", (-4,), [1, 2, 3], IndexError),
        # __delitem__: contiguous slice
        ("__delitem__", (slice(0, 2),), [3], None),
        # __delitem__: extended slice (step != 1) — deletes indices 0 and 2 from [1,2,3]
        ("__delitem__", (slice(None, None, 2),), [2], None),
        # __delitem__: negative step — deletes indices 2 and 0 from [1,2,3]
        ("__delitem__", (slice(2, 0, -1),), [1], None),
        # __delitem__: empty slice is a no-op
        ("__delitem__", (slice(1, 1),), [1, 2, 3], None),
        # __delitem__: invalid index type raises TypeError
        ("__delitem__", ("0",), [1, 2, 3], TypeError),
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


@pytest.fixture(params=[dict[str, int], defaultdict[str, int]], ids=["dict", "defaultdict"])
def dict_annotation(request):
    return request.param


def ators_dict_object(dict_annotation):
    from ators import Ators

    class A(Ators):
        a: dict_annotation

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


def test_dict_same_owner_member_reassignment_copies_container(dict_annotation):
    from ators import Ators

    class A(Ators):
        a: dict_annotation

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


def test_dict_assignment_from_dict_and_copy_on_reassignment(dict_annotation):
    from ators import Ators

    class A(Ators):
        a: dict_annotation

    obj = A(a={"a": 1, "b": 2})
    source = {"x": 10}
    obj.a = source
    source["y"] = 20
    assert dict(obj.a) == {"x": 10}

    original = obj.a
    obj.a = original
    assert obj.a == {"x": 10}
    assert obj.a is not original
    obj.a["z"] = 30
    assert "z" not in original


def test_defaultdict_missing_scalar_default():
    from ators import Ators

    class A(Ators):
        a: defaultdict[str, int]

    obj = A(a={})
    assert obj.a["missing"] == 0
    assert obj.a["missing"] == 0
    assert dict(obj.a) == {"missing": 0}
    with pytest.raises(TypeError):
        _ = obj.a[1]  # type: ignore[index]


def test_defaultdict_missing_nested_defaults_and_validation():
    from ators import Ators

    class A(Ators):
        l: defaultdict[str, list[int]]
        s: defaultdict[str, set[int]]
        d: defaultdict[str, dict[str, int]]
        dd: defaultdict[str, defaultdict[str, int]]

    obj = A(l={}, s={}, d={}, dd={})

    obj.l["k"].append(1)
    with pytest.raises(TypeError):
        obj.l["k"].append("bad")

    obj.s["k"].add(1)
    with pytest.raises(TypeError):
        obj.s["k"].add("bad")

    obj.d["k"]["x"] = 1
    with pytest.raises(TypeError):
        obj.d["k"]["x"] = "bad"

    assert obj.dd["outer"]["inner"] == 0
    with pytest.raises(TypeError):
        _ = obj.dd[1]  # type: ignore[index]


def test_defaultdict_missing_typed_default_requires_nullary_ctor():
    from ators import Ators

    class RequiresArg:
        def __init__(self, value):
            self.value = value

    class A(Ators):
        a: defaultdict[str, RequiresArg]

    obj = A(a={})
    with pytest.raises(TypeError):
        _ = obj.a["missing"]
