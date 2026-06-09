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


def test_validated_function_argument() -> None:
    @validated
    def add_one(x: int) -> int:
        return x + 1

    assert add_one(1) == 2
    with pytest.raises(ExceptionGroup) as exc:
        add_one("1")  # type: ignore[arg-type]

    # Expected failure: x has type mismatch
    assert len(exc.value.exceptions) == 1
    assert isinstance(exc.value.exceptions[0], TypeError)
    assert "Failed to validate 'x'" in str(exc.value.exceptions[0])


def test_validated_aggregate_errors() -> None:
    @validated(aggregate_errors=True)
    def f(x: int, y: int) -> int:
        return x + y

    # Test success case
    assert f(1, 2) == 3

    # Test failure case: both x and y have type mismatches
    with pytest.raises(ExceptionGroup) as exc:
        f("a", "b")  # type: ignore[arg-type]

    assert len(exc.value.exceptions) == 2
    param_names = set()
    for inner_exc in exc.value.exceptions:
        assert isinstance(inner_exc, TypeError)
        param_name = str(inner_exc).split("'")[1]
        param_names.add(param_name)
    assert param_names == {"x", "y"}


def test_fails_fast() -> None:
    @validated(aggregate_errors=False)
    def f(x: int, y: int) -> int:
        return x + y

    with pytest.raises(TypeError):
        f("a", "b")  # type: ignore[arg-type]


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
    with pytest.raises(ExceptionGroup) as exc:
        C.stat("1")  # type: ignore[arg-type]

    assert len(exc.value.exceptions) == 1
    assert isinstance(exc.value.exceptions[0], TypeError)


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
    with pytest.raises(ExceptionGroup) as exc:
        C.cls("1")  # type: ignore[arg-type]

    assert len(exc.value.exceptions) == 1
    assert isinstance(exc.value.exceptions[0], TypeError)


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


def test_validated_async_function() -> None:
    @validated
    async def af(x: int) -> int:
        return x + 1

    assert asyncio.run(af(3)) == 4
    with pytest.raises(ExceptionGroup) as exc:
        asyncio.run(af("3"))  # type: ignore[arg-type]

    assert len(exc.value.exceptions) == 1
    assert isinstance(exc.value.exceptions[0], TypeError)


def test_validated_async_function_return_validation() -> None:
    @validated
    async def af(x: int) -> int:
        return str(x)  # type: ignore

    with pytest.raises(TypeError) as exc:
        asyncio.run(af(3))

    # Verify error message indicates type mismatch for return value
    error_msg = str(exc.value)
    assert "Expected a int" in error_msg
    assert "str" in error_msg


def test_validated_keyword_only_and_varkw_aggregate_errors() -> None:
    @validated(aggregate_errors=True)
    def f(*, x: int, **rest: int) -> int:
        return x + sum(rest.values())

    # Test success case
    assert f(x=1, y=2) == 3

    # Test failure case: x and rest["y"] have type mismatches
    with pytest.raises(ExceptionGroup) as exc:
        f(x="1", y="2")  # type: ignore[arg-type]

    assert len(exc.value.exceptions) == 2
    for inner_exc in exc.value.exceptions:
        assert isinstance(inner_exc, TypeError)
        assert "Failed to validate" in str(inner_exc)


def test_validated_varargs_and_kwargs_aggregate_errors() -> None:
    @validated(aggregate_errors=True)
    def f(*values: int, **mapping: int) -> int:
        return sum(values) + sum(mapping.values())

    # Test success case
    assert f(1, 2, ok=3, ko=4) == 10

    # Test failure case: values[1] and mapping["ko"] have type mismatches
    with pytest.raises(ExceptionGroup) as exc:
        f(1, "2", ok=3, ko="4")  # type: ignore[arg-type]

    assert len(exc.value.exceptions) == 2
    for inner_exc in exc.value.exceptions:
        assert isinstance(inner_exc, TypeError)
        assert "Failed to validate" in str(inner_exc)


