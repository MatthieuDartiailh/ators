import pytest

from benchmarks.shared.pytest_frontend import (
    benchmark_case_params,
    run_pytest_benchmark,
)


@pytest.mark.benchmark(group="get_descriptor")
@pytest.mark.parametrize("case", benchmark_case_params(families=["get_descriptor"]))
def test_benchmark_get_descriptor(benchmark, case):
    run_pytest_benchmark(benchmark, case)
