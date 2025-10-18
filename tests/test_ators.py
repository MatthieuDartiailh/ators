# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test default behavior for ators object"""

import pytest

from ators import Ators, member


def test_member_slot_do_not_overlap():
    class A(Ators):
        a = member()
        b = member()

    a = A()
    a.a = 1
    a.b = 2
    assert a.a == 1
    assert a.b == 2


def test_dual_use_is_forbidden():
    with pytest.raises(RuntimeError) as e:

        class A(Ators):
            a = b = member()

    assert "assigned the same member" in e.exconly()
