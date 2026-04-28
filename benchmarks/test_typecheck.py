# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Pytest benchmark wrappers for the ``typecheck`` family."""

import pytest

from benchmarks.shared.pytest_frontend import (
    benchmark_case_params,
    run_pytest_benchmark,
)


@pytest.mark.benchmark(group="issubclass")
@pytest.mark.parametrize(
    "case", benchmark_case_params(families=["typecheck"], groups=["issubclass"])
)
def test_benchmark_issubclass(benchmark, case):
    run_pytest_benchmark(benchmark, case)


@pytest.mark.benchmark(group="isinstance")
@pytest.mark.parametrize(
    "case", benchmark_case_params(families=["typecheck"], groups=["isinstance"])
)
def test_benchmark_isinstance(benchmark, case):
    run_pytest_benchmark(benchmark, case)
