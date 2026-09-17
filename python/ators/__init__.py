# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Public Python API for Ators.

This module re-exports the core Rust extension types/functions and provides
the user-facing `Ators` base class built on top of `AtorsMeta`.
"""

import inspect
from typing import (
    Protocol,
    TypeVar,
    get_args,
    get_origin,
    runtime_checkable as _typing_runtime_checkable,
)

from ._ators import (
    AtorsBase as _Base,
    AtorsChange,
    AtorsRef,
    Event,
    EventCustomizationTool,
    Member,
    PicklePolicy,
    add_generic_type_attributes,
    atorsref,
    disable_notifications,
    enable_notifications,
    event,
    explain_callable_mismatch as _explain_callable_mismatch,
    freeze,
    get_event,
    get_event_customization_tool,
    get_events,
    get_events_by_tag,
    get_events_by_tag_and_value,
    get_member,
    get_member_customization_tool,
    get_members,
    get_members_by_tag,
    get_members_by_tag_and_value,
    is_frozen,
    is_notifications_enabled,
    is_runtime_callable_compatible as _is_runtime_callable_compatible,
    member,
    observe,
    register_type_mutability_info,
    unobserve,
    validated,
)
from ._meta import AtorsMeta as _Meta

# Register generic type attributes for numpy ndarray if numpy is available
try:
    import numpy as np

    add_generic_type_attributes(np.ndarray, ("shape", "dtype"))
    register_type_mutability_info(np.ndarray, lambda obj: obj.flags.writeable)
except ImportError:
    pass

# Register generic type attributes for pint Quantity if pint is available
try:
    from pint import Quantity

    add_generic_type_attributes(Quantity, ("_magnitude",))
except ImportError:
    pass


def __newobj__(cls, *args):
    """A compatibility pickler function.

    This function is not part of the public Atom api.

    """
    return cls.__new__(cls, *args)


class Ators(_Base, metaclass=_Meta):
    """Base class for Ators models.

    Subclasses declare members in the class body and receive validated,
    slotted storage plus optional freezing and observation support.
    """

    def __reduce_ex__(self, proto):
        """An implementation of the reduce protocol.

        This method creates a reduction tuple for Atom instances. This
        method should not be overridden by subclasses unless the author
        fully understands the rammifications.

        """
        args = (type(self), *self.__getnewargs__())
        return (__newobj__, args, self.__getstate__())

    def __getnewargs__(self) -> tuple:
        """Get the argument tuple to pass to __new__ on unpickling.

        See the Python.org docs for more information.

        """
        return ()


def runtime_checkable(protocol):
    """Compatibility wrapper for stdlib runtime_checkable.

    It sets the standard runtime-protocol marker and keeps the class usable with
    our call_checkable decorator when both are stacked.
    """
    if not isinstance(protocol, type) or not issubclass(protocol, Protocol):
        raise TypeError("runtime_checkable can only be applied to Protocol subclasses")

    protocol = _typing_runtime_checkable(protocol)
    protocol._is_runtime_protocol = True
    protocol.__call_checkable__ = getattr(protocol, "__call_checkable__", False)
    return protocol


def _replace_typevars(annotation, bindings):
    """Return a copy of annotation with the bound TypeVars resolved."""
    if annotation is None:
        return annotation
    if annotation in bindings:
        return bindings[annotation]

    origin = get_origin(annotation)
    if origin is None:
        return annotation

    args = get_args(annotation)
    if not args:
        return annotation

    replaced_args = tuple(_replace_typevars(arg, bindings) for arg in args)
    if hasattr(annotation, "copy_with"):
        try:
            return annotation.copy_with(replaced_args)
        except TypeError:
            pass

    try:
        return origin[replaced_args]
    except TypeError:
        try:
            if len(replaced_args) == 1:
                return origin[replaced_args[0]]
            return origin[tuple(replaced_args)]
        except Exception:
            return annotation


def _specialize_protocol_call(protocol, typevar_bindings):
    """Create a specialized __call__ wrapper that applies the narrowed TypeVars."""
    original_call = getattr(protocol, "__call__")
    original_signature = inspect.signature(original_call)

    new_parameters = []
    for parameter in original_signature.parameters.values():
        if parameter.name in {"self", "cls"}:
            new_parameters.append(parameter)
            continue
        new_annotation = _replace_typevars(parameter.annotation, typevar_bindings)
        new_parameters.append(parameter.replace(annotation=new_annotation))

    new_return = _replace_typevars(
        original_signature.return_annotation, typevar_bindings
    )
    new_signature = original_signature.replace(
        parameters=new_parameters,
        return_annotation=new_return,
    )

    def specialized_call(self, *args, **kwargs):
        return original_call(self, *args, **kwargs)

    specialized_call.__annotations__ = {
        parameter.name: parameter.annotation
        for parameter in new_signature.parameters.values()
        if parameter.name not in {"self", "cls"}
    }
    specialized_call.__annotations__["return"] = new_return
    specialized_call.__signature__ = new_signature
    specialized_call.__wrapped__ = original_call
    return specialized_call


def _make_call_checkable_metaclass(base):
    """Create a protocol-specific metaclass that delegates to the Rust checker."""

    class _CallCheckableMeta(base):
        def __instancecheck__(cls, instance):
            if not getattr(cls, "__call_checkable__", False):
                return super().__instancecheck__(instance)
            return _is_runtime_callable_compatible(instance, cls)

        def __subclasscheck__(cls, subclass):
            if not getattr(cls, "__call_checkable__", False):
                return super().__subclasscheck__(subclass)
            if not isinstance(subclass, type):
                return False
            return _is_runtime_callable_compatible(subclass, cls)

        def __getitem__(cls, params):
            if not getattr(cls, "__call_checkable__", False):
                return super().__getitem__(params)

            parameters = tuple(getattr(cls, "__parameters__", ()))
            if not parameters:
                return super().__getitem__(params)

            if not isinstance(params, tuple):
                params = (params,)

            if len(params) != len(parameters):
                return super().__getitem__(params)

            typevar_bindings = dict(zip(parameters, params))
            parameter_names = ", ".join(
                getattr(param, "__name__", str(param)) for param in params
            )
            name = f"{cls.__name__}[{parameter_names}]"
            namespace = {
                "__module__": cls.__module__,
                "__qualname__": f"{getattr(cls, '__qualname__', cls.__name__)}[{parameter_names}]",
                "__orig_bases__": getattr(cls, "__orig_bases__", ()),
                "__parameters__": (),
                "__call__": _specialize_protocol_call(cls, typevar_bindings),
                "__call_checkable__": True,
                "_is_runtime_protocol": True,
                "__typevar_bindings__": typevar_bindings,
                "__origin__": cls,
                "__args__": tuple(params),
            }
            specialized = type(cls)(name, (cls,), namespace)
            specialized.__module__ = cls.__module__
            specialized.__qualname__ = namespace["__qualname__"]
            return specialized

    return _CallCheckableMeta


def call_checkable(protocol):
    """Decorator that enables runtime call-signature checks for a Protocol.

    This follows the `runtime_checkable` pattern: it keeps the hook local to the
    decorated protocol instead of patching the global Protocol metaclass.
    """
    if not isinstance(protocol, type) or not issubclass(protocol, Protocol):
        raise TypeError("call_checkable can only be applied to Protocol subclasses")

    namespace = {
        key: value
        for key, value in protocol.__dict__.items()
        if key not in {"__dict__", "__weakref__", "__annotations__"}
    }
    namespace["__call_checkable__"] = True
    namespace["_is_runtime_protocol"] = getattr(protocol, "_is_runtime_protocol", True)
    namespace["__parameters__"] = getattr(protocol, "__parameters__", ())
    namespace["__orig_bases__"] = getattr(protocol, "__orig_bases__", ())
    namespace["__annotations__"] = dict(getattr(protocol, "__annotations__", {}))

    meta = _make_call_checkable_metaclass(type(protocol))
    new_protocol = meta(protocol.__name__, protocol.__bases__, namespace)
    new_protocol.__module__ = protocol.__module__
    new_protocol.__qualname__ = getattr(protocol, "__qualname__", protocol.__name__)
    return new_protocol


def explain_callable_mismatch(obj, proto):
    """Return a human-readable explanation when a candidate is incompatible."""
    if not isinstance(proto, type):
        try:
            origin = proto.__origin__
        except AttributeError:
            origin = None
        if origin is not None and isinstance(origin, type):
            proto = origin
    else:
        try:
            origin = proto.__origin__
        except AttributeError:
            origin = None
        if origin is not None and isinstance(origin, type):
            proto = origin
    if not isinstance(proto, type) or not issubclass(proto, Protocol):
        raise TypeError("explain_callable_mismatch requires a Protocol subclass")
    return _explain_callable_mismatch(obj, proto)


__all__ = [
    "Ators",
    "AtorsChange",
    "AtorsRef",
    "Event",
    "EventCustomizationTool",
    "Member",
    "PicklePolicy",
    "add_generic_type_attributes",
    "atorsref",
    "call_checkable",
    "runtime_checkable",
    "disable_notifications",
    "enable_notifications",
    "event",
    "explain_callable_mismatch",
    "freeze",
    "get_event",
    "get_event_customization_tool",
    "get_events",
    "get_events_by_tag",
    "get_events_by_tag_and_value",
    "get_member",
    "get_member_customization_tool",
    "get_members",
    "get_members_by_tag",
    "get_members_by_tag_and_value",
    "is_frozen",
    "is_notifications_enabled",
    "member",
    "observe",
    "register_type_mutability_info",
    "unobserve",
    "validated",
]
