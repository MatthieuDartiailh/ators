import pytest

from benchmarks.shared.pytest_frontend import (
    benchmark_case_params,
    run_pytest_benchmark,
)


@pytest.mark.benchmark(group="call_validation")
@pytest.mark.parametrize(
    "case",
    benchmark_case_params(
        families=[
            "call_validation_simple",
            "call_validation_positional_only",
            "call_validation_keyword_only",
            "call_validation_varargs",
            "call_validation_kwargs",
            "call_validation_mixed",
        ]
    ),
)
def test_benchmark_callable_validation(benchmark, case):
    run_pytest_benchmark(benchmark, case)
