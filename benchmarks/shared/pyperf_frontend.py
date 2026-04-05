# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Helpers for running shared benchmark cases with pyperf."""

from collections.abc import Sequence

import pyperf

from benchmarks.shared.case_registry import select_benchmark_cases


def run_benchmark_cases(
    *,
    families: Sequence[str] | None = None,
    groups: Sequence[str] | None = None,
    implementations: Sequence[str] | None = None,
) -> None:
    cases = select_benchmark_cases(
        families=families,
        groups=groups,
        implementations=implementations,
    )
    if not cases:
        raise SystemExit("No benchmark cases matched the requested filters.")

    runner = pyperf.Runner()
    for case in cases:
        runner.bench_func(case.benchmark_name, case.operation_factory())


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