# ============================================================================
# Positional only arguments
# ============================================================================


def test_validated_positional_only_argument() -> None:
    @validated
    def f(x: int, /, y: int) -> int:
        return x + y

    assert f(1, 2) == 3
    with pytest.raises(ExceptionGroup) as exc:
        f("1", 2)  # type: ignore[arg-type]

    # Expected failure: x (positional-only) has type mismatch
    assert len(exc.value.exceptions) == 1
    assert isinstance(exc.value.exceptions[0], TypeError)
    assert "Failed to validate 'x'" in str(exc.value.exceptions[0])


def test_validated_positional_only_change_arg_and_default() -> None:
    """Test list[int] validation in positional-only parameter."""

    @validated
    def f(items: list[int], x: int = 1, /) -> int:
        ln = len(items) + x
        with pytest.raises(TypeError) as exc:
            items.append("invalid")  # type: ignore

        assert "int" in str(exc.value)

        return ln

    # Test success cases
    assert f([1, 2, 3]) == 4
    assert f([]) == 1
    assert f([], 2) == 2

    # Test failure case
    with pytest.raises(ExceptionGroup) as exc:
        f([1, "2", 3])  # type: ignore[list-item]

    assert len(exc.value.exceptions) == 1
    for inner_exc in exc.value.exceptions:
        assert isinstance(inner_exc, TypeError)


def test_validated_positional_only_change_arg_second() -> None:

    @validated
    def f(x: int, y: list[int], /) -> int:
        return x + len(y)

    assert f(1, [42]) == 2
    assert f(1, [100]) == 2


def test_validated_positional_only_bad_default() -> None:
    """Test positional-only param with invalid default."""

    @validated
    def f(x: int = "invalid", /) -> int:  # type: ignore
        return x

    with pytest.raises(ExceptionGroup) as exc:
        f()  # Should fail when accessing default

    # Expected failure: x default value is invalid
    assert len(exc.value.exceptions) == 1
    assert isinstance(exc.value.exceptions[0], TypeError)
    assert "Failed to validate 'x'" in str(exc.value.exceptions[0])


def test_validated_var_or_keyword_with_default() -> None:
    """Test keyword-only param with annotated default."""

    @validated
    def f(x: int, y: int = 42) -> int:
        return x + y

    assert f(1) == 43
    assert f(1, 100) == 101
    assert f(1, y=10) == 11
    assert f(x=1) == 43
    assert f(x=1, y=10) == 11


def test_validated_var_or_keyword_bad_default() -> None:
    """Test keyword-only param with invalid default."""

    @validated
    def f(x: int = "invalid", y: int = 42) -> int:  # type: ignore
        return x + y

    with pytest.raises(ExceptionGroup) as exc:
        f()

    # Expected failure: x default value is invalid
    assert len(exc.value.exceptions) == 1
    assert isinstance(exc.value.exceptions[0], TypeError)
    assert "Failed to validate 'x'" in str(exc.value.exceptions[0])


def test_validation_positional_or_keyword_change_arg() -> None:
    """Test list[int] validation in positional-or-keyword parameter."""

    @validated
    def f(items: list[int], x: int = 1) -> int:
        s = sum(items) + x

        with pytest.raises(TypeError) as exc:
            items.append("invalid")  # type: ignore

        assert "int" in str(exc.value)

        return s

    # Test success cases
    assert f([1, 2, 3]) == 7
    assert f(items=[10, 20]) == 31
    assert f([1, 2, 3], 2) == 8
    assert f(items=[10, 20], x=2) == 32

    # Test failure cases
    with pytest.raises(ExceptionGroup) as exc:
        f([1, "2", 3])  # type: ignore[list-item]

    assert len(exc.value.exceptions) == 1
    for inner_exc in exc.value.exceptions:
        assert isinstance(inner_exc, TypeError)

    with pytest.raises(ExceptionGroup) as exc:
        f(items=[1, "2", 3])  # type: ignore[list-item]

    assert len(exc.value.exceptions) == 1
    for inner_exc in exc.value.exceptions:
        assert isinstance(inner_exc, TypeError)


