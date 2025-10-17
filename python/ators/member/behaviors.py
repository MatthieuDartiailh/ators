# --------------------------------------------------------------------------------------
# Copyright (c) 2025, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
""""""

import inspect

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


def default(member_builder: member):
    """"""

    def decorator(func):
        context = inspect.stack(1)[1].code_context
        if not any("@" in line for line in context):
            raise RuntimeError("'default' can only be used as a decorator.")
        member_builder.default(Default.ObjectMethod(func.__name__))
        return func

    return decorator


def preget(member_builder: member):
    """"""

    def decorator(func):
        context = inspect.stack(1)[1].code_context
        if not any("@" in line for line in context):
            raise RuntimeError("'preget' can only be used as a decorator.")
        member_builder.preget(PreGetAttr.ObjectMethod(func.__name__))
        return func

    return decorator


def postget(member_builder: member):
    """"""

    def decorator(func):
        context = inspect.stack(1)[1].code_context
        if not any("@" in line for line in context):
            raise RuntimeError("'postget' can only be used as a decorator.")
        member_builder.postget(PostGetAttr.ObjectMethod(func.__name__))
        return func

    return decorator


def preset(member_builder: member):
    """"""

    def decorator(func):
        context = inspect.stack(1)[1].code_context
        if not any("@" in line for line in context):
            raise RuntimeError("'preset' can only be used as a decorator.")
        member_builder.preset(PreSetAttr.ObjectMethod(func.__name__))
        return func

    return decorator


def postset(member_builder: member):
    """"""

    def decorator(func):
        context = inspect.stack(1)[1].code_context
        if not any("@" in line for line in context):
            raise RuntimeError("'postset' can only be used as a decorator.")
        member_builder.postset(PostSetAttr.ObjectMethod(func.__name__))
        return func

    return decorator


def coerce(member_builder: member):
    """"""

    def decorator(func):
        context = inspect.stack(1)[1].code_context
        if not any("@" in line for line in context):
            raise RuntimeError("'coerce' can only be used as a decorator.")
        member_builder.default(Coercer.ObjectMethod(func.__name__))
        return func

    return decorator


def coerce_init(member_builder: member):
    """"""

    def decorator(func):
        context = inspect.stack(1)[1].code_context
        if not any("@" in line for line in context):
            raise RuntimeError("'coerce_init' can only be used as a decorator.")
        member_builder.default(Coercer.ObjectMethod(func.__name__))
        return func

    return decorator


def append_value_validator(member_builder: member):
    """"""

    def decorator(func):
        context = inspect.stack(1)[1].code_context
        if not any("@" in line for line in context):
            raise RuntimeError(
                "'append_value_validator' can only be used as a decorator."
            )
        member_builder.append_value_validator(
            ValueValidator.ObjectMethod(func.__name__)
        )
        return func

    return decorator


__all__ = ["Default", "default"]
