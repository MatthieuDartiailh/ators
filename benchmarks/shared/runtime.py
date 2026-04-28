# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Runtime helpers for benchmark case selection."""

import importlib.util
import sys


def free_threaded_python() -> bool:
    """Return True if the current Python environment is free-threaded (GIL disabled)."""
    # sys._is_gil_enabled() is available on Python 3.12+ (returns False on 3.14t)
    return not sys._is_gil_enabled()


def atom_benchmarks_available() -> bool:
    """Return True if atom benchmarks can be run in the current Python environment.

    atom does not support free-threaded Python (3.14t), so this returns False
    when the GIL is disabled.

    """
    return not free_threaded_python() and bool(importlib.util.find_spec("atom"))
