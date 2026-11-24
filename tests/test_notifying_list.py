# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for NotifyingList behavior."""

import pickle
from typing import ForwardRef

import pytest

from ators import (
    Ators,
    AtorsChange,
    ContainerChange,
    ContainerOperation,
    NotifyingList,
    disable_notifications,
    enable_notifications,
    member,
    observe,
)


class _ObservableNotifyingListOwner(Ators, observable=True):
    items: NotifyingList[int] = member()


class _BareObservableNotifyingListOwner(Ators, observable=True):
    items: NotifyingList = member()


class _CoercedNotifyingListOwner(Ators, observable=True):
    items: NotifyingList[int] = member().coerce_init()


class _ForwardNotifyingListOwner(Ators, observable=True):
    items: ForwardRef("NotifyingList[int]") = member()


class _GenericNotifyingListOwner[T](Ators, observable=True):
    items: NotifyingList[T] = member()


def test_bare_notifying_list_annotation_in_valid_position():
    obj = _BareObservableNotifyingListOwner(items=[1, "two", 3])

    assert list(obj.items) == [1, "two", 3]
    obj.items.append("four")
    assert list(obj.items) == [1, "two", 3, "four"]


def test_notifying_list_insert_extend_remove_pop_and_clear():
    obj = _ObservableNotifyingListOwner(items=[1, 2])

    obj.items.insert(1, 10)
    obj.items.extend([20, 30])
    assert list(obj.items) == [1, 10, 2, 20, 30]

    obj.items.remove(10)
    assert list(obj.items) == [1, 2, 20, 30]

    assert obj.items.pop(-1) == 30
    assert obj.items.pop(-2) == 2
    obj.items.clear()
    assert list(obj.items) == []

    with pytest.raises(ValueError):
        obj.items.remove(999)
    with pytest.raises(IndexError):
        obj.items.pop(0)
    with pytest.raises(IndexError):
        obj.items.pop(100)


def test_notifying_list_iadd_imul_and_validation_errors():
    obj = _ObservableNotifyingListOwner(items=[1, 2])

    obj.items += [3, 4]
    assert list(obj.items) == [1, 2, 3, 4]

    obj.items *= 2
    assert list(obj.items) == [1, 2, 3, 4, 1, 2, 3, 4]

    obj.items *= 0
    assert list(obj.items) == []

    with pytest.raises(TypeError):
        obj.items += ["bad"]


def test_notifying_list_move_and_index_error_paths():
    obj = _ObservableNotifyingListOwner(items=[1, 2, 3])

    obj.items.move_item(0, 0)
    assert list(obj.items) == [1, 2, 3]
    with pytest.raises(IndexError):
        obj.items.move_item(100, 0)
    with pytest.raises(IndexError):
        obj.items.move_item(0, 100)

    with pytest.raises(IndexError):
        obj.items[100] = 10
    with pytest.raises(IndexError):
        del obj.items[100]


def test_container_operation_reprs_and_payload_properties():
    obj = _ObservableNotifyingListOwner(items=[1, 2, 3])

    changes = []
    observe(obj, "items", changes.append)
    obj.items[1] = 20
    obj.items.move_item(0, 2)
    del obj.items[2:]

    operations = [operation for change in changes for operation in change.operations]

    assert repr(operations[0]).startswith("Replaced(index=")
    assert operations[0].index == 1
    assert operations[0].old_value == 2
    assert operations[0].new_value == 20
    assert repr(operations[1]).startswith("Moved(from_index=")
    assert operations[1].from_index == 0
    assert operations[1].to_index == 2
    assert operations[1].payload == 1
    assert repr(operations[2]).startswith("Removed(old_index=")
    assert operations[2].old_index == 2
    assert operations[2].payload == 1


def test_forward_and_generic_notifying_list_annotations():
    forward = _ForwardNotifyingListOwner(items=[1, 2])
    assert list(forward.items) == [1, 2]
    forward.items.append(3)
    assert list(forward.items) == [1, 2, 3]

    generic = _GenericNotifyingListOwner[int](items=[1, 2])
    assert list(generic.items) == [1, 2]
    generic.items.append(3)
    assert list(generic.items) == [1, 2, 3]
    with pytest.raises(TypeError):
        generic.items.append("bad")


