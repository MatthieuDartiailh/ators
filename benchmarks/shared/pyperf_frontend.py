# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Helpers for running shared benchmark cases with pyperf."""

from collections.abc import Callable, Sequence
from contextlib import ExitStack, redirect_stderr, redirect_stdout
from io import StringIO

import pyperf

from benchmarks.shared.case_registry import select_benchmark_cases
from benchmarks.shared.registry_types import BenchmarkCase

BenchmarkResult = tuple[BenchmarkCase, pyperf.Benchmark]


OnBenchmarkStart = Callable[[int, int, BenchmarkCase], None]


def run_benchmark_cases(
    *,
    families: Sequence[str] | None = None,
    groups: Sequence[str] | None = None,
    implementations: Sequence[str] | None = None,
    program_args: Sequence[str] | None = None,
    suppress_pyperf_output: bool = False,
    on_benchmark_start: OnBenchmarkStart | None = None,
) -> list[BenchmarkResult]:
    cases = select_benchmark_cases(
        families=families,
        groups=groups,
        implementations=implementations,
    )
    if not cases:
        raise SystemExit("No benchmark cases matched the requested filters.")

    runner = pyperf.Runner(program_args=tuple(program_args) if program_args else None)
    results: list[BenchmarkResult] = []
    total = len(cases)
    for index, case in enumerate(cases):
        if on_benchmark_start is not None:
            on_benchmark_start(index, total, case)
        with ExitStack() as stack:
            if suppress_pyperf_output:
                stack.enter_context(redirect_stdout(StringIO()))
                stack.enter_context(redirect_stderr(StringIO()))
            benchmark = runner.bench_func(case.benchmark_name, case.operation_factory())

        if benchmark is not None:
            results.append((case, benchmark))

    return results


def describe_benchmark_cases(
    *,
    families: Sequence[str] | None = None,
    groups: Sequence[str] | None = None,
    implementations: Sequence[str] | None = None,
) -> str:
    cases = select_benchmark_cases(
        families=families,
        groups=groups,
        implementations=implementations,
    )
    if not cases:
        return "No matching benchmark cases."

    lines: list[str] = []
    current_family = ""
    current_group = ""
    for case in cases:
        if case.family != current_family:
            current_family = case.family
            current_group = ""
            lines.append(f"[{case.family}]")
        if case.group != current_group:
            current_group = case.group
            lines.append(f"  {case.group}")
        lines.append(f"    - {case.implementation}: {case.benchmark_name}")
    return "\n".join(lines)


def run_container_cases(
    *,
    families: Sequence[str] | None = None,
    methods: Sequence[str] | None = None,
    implementations: Sequence[str] | None = None,
) -> None:
    run_benchmark_cases(
        families=families,
        groups=methods,
        implementations=implementations,
    )


def describe_container_cases(
    *,
    families: Sequence[str] | None = None,
    methods: Sequence[str] | None = None,
    implementations: Sequence[str] | None = None,
) -> str:
    return describe_benchmark_cases(
        families=families,
        groups=methods,
        implementations=implementations,
    )
