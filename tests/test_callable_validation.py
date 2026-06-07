# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for callable validation decorators."""

import asyncio
import inspect
from typing import ClassVar

import pytest

from ators import Member, validated


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


def test_validated_methods_instance() -> None:
    class C:
        @validated
        def inst(self, x: int) -> int:
            return x

    c = C()
    assert c.inst(1) == 1

    with pytest.raises(TypeError):
        c.inst("1")  # type: ignore[arg-type]


def test_validated_rejects_staticmethod_target() -> None:
    with pytest.raises(TypeError, match="validated cannot be applied to staticmethod"):
        class C:
            @validated
            @staticmethod
            def stat(x: int) -> int:
                return x


def test_validated_then_staticmethod_works() -> None:
    class C:
        @staticmethod
        @validated
        def stat(x: int) -> int:
            return x

    assert C.stat(1) == 1
    assert C().stat(1) == 1
    with pytest.raises(TypeError):
        C.stat("1")  # type: ignore[arg-type]


def test_validated_rejects_classmethod_target() -> None:
    with pytest.raises(TypeError, match="validated cannot be applied to classmethod"):
        class C:
            @validated
            @classmethod
            def cls(cls, x: int) -> int:
                return x


def test_validated_then_classmethod_works() -> None:
    class C:
        @classmethod
        @validated
        def cls(cls, x: int) -> int:
            return x

    assert C.cls(1) == 1
    assert C().cls(2) == 2
    with pytest.raises(TypeError):
        C.cls("1")  # type: ignore[arg-type]


def test_validated_async_function() -> None:
    @validated
    async def af(x: int) -> int:
        return x + 1

    assert asyncio.run(af(3)) == 4
    with pytest.raises(TypeError):
        asyncio.run(af("3"))  # type: ignore[arg-type]


def test_validated_async_function_return_validation() -> None:
    @validated
    async def af(x: int) -> int:
        return str(x)  # type: ignore[return-value]

    with pytest.raises(TypeError):
        asyncio.run(af(3))


def test_validated_checks_default_values_when_argument_missing() -> None:
    @validated
    def f(x: int = "1"):  # type: ignore[assignment]
        return x

    with pytest.raises(TypeError):
        f()

    assert f(2) == 2


def test_strict_mode_fails_fast() -> None:
    @validated(strict=True)
    def f(x: int, y: int) -> int:
        return x + y

    with pytest.raises(TypeError):
        f("a", "b")  # type: ignore[arg-type]


def test_validated_varargs_and_kwargs_aggregate_errors() -> None:
    @validated(aggregate_errors=True)
    def f(*values: int, **mapping: int) -> int:
        return sum(values) + sum(mapping.values())

    with pytest.raises(TypeError) as exc:
        f(1, "2", ok=3, ko="4")  # type: ignore[arg-type]

    msg = str(exc.value)
    assert "values[1]" in msg
    assert "mapping.ko" in msg


def test_validated_positional_only_argument() -> None:
    @validated
    def f(x: int, /, y: int) -> int:
        return x + y

    assert f(1, 2) == 3
    with pytest.raises(TypeError):
        f("1", 2)  # type: ignore[arg-type]


def test_validated_keyword_only_and_varkw_aggregate_errors() -> None:
    @validated(aggregate_errors=True)
    def f(*, x: int, **rest: int) -> int:
        return x + sum(rest.values())

    with pytest.raises(TypeError) as exc:
        f(x="1", y="2")  # type: ignore[arg-type]

    msg = str(exc.value)
    assert "x" in msg
    assert "rest.y" in msg


def test_validated_validate_return_false() -> None:
    @validated(validate_return=False)
    def f(x: int) -> int:
        return str(x)  # type: ignore[return-value]

    assert f(1) == "1"


def test_validated_rejects_non_callable() -> None:
    with pytest.raises(TypeError, match="validated can only be applied to callables"):
        validated(1)  # type: ignore[arg-type]


def test_validated_factory_requires_callable_target() -> None:
    decorator = validated(aggregate_errors=True)
    with pytest.raises(TypeError):
        decorator()


def test_validated_preserves_wrapped_metadata() -> None:
    @validated
    def add_one(x: int) -> int:
        return x + 1

    assert add_one.__name__ == "add_one"
    assert hasattr(add_one, "__wrapped__")
    assert str(inspect.signature(add_one)) == "(x: int) -> int"


def test_validated_rejects_classvar_annotation() -> None:
    with pytest.raises(TypeError, match="ClassVar"):
        @validated
        def f(x: ClassVar[int]) -> int:
            return x  # type: ignore[return-value]


def test_validated_rejects_subscripted_member_annotation() -> None:
    with pytest.raises(TypeError, match="subscripted Member annotations"):
        @validated
        def f(x: Member[int, int]) -> int:
            return x  # type: ignore[return-value]
