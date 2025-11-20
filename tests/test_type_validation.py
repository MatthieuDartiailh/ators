# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test type validation for ators object"""

from abc import ABC
from typing import Any, Literal

import pytest

from ators import Ators, member


class OB:
    pass


class CustomBase(ABC):
    pass


class CustomObj:
    pass


CustomBase.register(CustomObj)


# FIXME validate error messages
@pytest.mark.parametrize(
    "ann, goods, bads",
    [
        (object, [1, object()], []),
        (Any, [1, object()], []),
        (bool, [False, True], [""]),
        (int, [0, 1, -1], [1.0, ""]),
        (float, [0.0, 0.1], [1, ""]),
        (str, ["a"], [1]),
        (bytes, [b"a"], [""]),
        (OB, [OB()], [""]),
        (tuple, [()], [1, ""]),
        (tuple[int, ...], [(), (1,), (1, 2, 3)], [1, ("a",)]),
        (tuple[int, int], [(1, 2)], [1, (), (1,), (1, 2, 3), (1, "a")]),
        (Literal[1, 2, 3], [1, 2, 3], [0, 4, "a"]),
        (CustomBase, [CustomObj()], ["", 1, object()]),
        (int | str, [1, "a"], [1.0, object()]),
        (int | str | None, [1, "a", None], [1.0, object()]),
        (int | tuple[int, int], [1, (1, 2)], [1.0, (1, 2, 3), "c", object()]),
        (int | Literal["a", "b"], [1, "a", "b"], [1.0, "c", object()]),
    ],
)
def test_type_validators(ann, goods, bads):
    class A(Ators):
        a: ann = member()

    a = A()
    for good in goods:
        a.a = good
        assert a.a == good

    for bad in bads:
        with pytest.raises((TypeError, ValueError)):
            a.a = bad


def test_inherited_type_validator():
    class A(Ators):
        a: int

    class B(A):
        a = member().inherit()

    b = B()
    b.a = 5
    assert b.a == 5
    with pytest.raises(TypeError):
        b.a = ""
