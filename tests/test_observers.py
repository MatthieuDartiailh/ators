# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test observer behavior for ators object"""

import gc

import pytest

from ators import (
    Ators,
    AtorsChange,
    disable_notifications,
    enable_notifications,
    is_notifications_enabled,
    member,
    observe,
    unobserve,
)


def test_observable_member_indexes_are_shifted():
    class A(Ators, observable=True):
        a = member()
        b = member()

    assert A.a.slot_index == 1
    assert A.b.slot_index == 2


def test_non_observable_member_indexes_are_not_shifted():
    class A(Ators):
        a = member()
        b = member()

    assert A.a.slot_index == 0
    assert A.b.slot_index == 1


def test_observe_and_unobserve():
    calls = []

    def callback(change):
        calls.append(change)

    class A(Ators, observable=True):
        a: int = member()

    a = A()
    observe(a, "a", callback)

    a.a = 1
    assert len(calls) == 1
    assert isinstance(calls[0], AtorsChange)
    assert calls[0].object is a
    assert calls[0].member_name == "a"
    assert calls[0].oldvalue is None
    assert calls[0].newvalue == 1

    a.a = 2
    assert len(calls) == 2
    assert calls[1].oldvalue == 1
    assert calls[1].newvalue == 2

    unobserve(a, "a", callback)
    a.a = 3
    assert len(calls) == 2


def test_observe_fails_for_non_observable():
    class A(Ators):
        a = member()

    a = A()
    with pytest.raises(TypeError):
        observe(a, "a", lambda c: None)


def test_notification_controls_and_invariant():
    class A(Ators):
        a = member()

    class B(Ators, observable=True):
        a = member()

    a = A()
    b = B()

    assert is_notifications_enabled(a) is False
    assert is_notifications_enabled(b) is True

    disable_notifications(b)
    assert is_notifications_enabled(b) is False

    enable_notifications(b)
    assert is_notifications_enabled(b) is True

    with pytest.raises(TypeError):
        enable_notifications(a)


def test_notifications_disabled_skip_callbacks():
    i = 0

    def callback(change):
        nonlocal i
        i += 1

    class A(Ators, observable=True):
        a = member()

    a = A()
    observe(a, "a", callback)

    disable_notifications(a)
    a.a = 1
    assert i == 0

    enable_notifications(a)
    a.a = 2
    assert i == 1


def test_observer_errors_grouped_after_all_callbacks():
    seen = []

    def fail_1(change):
        seen.append("fail_1")
        raise ValueError("a")

    def fail_2(change):
        seen.append("fail_2")
        raise RuntimeError("b")

    def ok(change):
        seen.append("ok")

    class A(Ators, observable=True):
        a = member()

    a = A()
    observe(a, "a", fail_1)
    observe(a, "a", ok)
    observe(a, "a", fail_2)

    with pytest.raises(ExceptionGroup) as err:
        a.a = 1

    assert seen == ["fail_1", "ok", "fail_2"]
    assert len(err.value.exceptions) == 2
    assert any(isinstance(e, ValueError) for e in err.value.exceptions)
    assert any(isinstance(e, RuntimeError) for e in err.value.exceptions)


def test_bound_method_uses_weakref_and_is_pruned_after_fire():
    calls = []

    def keep_alive(change):
        calls.append("alive")

    class Recorder:
        def cb(self, change):
            calls.append("dead")

    class A(Ators, observable=True):
        a = member()

    a = A()
    observe(a, "a", keep_alive)

    rec = Recorder()
    observe(a, "a", rec.cb)
    del rec
    gc.collect()

    a.a = 1
    assert calls == ["alive"]


def test_observable_is_inherited():
    class A(Ators, observable=True):
        a = member()

    class B(A):
        b = member()

    assert A.a.slot_index == 1
    assert B.a.slot_index >= 1
    assert B.b.slot_index >= 1

    b = B()
    hits = []
    observe(b, "a", lambda change: hits.append(change.newvalue))
    b.a = 5
    assert hits == [5]


def test_observer_not_called_when_value_unchanged():
    calls = []

    def callback(change):
        calls.append(change)

    class A(Ators, observable=True):
        a: int = member()

    a = A()
    observe(a, "a", callback)

    # Setting to a new value triggers the observer
    a.a = 1
    assert len(calls) == 1

    # Setting the exact same object again should not trigger the observer
    val = 1
    a.a = val  # same object (Python caches small ints), no new call
    assert len(calls) == 1

    # Setting a different value triggers again
    a.a = 2
    assert len(calls) == 2
