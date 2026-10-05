# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for metaclass behavior: general functionality and abstract method support."""

from abc import abstractmethod

import pytest

from ators import Ators, member

# -------------------------------------------------------------------------------------
# A. Basic metaclass functionality
# -------------------------------------------------------------------------------------


def test_metaclass_with_abstract_method():
    """Test metaclass handling of abstract methods."""

    # Ators already handles abstract methods via its metaclass
    class Base(Ators):
        @abstractmethod
        def process(self):
            pass

    # Cannot instantiate abstract class
    with pytest.raises(TypeError):
        Base()  # type: ignore

    class Concrete(Base):
        def process(self):
            return "done"

    # Concrete implementation can be instantiated
    obj = Concrete()
    assert obj.process() == "done"


def test_metaclass_mro_with_multiple_inheritance():
    """Test metaclass MRO (Method Resolution Order) with multiple inheritance."""

    class Mixin1:
        value1 = "mixin1"

    class Mixin2:
        value2 = "mixin2"

    class Combined(Mixin1, Mixin2, Ators):
        data: int = member()

    obj = Combined()
    assert hasattr(obj, "value1")
    assert hasattr(obj, "value2")
    assert obj.value1 == "mixin1"
    assert obj.value2 == "mixin2"


def test_metaclass_slot_creation_with_inheritance():
    """Test that metaclass properly creates slots in inheritance hierarchy."""

    class Base(Ators):
        x: int = member()

    class Derived(Base):
        y: str = member()

    obj = Derived()
    obj.x = 42
    obj.y = "text"

    assert obj.x == 42
    assert obj.y == "text"

    # Cannot add new attributes (slots prevent it)
    with pytest.raises(AttributeError):
        obj.new_attr = "value"


def test_metaclass_handles_property_descriptors():
    """Test metaclass interaction with property descriptors."""

    class PropClass(Ators):
        _internal: int = member()

        @property
        def value(self):
            return self._internal * 2

    obj = PropClass()
    obj._internal = 21
    assert obj.value == 42


def test_metaclass_with_classmethod():
    """Test metaclass handling of classmethods."""

    class WithClassMethod(Ators):
        data: int = member()

        @classmethod
        def create(cls, value):
            obj = cls()
            obj.data = value
            return obj

    obj = WithClassMethod.create(99)
    assert obj.data == 99


def test_metaclass_with_staticmethod():
    """Test metaclass handling of staticmethods."""

    class WithStaticMethod(Ators):
        data: int = member()

        @staticmethod
        def process(x: int) -> int:
            return x * 2

    assert WithStaticMethod.process(21) == 42

    obj = WithStaticMethod()
    obj.data = 10
    assert obj.data == 10


# -------------------------------------------------------------------------------------
# B. Abstract method tracking
# -------------------------------------------------------------------------------------


def test_single_abstract_method_is_tracked():
    class A(Ators):
        @abstractmethod
        def foo(self): ...

    assert A.__abstractmethods__ == frozenset({"foo"})


def test_no_abstract_methods_gives_empty_frozenset():
    class A(Ators):
        def foo(self):
            return 1

    assert A.__abstractmethods__ == frozenset()


def test_multiple_abstract_methods_are_tracked():
    class A(Ators):
        @abstractmethod
        def foo(self): ...

        @abstractmethod
        def bar(self): ...

    assert A.__abstractmethods__ == frozenset({"foo", "bar"})


# -------------------------------------------------------------------------------------
# C. Abstract method inheritance resolution
# -------------------------------------------------------------------------------------


def test_abstract_method_removed_when_overridden_concretely():
    class Base(Ators):
        @abstractmethod
        def foo(self): ...

    class Sub(Base):
        def foo(self):
            return 42

    assert Sub.__abstractmethods__ == frozenset()


def test_abstract_method_remains_when_not_overridden():
    class Base(Ators):
        @abstractmethod
        def foo(self): ...

    class Sub(Base):
        pass

    assert Sub.__abstractmethods__ == frozenset({"foo"})


def test_abstract_overriding_abstract_stays_abstract():
    class Base(Ators):
        @abstractmethod
        def foo(self): ...

    class Sub(Base):
        @abstractmethod
        def foo(self): ...

    assert Sub.__abstractmethods__ == frozenset({"foo"})


def test_new_abstract_method_added_in_subclass():
    class Base(Ators):
        def foo(self):
            return 1

    class Sub(Base):
        @abstractmethod
        def bar(self): ...

    assert Sub.__abstractmethods__ == frozenset({"bar"})


def test_re_abstracting_concrete_base_method():
    class Base(Ators):
        def foo(self):
            return 1

    class Sub(Base):
        @abstractmethod
        def foo(self): ...

    assert Sub.__abstractmethods__ == frozenset({"foo"})


def test_multiple_inheritance_union_of_abstracts():
    class A(Ators):
        @abstractmethod
        def a1(self): ...

        @abstractmethod
        def a2(self): ...

    class B(Ators):
        @abstractmethod
        def b1(self): ...

    class C(A, B):
        pass

    assert C.__abstractmethods__ == frozenset({"a1", "a2", "b1"})