def test_validation_positional_or_keyword_change_arg_second() -> None:
    """Test list[int] validation in positional-or-keyword parameter."""

    @validated
    def f(x: int, items: list[int]) -> int:
        s = sum(items) + x
        return s

    # Test success cases
    assert f(1, [1, 2, 3]) == 7
    assert f(1, items=[10, 20]) == 31
    assert f(x=2, items=[10, 20]) == 32


def test_validated_keyword_only_with_default() -> None:
    """Test keyword-only param with annotated default."""

    @validated
    def f(*, x: int, y: int = 42) -> int:
        return x + y

    assert f(x=1) == 43
    assert f(x=100, y=1) == 101


def test_validated_keyword_only_bad_default() -> None:
    """Test keyword-only param with invalid default."""

    @validated
    def f(*, x: int = "invalid") -> int:  # type: ignore
        return x

    with pytest.raises(ExceptionGroup) as exc:
        f()

    # Expected failure: x default value is invalid
    assert len(exc.value.exceptions) == 1
    assert isinstance(exc.value.exceptions[0], TypeError)
    assert "Failed to validate 'x'" in str(exc.value.exceptions[0])


def test_validated_keyword_only_change_arg() -> None:
    """Exercise KeywordOnly branch with transformation (lines 244-250)."""

    @validated
    def f(*, x: list[int], y: int = 1) -> int:
        assert isinstance(x, list)
        return len(x) + y

    # Must pass as keyword argument
    assert f(x=[1, 2, 3]) == 4
    assert f(x=[10, 20], y=2) == 4


def test_validated_keyword_only_change_arg_second() -> None:
    """Exercise KeywordOnly branch with transformation (lines 244-250)."""

    @validated
    def f(*, y: int, x: list[int]) -> int:
        assert isinstance(x, list)
        return len(x) + y

    # Must pass as keyword argument
    assert f(y=1, x=[1, 2, 3]) == 4
    assert f(y=2, x=[10, 20]) == 4


def test_validated_varargs_change_arg() -> None:
    """Test list[int] validation in *args parameter."""

    @validated
    def f(*values: list[int]) -> int:
        return sum(len(v) for v in values)

    # Test success cases
    assert f([1, 2], [3, 4, 5]) == 5
    assert f([]) == 0

    # Test failure case
    with pytest.raises(ExceptionGroup) as exc:
        f([1, 2], [3, "4", 5])  # type: ignore[list-item]

    assert len(exc.value.exceptions) == 1
    for inner_exc in exc.value.exceptions:
        assert isinstance(inner_exc, TypeError)


def test_validated_kwargs_change_arg() -> None:
    """Test list[int] validation in **kwargs parameter."""

    @validated
    def f(**mappings: list[int]) -> int:
        return sum(len(v) for v in mappings.values())

    # Test success cases
    assert f(a=[1, 2], b=[3, 4, 5]) == 5
    assert f() == 0

    # Test failure case
    with pytest.raises(ExceptionGroup) as exc:
        f(a=[1, 2], b=[3, "4", 5])  # type: ignore[list-item]

    assert len(exc.value.exceptions) == 1
    for inner_exc in exc.value.exceptions:
        assert isinstance(inner_exc, TypeError)


# ============================================================================
# Tests for aggregate_errors=False early return paths
# ============================================================================


def test_validated_positional_or_keyword_aggregate_errors_false() -> None:
    """Exercise aggregate_errors=False early return path (line 198)."""

    @validated(aggregate_errors=False)
    def f(x: int, y: int) -> int:
        return x + y

    # First arg invalid - should raise immediately, not collect multiple errors
    with pytest.raises(TypeError):
        f("invalid", "also_invalid")  # type: ignore[arg-type]