def test_coerced_notifying_list_member():
    obj = _CoercedNotifyingListOwner(items=["1", "2"])

    assert list(obj.items) == [1, 2]
    with pytest.raises(TypeError):
        obj.items.append("3")


def test_notifying_list_direct_batch_api_and_assignment_clone():
    obj = _ObservableNotifyingListOwner(items=[1])
    changes = []
    observe(obj, "items", changes.append)

    obj.items.begin_batch_notifications()
    obj.items.append(2)
    obj.items.append(3)
    obj.items.end_batch()

    assert len(changes) == 1
    assert len(changes[0].operations) == 2

    other = _ObservableNotifyingListOwner(items=obj.items)
    assert list(other.items) == [1, 2, 3]
    assert other.items is not obj.items


def test_notifying_list_observer_error_is_grouped():
    obj = _ObservableNotifyingListOwner(items=[1])

    def fail(change):
        raise RuntimeError("observer failure")

    observe(obj, "items", fail)
    with pytest.raises(BaseExceptionGroup):
        obj.items.append(2)


def test_notifying_list_annotation_creates_notifying_container():
    obj = _ObservableNotifyingListOwner(items=[1, 2, 3])

    assert list(obj.items) == [1, 2, 3]
    obj.items.append(4)
    assert list(obj.items) == [1, 2, 3, 4]

    with pytest.raises(TypeError):
        obj.items.append("bad")


def test_notifying_list_emits_container_change():
    obj = _ObservableNotifyingListOwner(items=[1, 2])
    changes = []

    observe(obj, "items", changes.append)
    obj.items.append(3)

    assert len(changes) == 1
    assert isinstance(changes[0], AtorsChange)
    assert type(changes[0]) is ContainerChange
    assert isinstance(changes[0], ContainerChange)
    assert changes[0].object is obj
    assert changes[0].member_name == "items"
    assert list(changes[0].newvalue) == [1, 2, 3]
    assert len(changes[0].operations) == 1
    operation = changes[0].operations[0]
    assert isinstance(operation, ContainerOperation.Added)
    assert operation.index == 2
    assert operation.payload == 3


def test_notifying_list_setitem_emits_replaced_operation():
    obj = _ObservableNotifyingListOwner(items=[1, 2, 3])
    changes = []

    observe(obj, "items", changes.append)
    obj.items[1] = 20

    assert list(obj.items) == [1, 20, 3]
    assert len(changes) == 1
    assert type(changes[0]) is ContainerChange
    assert len(changes[0].operations) == 1
    operation = changes[0].operations[0]
    assert isinstance(operation, ContainerOperation.Replaced)
    assert operation.index == 1
    assert operation.old_value == 2
    assert operation.new_value == 20


def test_notifying_list_slice_setitem_replaces_in_place_when_lengths_match():
    obj = _ObservableNotifyingListOwner(items=[1, 2, 3, 4])
    changes = []

    observe(obj, "items", changes.append)
    obj.items[1:3] = [10, 11]

    assert list(obj.items) == [1, 10, 11, 4]
    assert len(changes) == 1
    assert type(changes[0]) is ContainerChange
    assert len(changes[0].operations) == 1
    operation = changes[0].operations[0]
    assert isinstance(operation, ContainerOperation.Replaced)
    assert operation.index == 1
    assert operation.old_value == (2, 3)
    assert operation.new_value == (10, 11)


def test_notifying_list_slice_setitem_emits_atomic_changes():
    obj = _ObservableNotifyingListOwner(items=[1, 2, 3, 4])
    changes = []

    observe(obj, "items", changes.append)
    obj.items[1:3] = [10, 11, 12]

    assert list(obj.items) == [1, 10, 11, 12, 4]
    assert len(changes) == 1
    assert type(changes[0]) is ContainerChange
    assert len(changes[0].operations) == 1
    operation = changes[0].operations[0]
    assert isinstance(operation, ContainerOperation.Replaced)
    assert operation.index == 1
    assert operation.old_value == (2, 3)
    assert operation.new_value == (10, 11, 12)


