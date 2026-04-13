# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Shared init benchmark case registry.

Measures object construction (initialization) cost for:
- ``no_validators``: plain construction with no coercion or validation.
- ``init_coercion``: construction where a value is coerced to ``int`` in
  ``__init__``.  A string ``"123"`` is passed so the coercion path is always
  exercised.
"""

import importlib.util
from typing import Any, Callable

from ators import Ators, member
from benchmarks.shared.registry_types import BenchmarkCase

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))

if ATOM_AVAILABLE:
    from atom.api import Atom, Value


# ---------------------------------------------------------------------------
# Pure-Python baselines
# ---------------------------------------------------------------------------


class PyNoValidators:
    """Plain Python class - stores value as-is, no validation."""

    def __init__(self, value: Any = 0) -> None:
        self.value = value


class PyInitCoercion:
    """Plain Python class - coerces *value* to ``int`` inside ``__init__``."""

    def __init__(self, value: Any = 0) -> None:
        if not isinstance(value, int):
            value = int(value)
        self.value = value


# ---------------------------------------------------------------------------
# Ators implementations
# ---------------------------------------------------------------------------


class AtorsNoValidators(Ators):
    """Ators class with an untyped member - no coercion, no validation."""

    value: Any = member()


class AtorsInitCoercion(Ators):
    """Ators class that coerces *value* to ``int`` in ``__init__``."""

    value: Any = member()

    def __init__(self, value: Any = 0) -> None:
        if not isinstance(value, int):
            value = int(value)
        super().__init__(value=value)


# ---------------------------------------------------------------------------
# Atom implementations (only when atom is installed)
# ---------------------------------------------------------------------------

if ATOM_AVAILABLE:

    class AtomNoValidators(Atom):
        """Atom class with a ``Value`` member - no coercion, no validation."""

        value = Value()

    class AtomInitCoercion(Atom):
        """Atom class that coerces *value* to ``int`` in ``__init__``."""

        value = Value()

        def __init__(self, value: Any = 0) -> None:
            if not isinstance(value, int):
                value = int(value)
            super().__init__()
            self.value = value


# ---------------------------------------------------------------------------
# Case helpers
# ---------------------------------------------------------------------------


def _make_case(
    group: str,
    implementation: str,
    factory: Callable[[], Any],
    op_builder: Callable[[Any], Callable[[], None]],
) -> BenchmarkCase:
    return BenchmarkCase(
        family="init",
        group=group,
        implementation=implementation,
        benchmark_name=f"init.{group}.{implementation}",
        operation_factory=lambda: op_builder(factory()),
    )


def iter_init_cases() -> list[BenchmarkCase]:
    cases: list[BenchmarkCase] = [
        # no_validators - pass a plain int, nothing to coerce
        _make_case(
            "no_validators",
            "py",
            lambda: PyNoValidators,
            lambda cls: lambda: cls(123),
        ),
        _make_case(
            "no_validators",
            "ators",
            lambda: AtorsNoValidators,
            lambda cls: lambda: cls(123),
        ),
        # init_coercion - pass a string so coercion is always triggered
        _make_case(
            "init_coercion",
            "py",
            lambda: PyInitCoercion,
            lambda cls: lambda: cls("123"),
        ),
        _make_case(
            "init_coercion",
            "ators",
            lambda: AtorsInitCoercion,
            lambda cls: lambda: cls("123"),
        ),
    ]
    if ATOM_AVAILABLE:
        cases.extend(
            [
                _make_case(
                    "no_validators",
                    "atom",
                    lambda: AtomNoValidators,
                    lambda cls: lambda: cls(123),
                ),
                _make_case(
                    "init_coercion",
                    "atom",
                    lambda: AtomInitCoercion,
                    lambda cls: lambda: cls("123"),
                ),
            ]
        )
    return cases


def select_init_cases(
    *,
    families: set[str] | None = None,
    groups: set[str] | None = None,
    implementations: set[str] | None = None,
) -> list[BenchmarkCase]:
    cases = iter_init_cases()
    if families is not None:
        cases = [case for case in cases if case.family in families]
    if groups is not None:
        cases = [case for case in cases if case.group in groups]
    if implementations is not None:
        cases = [case for case in cases if case.implementation in implementations]
    return sorted(
        cases, key=lambda case: (case.family, case.group, case.implementation)
    )
