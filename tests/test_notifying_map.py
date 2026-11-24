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
    ContainerOperation,
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
    assert isinstance(changes[0].operations[0], ContainerOperation.Added)
    assert changes[0].operations[0].payload == ("a", 1)
    assert list(changes[1].newvalue) == ["a", "b"]

    with obj.items.batched_notifications():
        obj.items.move("a", None)
        obj.items.remove("b")

    assert len(changes) == 3
    assert isinstance(changes[-1], ContainerChange)
    assert list(changes[-1].newvalue) == ["a"]


def test_notifying_map_setitem_existing_key_emits_replaced_operation():
    obj = _ObservableNotifyingMapOwner(items={"a": 1, "b": 2})
    changes = []

    observe(obj, "items", changes.append)
    obj.items["a"] = 10

    assert list(obj.items) == ["a", "b"]
    assert obj.items["a"] == 10
    assert len(changes) == 1
    assert type(changes[0]) is ContainerChange
    assert len(changes[0].operations) == 1
    operation = changes[0].operations[0]
    assert isinstance(operation, ContainerOperation.Replaced)
    assert operation.index == 0
    assert operation.old_value == ("a", 1)
    assert operation.new_value == ("a", 10)


def test_notifying_map_setitem_missing_key_appends_to_end():
    obj = _ObservableNotifyingMapOwner(items={"a": 1})
    changes = []

    observe(obj, "items", changes.append)
    obj.items["b"] = 2

    assert list(obj.items) == ["a", "b"]
    assert obj.items["b"] == 2
    assert len(changes) == 1
    assert type(changes[0]) is ContainerChange
    assert len(changes[0].operations) == 1
    operation = changes[0].operations[0]
    assert isinstance(operation, ContainerOperation.Added)
    assert operation.index == 1
    assert operation.payload == ("b", 2)


def test_notifying_map_get_returns_default_for_missing_key():
    obj = _ObservableNotifyingMapOwner(items={"a": 1})

    assert obj.items.get("missing", 99) == 99
    assert obj.items.get("a") == 1


def test_notifying_map_setdefault_appends_missing_key():
    obj = _ObservableNotifyingMapOwner(items={"a": 1})
    changes = []

    observe(obj, "items", changes.append)

    assert obj.items.setdefault("b", 2) == 2
    assert list(obj.items) == ["a", "b"]
    assert len(changes) == 1
    assert isinstance(changes[0].operations[0], ContainerOperation.Added)
    assert changes[0].operations[0].payload == ("b", 2)


def test_notifying_map_update_appends_new_entries_in_order():
    obj = _ObservableNotifyingMapOwner(items={"a": 1})
    changes = []

    observe(obj, "items", changes.append)

    obj.items.update({"b": 2}, c=3)

    assert list(obj.items) == ["a", "b", "c"]
    assert len(changes) == 2
    assert isinstance(changes[0].operations[0], ContainerOperation.Added)
    assert isinstance(changes[1].operations[0], ContainerOperation.Added)


def test_notifying_map_pop_removes_and_returns_value():
    obj = _ObservableNotifyingMapOwner(items={"a": 1, "b": 2})
    changes = []

    observe(obj, "items", changes.append)

    assert obj.items.pop("b") == 2
    assert list(obj.items) == ["a"]
    assert len(changes) == 1
    assert isinstance(changes[0].operations[0], ContainerOperation.Removed)
    assert changes[0].operations[0].payload == ("b", 2)


def test_notifying_map_membership_and_len_reflect_ordered_keys():
    obj = _ObservableNotifyingMapOwner(items={"a": 1, "b": 2})

    assert len(obj.items) == 2
    assert "a" in obj.items
    assert "b" in obj.items
    assert "c" not in obj.items


def test_notifying_map_getitem_missing_key_raises_keyerror():
    obj = _ObservableNotifyingMapOwner(items={"a": 1})

    with pytest.raises(KeyError):
        _ = obj.items["missing"]


def test_notifying_map_add_duplicate_key_raises_keyerror():
    obj = _ObservableNotifyingMapOwner(items={"a": 1})

    with pytest.raises(KeyError):
        obj.items.add("a", 2)


def test_notifying_map_add_before_missing_key_raises_keyerror():
    obj = _ObservableNotifyingMapOwner(items={"a": 1})

    with pytest.raises(KeyError):
        obj.items.add("b", 2, before="missing")


def test_notifying_map_move_missing_or_same_key_behaves_as_contract():
    obj = _ObservableNotifyingMapOwner(items={"a": 1, "b": 2})

    with pytest.raises(KeyError):
        obj.items.move("missing", None)

    with pytest.raises(KeyError):
        obj.items.move("a", "missing")

    order_before = list(obj.items)
    obj.items.move("a", "a")
    assert list(obj.items) == order_before


def test_notifying_map_batch_buffer_is_shared_across_mutations():
    obj = _ObservableNotifyingMapOwner(items={})
    changes = []
    observe(obj, "items", changes.append)

    with obj.items.batched_notifications():
        obj.items.add("a", 1)
        obj.items.add("b", 2, before="a")
        obj.items.move("b", None)

    assert len(changes) == 1
    assert isinstance(changes[0], ContainerChange)
    assert list(changes[0].newvalue) == ["a", "b"]
    assert len(changes[0].operations) == 3


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