def test_notifying_list_slice_delitem_emits_atomic_changes():
    obj = _ObservableNotifyingListOwner(items=[1, 2, 3, 4])
    changes = []

    observe(obj, "items", changes.append)
    del obj.items[1:3]

    assert list(obj.items) == [1, 4]
    assert len(changes) == 1
    assert type(changes[0]) is ContainerChange
    assert len(changes[0].operations) == 2
    operations = changes[0].operations
    assert isinstance(operations[0], ContainerOperation.Removed)
    assert isinstance(operations[1], ContainerOperation.Removed)
    assert operations[0].old_index == 1
    assert operations[0].payload == 2
    assert operations[1].old_index == 2
    assert operations[1].payload == 3


def test_notifying_list_context_manager_batches_operations():
    obj = _ObservableNotifyingListOwner(items=[1, 2, 3])
    changes = []

    observe(obj, "items", changes.append)

    with obj.items.batched_notifications():
        obj.items.append(4)
        obj.items.append(5)

    assert len(changes) == 1
    assert type(changes[0]) is ContainerChange
    assert len(changes[0].operations) == 2
    assert list(obj.items) == [1, 2, 3, 4, 5]


def test_notifying_list_batch_buffer_is_shared_across_mutations():
    obj = _ObservableNotifyingListOwner(items=[1])
    changes = []
    observe(obj, "items", changes.append)

    with obj.items.batched_notifications():
        obj.items.append(2)
        obj.items.insert(0, 0)
        obj.items.remove(1)

    assert len(changes) == 1
    assert type(changes[0]) is ContainerChange
    assert list(changes[0].newvalue) == [0, 2]
    assert len(changes[0].operations) == 3


def test_notifying_list_move_item_emits_notification():
    obj = _ObservableNotifyingListOwner(items=[1, 2, 3])
    changes = []

    observe(obj, "items", changes.append)
    obj.items.move_item(0, 2)

    assert len(changes) == 1
    assert type(changes[0]) is ContainerChange
    assert len(changes[0].operations) == 1
    operation = changes[0].operations[0]
    assert isinstance(operation, ContainerOperation.Moved)
    assert operation.from_index == 0
    assert operation.to_index == 2
    assert operation.payload == 1
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
    assert type(changes[0]) is ContainerChange
    assert len(changes[0].operations) == 2


def test_notifying_list_forbidden_in_non_observable_class():
    """NotifyingList cannot be used in a non-observable class."""
    with pytest.raises(TypeError) as exc_info:

        class _NonObservable(Ators, observable=False):
            items: NotifyingList[int] = member()

    assert "NotifyingList can only be used in observable classes" in str(
        exc_info.value.__cause__.__cause__
    )


def test_notifying_list_forbidden_inside_list():
    """NotifyingList cannot be nested inside a list annotation."""
    with pytest.raises(TypeError) as exc_info:

        class _Nested(Ators, observable=True):
            items: list[NotifyingList[int]] = member()

    assert "NotifyingList can only be used as a top-level annotation" in str(
        exc_info.value.__cause__.__cause__
    )


def test_notifying_list_forbidden_inside_dict_value():
    """NotifyingList cannot be nested as a dict value annotation."""
    with pytest.raises(TypeError) as exc_info:

        class _Nested(Ators, observable=True):
            mapping: dict[str, NotifyingList[int]] = member()

    assert "NotifyingList can only be used as a top-level annotation" in str(
        exc_info.value.__cause__.__cause__
    )


def test_notifying_list_forbidden_inside_tuple():
    """NotifyingList cannot be nested inside a tuple annotation."""
    with pytest.raises(TypeError) as exc_info:

        class _Nested(Ators, observable=True):
            items: tuple[NotifyingList[int], ...] = member()

    assert "NotifyingList can only be used as a top-level annotation" in str(
        exc_info.value.__cause__.__cause__
    )


def test_bare_notifying_list_forbidden_inside_list():
    with pytest.raises(TypeError) as exc_info:

        class _Nested(Ators, observable=True):
            items: list[NotifyingList] = member()

    assert "NotifyingList can only be used as a top-level annotation" in str(
        exc_info.value.__cause__.__cause__
    )
