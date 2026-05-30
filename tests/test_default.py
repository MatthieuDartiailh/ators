# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test default behavior for ators object"""

import pytest

from ators import Ators, Member, member
from ators.behaviors import Default, default


def test_no_default():
    class A(Ators):
        a: int

    a = A()  # type: ignore[missing-argument]
    with pytest.raises(TypeError) as e:
        a.a

    assert "value is unset and has no default" in e.value.__cause__.args[0]


def test_static_default():
    class A(Ators):
        a: int = 2

    a = A()
    assert a.a == 2

    class B(Ators):
        a: int = member().default(2)

    a = B()
    assert a.a == 2


def test_static_set_default():
    default = {2}

    class A(Ators):
        a: set[int] = default

    a = A()
    assert a.a == default
    assert a.a is not default  # Ensure a copy is made

    class B(Ators):
        a: set[int] = member().default(default)

    a = B()
    assert a.a == default
    assert a.a is not default  # Ensure a copy is made


def test_static_dict_default():
    default = {2: 4}

    class A(Ators):
        a: dict[int, int] = default

    a = A()
    assert a.a == default
    assert a.a is not default  # Ensure a copy is made

    class B(Ators):
        a: dict[int, int] = member().default(default)

    a = B()
    assert a.a == default
    assert a.a is not default  # Ensure a copy is made


def test_call_default():
    i = 0

    def make_default():
        nonlocal i
        i += 1
        return 5

    class A(Ators):
        a = member().default(Default.Call(make_default))

    a = A()
    assert a.a == 5
    assert i == 1
    assert a.a == 5
    assert i == 1


def test_call_name_object_default():
    i = 0
    m = None
    obj = None

    def make_default(name, object):
        nonlocal i, m, obj
        i += 1
        m = name
        obj = object
        return 5

    class A(Ators):
        a: int = member().default(Default.CallMemberObject(make_default))

    a = A()
    assert a.a == 5
    assert i == 1
    assert isinstance(m, str)
    assert isinstance(obj, A)
    assert a.a == 5
    assert i == 1


def test_method_default():
    i = 0
    me = None

    class A(Ators):
        a: int = member()

        @default(a)
        def _default_a(self, m):
            nonlocal i, me
            me = m
            i += 1
            return 8

    a = A()
    assert a.a == 8
    assert i == 1
    assert isinstance(me, Member)
    assert a.a == 8
    assert i == 1

    class B(A):
        def _default_a(self, m):
            return 9

    assert B().a == 9


def test_inherited_default_behavior():
    class A(Ators):
        a: int = 2

    class B(A):
        a = member().inherit()

    b = B()
    assert b.a == 2


@pytest.mark.parametrize(
    "behavior, callable, expected, got",
    [(Default.Call, lambda x: 1, 0, 1), (Default.CallMemberObject, lambda: 1, 2, 0)],
)
def test_bad_signature(behavior, callable, expected, got):
    with pytest.raises(ValueError) as e:

        class A(Ators):
            a: int = member().default(behavior(callable))

    assert f"callable taking {expected}" in e.exconly()
    assert f"which takes {got}" in e.exconly()


def test_default_not_as_decorator():
    with pytest.raises(RuntimeError) as e:

        class A(Ators):
            m = member()

            def f(self, m):
                pass

            default(m)(f)

    assert "'default' can only be used as a decorator" in e.exconly()


def test_default_outside_class_body():
    with pytest.raises(RuntimeError) as e:
        m = member()

        @default(m)
        def f(self, m):
            pass

    assert "'default' can only be used inside a class body" in e.exconly()


def test_bad_signature_of_method():
    with pytest.raises(TypeError) as e:

        class A(Ators):
            m = member()

            @default(m)  # type: ignore[invalid-argument-type]
            def f(self):
                pass

    assert "Method signature for 'default'" in e.exconly()


def test_warn_on_multiple_setting_of_default():
    with pytest.warns(UserWarning):

        class A(Ators):
            a: int = member().default(Default.Call(lambda: 1)).default(1)


# ---------------------------------------------------------------------------
# Constructor kwargs: default= and default_factory=
# ---------------------------------------------------------------------------


def test_member_ctor_default_raw_value_sets_static_behavior():
    """member(default=x) should behave identically to member().default(x)."""

    class A(Ators):
        a: int = member(default=42)

    assert A().a == 42


def test_member_ctor_default_none_is_explicit():
    """member(default=None) should set None as the static default, not NoDefault."""

    class A(Ators):
        a: int | None = member(default=None)

    assert A().a is None


def test_member_ctor_default_factory_sets_call_behavior():
    """member(default_factory=f) should call the factory and cache the result."""
    i = 0

    def factory():
        nonlocal i
        i += 1
        return 99

    class A(Ators):
        a = member(default_factory=factory)

    a = A()
    assert a.a == 99
    assert i == 1
    # Second access should use cached value, not call factory again.
    assert a.a == 99
    assert i == 1


def test_member_ctor_default_factory_rejects_wrong_arity():
    """default_factory must accept zero arguments."""
    with pytest.raises(ValueError) as exc_info:

        class A(Ators):
            a = member(default_factory=lambda x: x)

    assert "callable taking 0" in exc_info.exconly()


def test_member_ctor_default_factory_rejects_non_callable():
    """default_factory must be callable."""
    with pytest.raises((TypeError, ValueError)):

        class A(Ators):
            a = member(default_factory=42)


def test_member_ctor_default_and_factory_conflict_raises_typeerror():
    """Specifying both default and default_factory must raise TypeError."""
    with pytest.raises(TypeError) as exc_info:
        member(default=1, default_factory=lambda: 1)

    assert "default" in exc_info.exconly()
    assert "default_factory" in exc_info.exconly()


def test_member_ctor_default_rejects_defaultbehavior_instance():
    """Passing a DefaultBehavior instance to default= must raise TypeError."""
    with pytest.raises(TypeError) as exc_info:
        member(default=Default.Call(lambda: 1))

    assert (
        "DefaultBehavior" in exc_info.exconly() or "plain values" in exc_info.exconly()
    )


def test_member_ctor_default_then_chain_warns():
    """Calling .default() on a builder that already has a ctor default warns."""
    with pytest.warns(UserWarning):

        class A(Ators):
            a: int = member(default=1).default(2)


def test_member_ctor_default_inherits():
    """Constructor-set default propagates through .inherit() in subclasses."""

    class A(Ators):
        a: int = member(default=7)

    class B(A):
        a = member().inherit()

    assert B().a == 7


def test_member_ctor_default_factory_inherits():
    """Constructor-set default_factory propagates through .inherit() in subclasses."""
    i = 0

    def factory():
        nonlocal i
        i += 1
        return 55

    class A(Ators):
        a = member(default_factory=factory)

    class B(A):
        a = member().inherit()

    assert B().a == 55
