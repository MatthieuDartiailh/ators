import pytest

from benchmarks.shared.pytest_frontend import benchmark_case_params, run_pytest_benchmark


@pytest.mark.benchmark(group="get_untyped")
@pytest.mark.parametrize("case", benchmark_case_params(families=["get_untyped"]))
def test_benchmark_get_untyped(benchmark, case):
    run_pytest_benchmark(benchmark, case)
