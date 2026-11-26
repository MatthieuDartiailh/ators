# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for keyword-reserved member method decorators."""

from typing import Any

import pytest

from ators import Ators, Member, member
from ators.behaviors import (
    Coercer,
    PostGetAttr,
    PostSetAttr,
    PreGetAttr,
    PreSetAttr,
    ValueValidator,
    coerce,
    coerce_init,
    default,
    postget,
    postset,
    preset,
)

DECORATOR_CASES = [
    ("default", int, 1, "default"),
    ("preget", int, PreGetAttr.NoOp(), "preget"),
    ("postget", int, PostGetAttr.NoOp(), "postget"),
    ("preset", int, PreSetAttr.NoOp(), "preset"),
    ("postset", int, PostSetAttr.NoOp(), "postset"),
    ("coerce", Member[int, Any], Coercer.TypeInferred(), "coerce"),
    ("coerce_init", int, Coercer.TypeInferred(), "coerce_init"),
    (
        "append_value_validator",
        int,
        ValueValidator.Values(frozenset({1})),
        "append_value_validator",
    ),
]


@pytest.mark.parametrize("method,ann,direct_value,_name", DECORATOR_CASES)
def test_decorator_reservation_missing(method, ann, direct_value, _name):
    builder = getattr(member(), method)(decorator=True)

    with pytest.raises(TypeError) as exc:

        class A(Ators):
            a: ann = builder

    assert "Member 'a'" in exc.exconly()
    assert f"@{method}" in exc.exconly()


WRONG_DECORATOR_CASES = [
    ("default", postset, "postset", 4),
    ("preget", postget, "postget", 3),
    ("postget", postset, "postset", 4),
    ("preset", postset, "postset", 4),
    ("postset", default, "default", 2),
    ("coerce", coerce_init, "coerce_init", 4),
    ("coerce_init", coerce, "coerce", 4),
    ("append_value_validator", preset, "preset", 3),
]


@pytest.mark.parametrize(
    "reserved_method,wrong_factory,wrong_method,wrong_signature",
    WRONG_DECORATOR_CASES,
)
def test_decorator_reservation_wrong_factory(
    reserved_method, wrong_factory, wrong_method, wrong_signature
):
    builder = getattr(member(), reserved_method)(decorator=True)

    with pytest.raises(TypeError) as exc:
        if wrong_signature == 2:

            class A(Ators):
                a: int = builder

                @wrong_factory(builder)
                def wrong(self, member):
                    pass

        elif wrong_signature == 3:

            class A(Ators):
                a: int = builder

                @wrong_factory(builder)
                def wrong(self, member, value):
                    pass

        else:

            class A(Ators):
                a: int = builder

                @wrong_factory(builder)
                def wrong(self, member, old, new):
                    pass

    assert f"no pending '{wrong_method}' decorator" in exc.exconly()


@pytest.mark.parametrize("method,ann,direct_value,_name", DECORATOR_CASES)
def test_decorator_reservation_repeated(method, ann, direct_value, _name):
    builder = getattr(member(), method)(decorator=True)

    with pytest.raises(TypeError) as exc:
        getattr(builder, method)(decorator=True)

    assert f"already reserves decorator '{method}'" in exc.exconly()


@pytest.mark.parametrize("method,ann,direct_value,_name", DECORATOR_CASES)
def test_direct_call_conflicts_with_decorator_reservation(
    method, ann, direct_value, _name
):
    builder = getattr(member(), method)(decorator=True)

    with pytest.raises(TypeError) as exc:
        getattr(builder, method)(direct_value)

    assert f"reserves the '{method}' decorator" in exc.exconly()


@pytest.mark.parametrize("method,ann,direct_value,_name", DECORATOR_CASES)
def test_decorator_keyword_rejects_false_and_behavior_argument(
    method, ann, direct_value, _name
):
    with pytest.raises(TypeError) as exc:
        getattr(member(), method)(decorator=False)

    assert "decorator=False" in exc.exconly()

    with pytest.raises(TypeError) as exc:
        getattr(member(), method)(direct_value, decorator=True)

    assert "decorator=True" in exc.exconly()
