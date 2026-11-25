# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Python-side container types that combine stdlib base classes with Rust validation."""

from collections import OrderedDict

from ._ators import AtorsOrderedDictCore


class AtorsOrderedDict(OrderedDict):
    """A validated ``OrderedDict`` that enforces key/value types on every mutation.

    Instances are produced automatically when an Ators member is annotated with
    ``typing.OrderedDict[K, V]``.  Direct instantiation is not supported; always
    assign an ``OrderedDict`` (or subclass) to the member.

    Validation is delegated to the Rust ``AtorsOrderedDictCore`` held in the
    ``_core`` attribute.  ``move_to_end`` and ``popitem`` are inherited unchanged
    from :class:`collections.OrderedDict`.
    """

    __slots__ = ("_core",)

    # ------------------------------------------------------------------
    # Internal factory — used by the Rust validator, not by end users.
    # ------------------------------------------------------------------

    @classmethod
    def _from_core_and_items(
        cls, core: AtorsOrderedDictCore, items
    ) -> "AtorsOrderedDict":
        """Create an instance with pre-validated *items* and the given *core*.

        ``items`` must be an iterable of ``(key, value)`` pairs where both key
        and value have already been validated by the Rust side.  This method
        bypasses ``__setitem__`` validation intentionally.
        """
        instance = OrderedDict.__new__(cls)
        instance._core = core
        for k, v in items:
            OrderedDict.__setitem__(instance, k, v)
        return instance

    # ------------------------------------------------------------------
    # Prevent direct construction — instances must come from the factory.
    # ------------------------------------------------------------------

    def __init__(self, *args, **kwargs):
        if not hasattr(self, "_core"):
            raise TypeError(
                "AtorsOrderedDict cannot be instantiated directly. "
                "Assign an OrderedDict to an Ators member annotated with "
                "typing.OrderedDict[K, V]."
            )
        # Already initialised by _from_core_and_items; nothing to do.

    # ------------------------------------------------------------------
    # Mutating operations — all delegate validation to self._core.
    # ------------------------------------------------------------------

    def __setitem__(self, key, value):
        valid_key, valid_value = self._core.validate_item(key, value)
        OrderedDict.__setitem__(self, valid_key, valid_value)

    def update(self, other=None, **kwargs):
        """Update the dict, validating every key-value pair."""
        if other is not None:
            if hasattr(other, "keys"):
                for k in other.keys():
                    self[k] = other[k]
            else:
                for k, v in other:
                    self[k] = v
        for k, v in kwargs.items():
            self[k] = v

    def setdefault(self, key, default=None):
        """Return the value for *key*; insert ``default`` if absent (validated)."""
        valid_key = self._core.validate_key(key)
        if valid_key not in self:
            valid_value = self._core.validate_value(default)
            OrderedDict.__setitem__(self, valid_key, valid_value)
        return self[valid_key]

    def __ior__(self, other):
        self.update(other)
        return self

    # move_to_end and popitem are inherited from OrderedDict unchanged.
