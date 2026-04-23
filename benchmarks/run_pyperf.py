# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Run grouped benchmark families with pyperf.

This shared pyperf frontend can list available benchmark cases or run selected
families, groups, and implementations across descriptor and container slices.

Examples:
    python benchmarks/run_pyperf.py --list
    python benchmarks/run_pyperf.py --family list
    python benchmarks/run_pyperf.py --family get_untyped
    python benchmarks/run_pyperf.py --family dict --group update_dict --implementation ators
"""

import argparse
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))


def _try_make_console():
    try:
        from rich.console import Console

        return Console()
    except ImportError:
        return None


def _print_case_listing(cases) -> None:
    console = _try_make_console()
    if console is None:
        current_family = None
        current_group = None
        for case in cases:
            if case.family != current_family:
                current_family = case.family
                current_group = None
                print(f"[{case.family}]")
            if case.group != current_group:
                current_group = case.group
                print(f"  {case.group}")
            print(f"    - {case.implementation}: {case.benchmark_name}")
        return

    from rich.panel import Panel
    from rich.table import Table
    from rich.text import Text

    family_count = len({case.family for case in cases})
    group_count = len({(case.family, case.group) for case in cases})
    implementation_count = len({case.implementation for case in cases})

    summary = Text()
    summary.append(f"{len(cases)} cases", style="bold")
    summary.append(f" across {family_count} families, {group_count} groups, and ")
    summary.append(f"{implementation_count} implementations", style="bold")
    console.print(Panel(summary, title="Matched Benchmarks", expand=False))

    table = Table(show_header=True, header_style="bold cyan")
    table.add_column("Family", style="green")
    table.add_column("Group", style="magenta")
    table.add_column("Impl", style="yellow")
    table.add_column("Benchmark")

    previous_family = None
    previous_group = None
    for case in cases:
        family = case.family if case.family != previous_family else ""
        group = (
            case.group
            if (case.family, case.group) != (previous_family, previous_group)
            else ""
        )
        table.add_row(family, group, case.implementation, case.benchmark_name)
        previous_family = case.family
        previous_group = case.group

    console.print(table)


def _build_pyperf_args(pyperf_args: list[str]) -> list[str]:
    """Build pyperf arguments while keeping pyperf's own output quiet."""
    forwarded_args = list(pyperf_args)
    if "--quiet" not in forwarded_args and "-q" not in forwarded_args:
        forwarded_args.append("--quiet")
    return forwarded_args


def _make_progress_callback(total: int, console):
    """Return a callback that prints a progress line before each benchmark."""
    if console is None:

        def _plain(index: int, _total: int, case) -> None:
            print(f"[{index + 1}/{total}] Running: {case.benchmark_name}")

        return _plain

    from rich.text import Text

    def _rich(index: int, _total: int, case) -> None:
        line = Text()
        line.append(f"[{index + 1}/{total}]", style="bold cyan")
        line.append(" Running: ")
        line.append(case.benchmark_name, style="bold")
        console.print(line)

    return _rich


