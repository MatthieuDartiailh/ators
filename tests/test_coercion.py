# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test coercion behavior for ators object"""

import pytest

from ators import Ators, member, Member
from ators.behaviors import Coercer, coerce, coerce_init


# XXX refactor to support all types and follow following tests to simplify
# testing both init true and false
@pytest.mark.parametrize(
    "ty, init, inputs, expected",
    [
        (int, False, ["1", "2"], [1, 2]),
        (int, True, ["1", "2"], [1, TypeError("")]),
        (float, False, ["1.5", "2.5"], [1.5, 2.5]),
        (float, True, ["1.5", "2.5"], [1.5, TypeError("")]),
        (int | None, False, ["1", None, "2"], [1, None, 2]),
        (int | None, True, ["1", None, "2"], [1, None, TypeError("")]),
        # coerce for instance and typed
    ],
)
def test_type_inferred_coercion(ty, init, inputs, expected):
    class A(Ators):
        a: ty = getattr(member(), "coerce_init" if init else "coerce")()

    a = A(**{"a": inputs[0]})
    assert a.a == expected[0]

    for inp, exp in zip(inputs[1:], expected[1:]):
        if isinstance(exp, Exception):
            with pytest.raises(type(exp)) as e:
                a.a = inp
            assert str(exp) in e.exconly()
        else:
            a.a = inp
            assert a.a == exp


@pytest.mark.parametrize(
    "init, inputs, called, expected",
    [
        (False, ["1", 2, "3"], [1, 1, 2], [1, 2, 3]),
        (True, ["1", 2, "3"], [1, 1, 2], [1, 2, TypeError("")]),
    ],
)
def test_call_coerce(init, inputs, called, expected):
    i = 0

    def make_coerce(n):
        nonlocal i
        i += 1
        return int(n)

    class A(Ators):
        a: int = getattr(member(), "coerce_init" if init else "coerce")(
            Coercer.CallValue(make_coerce)
        )

    a = A(**{"a": inputs[0]})
    assert i == called[0]
    assert a.a == expected[0]

    for inp, c, exp in zip(inputs[1:], called[1:], expected[1:]):
        if isinstance(exp, Exception):
            with pytest.raises(type(exp)) as e:
                a.a = inp
            assert str(exp) in e.exconly()
        else:
            a.a = inp
            assert a.a == exp
            assert i == c


@pytest.mark.parametrize(
    "init, inputs, called, expected",
    [
        (False, ["1", 2, "3"], [1, 1, 2], [1, 2, 3]),
        (True, ["1", 2, "3"], [1, 1, 2], [1, 2, TypeError("")]),
    ],
)
def test_call_member_object_coerce(init, inputs, called, expected):
    i = 0
    m = None
    obj = None
    init_coercion = None

    def make_coerce(member, object, value, init):
        nonlocal i, m, obj, init_coercion
        i += 1
        m = member
        obj = object
        init_coercion = init
        return int(value)

    class A(Ators):
        a: int = getattr(member(), "coerce_init" if init else "coerce")(
            Coercer.CallNameObjectValueInit(make_coerce)
        )

    a = A(**{"a": inputs[0]})
    assert i == called[0]
    assert a.a == expected[0]
    if called[0]:
        assert init_coercion is init
        assert isinstance(m, str)
        assert isinstance(obj, Ators)

    for inp, c, exp in zip(inputs[1:], called[1:], expected[1:]):
        if isinstance(exp, Exception):
            with pytest.raises(type(exp)) as e:
                a.a = inp
            assert str(exp) in e.exconly()
        else:
            a.a = inp
            assert a.a == exp
            assert i == c


@pytest.mark.parametrize(
    "init, inputs, called, expected",
    [
        (False, ["1", 2, "3"], [1, 1, 2], [1, 2, 3]),
        (True, ["1", 2, "3"], [1, 1, 2], [1, 2, TypeError("")]),
    ],
)
def test_method_coerce(init, inputs, called, expected):
    i = 0
    me = None
    init_coercion = None

    class A(Ators):
        a: int = member()

        @(coerce_init if init else coerce)(a)
        def _coerce_a(self, m, v, init):
            nonlocal i, me, init_coercion
            me = m
            i += 1
            init_coercion = init
            return int(v)

    a = A(**{"a": inputs[0]})
    assert i == called[0]
    assert a.a == expected[0]
    if called[0]:
        assert init_coercion is init
        assert isinstance(me, str)

    for inp, c, exp in zip(inputs[1:], called[1:], expected[1:]):
        if isinstance(exp, Exception):
            with pytest.raises(type(exp)) as e:
                a.a = inp
            assert str(exp) in e.exconly()
        else:
            a.a = inp
            assert a.a == exp
            assert i == c

    class B(A):
        def _coerce_a(self, m, v, i):
            return 9

    assert B(**{"a": ""}).a == 9


@pytest.mark.parametrize("init", [False, True])
def test_inherited_coerce_behavior(init):
    class A(Ators):
        a: int = getattr(member(), "coerce_init" if init else "coerce")()

    class B(A):
        a = member().inherit()

    b = B(**{"a": "2"})
    assert b.a == 2


@pytest.mark.parametrize("init", [False, True])
@pytest.mark.parametrize(
    "behavior, callable, expected, got",
    [
        (Coercer.CallValue, lambda: 1, 1, 0),
        (Coercer.CallNameObjectValueInit, lambda: 1, 4, 0),
    ],
)
def test_bad_signature(init, behavior, callable, expected, got):
    with pytest.raises(ValueError) as e:

        class A(Ators):
            a: int = getattr(member(), "coerce_init" if init else "coerce")(
                behavior(callable)
            )

    assert f"callable taking {expected}" in e.exconly()
    assert f"which takes {got}" in e.exconly()


@pytest.mark.parametrize("init", [False, True])
def test_coerce_not_as_decorator(init):
    with pytest.raises(RuntimeError) as e:

        class A(Ators):
            m = member()

            def f(self, m):
                pass

            (coerce_init if init else coerce)(m)(f)

    assert (
        f"'{('coerce_init' if init else 'coerce')}' can only be used as a decorator"
        in e.exconly()
    )


@pytest.mark.parametrize("init", [False, True])
def test_coerce_outside_class_body(init):
    with pytest.raises(RuntimeError) as e:
        m = member()

        @(coerce_init if init else coerce)(m)
        def f(self, m):
            pass

    assert (
        f"'{('coerce_init' if init else 'coerce')}' can only be used inside a class body"
        in e.exconly()
    )


@pytest.mark.parametrize("init", [False, True])
def test_bad_signature_of_method(init):
    with pytest.raises(TypeError) as e:

        class A(Ators):
            m = member()

            @(coerce_init if init else coerce)(m)
            def f(self):
                pass

    assert (
        f"Method signature for '{('coerce_init' if init else 'coerce')}'" in e.exconly()
    )


@pytest.mark.parametrize("init", [False, True])
def test_warn_on_multiple_setting_of_coerce(init):
    with pytest.warns(UserWarning):

        class A(Ators):
            a: int = getattr(
                getattr(member(), "coerce_init" if init else "coerce")(
                    Coercer.CallValue(lambda v: 1)
                ),
                "coerce_init" if init else "coerce",
            )()


@pytest.mark.parametrize("init", [False, True])
def test_warn_on_useless_coercion(init):
    with pytest.warns(UserWarning):

        class A(Ators):
            a = getattr(member(), "coerce_init" if init else "coerce")(
                Coercer.CallValue(lambda v: 1)
            )
