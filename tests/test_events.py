# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for Event descriptor and event() builder."""

import pytest

from ators import (
    Ators,
    AtorsChange,
    Event,
    event,
    get_event,
    get_events,
    observe,
    unobserve,
)


# ---------------------------------------------------------------------------
# 1. Class declaration tests
# ---------------------------------------------------------------------------


def test_event_class_getitem_returns_generic_alias():
    """Event[int] returns a GenericAlias with correct origin and args."""
    from types import GenericAlias

    ga = Event[int]
    assert isinstance(ga, GenericAlias)
    assert ga.__origin__ is Event
    assert ga.__args__ == (int,)


def test_event_annotation_valid():
    """A simple Event[T] annotation is accepted."""

    class A(Ators, observable=True):
        clicked: Event[int]

    assert isinstance(A.clicked, Event)
    assert A.clicked.name == "clicked"


def test_event_with_builder():
    """Event[T] = event() is accepted."""

    class A(Ators, observable=True):
        clicked: Event[int] = event()

    assert isinstance(A.clicked, Event)


def test_event_with_builder_metadata():
    """event().tag(...) attaches metadata."""

    class A(Ators, observable=True):
        clicked: Event[int] = event().tag(ui="button")

    ev = get_event(A, "clicked")
    assert ev.metadata["ui"] == "button"


def test_event_bare_annotation_raises():
    """Bare Event (no subscript) raises a TypeError at class creation."""
    with pytest.raises(TypeError, match="subscripted"):

        class A(Ators, observable=True):
            clicked: Event  # type: ignore[type-arg]


def test_event_wrong_arg_count_zero_raises():
    """Event[()] (zero args) raises a TypeError at class creation."""
    with pytest.raises(TypeError, match="exactly 1 type argument"):

        class A(Ators, observable=True):
            clicked: Event[()]  # type: ignore[type-arg]


def test_event_wrong_arg_count_two_raises():
    """Event[int, str] (two args) raises a TypeError at class creation."""
    with pytest.raises(TypeError, match="exactly 1 type argument"):

        class A(Ators, observable=True):
            clicked: Event[int, str]  # type: ignore[type-arg]


def test_event_builder_without_annotation_raises():
    """event() on RHS with no Event[T] annotation (and no inherit) raises."""
    with pytest.raises(TypeError, match="Event\\[T\\] annotation"):

        class A(Ators, observable=True):
            clicked = event()


def test_event_builder_with_inherit_no_annotation_allowed():
    """event().inherit() without annotation is allowed when a base event exists."""

    class Base(Ators, observable=True):
        clicked: Event[int]

    class Child(Base):
        clicked = event().inherit()

    assert isinstance(Child.clicked, Event)


# ---------------------------------------------------------------------------
# 2. Runtime write-only behavior
# ---------------------------------------------------------------------------


def test_event_read_raises():
    """Reading obj.event raises AttributeError (write-only)."""

    class A(Ators, observable=True):
        clicked: Event[int]

    a = A()
    with pytest.raises(AttributeError, match="write-only"):
        _ = a.clicked


def test_event_class_read_returns_descriptor():
    """Reading Class.event returns the Event descriptor object."""

    class A(Ators, observable=True):
        clicked: Event[int]

    assert isinstance(A.clicked, Event)


def test_event_set_valid_notifies():
    """A valid assignment triggers the observer callback."""
    hits = []

    class A(Ators, observable=True):
        clicked: Event[int]

    a = A()
    observe(a, "clicked", lambda c: hits.append(c.newvalue))
    a.clicked = 42
    assert hits == [42]


def test_event_set_no_storage():
    """Values assigned to an event are never stored (no attribute set)."""

    class A(Ators, observable=True):
        clicked: Event[int]

    a = A()
    a.clicked = 99
    # Read should still fail — nothing was stored.
    with pytest.raises(AttributeError):
        _ = a.clicked


def test_event_set_invalid_does_not_notify():
    """Validation failure on set does not call observers."""
    hits = []

    class A(Ators, observable=True):
        clicked: Event[int]

    a = A()
    observe(a, "clicked", lambda c: hits.append(c.newvalue))
    with pytest.raises((TypeError, ValueError)):
        a.clicked = "not an int"
    assert hits == []


def test_event_each_valid_set_notifies():
    """Repeated identical assignments each trigger a notification (no value comparison)."""
    hits = []

    class A(Ators, observable=True):
        clicked: Event[int]

    a = A()
    observe(a, "clicked", lambda c: hits.append(c.newvalue))
    a.clicked = 1
    a.clicked = 1  # same value — still notifies
    a.clicked = 1
    assert hits == [1, 1, 1]


def test_event_oldvalue_is_none():
    """AtorsChange.oldvalue is None for events (no stored prior value)."""
    changes = []

    class A(Ators, observable=True):
        clicked: Event[int]

    a = A()
    observe(a, "clicked", changes.append)
    a.clicked = 7
    assert changes[0].oldvalue is None
    assert changes[0].newvalue == 7


