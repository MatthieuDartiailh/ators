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

Each group is parameterised over 1, 10 and 100 attributes so that fixed and
per-attribute overhead can be distinguished.
"""

import importlib.util
from functools import partial
from typing import Any, Callable

from ators import Ators, member
from benchmarks.shared.registry_types import BenchmarkCase

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))

if ATOM_AVAILABLE:
    from atom.api import Atom, Value

#: Attribute counts used to parameterise each group.
N_ATTRS = (1, 10, 100)

# Pre-computed kwargs dicts for each attribute count, created once at import time.
_NO_VAL_KWARGS: dict[int, dict[str, int]] = {
    n: {f"attr_{i}": 123 for i in range(n)} for n in N_ATTRS
}
_COERCE_KWARGS: dict[int, dict[str, str]] = {
    n: {f"attr_{i}": "123" for i in range(n)} for n in N_ATTRS
}


# ---------------------------------------------------------------------------
# Class factories
# ---------------------------------------------------------------------------


def _make_py_no_validators(n: int) -> type:
    """Return a plain Python class with *n* attributes and no coercion."""

    def __init__(self, **kwargs: Any) -> None:
        self.__dict__.update(kwargs)

    return type(f"PyNoValidators_{n}", (), {"__init__": __init__})


def _make_py_init_coercion(n: int) -> type:
    """Return a plain Python class with *n* attributes, coercing each to ``int``."""

    def __init__(self, **kwargs: Any) -> None:
        for k, v in kwargs.items():
            if not isinstance(v, int):
                v = int(v)
            self.__dict__[k] = v

    return type(f"PyInitCoercion_{n}", (), {"__init__": __init__})


def _make_ators_no_validators(n: int) -> type:
    """Return an Ators class with *n* untyped members and no coercion."""
    attrs: dict[str, Any] = {f"attr_{i}": member() for i in range(n)}
    return type(f"AtorsNoValidators_{n}", (Ators,), attrs)


def _make_ators_init_coercion(n: int) -> type:
    """Return an Ators class with *n* members, coercing each to ``int``."""
    attrs: dict[str, Any] = {f"attr_{i}": member() for i in range(n)}

    def __init__(self, **kwargs: Any) -> None:
        coerced = {
            k: int(v) if not isinstance(v, int) else v for k, v in kwargs.items()
        }
        Ators.__init__(self, **coerced)

    attrs["__init__"] = __init__
    return type(f"AtorsInitCoercion_{n}", (Ators,), attrs)


if ATOM_AVAILABLE:

    def _make_atom_no_validators(n: int) -> type:
        """Return an Atom class with *n* ``Value`` members and no coercion."""
        attrs: dict[str, Any] = {f"attr_{i}": Value() for i in range(n)}
        return type(f"AtomNoValidators_{n}", (Atom,), attrs)

    def _make_atom_init_coercion(n: int) -> type:
        """Return an Atom class with *n* ``Value`` members, coercing each to ``int``."""
        attrs: dict[str, Any] = {f"attr_{i}": Value() for i in range(n)}

        def __init__(self, **kwargs: Any) -> None:
            Atom.__init__(self)
            for k, v in kwargs.items():
                if not isinstance(v, int):
                    v = int(v)
                setattr(self, k, v)

        attrs["__init__"] = __init__
        return type(f"AtomInitCoercion_{n}", (Atom,), attrs)


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
    cases: list[BenchmarkCase] = []
    for n in N_ATTRS:
        no_val_kwargs = _NO_VAL_KWARGS[n]
        coerce_kwargs = _COERCE_KWARGS[n]
        group_no_val = f"no_validators_{n}"
        group_coerce = f"init_coercion_{n}"

        # no_validators: pass plain ints, nothing to coerce
        cases.extend(
            [
                _make_case(
                    group_no_val,
                    "py",
                    partial(_make_py_no_validators, n),
                    lambda cls, kw=no_val_kwargs: lambda: cls(**kw),
                ),
                _make_case(
                    group_no_val,
                    "ators",
                    partial(_make_ators_no_validators, n),
                    lambda cls, kw=no_val_kwargs: lambda: cls(**kw),
                ),
            ]
        )
        if ATOM_AVAILABLE:
            cases.append(
                _make_case(
                    group_no_val,
                    "atom",
                    partial(_make_atom_no_validators, n),
                    lambda cls, kw=no_val_kwargs: lambda: cls(**kw),
                )
            )

        # init_coercion: pass strings so coercion is always triggered
        cases.extend(
            [
                _make_case(
                    group_coerce,
                    "py",
                    partial(_make_py_init_coercion, n),
                    lambda cls, kw=coerce_kwargs: lambda: cls(**kw),
                ),
                _make_case(
                    group_coerce,
                    "ators",
                    partial(_make_ators_init_coercion, n),
                    lambda cls, kw=coerce_kwargs: lambda: cls(**kw),
                ),
            ]
        )
        if ATOM_AVAILABLE:
            cases.append(
                _make_case(
                    group_coerce,
                    "atom",
                    partial(_make_atom_init_coercion, n),
                    lambda cls, kw=coerce_kwargs: lambda: cls(**kw),
                )
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
