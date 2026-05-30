# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmark alternating untyped writes through the shared benchmark registry."""

import pytest

from benchmarks.shared.pytest_frontend import (
    benchmark_case_params,
    run_pytest_benchmark,
)


@pytest.mark.benchmark(group="set_untyped_alternating")
@pytest.mark.parametrize(
    "case",
    benchmark_case_params(families=["set_untyped_alternating"]),
)
def test_benchmark_set_untyped_alternating(benchmark, case):
    run_pytest_benchmark(benchmark, case)
