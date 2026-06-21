# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Shared callable validation benchmark case registry."""

from collections.abc import Callable

from ators import validated
from benchmarks.shared.registry_types import BenchmarkCase


def _make_case(
    family: str,
    implementation: str,
    operation_factory: Callable[[], Callable[[], None]],
) -> BenchmarkCase:
    return BenchmarkCase(
        family=family,
        group="call",
        implementation=implementation,
        benchmark_name=f"call_validation.{family}.{implementation}",
        operation_factory=operation_factory,
    )


# ============================================================================
# Simple: single positional-or-keyword argument
# ============================================================================


def _simple_py() -> Callable[[], None]:
    def f(x: int) -> int:
        return x + 1

    def op() -> None:
        f(42)

    return op


def _simple_validated() -> Callable[[], None]:
    @validated
    def f(x: int) -> int:
        return x + 1

    def op() -> None:
        f(42)

    return op


def _simple_manual() -> Callable[[], None]:
    def f(x: int) -> int:
        if not isinstance(x, int):
            raise TypeError(f"expected int, got {type(x).__name__}")
        return x + 1

    def op() -> None:
        f(42)

    return op


# ============================================================================
# Positional-only + positional-or-keyword
# ============================================================================


def _positional_only_py() -> Callable[[], None]:
    def f(x: int, /, y: int) -> int:
        return x + y

    def op() -> None:
        f(42, 43)

    return op


def _positional_only_validated() -> Callable[[], None]:
    @validated
    def f(x: int, /, y: int) -> int:
        return x + y

    def op() -> None:
        f(42, 43)

    return op


def _positional_only_manual() -> Callable[[], None]:
    def f(x: int, /, y: int) -> int:
        if not isinstance(x, int):
            raise TypeError(f"expected int for x, got {type(x).__name__}")
        if not isinstance(y, int):
            raise TypeError(f"expected int for y, got {type(y).__name__}")
        return x + y

    def op() -> None:
        f(42, 43)

    return op


# ============================================================================
# Keyword-only
# ============================================================================


def _keyword_only_py() -> Callable[[], None]:
    def f(x: int, *, y: int) -> int:
        return x + y

    def op() -> None:
        f(42, y=43)

    return op


def _keyword_only_validated() -> Callable[[], None]:
    @validated
    def f(x: int, *, y: int) -> int:
        return x + y

    def op() -> None:
        f(42, y=43)

    return op


def _keyword_only_manual() -> Callable[[], None]:
    def f(x: int, *, y: int) -> int:
        if not isinstance(x, int):
            raise TypeError(f"expected int for x, got {type(x).__name__}")
        if not isinstance(y, int):
            raise TypeError(f"expected int for y, got {type(y).__name__}")
        return x + y

    def op() -> None:
        f(42, y=43)

    return op


# ============================================================================
# Varargs (*args)
# ============================================================================


def _varargs_py() -> Callable[[], None]:
    def f(*values: int) -> int:
        return sum(values)

    def op() -> None:
        f(1, 2, 3, 4, 5)

    return op


def _varargs_validated() -> Callable[[], None]:
    @validated
    def f(*values: int) -> int:
        return sum(values)

    def op() -> None:
        f(1, 2, 3, 4, 5)

    return op


def _varargs_manual() -> Callable[[], None]:
    def f(*values: int) -> int:
        for v in values:
            if not isinstance(v, int):
                raise TypeError(f"expected int, got {type(v).__name__}")
        return sum(values)

    def op() -> None:
        f(1, 2, 3, 4, 5)

    return op


# ============================================================================
# Keyword arguments (**kwargs)
# ============================================================================


def _kwargs_py() -> Callable[[], None]:
    def f(**mapping: int) -> int:
        return sum(mapping.values())

    def op() -> None:
        f(a=1, b=2, c=3, d=4, e=5)

    return op


