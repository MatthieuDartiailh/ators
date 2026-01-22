# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test freezing ators object"""

import pytest

from ators import Ators, freeze, is_frozen

# XXX forbid freezing object containing mutable members


def test_freezing_post_init():
    class A(Ators):
        a: int

    a = A()
    a.a = 12
    assert a.a == 12
    assert not is_frozen(a)
    freeze(a)
    assert is_frozen(a)
    with pytest.raises(TypeError) as e:
        a.a = 1
    assert "Cannot modify" in e.exconly()


def test_frozen_class():
    class A(Ators, frozen=True):
        a: int

    a = A(a=12)
    assert a.a == 12
    assert is_frozen(a)
    with pytest.raises(TypeError) as e:
        a.a = 1
    assert "Cannot modify" in e.exconly()


def test_frozen_inheritance():
    class A(Ators, frozen=True):
        a: int

    class B(A, frozen=True):
        b: int

    with pytest.raises(TypeError) as e:

        class C(A):
            c: int

    assert "not frozen but inherit" in e.exconly()
