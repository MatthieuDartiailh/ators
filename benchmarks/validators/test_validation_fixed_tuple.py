# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for fixed tuple[int, int, str] container field validation."""

import pytest

from benchmarks.validators._shared_validation import (
    run_validation_benchmark,
    validation_case_params,
)


@pytest.mark.benchmark(group="validation_fixed_tuple")
@pytest.mark.parametrize("case", validation_case_params("validation_fixed_tuple"))
def test_benchmark_validation_fixed_tuple(benchmark, case):
    run_validation_benchmark(benchmark, case)
