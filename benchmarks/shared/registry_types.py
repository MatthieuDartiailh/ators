# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Shared benchmark case types."""

from dataclasses import dataclass
from typing import Callable


@dataclass(frozen=True)
class BenchmarkCase:
    """Single benchmark case shared across pyperf and future pytest frontends."""

    family: str
    group: str
    implementation: str
    benchmark_name: str
    operation_factory: Callable[[], Callable[[], None]]