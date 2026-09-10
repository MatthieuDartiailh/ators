# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for ordered NotifyingMap behavior."""

import pytest

from ators import (
    Ators,
    ContainerChange,
    NotifyingMap,
    disable_notifications,
    enable_notifications,
    member,
    observe,
)


class _ObservableNotifyingMapOwner(Ators, observable=True):
    items: NotifyingMap[str, int] = member()


def test_notifying_map_add_move_remove_preserve_order():
    obj = _ObservableNotifyingMapOwner(items={})

    obj.items.add("a", 1)
    obj.items.add("b", 2)
    obj.items.add("c", 3, before="b")

    assert list(obj.items) == ["a", "c", "b"]
    assert list(obj.items.keys()) == ["a", "c", "b"]
    assert list(obj.items.values()) == [1, 3, 2]
    assert obj.items["b"] == 2

    obj.items.move("a", None)
    assert list(obj.items) == ["c", "b", "a"]

    assert obj.items.remove("c") == 3
    assert list(obj.items) == ["b", "a"]

    with pytest.raises(KeyError):
        obj.items.remove("missing")


def test_notifying_map_emits_change_events_and_batches():
    obj = _ObservableNotifyingMapOwner(items={})
    changes = []
    observe(obj, "items", changes.append)

    obj.items.add("a", 1)
    obj.items.add("b", 2)

    assert len(changes) == 2
    assert isinstance(changes[0], ContainerChange)
    assert list(changes[0].newvalue) == ["a"]
    assert changes[0].operations[0].payload == ("a", 1)
    assert list(changes[1].newvalue) == ["a", "b"]

    with obj.items.batched_notifications():
        obj.items.move("a", None)
        obj.items.remove("b")

    assert len(changes) == 3
    assert isinstance(changes[-1], ContainerChange)
    assert list(changes[-1].newvalue) == ["a"]


def test_notifying_map_respects_parent_notification_controls():
    obj = _ObservableNotifyingMapOwner(items={"a": 1})
    changes = []
    observe(obj, "items", changes.append)

    disable_notifications(obj)
    with obj.items.batched_notifications():
        obj.items.add("b", 2)
        obj.items.add("c", 3)

    enable_notifications(obj)
    obj.items.add("d", 4)

    assert len(changes) == 1
    assert list(changes[0].newvalue) == ["a", "b", "c", "d"]


def test_notifying_map_forbidden_in_non_observable_class():
    with pytest.raises(TypeError):

        class _NonObservable(Ators, observable=False):
            items: NotifyingMap[str, int] = member()


def test_notifying_map_rejects_nested_usage():
    with pytest.raises(TypeError):

        class _Nested(Ators, observable=True):
            items: list[NotifyingMap[str, int]] = member()