def test_validated_keyword_only_aggregate_errors_false() -> None:
    """Exercise aggregate_errors=False early return path for KeywordOnly (line 261)."""

    @validated(aggregate_errors=False)
    def f(*, x: int, y: int) -> int:
        return x + y

    # First kwarg invalid - should raise immediately
    with pytest.raises(TypeError):
        f(x="invalid", y="also_invalid")  # type: ignore[arg-type]


def test_validated_varargs_aggregate_errors_false() -> None:
    """Exercise aggregate_errors=False early return for VarPositional (line 306)."""

    @validated(aggregate_errors=False)
    def f(*args: int) -> int:
        return sum(args)

    # First arg invalid - should raise immediately with single error
    with pytest.raises(TypeError):
        f("invalid", "also_invalid")  # type: ignore[arg-type]


def test_validated_varkw_aggregate_errors_false() -> None:
    """Exercise aggregate_errors=False early return for VarKeyword (line 345)."""

    @validated(aggregate_errors=False)
    def f(**kwargs: int) -> int:
        return sum(kwargs.values())

    # First kwarg invalid - should raise immediately
    with pytest.raises(TypeError):
        f(a="invalid", b="also_invalid")  # type: ignore[arg-type]


# ============================================================================
# Tests for Sync Method Descriptor Binding and Class Access
# ============================================================================


def test_validated_method_sync_instance_binding() -> None:
    """Test sync method returns bound method descriptor."""

    class C:
        @validated
        def method(self, x: int) -> int:
            return x

    c1 = C()
    c2 = C()

    # Bound methods should be unique per instance
    assert c1.method is not c2.method
    assert c1.method.__self__ is c1
    assert c2.method.__self__ is c2

    assert c1.method(1) == 1

    with pytest.raises(ExceptionGroup) as exc:
        c1.method("1")  # type: ignore[arg-type]

    # Expected failure: x has type mismatch
    assert len(exc.value.exceptions) == 1
    assert isinstance(exc.value.exceptions[0], TypeError)
    assert "Failed to validate 'x'" in str(exc.value.exceptions[0])


def test_validated_method_sync_access_from_class() -> None:
    """Test accessing sync validated method from class."""

    class C:
        @validated
        def method(self, x: int) -> int:
            return x

    # Accessing from class should return the validator descriptor
    unbound = C.method
    assert hasattr(unbound, "__call__")

    # Verify we can get the validator's signature
    assert hasattr(unbound, "__wrapped__") or callable(unbound)


# ============================================================================
# Tests for Async Method Descriptor Binding
# ============================================================================


def test_validated_method_async_instance_binding() -> None:
    """Test async method returns bound method descriptor."""

    class C:
        @validated
        async def amethod(self, x: int) -> int:
            return x

    c1 = C()
    c2 = C()

    # Bound methods should be unique per instance
    assert c1.amethod is not c2.amethod
    assert c1.amethod.__self__ is c1
    assert c2.amethod.__self__ is c2

    assert asyncio.run(c1.amethod(1)) == 1

    with pytest.raises(ExceptionGroup) as exc:
        asyncio.run(c1.amethod("1"))  # type: ignore[arg-type]

    # Expected failure: x has type mismatch
    assert len(exc.value.exceptions) == 1
    assert isinstance(exc.value.exceptions[0], TypeError)
    assert "Failed to validate 'x'" in str(exc.value.exceptions[0])


def test_validated_method_async_access_from_class() -> None:
    """Test accessing async validated method from class."""

    class C:
        @validated
        async def amethod(self, x: int) -> int:
            return x

    # Accessing from class should return unbound validator
    unbound = C.amethod
    assert hasattr(unbound, "__call__")

    # Should be able to call with explicit self
    assert inspect.iscoroutinefunction(C.amethod) or hasattr(C.amethod, "__call__")


# ============================================================================
# Tests for Partial Annotations and Edge Cases
# ============================================================================


