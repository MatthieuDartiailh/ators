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
        group = case.group if (case.family, case.group) != (previous_family, previous_group) else ""
        table.add_row(family, group, case.implementation, case.benchmark_name)
        previous_family = case.family
        previous_group = case.group

    console.print(table)


def main() -> None:
    from benchmarks.shared.case_registry import select_benchmark_cases
    from benchmarks.shared.pyperf_frontend import run_benchmark_cases

    parser = argparse.ArgumentParser(description="Run grouped benchmarks with pyperf.")
    parser.add_argument("--list", action="store_true", help="List matching benchmark cases and exit.")
    parser.add_argument("--family", action="append", help="Filter by benchmark family.")
    parser.add_argument("--group", action="append", help="Filter by benchmark group or method name.")
    parser.add_argument("--method", action="append", help="Alias for --group.")
    parser.add_argument(
        "--implementation",
        action="append",
        help="Filter by implementation (py, py_typed, ators, atom).",
    )
    args, _unknown = parser.parse_known_args()

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

    _print_case_listing(cases)
    run_benchmark_cases(
        families=args.family,
        groups=args.group or args.method,
        implementations=args.implementation,
    )


if __name__ == "__main__":
    main()
