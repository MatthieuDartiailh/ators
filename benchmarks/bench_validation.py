# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Run shared validation benchmark families with pyperf."""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))


if __name__ == "__main__":
    from benchmarks.shared.pyperf_frontend import run_benchmark_cases
    from benchmarks.shared.validation_registry import VALIDATION_FAMILIES

    run_benchmark_cases(families=VALIDATION_FAMILIES)