def _kwargs_validated() -> Callable[[], None]:
    @validated
    def f(**mapping: int) -> int:
        return sum(mapping.values())

    def op() -> None:
        f(a=1, b=2, c=3, d=4, e=5)

    return op


def _kwargs_manual() -> Callable[[], None]:
    def f(**mapping: int) -> int:
        for v in mapping.values():
            if not isinstance(v, int):
                raise TypeError(f"expected int, got {type(v).__name__}")
        return sum(mapping.values())

    def op() -> None:
        f(a=1, b=2, c=3, d=4, e=5)

    return op


# ============================================================================
# Mixed: all parameter types
# ============================================================================


def _mixed_py() -> Callable[[], None]:
    def f(x: int, /, y: int, *, z: int) -> int:
        return x + y + z

    def op() -> None:
        f(1, 2, z=3)

    return op


def _mixed_validated() -> Callable[[], None]:
    @validated
    def f(x: int, /, y: int, *, z: int) -> int:
        return x + y + z

    def op() -> None:
        f(1, 2, z=3)

    return op


def _mixed_manual() -> Callable[[], None]:
    def f(x: int, /, y: int, *, z: int) -> int:
        if not isinstance(x, int):
            raise TypeError(f"expected int for x, got {type(x).__name__}")
        if not isinstance(y, int):
            raise TypeError(f"expected int for y, got {type(y).__name__}")
        if not isinstance(z, int):
            raise TypeError(f"expected int for z, got {type(z).__name__}")
        return x + y + z

    def op() -> None:
        f(1, 2, z=3)

    return op


# ============================================================================
# Case Registry
# ============================================================================

CALLABLE_VALIDATION_SPECS: tuple[tuple[str, tuple[str, str, str]], ...] = (
    (
        "call_validation_simple",
        (
            "_simple_py",
            "_simple_validated",
            "_simple_manual",
        ),
    ),
    (
        "call_validation_positional_only",
        (
            "_positional_only_py",
            "_positional_only_validated",
            "_positional_only_manual",
        ),
    ),
    (
        "call_validation_keyword_only",
        (
            "_keyword_only_py",
            "_keyword_only_validated",
            "_keyword_only_manual",
        ),
    ),
    (
        "call_validation_varargs",
        (
            "_varargs_py",
            "_varargs_validated",
            "_varargs_manual",
        ),
    ),
    (
        "call_validation_kwargs",
        (
            "_kwargs_py",
            "_kwargs_validated",
            "_kwargs_manual",
        ),
    ),
    (
        "call_validation_mixed",
        (
            "_mixed_py",
            "_mixed_validated",
            "_mixed_manual",
        ),
    ),
)

CALLABLE_VALIDATION_FAMILIES = tuple(spec[0] for spec in CALLABLE_VALIDATION_SPECS)


def iter_callable_validation_cases() -> list[BenchmarkCase]:
    cases: list[BenchmarkCase] = []
    module_globals = globals()
    for family, (py_name, validated_name, manual_name) in CALLABLE_VALIDATION_SPECS:
        cases.append(
            _make_case(
                family,
                "py",
                module_globals[py_name],
            )
        )
        cases.append(
            _make_case(
                family,
                "validated",
                module_globals[validated_name],
            )
        )
        cases.append(
            _make_case(
                family,
                "manual",
                module_globals[manual_name],
            )
        )
    return cases


def select_callable_validation_cases(
    *,
    families: set[str] | None = None,
    groups: set[str] | None = None,
    implementations: set[str] | None = None,
) -> list[BenchmarkCase]:
    cases = iter_callable_validation_cases()
    if families is not None:
        cases = [case for case in cases if case.family in families]
    if groups is not None:
        cases = [case for case in cases if case.group in groups]
    if implementations is not None:
        cases = [case for case in cases if case.implementation in implementations]
    return sorted(
        cases, key=lambda case: (case.family, case.group, case.implementation)
    )
