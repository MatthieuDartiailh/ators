# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for NotifyingList behavior."""

import pickle

import pytest

from ators import (
    Ators,
    AtorsChange,
    ListChange,
    NotifyingList,
    disable_notifications,
    enable_notifications,
    member,
    observe,
)


class _ObservableNotifyingListOwner(Ators, observable=True):
    items: NotifyingList[int] = member()


def test_notifying_list_annotation_creates_notifying_container():
    obj = _ObservableNotifyingListOwner(items=[1, 2, 3])

    assert list(obj.items) == [1, 2, 3]
    obj.items.append(4)
    assert list(obj.items) == [1, 2, 3, 4]

    with pytest.raises(TypeError):
        obj.items.append("bad")


def test_notifying_list_emits_list_change_subclass():
    obj = _ObservableNotifyingListOwner(items=[1, 2])
    changes = []

    observe(obj, "items", changes.append)
    obj.items.append(3)

    assert len(changes) == 1
    assert isinstance(changes[0], AtorsChange)
    assert isinstance(changes[0], ListChange)
    assert changes[0].object is obj
    assert changes[0].member_name == "items"
    assert list(changes[0].newvalue) == [1, 2, 3]
    assert len(changes[0].operations) == 1
    assert "Added(index=2)" in repr(changes[0].operations[0])


def test_notifying_list_context_manager_batches_operations():
    obj = _ObservableNotifyingListOwner(items=[1, 2, 3])
    changes = []

    observe(obj, "items", changes.append)

    with obj.items.batched_notifications() as items:
        items.append(4)
        items.append(5)

    assert len(changes) == 1
    assert isinstance(changes[0], ListChange)
    assert len(changes[0].operations) == 2
    assert list(obj.items) == [1, 2, 3, 4, 5]


def test_notifying_list_move_item_emits_notification():
    obj = _ObservableNotifyingListOwner(items=[1, 2, 3])
    changes = []

    observe(obj, "items", changes.append)
    obj.items.move_item(0, 2)

    assert len(changes) == 1
    assert isinstance(changes[0], ListChange)
    assert len(changes[0].operations) == 1
    assert "Moved(from_index=0, to_index=2)" in repr(changes[0].operations[0])
    assert list(obj.items) == [2, 3, 1]


def test_notifying_list_respects_parent_notification_controls():
    obj = _ObservableNotifyingListOwner(items=[1, 2])
    changes = []

    observe(obj, "items", changes.append)

    disable_notifications(obj)
    with obj.items.batched_notifications():
        obj.items.append(3)
        obj.items.append(4)

    enable_notifications(obj)
    obj.items.append(5)

    assert len(changes) == 1
    assert list(changes[0].newvalue) == [1, 2, 3, 4, 5]


def test_notifying_list_member_validates_after_pickle_restore():
    obj = _ObservableNotifyingListOwner(items=[1, 2])

    restored = pickle.loads(pickle.dumps(obj))

    assert list(restored.items) == [1, 2]
    restored.items.append(3)
    assert list(restored.items) == [1, 2, 3]

    with pytest.raises(TypeError):
        restored.items.append("bad")

    changes = []
    observe(restored, "items", changes.append)
    with restored.items.batched_notifications():
        restored.items.append(4)
        restored.items.append(5)

    assert len(changes) == 1
    assert isinstance(changes[0], ListChange)
    assert len(changes[0].operations) == 2


def test_notifying_list_forbidden_in_non_observable_class():
    """NotifyingList cannot be used in a non-observable class."""
    with pytest.raises(TypeError, match="NotifyingList can only be used in observable classes"):

        class _NonObservable(Ators, observable=False):
            items: NotifyingList[int] = member()
