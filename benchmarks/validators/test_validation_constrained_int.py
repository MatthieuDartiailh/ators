# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmarks for constrained int field validation."""

import pytest

from benchmarks.validators._shared_validation import (
    run_validation_benchmark,
    validation_case_params,
)


@pytest.mark.benchmark(group="validation_constrained_int")
@pytest.mark.parametrize("case", validation_case_params("validation_constrained_int"))
def test_benchmark_validation_constrained_int(benchmark, case):
    run_validation_benchmark(benchmark, case)
