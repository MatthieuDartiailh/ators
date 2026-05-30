# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Suite-wide benchmark case selection helpers."""

from collections.abc import Callable, Sequence

from benchmarks.shared.assignment_registry import select_assignment_cases
from benchmarks.shared.container_registry import select_container_cases
from benchmarks.shared.descriptor_registry import select_descriptor_cases
from benchmarks.shared.init_registry import select_init_cases
from benchmarks.shared.registry_types import BenchmarkCase
from benchmarks.shared.typecheck_registry import select_typecheck_cases
from benchmarks.shared.validation_registry import select_validation_cases

CaseSelector = Callable[
    [set[str] | None, set[str] | None, set[str] | None],
    list[BenchmarkCase],
]


def normalize_filter(values: Sequence[str] | None) -> set[str] | None:
    if values is None:
        return None
    normalized = {value for value in values if value}
    return normalized or None


def _registered_selectors() -> list[CaseSelector]:
    return [
        lambda families, groups, implementations: select_assignment_cases(
            families=families,
            groups=groups,
            implementations=implementations,
        ),
        lambda families, groups, implementations: select_descriptor_cases(
            families=families,
            groups=groups,
            implementations=implementations,
        ),
        lambda families, groups, implementations: select_container_cases(
            families=families,
            groups=groups,
            implementations=implementations,
        ),
        lambda families, groups, implementations: select_init_cases(
            families=families,
            groups=groups,
            implementations=implementations,
        ),
        lambda families, groups, implementations: select_validation_cases(
            families=families,
            groups=groups,
            implementations=implementations,
        ),
        lambda families, groups, implementations: select_typecheck_cases(
            families=families,
            groups=groups,
            implementations=implementations,
        ),
    ]


def select_benchmark_cases(
    *,
    families: Sequence[str] | None = None,
    groups: Sequence[str] | None = None,
    implementations: Sequence[str] | None = None,
) -> list[BenchmarkCase]:
    normalized_families = normalize_filter(families)
    normalized_groups = normalize_filter(groups)
    normalized_implementations = normalize_filter(implementations)

    cases: list[BenchmarkCase] = []
    for selector in _registered_selectors():
        cases.extend(
            selector(
                normalized_families,
                normalized_groups,
                normalized_implementations,
            )
        )

    return sorted(
        cases, key=lambda case: (case.family, case.group, case.implementation)
    )
