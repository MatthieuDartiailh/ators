# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
""""""

from typing import Any, dataclass_transform

from ._ators import (
    PicklePolicy,
    create_ators_specialized_subclass as _create_ators_specialized_subclass,
    create_ators_subclass as _create_ators_subclass,
    drop_class_info as _drop_class_info,
    get_ators_args as _get_ators_args,
    get_ators_frozen_flag as _get_ators_frozen_flag,
    get_ators_origin as _get_origin,
    maybe_freeze_instance_after_call as _maybe_freeze_instance_after_call,
    rust_instancecheck as _rust_instancecheck,
    rust_subclasscheck as _rust_subclasscheck,
)


@dataclass_transform(
    field_descriptors=("member",), kw_only_default=True, frozen_default=False
)
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

    __ators_frozen__: bool
    __origin__: type | None
    __args__: tuple[type, ...] | None

    def __new__(
        meta,
        name: str,
        bases: tuple[type, ...],
        dct: dict[str, Any],
        frozen: bool = False,
        observable: bool = False,
        enable_weakrefs: bool = False,
        type_containers: int = -1,
        pickle_policy: PicklePolicy | None = None,
        validate_attr: bool = True,
    ):
        # Ensure there is no weird mro calculation and that we can use our
        # re-implementation of C3
        assert meta.mro is type.mro, "Custom MRO calculation are not supported"

        return _create_ators_subclass(
            meta,
            name,
            bases,
            dct,
            frozen,
            observable,
            enable_weakrefs,
            type_containers,
            pickle_policy,
            validate_attr,
        )

    def __call__(self, *args, **kwds):
        return _maybe_freeze_instance_after_call(super().__call__(*args, **kwds))

    @property
    def __ators_frozen__(cls) -> bool:
        return _get_ators_frozen_flag(cls)

    @property
    def __origin__(cls) -> type | None:
        return _get_origin(cls)

    @property
    def __args__(cls) -> tuple[type, ...] | None:
        return _get_ators_args(cls)

    def __getitem__(self, params):
        return _create_ators_specialized_subclass(self, params)

    def __subclasscheck__(cls, sub):  # type: ignore[override]
        return _rust_subclasscheck(cls, sub)

    def __instancecheck__(cls, instance):  # type: ignore[override]
        return _rust_instancecheck(cls, instance)

    def __del__(cls):
        _drop_class_info(cls)
