# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Shared pytest adapters for validation benchmark families."""

from benchmarks.shared.pytest_frontend import (
    benchmark_case_params,
    run_pytest_benchmark,
)


def validation_case_params(family: str) -> list[object]:
    return benchmark_case_params(families=[family])


def run_validation_benchmark(benchmark: object, case: object) -> None:
    run_pytest_benchmark(benchmark, case)