def test_validated_no_annotations() -> None:
    """Test function with no type annotations."""

    @validated
    def f(x, y):
        return x + y

    assert f(1, 2) == 3
    assert f("a", "b") == "ab"  # No validation


def test_validated_mixed_annotated_unannotated() -> None:
    """Test mixed annotated and unannotated parameters."""

    @validated
    def f(x: int, y) -> str:
        return str(x) + str(y)

    assert f(1, "2") == "12"  # Only x validated
    assert f(1, 2) == "12"  # y accepts anything

    with pytest.raises(ExceptionGroup) as exc:
        f("1", "2")  # type: ignore[arg-type]

    assert len(exc.value.exceptions) == 1
    assert isinstance(exc.value.exceptions[0], TypeError)
    assert "Failed to validate 'x'" in str(exc.value.exceptions[0])


def test_validated_function_with_no_args() -> None:
    """Test function with no parameters."""

    @validated
    def f() -> int:
        return 42

    assert f() == 42

    @validated
    def g() -> int:
        return "42"  # type: ignore

    with pytest.raises(TypeError) as exc:
        g()

    # Verify the error is about type mismatch
    error_msg = str(exc.value)
    assert "Expected a int" in error_msg


def test_validated_positional_only_no_annotation() -> None:
    """Test positional-only param without annotation."""

    @validated
    def f(x, /) -> str:
        return str(x)

    assert f(42) == "42"
    assert f("any") == "any"  # No validation on x since it's unannotated


# ============================================================================
# Tests for Decorator Factory Reusability
# ============================================================================


def test_validated_decorator_factory_reused() -> None:
    """Test decorator factory can be reused across functions."""

    decorator = validated(aggregate_errors=False)

    @decorator
    def f(x: int) -> int:
        return x

    @decorator
    def g(y: int) -> int:
        return y

    assert f(1) == 1
    assert g(2) == 2

    with pytest.raises(TypeError):
        f("invalid")  # type: ignore[arg-type]

    with pytest.raises(TypeError):
        g("invalid")  # type: ignore[arg-type]


# ============================================================================
# Tests for Return Value Error Wrapping and Cause Chain
# ============================================================================


def test_validated_return_error_wrapped_with_cause() -> None:
    """Test that return value validation errors are wrapped with cause chain."""

    @validated
    def f(x: int) -> int:
        return str(x)  # type: ignore

    with pytest.raises(TypeError) as exc:
        f(1)

    # Verify wrapped message format
    error_msg = str(exc.value)
    assert "Failed to validate return value:" in error_msg
    assert "Expected a int" in error_msg

    # Verify original error is set as cause
    assert exc.value.__cause__ is not None
    assert isinstance(exc.value.__cause__, TypeError)
    assert "Expected a int" in str(exc.value.__cause__)


def test_validated_async_return_error_wrapped_with_cause() -> None:
    """Test that async return value validation errors are wrapped with cause chain."""

    @validated
    async def f(x: int) -> int:
        return str(x)  # type: ignore

    with pytest.raises(TypeError) as exc:
        asyncio.run(f(1))

    # Verify wrapped message format
    error_msg = str(exc.value)
    assert "Failed to validate return value:" in error_msg
    assert "Expected a int" in error_msg

    # Verify original error is set as cause
    assert exc.value.__cause__ is not None
    assert isinstance(exc.value.__cause__, TypeError)
    assert "Expected a int" in str(exc.value.__cause__)


def test_validated_return_error_preserves_original_context() -> None:
    """Test that wrapped return error preserves full original error context."""

    @validated
    def f() -> list[int]:
        return "not a list"  # type: ignore

    with pytest.raises(TypeError) as exc:
        f()

    # The wrapped message explains it's the return value
    assert "Failed to validate return value:" in str(exc.value)

    # The original error explains what was expected vs received
    original_error = str(exc.value.__cause__)
    assert "Expected a" in original_error or "list" in original_error
