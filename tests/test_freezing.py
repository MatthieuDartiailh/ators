# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Test freezing ators object"""

import pytest
from dataclasses import dataclass

from ators import Ators, freeze, is_frozen, member


@pytest.mark.parametrize(
    "frozen,should_work",
    [
        pytest.param(False, True, id="post_init_unfrozen"),
        pytest.param(True, False, id="frozen_class"),
    ],
)
def test_freezing(frozen, should_work):
    """Test freezing behavior with different class configurations"""
    if frozen:

        class A(Ators, frozen=True):
            a: int

        a = A(a=12)
    else:

        class A(Ators):
            a: int

        a = A()
        a.a = 12

    assert a.a == 12
    if not frozen:
        assert not is_frozen(a)
    else:
        assert is_frozen(a)

    if not frozen:
        freeze(a)
        assert is_frozen(a)

    with pytest.raises(TypeError) as e:
        a.a = 1
    assert "Cannot modify" in e.exconly()


def test_frozen_inheritance():
    class A(Ators, frozen=True):
        a: int

    class B(A, frozen=True):
        b: int

    with pytest.raises(TypeError) as e:

        class C(A):
            c: int

    assert "not frozen but inherit" in e.exconly()


def test_cannot_freeze_mutable_list():
    """Test that freezing fails when object contains a mutable list"""

    class A(Ators):
        items: list[int]

    a = A()
    a.items = [1, 2, 3]

    # Attempting to freeze should raise an error
    with pytest.raises(TypeError) as e:
        freeze(a)
    assert "Cannot freeze" in e.exconly()


@pytest.mark.parametrize(
    "mutable_value,type_hint",
    [
        pytest.param({"key": 1}, dict[str, int], id="dict"),
        pytest.param({"tag1", "tag2"}, set[str], id="set"),
    ],
)
def test_cannot_freeze_mutable_containers(mutable_value, type_hint):
    """Test that freezing fails with various mutable container types"""

    class A(Ators):
        value: type_hint

    a = A()
    a.value = mutable_value

    # Attempting to freeze should raise an error
    with pytest.raises(TypeError) as e:
        freeze(a)
    assert "Cannot freeze" in e.exconly()


@pytest.mark.parametrize(
    "immutable_value,type_hint",
    [
        pytest.param((1, 2, 3), tuple[int, ...], id="tuple"),
        pytest.param(frozenset({"tag1", "tag2"}), frozenset[str], id="frozenset"),
    ],
)
def test_can_freeze_immutable_containers(immutable_value, type_hint):
    """Test that freezing succeeds when object contains immutable container types"""

    class A(Ators):
        value: type_hint

    a = A()
    a.value = immutable_value

    # Freezing should succeed
    freeze(a)
    assert is_frozen(a)


def test_cannot_freeze_any_type():
    """Test that freezing fails when object has untyped (Any) members"""

    class A(Ators):
        value = member()  # No type annotation means Any

    a = A()
    a.value = [1, 2, 3]  # Mutable list

    # Attempting to freeze should raise an error
    with pytest.raises(TypeError) as e:
        freeze(a)
    assert "Cannot freeze object" in e.exconly()


@pytest.mark.parametrize(
    "value,should_freeze",
    [
        # Mutable types - should fail to freeze
        pytest.param([1, 2, 3], False, id="mutable_list"),
        pytest.param({"key": 1}, False, id="mutable_dict"),
        pytest.param({"tag1", "tag2"}, False, id="mutable_set"),
        # Immutable types - should succeed to freeze (for untyped Any)
        pytest.param(42, True, id="immutable_int"),
        pytest.param(3.14, True, id="immutable_float"),
        pytest.param("hello", True, id="immutable_str"),
        pytest.param(b"bytes", True, id="immutable_bytes"),
    ],
)
def test_freeze_any_type_with_various_values(value, should_freeze):
    """Test freezing objects with untyped (Any) members containing various types"""

    class A(Ators):
        value = member()  # No type annotation means Any

    a = A()
    a.value = value

    if should_freeze:
        # Freezing should succeed for immutable types
        freeze(a)
        assert is_frozen(a)
    else:
        # Freezing should fail for mutable types
        with pytest.raises(TypeError) as e:
            freeze(a)
        assert "Cannot freeze" in e.exconly()


@pytest.mark.parametrize(
    "create_obj,should_freeze",
    [
        pytest.param(
            lambda: (
                type("Inner", (Ators,), {"value": (int, None)}),
                lambda obj_cls: (obj_cls(value=42), True),
            ),
            True,
            id="frozen_ators",
        ),
        pytest.param(
            lambda: (
                type("Container", (), {}),
                lambda obj_cls: (obj_cls(), None),  # Will be replaced in test
            ),
            True,
            id="frozen_dataclass",
        ),
    ],
)
def test_freeze_any_type_with_complex_objects(create_obj, should_freeze):
    """Test freezing Any-typed members with frozen Ators and dataclass objects"""

    class Inner(Ators):
        value: int

    @dataclass(frozen=True)
    class FrozenData:
        value: int

    class Container(Ators):
        obj = member()  # No type annotation means Any

    # Test with frozen Ators object - should succeed
    frozen_inner = Inner(value=42)
    freeze(frozen_inner)
    container = Container(obj=frozen_inner)
    freeze(container)
    assert is_frozen(container)

    # Test with frozen dataclass - should succeed
    container2 = Container(obj=FrozenData(value=42))
    freeze(container2)
    assert is_frozen(container2)


def test_custom_mutability_callable():
    """Test registering and using a custom mutability callable in TypeMutabilityMap"""
    from ators._ators import add_type_mutability

    class CustomClass:
        """Custom class with explicit mutable/immutable tracking"""

        def __init__(self, is_mutable: bool):
            self.is_mutable = is_mutable

    class Container(Ators):
        obj = member()  # No type annotation means Any

    # Create a callable that checks the is_mutable attribute
    def check_mutability(obj):
        if isinstance(obj, CustomClass):
            return obj.is_mutable
        return True  # Default to mutable if not CustomClass

    # Register the custom callable
    add_type_mutability(CustomClass, check_mutability)

    # Test with immutable custom object - should succeed
    immutable_obj = CustomClass(is_mutable=False)
    container1 = Container(obj=immutable_obj)
    freeze(container1)
    assert is_frozen(container1)

    # Test with mutable custom object - should fail
    mutable_obj = CustomClass(is_mutable=True)
    container2 = Container(obj=mutable_obj)
    with pytest.raises(TypeError) as e:
        freeze(container2)
    assert "Cannot freeze" in e.exconly()


def test_cannot_freeze_with_mutable_nested_container():
    """Test that freezing fails with nested containers that have mutable inner types"""

    class A(Ators):
        nested: tuple[list[int], ...]

    a = A()
    a.nested = ([1, 2], [3, 4])  # Tuple of mutable lists

    # Attempting to freeze should raise an error
    with pytest.raises(TypeError) as e:
        freeze(a)
    assert "Cannot freeze" in e.exconly()


def test_frozen_object_is_immutable():
    """Test that a frozen object prevents value changes"""

    class A(Ators):
        a: int

    a = A(a=1)
    freeze(a)

    with pytest.raises(TypeError) as e:
        a.a = 2
    assert "Cannot modify" in e.exconly()
