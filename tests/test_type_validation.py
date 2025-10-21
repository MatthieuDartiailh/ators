# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test type validation for ators object"""

from typing import Any

import pytest

from ators import Ators, member


class OB:
    pass


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
        # XXX add tuple once the Rust impl is complete
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
        with pytest.raises(TypeError):
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
