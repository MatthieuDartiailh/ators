# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Convenience decorators to register member behaviors.

This module provides decorator factories used inside Ators class bodies
to attach behavior implementations (methods) to member builders. Each
factory validates correct usage (decorator context and method signature)
and registers an object-method wrapper on the provided member builder.

Notes
-----
Decorators created here are intended to be used within class bodies
and expect the decorated functions to be instance methods with specific
signatures. The helpers perform runtime validation and will raise or
warn when misused.

"""

import inspect
import warnings
from typing import Callable, Any, TYPE_CHECKING

if TYPE_CHECKING:
    from . import Ators

from ators._ators import (
    member,
    DefaultBehavior as Default,
    PreGetattrBehavior as PreGetAttr,
    PostGetattrBehavior as PostGetAttr,
    PreSetattrBehavior as PreSetAttr,
    PostSetattrBehavior as PostSetAttr,
    DelattrBehavior as DelAttr,
)
from .validators import ValueValidator, Coercer

# Reporting the error at call site is sufficient since users will be pointed
# to exact problematic behavior.


def _validate_use_and_sig(
    stack: list[inspect.FrameInfo],
    behavior: str,
    func: Callable[..., Any],
    expected_sig: tuple[str],
) -> None:
    """Validate decorator usage and the decorated function signature.

    The function inspects the call stack to ensure the decorator is
    applied within a class body and that the decorated function has
    the expected parameter names/length. If source context is not
    available a :class:`UserWarning` is emitted.

    Parameters
    ----------
    stack : list[inspect.FrameInfo]
        Inspection stack returned by :func:`inspect.stack` used to
        infer decoration and class context.
    behavior : str
        Human-readable name of the behavior being validated (used in
        error messages).
    func : types.FunctionType
        The function object being validated.
    expected_sig : tuple[str]
        Tuple of expected parameter names in order.

    Raises
    ------
    RuntimeError
        If the decorator is not used inside a class body or not used
        as a decorator (when source context is available).
    TypeError
        If the decorated function does not accept the expected number
        of parameters.

    """
    decoration_context = stack[1].code_context
    if decoration_context is None:
        warnings.warn(
            UserWarning(
                f"Code has no source preventing to check '{behavior}' is used properly."
            ),
            stacklevel=2,
        )
    else:
        if not any("@" in line for line in decoration_context):
            raise RuntimeError(f"'{behavior}' can only be used as a decorator.")

        class_context = stack[2].code_context
        if not any("class" in line for line in class_context):
            raise RuntimeError(f"'{behavior}' can only be used inside a class body.")

    sig = inspect.signature(func)
    if len(sig.parameters) != len(expected_sig):
        raise TypeError(
            f"Method signature for '{behavior}' should be ({', '.join(expected_sig)}),"
            f" got {sig}"
        )


def default(
    member_builder: member,
) -> Callable[[Callable[[Ators, member], Any]], Callable[[Ators, member], Any]]:
    """Return a decorator that registers a default value provider.

    The decorated method must have the signature ``(self, member)``.
    When applied, the member builder receives an object-method wrapper
    pointing to the named instance method which will be called to
    produce default values for the member.

    Parameters
    ----------
    member_builder : ators._ators.member
        The member builder to which the default provider will be
        attached.

    Returns
    -------
    callable
        A decorator that registers the default provider and returns
        the original function unchanged.

    """

    def decorator(
        func: Callable[[Ators, member], Any],
    ) -> Callable[[Ators, member], Any]:
        st = inspect.stack(1)
        _validate_use_and_sig(st, "default", func, ("self", "member"))
        member_builder.default(Default.ObjectMethod(func.__name__))
        return func

    return decorator


def preget(
    member_builder: member,
) -> Callable[[Callable[[Ators, member], Any]], Callable[[Ators, member], Any]]:
    """Return a decorator that registers a pre-get hook.

    The decorated method must have the signature ``(self, member)``.
    Registered methods are called before attribute retrieval completes
    and can be used to prepare instance state or short-circuit access.

    Parameters
    ----------
    member_builder : ators._ators.member
        The member builder to which the pre-get hook will be attached.

    Returns
    -------
    callable
        A decorator that registers the pre-get hook and returns the
        original function.

    """

    def decorator(
        func: Callable[[Ators, member], Any],
    ) -> Callable[[Ators, member], Any]:
        st = inspect.stack(1)
        _validate_use_and_sig(st, "preget", func, ("self", "member"))
        member_builder.preget(PreGetAttr.ObjectMethod(func.__name__))
        return func

    return decorator


def postget(
    member_builder: member,
) -> Callable[
    [Callable[[Ators, member, Any], Any]], Callable[[Ators, member, Any], Any]
]:
    """Return a decorator that registers a post-get hook.

    The decorated method must have the signature ``(self, member, value)``.
    Registered methods are invoked after the value has been retrieved
    and may inspect or transform it before it is returned to callers.

    Parameters
    ----------
    member_builder : ators._ators.member
        The member builder to which the post-get hook will be attached.

    Returns
    -------
    callable
        A decorator that registers the post-get hook and returns the
        original function.

    """

    def decorator(
        func: Callable[[Ators, member, Any], Any],
    ) -> Callable[[Ators, member, Any], Any]:
        st = inspect.stack(1)
        _validate_use_and_sig(st, "postget", func, ("self", "member", "value"))
        member_builder.postget(PostGetAttr.ObjectMethod(func.__name__))
        return func

    return decorator


def preset(
    member_builder: member,
) -> Callable[
    [Callable[[Ators, member, Any], Any]], Callable[[Ators, member, Any], Any]
]:
    """Return a decorator that registers a pre-set hook.

    The decorated method must have the signature
    ``(self, member, current)`` and is called before an attribute is
    assigned so the instance may validate or adjust state.

    Parameters
    ----------
    member_builder : ators._ators.member
        The member builder to which the pre-set hook will be attached.

    Returns
    -------
    callable
        A decorator that registers the pre-set hook and returns the
        original function.
    """

    def decorator(
        func: Callable[[Ators, member, Any], Any],
    ) -> Callable[[Ators, member, Any], Any]:
        st = inspect.stack(1)
        _validate_use_and_sig(st, "preset", func, ("self", "member", "current"))
        member_builder.preset(PreSetAttr.ObjectMethod(func.__name__))
        return func

    return decorator


def postset(
    member_builder: member,
) -> Callable[
    [Callable[[Ators, member, Any, Any], Any]], Callable[[Ators, member, Any, Any], Any]
]:
    """Return a decorator that registers a post-set hook.

    The decorated method must have the signature
    ``(self, member, old, new)`` and is invoked after an attribute
    assignment. Use it to react to value changes (e.g., emit
    notifications or update derived state).

    Parameters
    ----------
    member_builder : ators._ators.member
        The member builder to which the post-set hook will be attached.

    Returns
    -------
    callable
        A decorator that registers the post-set hook and returns the
        original function.

    """

    def decorator(
        func: Callable[[Ators, member, Any, Any], Any],
    ) -> Callable[[Ators, member, Any, Any], Any]:
        st = inspect.stack(1)
        _validate_use_and_sig(st, "postset", func, ("self", "member", "old", "new"))
        member_builder.postset(PostSetAttr.ObjectMethod(func.__name__))
        return func

    return decorator


def coerce(
    member_builder: member,
) -> Callable[
    [Callable[[Ators, member, Any, bool], Any]],
    Callable[[Ators, member, Any, bool], Any],
]:
    """Return a decorator that registers a runtime coercion method.

    The decorated method must have the signature
    ``(self, member, value, is_init_coercion)``. The registered method
    is used to coerce values assigned to the member after
    initialization.

    Parameters
    ----------
    member_builder : ators._ators.member
        The member builder to which the coercion method will be
        attached.

    Returns
    -------
    callable
        A decorator that registers the coercion method and returns the
        original function.

    """

    def decorator(
        func: Callable[[Ators, member, Any, bool], Any],
    ) -> Callable[[Ators, member, Any, bool], Any]:
        st = inspect.stack(1)
        _validate_use_and_sig(
            st, "coerce", func, ("self", "member", "value", "is_init_coercion")
        )
        member_builder.coerce(Coercer.ObjectMethod(func.__name__))
        return func

    return decorator


def coerce_init(
    member_builder: member,
) -> Callable[
    [Callable[[Ators, member, Any, bool], Any]],
    Callable[[Ators, member, Any, bool], Any],
]:
    """Return a decorator that registers an initialization coercion method.

    The decorated method must have the signature
    ``(self, member, value, is_init_coercion)``. The registered method
    is used to coerce values assigned to the member during object
    initialization.

    Parameters
    ----------
    member_builder : ators._ators.member
        The member builder to which the initialization coercion method
        will be attached.

    Returns
    -------
    callable
        A decorator that registers the coercion method and returns the
        original function.

    """

    def decorator(
        func: Callable[[Ators, member, Any, bool], Any],
    ) -> Callable[[Ators, member, Any, bool], Any]:
        st = inspect.stack(1)
        _validate_use_and_sig(
            st, "coerce_init", func, ("self", "member", "value", "is_init_coercion")
        )
        member_builder.coerce_init(Coercer.ObjectMethod(func.__name__))
        return func

    return decorator


def append_value_validator(
    member_builder: member,
) -> Callable[
    [Callable[[Ators, member, Any], Any]], Callable[[Ators, member, Any], Any]
]:
    """Return a decorator that appends a value validator to a member.

    The decorated method must have the signature ``(self, member, value)``.
    The method will be wrapped as an object-method validator and
    appended to the member's validators. Validators should raise an
    exception for invalid values.

    Parameters
    ----------
    member_builder : ators._ators.member
        The member builder to which the value validator will be
        appended.

    Returns
    -------
    callable
        A decorator that appends the validator and returns the original
        function.

    """

    def decorator(
        func: Callable[[Ators, member, Any], Any],
    ) -> Callable[[Ators, member, Any], Any]:
        st = inspect.stack(1)
        _validate_use_and_sig(
            st, "append_value_validator", func, ("self", "member", "value")
        )
        member_builder.append_value_validator(
            ValueValidator.ObjectMethod(func.__name__)
        )
        return func

    return decorator


__all__ = [
    "Default",
    "default",
    "PreGetAttr",
    "preget",
    "PreSetAttr",
    "preset",
    "PostGetAttr",
    "postget",
    "PostSetAttr",
    "postset",
    "DelAttr",
    "append_value_validator",
]
