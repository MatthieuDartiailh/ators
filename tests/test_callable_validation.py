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
    with pytest.raises(BaseException) as exc:
        add_one("1")  # type: ignore[arg-type]

    # Should be an ExceptionGroup since aggregate_errors defaults to True
    assert exc.typename == "ExceptionGroup"
    assert hasattr(exc.value, "exceptions")
    assert len(exc.value.exceptions) == 1


def test_validated_aggregate_errors() -> None:
    @validated(aggregate_errors=True)
    def f(x: int, y: int) -> int:
        return x + y

    with pytest.raises(BaseException) as exc:
        f("a", "b")  # type: ignore[arg-type]

    # Should be an ExceptionGroup with multiple errors
    assert exc.typename == "ExceptionGroup"
    assert hasattr(exc.value, "exceptions")
    assert len(exc.value.exceptions) == 2


def test_validated_methods_instance() -> None:
    class C:
        @validated
        def inst(self, x: int) -> int:
            return x

    c = C()
    assert c.inst(1) == 1

    with pytest.raises(BaseException) as exc:
        c.inst("1")  # type: ignore[arg-type]

    assert exc.typename == "ExceptionGroup"


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
    with pytest.raises(BaseException) as exc:
        C.stat("1")  # type: ignore[arg-type]

    assert exc.typename == "ExceptionGroup"


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
    with pytest.raises(BaseException) as exc:
        C.cls("1")  # type: ignore[arg-type]

    assert exc.typename == "ExceptionGroup"


def test_validated_async_function() -> None:
    @validated
    async def af(x: int) -> int:
        return x + 1

    assert asyncio.run(af(3)) == 4
    with pytest.raises(BaseException) as exc:
        asyncio.run(af("3"))  # type: ignore[arg-type]

    assert exc.typename == "ExceptionGroup"


def test_validated_async_function_return_validation() -> None:
    @validated
    async def af(x: int) -> int:
        return str(x)  # type: ignore

    with pytest.raises(TypeError):
        asyncio.run(af(3))


def test_validated_checks_default_values_when_argument_missing() -> None:
    @validated
    def f(x: int = "1"):  # type: ignore
        return x

    with pytest.raises(BaseException) as exc:
        f()

    assert exc.typename == "ExceptionGroup"

    assert f(2) == 2


def test_strict_mode_fails_fast() -> None:
    @validated(aggregate_errors=False)
    def f(x: int, y: int) -> int:
        return x + y

    with pytest.raises(TypeError):
        f("a", "b")  # type: ignore[arg-type]


def test_validated_varargs_and_kwargs_aggregate_errors() -> None:
    @validated(aggregate_errors=True)
    def f(*values: int, **mapping: int) -> int:
        return sum(values) + sum(mapping.values())

    with pytest.raises(BaseException) as exc:
        f(1, "2", ok=3, ko="4")  # type: ignore[arg-type]

    # Should be an ExceptionGroup with multiple errors
    assert exc.typename == "ExceptionGroup"
    assert hasattr(exc.value, "exceptions")
    assert len(exc.value.exceptions) == 2


def test_validated_positional_only_argument() -> None:
    @validated
    def f(x: int, /, y: int) -> int:
        return x + y

    assert f(1, 2) == 3
    with pytest.raises(BaseException) as exc:
        f("1", 2)  # type: ignore[arg-type]

    assert exc.typename == "ExceptionGroup"


def test_validated_keyword_only_and_varkw_aggregate_errors() -> None:
    @validated(aggregate_errors=True)
    def f(*, x: int, **rest: int) -> int:
        return x + sum(rest.values())

    with pytest.raises(BaseException) as exc:
        f(x="1", y="2")  # type: ignore[arg-type]

    # Should be an ExceptionGroup with multiple errors
    assert exc.typename == "ExceptionGroup"
    assert hasattr(exc.value, "exceptions")
    assert len(exc.value.exceptions) == 2


def test_validated_validate_return_false() -> None:
    @validated(validate_return=False)
    def f(x: int) -> int:
        return str(x)  # type: ignore

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
        def f(x: ClassVar[int]) -> int:  # type: ignore
            return x


def test_validated_rejects_subscripted_member_annotation() -> None:
    with pytest.raises(TypeError, match="subscripted Member annotations"):

        @validated
        def f(x: Member[int, int]) -> int:
            return x  # type: ignore[return-value]


# ============================================================================
# Tests for validation with argument transformation (list[int] across positions)
# ============================================================================


def test_list_validation_positional_only() -> None:
    """Test list[int] validation in positional-only parameter."""

    @validated
    def f(items: list[int], /) -> int:
        return len(items)

    assert f([1, 2, 3]) == 3
    assert f([]) == 0


def test_list_validation_positional_only_invalid_items() -> None:
    """Test list[int] validation error in positional-only parameter."""

    @validated
    def f(items: list[int], /) -> int:
        return len(items)

    with pytest.raises(BaseException) as exc:
        f([1, "2", 3])  # type: ignore[list-item]

    assert exc.typename == "ExceptionGroup"


def test_list_validation_positional_or_keyword() -> None:
    """Test list[int] validation in positional-or-keyword parameter."""

    @validated
    def f(items: list[int]) -> int:
        return sum(items)

    assert f([1, 2, 3]) == 6
    assert f(items=[10, 20]) == 30


def test_list_validation_positional_or_keyword_invalid_items() -> None:
    """Test list[int] validation error in positional-or-keyword parameter."""

    @validated
    def f(items: list[int]) -> int:
        return sum(items)

    with pytest.raises(BaseException) as exc:
        f([1, "2", 3])  # type: ignore[list-item]

    assert exc.typename == "ExceptionGroup"

    with pytest.raises(BaseException) as exc:
        f(items=[1, "2", 3])  # type: ignore[list-item]

    assert exc.typename == "ExceptionGroup"


def test_list_validation_varargs() -> None:
    """Test list[int] validation in *args parameter."""

    @validated
    def f(*values: list[int]) -> int:
        return sum(len(v) for v in values)

    assert f([1, 2], [3, 4, 5]) == 5
    assert f([]) == 0


def test_list_validation_varargs_invalid_items() -> None:
    """Test list[int] validation error in *args parameter."""

    @validated
    def f(*values: list[int]) -> int:
        return sum(len(v) for v in values)

    with pytest.raises(BaseException) as exc:
        f([1, 2], [3, "4", 5])  # type: ignore[list-item]

    assert exc.typename == "ExceptionGroup"
    assert hasattr(exc.value, "exceptions")
    assert len(exc.value.exceptions) == 1


def test_list_validation_kwargs() -> None:
    """Test list[int] validation in **kwargs parameter."""

    @validated
    def f(**mappings: list[int]) -> int:
        return sum(len(v) for v in mappings.values())

    assert f(a=[1, 2], b=[3, 4, 5]) == 5
    assert f() == 0


def test_list_validation_kwargs_invalid_items() -> None:
    """Test list[int] validation error in **kwargs parameter."""

    @validated
    def f(**mappings: list[int]) -> int:
        return sum(len(v) for v in mappings.values())

    with pytest.raises(BaseException) as exc:
        f(a=[1, 2], b=[3, "4", 5])  # type: ignore[list-item]

    assert exc.typename == "ExceptionGroup"
    assert hasattr(exc.value, "exceptions")
    assert len(exc.value.exceptions) == 1
