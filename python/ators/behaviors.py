# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
""""""

import inspect
import warnings
from types import FunctionType

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
    func: FunctionType,
    expected_sig: tuple[str],
) -> None:
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


def default(member_builder: member):
    """"""

    def decorator(func):
        st = inspect.stack(1)
        _validate_use_and_sig(st, "default", func, ("self", "member"))
        member_builder.default(Default.ObjectMethod(func.__name__))
        return func

    return decorator


def preget(member_builder: member):
    """"""

    def decorator(func):
        st = inspect.stack(1)
        _validate_use_and_sig(st, "preget", func, ("self", "member"))
        member_builder.preget(PreGetAttr.ObjectMethod(func.__name__))
        return func

    return decorator


def postget(member_builder: member):
    """"""

    def decorator(func):
        st = inspect.stack(1)
        _validate_use_and_sig(st, "postget", func, ("self", "member", "value"))
        member_builder.postget(PostGetAttr.ObjectMethod(func.__name__))
        return func

    return decorator


def preset(member_builder: member):
    """"""

    def decorator(func):
        st = inspect.stack(1)
        _validate_use_and_sig(st, "preset", func, ("self", "member", "current"))
        member_builder.preset(PreSetAttr.ObjectMethod(func.__name__))
        return func

    return decorator


def postset(member_builder: member):
    """"""

    def decorator(func):
        st = inspect.stack(1)
        _validate_use_and_sig(st, "postset", func, ("self", "member", "old", "new"))
        member_builder.postset(PostSetAttr.ObjectMethod(func.__name__))
        return func

    return decorator


def coerce(member_builder: member):
    """"""

    def decorator(func):
        st = inspect.stack(1)
        _validate_use_and_sig(
            st, "coerce", func, ("self", "member", "value", "is_init_coercion")
        )
        member_builder.coerce(Coercer.ObjectMethod(func.__name__))
        return func

    return decorator


def coerce_init(member_builder: member):
    """"""

    def decorator(func):
        st = inspect.stack(1)
        _validate_use_and_sig(
            st, "coerce_init", func, ("self", "member", "value", "is_init_coercion")
        )
        member_builder.coerce_init(Coercer.ObjectMethod(func.__name__))
        return func

    return decorator


def append_value_validator(member_builder: member):
    """"""

    def decorator(func):
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
