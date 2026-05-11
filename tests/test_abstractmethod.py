# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for abstractmethod support in AtorsMeta."""

from abc import abstractmethod

import pytest

from ators import Ators, member

# -------------------------------------------------------------------------------------
# A. Class creation tracking
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
# B. Inheritance resolution
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
# C. Instantiation enforcement
# -------------------------------------------------------------------------------------


def test_instantiation_fails_with_unresolved_abstract():
    class A(Ators):
        @abstractmethod
        def foo(self): ...

    with pytest.raises(TypeError, match="Can't instantiate abstract class A"):
        A()


def test_instantiation_error_includes_method_name():
    class A(Ators):
        @abstractmethod
        def foo(self): ...

    with pytest.raises(TypeError, match="foo"):
        A()


def test_instantiation_error_includes_sorted_method_names():
    class A(Ators):
        @abstractmethod
        def zoo(self): ...

        @abstractmethod
        def alpha(self): ...

    with pytest.raises(TypeError) as exc_info:
        A()
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
        Partial()


# -------------------------------------------------------------------------------------
# D. Wrapper/decorator cases
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
# E. Introspection consistency
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
# F. Regression: non-abstract classes are unchanged
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
        A(x=1)

    class B(A):
        def process(self):
            return self.x * 2

    b = B(x=3)
    assert b.process() == 6
