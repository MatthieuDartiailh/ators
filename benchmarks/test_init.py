import pytest

from benchmarks.shared.pytest_frontend import (
    benchmark_case_params,
    run_pytest_benchmark,
)


@pytest.mark.benchmark(group="init")
@pytest.mark.parametrize("case", benchmark_case_params(families=["init"]))
def test_benchmark_init(benchmark, case):
    run_pytest_benchmark(benchmark, case)
