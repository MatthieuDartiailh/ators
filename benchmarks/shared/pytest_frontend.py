# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Helpers for exposing shared benchmark cases through pytest benchmark tests."""

from collections.abc import Sequence

import pytest

from benchmarks.shared.case_registry import select_benchmark_cases
from benchmarks.shared.registry_types import BenchmarkCase


def _case_param_id(case: BenchmarkCase, *, include_group: bool) -> str:
    if include_group:
        return f"{case.group}-{case.implementation}"
    return case.implementation


def benchmark_case_params(
    *,
    families: Sequence[str] | None = None,
    groups: Sequence[str] | None = None,
    implementations: Sequence[str] | None = None,
) -> list[object]:
    cases = select_benchmark_cases(
        families=families,
        groups=groups,
        implementations=implementations,
    )
    include_group = len({case.group for case in cases}) > 1
    return [
        pytest.param(case, id=_case_param_id(case, include_group=include_group))
        for case in cases
    ]


def run_pytest_benchmark(benchmark: object, case: BenchmarkCase) -> None:
    benchmark(case.operation_factory())
