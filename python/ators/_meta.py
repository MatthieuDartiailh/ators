# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Matthieu C. Dartiailh
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
""""""

from itertools import chain
from typing import Any, Mapping, dataclass_transform

from ._ators import Member


@dataclass_transform(frozen=False)
class AtorsMeta(type):
    """The metaclass for classes derived from Ators.

    This metaclass computes the memory layout of the members in a given
    class so that the BaseAtors class can allocate exactly enough space for
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

        # Ators subclasses do not support slots (beyond support for weakrefs
        # through the enable_weakrefs metaclass argument)
        if "__slots__" in dct:
            raise TypeError("__slots__ not supported in Ators subclasses")

        dct["__slots__"] = ()
        # Add support for weakrefs if requested and no base class already
        # supports them
        if enable_weakrefs and not any(
            "__weakref__" in b.__slots__
            for b in chain.from_iterable(b.mro() for b in bases)
        ):
            dct["__slots__"] += ("__weakref__",)

        generate_members_from_cls_namespace(name, dct, type_containers)

        # Create the helper used to analyze the namespace and customize members
        helper = _AtomMetaHelper(name, bases, dct)

        # Analyze and clean the namespace
        helper.scan_and_clear_namespace()

        # Assign each member a unique ID
        helper.assign_members_indexes()

        # Customize the members based on the specified static modifiers
        helper.apply_members_static_behaviors()

        cls = helper.create_class(meta)
        cls.__ators_freeze__ = frozen

    def __call__(self, *args, **kwds):
        return super().__call__(*args, **kwds)
