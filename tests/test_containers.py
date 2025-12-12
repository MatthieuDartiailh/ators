# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test ators object containers validation behavior."""

from contextlib import nullcontext

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
