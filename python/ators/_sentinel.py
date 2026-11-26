# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Compatibility definition for the value used to represent an unset member."""

import sys
from enum import Enum

if sys.version_info >= (3, 15):
    import builtins

    UNSET = getattr(builtins, "sentinel")("UNSET")
else:

    class _Unset(Enum):
        UNSET = "UNSET"

        def __repr__(self) -> str:
            return "UNSET"

        def __str__(self) -> str:
            return "UNSET"

    UNSET = _Unset.UNSET
