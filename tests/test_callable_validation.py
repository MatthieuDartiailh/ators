# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for callable validation decorators."""

import asyncio

import pytest

from ators import validated


def test_validated_function_argument_and_return() -> None:
    @validated
    def add_one(x: int) -> int:
        return x + 1

    assert add_one(1) == 2
    with pytest.raises(TypeError):
        add_one("1")  # type: ignore[arg-type]


def test_validated_aggregate_errors() -> None:
    @validated(aggregate_errors=True)
    def f(x: int, y: int) -> int:
        return x + y

    with pytest.raises(TypeError) as exc:
        f("a", "b")  # type: ignore[arg-type]

    msg = str(exc.value)
    assert "x" in msg
    assert "y" in msg


def test_validated_methods_static_and_class() -> None:
    class C:
        @validated
        def inst(self, x: int) -> int:
            return x

        @validated
        @staticmethod
        def stat(x: int) -> int:
            return x

        @validated
        @classmethod
        def cls(cls, x: int) -> int:
            return x

    c = C()
    assert c.inst(1) == 1
    assert c.stat(2) == 2
    assert c.cls(3) == 3

    with pytest.raises(TypeError):
        c.inst("1")  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        c.stat("2")  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        c.cls("3")  # type: ignore[arg-type]


def test_validated_async_function() -> None:
    @validated
    async def af(x: int) -> int:
        return x + 1

    assert asyncio.run(af(3)) == 4
    with pytest.raises(TypeError):
        asyncio.run(af("3"))  # type: ignore[arg-type]


def test_strict_mode_fails_fast() -> None:
    @validated(strict=True)
    def f(x: int, y: int) -> int:
        return x + y

    with pytest.raises(TypeError):
        f("a", "b")  # type: ignore[arg-type]