def test_event_notification_disabled_skips():
    """When notifications are disabled, events do not call observers."""
    from ators import disable_notifications, enable_notifications

    hits = []

    class A(Ators, observable=True):
        clicked: Event[int]

    a = A()
    observe(a, "clicked", lambda c: hits.append(c.newvalue))

    from ators import disable_notifications

    disable_notifications(a)
    a.clicked = 5
    assert hits == []

    enable_notifications(a)
    a.clicked = 5
    assert hits == [5]


def test_event_deletion_raises():
    """del obj.event raises AttributeError."""

    class A(Ators, observable=True):
        clicked: Event[int]

    a = A()
    with pytest.raises(AttributeError):
        del a.clicked


# ---------------------------------------------------------------------------
# 3. Inheritance behavior
# ---------------------------------------------------------------------------


def test_event_inherited_by_subclass():
    """Events from base classes are accessible on subclasses."""

    class Base(Ators, observable=True):
        clicked: Event[int]

    class Child(Base):
        pass

    assert isinstance(Child.clicked, Event)
    c = Child()
    hits = []
    observe(c, "clicked", lambda ch: hits.append(ch.newvalue))
    c.clicked = 3
    assert hits == [3]


def test_event_inherit_builder():
    """event().inherit() merges behavior from base event."""

    class Base(Ators, observable=True):
        clicked: Event[int] = event().tag(source="base")

    class Child(Base):
        clicked: Event[int] = event().inherit()

    # Metadata from base should be inherited.
    ev = get_event(Child, "clicked")
    assert ev.metadata is not None and ev.metadata.get("source") == "base"


def test_event_inherit_missing_base_raises():
    """event().inherit() without a matching base event raises TypeError."""
    with pytest.raises(TypeError, match="no such event"):

        class Base(Ators, observable=True):
            pass

        class Child(Base):
            clicked = event().inherit()


# ---------------------------------------------------------------------------
# 4. Accessor functions
# ---------------------------------------------------------------------------


def test_get_event_on_class():
    """get_event(cls, name) returns the Event descriptor."""

    class A(Ators, observable=True):
        clicked: Event[int]

    ev = get_event(A, "clicked")
    assert isinstance(ev, Event)
    assert ev.name == "clicked"


def test_get_event_on_instance():
    """get_event(instance, name) also works."""

    class A(Ators, observable=True):
        clicked: Event[int]

    a = A()
    ev = get_event(a, "clicked")
    assert isinstance(ev, Event)


def test_get_event_unknown_raises():
    """get_event for an unknown name raises AttributeError."""

    class A(Ators, observable=True):
        clicked: Event[int]

    with pytest.raises(AttributeError, match="Unknown event"):
        get_event(A, "no_such_event")


def test_get_events_returns_dict():
    """get_events returns a dict mapping name → Event for all events."""

    class A(Ators, observable=True):
        e1: Event[int]
        e2: Event[str]

    events = get_events(A)
    assert set(events.keys()) == {"e1", "e2"}
    assert all(isinstance(e, Event) for e in events.values())


def test_get_events_includes_inherited():
    """get_events on a subclass includes events from base classes."""

    class Base(Ators, observable=True):
        base_event: Event[int]

    class Child(Base):
        child_event: Event[str]

    events = get_events(Child)
    assert "base_event" in events
    assert "child_event" in events


# ---------------------------------------------------------------------------
# 5. Observe / unobserve with events
# ---------------------------------------------------------------------------


def test_observe_event_works():
    """observe() accepts event names."""
    hits = []

    class A(Ators, observable=True):
        clicked: Event[int]

    a = A()
    observe(a, "clicked", lambda c: hits.append(c.newvalue))
    a.clicked = 10
    assert hits == [10]


def test_unobserve_event_stops_notifications():
    """unobserve() removes event callbacks."""
    hits = []

    def cb(c):
        hits.append(c.newvalue)

    class A(Ators, observable=True):
        clicked: Event[int]

    a = A()
    observe(a, "clicked", cb)
    a.clicked = 1
    unobserve(a, "clicked", cb)
    a.clicked = 2
    assert hits == [1]


# ---------------------------------------------------------------------------
# 6. Frozen class behavior
# ---------------------------------------------------------------------------


def test_event_set_on_frozen_raises():
    """Assigning to an event on a frozen class raises TypeError."""
    from ators import freeze

    class A(Ators, observable=True, frozen=True):
        clicked: Event[int]

    a = A()
    with pytest.raises(TypeError, match="frozen"):
        a.clicked = 1


# ---------------------------------------------------------------------------
# 7. Non-observable class still allows event declaration
# ---------------------------------------------------------------------------


def test_event_declaration_non_observable():
    """Events can be declared on non-observable classes; set succeeds silently."""

    class A(Ators):
        clicked: Event[int]

    a = A()
    # No notification, but should not raise on set.
    a.clicked = 42
    # Read is still write-only.
    with pytest.raises(AttributeError, match="write-only"):
        _ = a.clicked
