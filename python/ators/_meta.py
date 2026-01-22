# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
""""""

from typing import Any, Mapping, dataclass_transform

from ._ators import Member, create_ators_subclass as _create_ators_subclass, freeze


@dataclass_transform(frozen=False)
class AtorsMeta(type):
    """The metaclass for classes derived from Ators.

    This metaclass computes the memory layout of the members in a given
    class so that the AtorsBase class can allocate exactly enough space for
    the object data slots when it instantiates an object.

    All classes deriving from Ators are automatically slotted, which prevents
    the creation of an instance dictionary and also the ability of an Ators to
    be weakly referenceable.

    Support for weak references can be enabled by passing enable_weakrefs=True
    to the metaclass constructor, instance dictionary and additional slots are
    not supported.

    """

    __ators_members__: Mapping[str, Member]
    __ators_specific_members__: frozenset[str]
    __ators_freeze__: bool

    def __new__(
        meta,
        name: str,
        bases: tuple[type, ...],
        dct: dict[str, Any],
        frozen: bool = False,
        enable_weakrefs: bool = False,
        type_containers: int = -1,
    ):
        # Ensure there is no weird mro calculation and that we can use our
        # re-implementation of C3
        assert meta.mro is type.mro, "Custom MRO calculation are not supported"

        return _create_ators_subclass(
            meta, name, bases, dct, frozen, enable_weakrefs, type_containers
        )

    def __call__(self, *args, **kwds):
        new = super().__call__(*args, **kwds)
        if self.__ators_frozen__:
            freeze(new)
        return new