def test_multiple_inheritance_partial_override():
    class A(Ators):
        @abstractmethod
        def a1(self): ...

        @abstractmethod
        def a2(self): ...

    class B(Ators):
        @abstractmethod
        def b1(self): ...

    class C(A, B):
        def a1(self):
            return 1

    assert C.__abstractmethods__ == frozenset({"a2", "b1"})


def test_multiple_inheritance_all_overridden():
    class A(Ators):
        @abstractmethod
        def a1(self): ...

    class B(Ators):
        @abstractmethod
        def b1(self): ...

    class C(A, B):
        def a1(self):
            return 1

        def b1(self):
            return 2

    assert C.__abstractmethods__ == frozenset()


# -------------------------------------------------------------------------------------
# D. Instantiation enforcement
# -------------------------------------------------------------------------------------


def test_instantiation_fails_with_unresolved_abstract():
    class A(Ators):
        @abstractmethod
        def foo(self): ...

    with pytest.raises(TypeError, match="Can't instantiate abstract class A"):
        A()  # type: ignore


def test_instantiation_error_includes_method_name():
    class A(Ators):
        @abstractmethod
        def foo(self): ...

    with pytest.raises(TypeError, match="foo"):
        A()  # type: ignore


def test_instantiation_error_includes_sorted_method_names():
    class A(Ators):
        @abstractmethod
        def zoo(self): ...

        @abstractmethod
        def alpha(self): ...

    with pytest.raises(TypeError) as exc_info:
        A()  # type: ignore
    assert "alpha, zoo" in str(exc_info.value)


def test_instantiation_succeeds_when_all_abstracts_implemented():
    class Base(Ators):
        @abstractmethod
        def foo(self): ...

    class Concrete(Base):
        def foo(self):
            return 42

    obj = Concrete()
    assert obj.foo() == 42


def test_instantiation_fails_if_any_abstract_remains():
    class Base(Ators):
        @abstractmethod
        def foo(self): ...

        @abstractmethod
        def bar(self): ...

    class Partial(Base):
        def foo(self):
            return 1

    with pytest.raises(TypeError, match="bar"):
        Partial()  # type: ignore


# -------------------------------------------------------------------------------------
# E. Decorator cases
# -------------------------------------------------------------------------------------


def test_classmethod_abstractmethod_detected():
    class A(Ators):
        @classmethod
        @abstractmethod
        def foo(cls): ...

    assert "foo" in A.__abstractmethods__


def test_classmethod_abstractmethod_removed_by_concrete_override():
    class Base(Ators):
        @classmethod
        @abstractmethod
        def foo(cls): ...

    class Sub(Base):
        @classmethod
        def foo(cls):
            return 42

    assert Sub.__abstractmethods__ == frozenset()


def test_staticmethod_abstractmethod_detected():
    class A(Ators):
        @staticmethod
        @abstractmethod
        def foo(): ...

    assert "foo" in A.__abstractmethods__


def test_staticmethod_abstractmethod_removed_by_concrete_override():
    class Base(Ators):
        @staticmethod
        @abstractmethod
        def foo(): ...

    class Sub(Base):
        @staticmethod
        def foo():
            return 42

    assert Sub.__abstractmethods__ == frozenset()


def test_property_abstractmethod_detected():
    class A(Ators):
        @property
        @abstractmethod
        def value(self): ...

    assert "value" in A.__abstractmethods__


def test_property_abstractmethod_removed_by_concrete_override():
    class Base(Ators):
        @property
        @abstractmethod
        def value(self): ...

    class Sub(Base):
        @property
        def value(self):
            return 42

    assert Sub.__abstractmethods__ == frozenset()


# -------------------------------------------------------------------------------------
# F. Introspection consistency
# -------------------------------------------------------------------------------------


def test_abstractmethods_is_frozenset():
    class A(Ators):
        @abstractmethod
        def foo(self): ...

    assert isinstance(A.__abstractmethods__, frozenset)


def test_abstractmethods_empty_is_frozenset():
    class A(Ators):
        def foo(self):
            return 1

    assert isinstance(A.__abstractmethods__, frozenset)
    assert len(A.__abstractmethods__) == 0


def test_abstractmethods_deep_inheritance_chain():
    class L1(Ators):
        @abstractmethod
        def m1(self): ...

    class L2(L1):
        @abstractmethod
        def m2(self): ...

    class L3(L2):
        def m1(self):
            return 1

    class L4(L3):
        def m2(self):
            return 2

    assert L1.__abstractmethods__ == frozenset({"m1"})
    assert L2.__abstractmethods__ == frozenset({"m1", "m2"})
    assert L3.__abstractmethods__ == frozenset({"m2"})
    assert L4.__abstractmethods__ == frozenset()


# -------------------------------------------------------------------------------------
# G. Regressions: non-abstract classes and mixed cases
# -------------------------------------------------------------------------------------


def test_non_abstract_class_instantiates_normally():
    class A(Ators):
        x: int = member()

    a = A(x=5)
    assert a.x == 5


def test_abstract_class_with_members_tracks_both():
    class A(Ators):
        x: int = member()

        @abstractmethod
        def process(self): ...

    assert A.__abstractmethods__ == frozenset({"process"})
    with pytest.raises(TypeError):
        A(x=1)  # type: ignore

    class B(A):
        def process(self):
            return self.x * 2

    b = B(x=3)
    assert b.process() == 6
