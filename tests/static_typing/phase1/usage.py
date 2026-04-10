# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Static type examples for Phase 1: init support in member and AtorsMeta.

These examples are intended to be checked with mypy / ty to verify that
dataclass_transform-aware type checkers correctly infer __init__ signatures.
"""

from ators import Ators, member


class PublicMembers(Ators):
    x: int
    y: str


# Both x and y should be accepted as keyword arguments.
obj1 = PublicMembers(x=1, y="hello")


class PrivateMember(Ators):
    x: int
    _internal: int


# x is public (init=True), _internal is private (init=False by default).
# A conforming type checker should flag _internal as not part of __init__.
obj2 = PrivateMember(x=42)


class ExplicitInitFalse(Ators):
    x: int
    computed: int = member(init=False)


# computed is excluded from __init__ by explicit init=False.
obj3 = ExplicitInitFalse(x=10)


class ExplicitInitTrueOnPrivate(Ators):
    x: int
    _override: int = member(init=True)


# _override is explicitly opted in so it should appear in __init__.
obj4 = ExplicitInitTrueOnPrivate(x=1, _override=2)