def _print_benchmark_results(results) -> None:
    if not results:
        return

    console = _try_make_console()
    if console is None:
        for case, benchmark in results:
            mean = benchmark.format_value(benchmark.mean())
            median = benchmark.format_value(benchmark.median())
            stdev = benchmark.format_value(benchmark.stdev())
            spread = (
                (benchmark.stdev() / benchmark.mean() * 100.0)
                if benchmark.mean()
                else 0.0
            )
            print(
                f"{case.benchmark_name}: mean {mean}, median {median}, "
                f"stdev {stdev}, spread {spread:.2f}%, samples {benchmark.get_nvalue()}"
            )
        return

    from rich.panel import Panel
    from rich.table import Table
    from rich.text import Text

    means = [benchmark.mean() for _, benchmark in results]
    fastest = min(means)
    slowest = max(means)

    summary = Text()
    summary.append(f"{len(results)} completed", style="bold")
    summary.append(" benchmark cases. Fastest mean: ")
    summary.append(
        results[means.index(fastest)][1].format_value(fastest), style="bold green"
    )
    summary.append(". Slowest mean: ")
    summary.append(
        results[means.index(slowest)][1].format_value(slowest), style="bold yellow"
    )
    console.print(Panel(summary, title="Benchmark Results", expand=False))

    table = Table(show_header=True, header_style="bold cyan")
    table.add_column("Family", style="green")
    table.add_column("Group", style="magenta")
    table.add_column("Impl", style="yellow")
    table.add_column("Mean", justify="right")
    table.add_column("Median", justify="right")
    table.add_column("Stdev", justify="right")
    table.add_column("Spread", justify="right")
    table.add_column("Samples", justify="right")

    previous_family = None
    previous_group = None
    for case, benchmark in results:
        family = case.family if case.family != previous_family else ""
        group = (
            case.group
            if (case.family, case.group) != (previous_family, previous_group)
            else ""
        )
        mean = benchmark.mean()
        stdev = benchmark.stdev()
        spread = (stdev / mean * 100.0) if mean else 0.0
        table.add_row(
            family,
            group,
            case.implementation,
            benchmark.format_value(mean),
            benchmark.format_value(benchmark.median()),
            benchmark.format_value(stdev),
            f"{spread:.2f}%",
            str(benchmark.get_nvalue()),
        )
        previous_family = case.family
        previous_group = case.group

    console.print(table)


def _forward_only_pyperf_args(pyperf_args: list[str]) -> None:
    """Keep only script name and pyperf-specific arguments in sys.argv."""
    sys.argv = [sys.argv[0], *pyperf_args]


def _is_pyperf_worker_process(pyperf_args: list[str]) -> bool:
    """Return True when invoked as a pyperf worker subprocess."""
    return any(arg == "--worker" for arg in pyperf_args)


def _build_script_program_args(args) -> list[str]:
    """Build script CLI arguments to replay in pyperf worker subprocesses."""
    program_args = [sys.argv[0]]
    for family in args.family or ():
        program_args.extend(["--family", family])
    for group in args.group or ():
        program_args.extend(["--group", group])
    for method in args.method or ():
        program_args.extend(["--method", method])
    for implementation in args.implementation or ():
        program_args.extend(["--implementation", implementation])
    return program_args


def main() -> None:
    from benchmarks.shared.case_registry import select_benchmark_cases
    from benchmarks.shared.pyperf_frontend import run_benchmark_cases

    parser = argparse.ArgumentParser(description="Run grouped benchmarks with pyperf.")
    parser.add_argument(
        "--list", action="store_true", help="List matching benchmark cases and exit."
    )
    parser.add_argument("--family", action="append", help="Filter by benchmark family.")
    parser.add_argument(
        "--group", action="append", help="Filter by benchmark group or method name."
    )
    parser.add_argument("--method", action="append", help="Alias for --group.")
    parser.add_argument(
        "--implementation",
        action="append",
        help="Filter by implementation (py, py_typed, ators, atom).",
    )
    args, unknown = parser.parse_known_args()
    pyperf_args = _build_pyperf_args(unknown)
    program_args = _build_script_program_args(args)

    cases = select_benchmark_cases(
        families=args.family,
        groups=args.group or args.method,
        implementations=args.implementation,
    )
    if not cases:
        raise SystemExit("No benchmark cases matched the requested filters.")

    if args.list:
        _print_case_listing(cases)
        return

    is_worker = _is_pyperf_worker_process(pyperf_args)
    if not is_worker:
        _print_case_listing(cases)
    _forward_only_pyperf_args(pyperf_args)
    console = None if is_worker else _try_make_console()
    results = run_benchmark_cases(
        families=args.family,
        groups=args.group or args.method,
        implementations=args.implementation,
        program_args=program_args,
        suppress_pyperf_output=True,
        on_benchmark_start=None
        if is_worker
        else _make_progress_callback(len(cases), console),
    )
    if not is_worker:
        _print_benchmark_results(results)


if __name__ == "__main__":
    main()
