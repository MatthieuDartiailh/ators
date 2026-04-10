# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Tests for pickling support in Ators."""

import pickle

import pytest

from ators import Ators, PicklePolicy, member

# ---------------------------------------------------------------------------
# Module-level class definitions (pickle requires a findable qualified name)
# ---------------------------------------------------------------------------

_PN = getattr(PicklePolicy, "None")


class _ScalarClass(Ators):
    x: int
    y: str
    z: float


class _PolicyAllClass(Ators, pickle_policy=PicklePolicy.All()):
    x: int
    y: int


class _PolicyNoneClass(Ators, pickle_policy=_PN()):
    x: int
    y: int


class _PolicyNoneWithDefaultClass(Ators, pickle_policy=_PN()):
    x: int = member().default(42)


class _PolicyPublicClass(Ators, pickle_policy=PicklePolicy.Public()):
    x: int
    _private: int = member(init=True)


class _PolicyPublicDunderClass(Ators, pickle_policy=PicklePolicy.Public()):
    x: int
    _hidden: int = member(init=True)


class _MemberOverrideTrueClass(Ators, pickle_policy=_PN()):
    x: int = member().pickle(True)
    y: int


class _MemberOverrideFalseClass(Ators, pickle_policy=PicklePolicy.All()):
    x: int = member().pickle(False)
    y: int


class _MemberOverrideTruePublicClass(Ators, pickle_policy=PicklePolicy.Public()):
    x: int
    _private: int = member(init=True).pickle(True)


class _UnknownKeyClass(Ators):
    x: int


class _SetStateClass(Ators):
    x: int
    y: str


class _SetStateBypassClass(Ators):
    x: int


class _ListClass(Ators):
    items: list[int]


class _SetClass(Ators):
    s: set[int]


class _DictClass(Ators):
    mapping: dict[str, int]


class _InheritanceBase(Ators):
    x: int


class _InheritanceChild(_InheritanceBase):
    y: str


class _PolicyNoneBase(Ators, pickle_policy=_PN()):
    x: int


class _PolicyNoneChild(_PolicyNoneBase):
    y: int


class _PolicyNoneChildOverride(_PolicyNoneBase, pickle_policy=PicklePolicy.All()):
    y: int


class _Getnewargs(Ators):
    x: int


class _MultiProtocol(Ators):
    x: int
    y: str


# ---------------------------------------------------------------------------
# PicklePolicy enum existence and class policy wiring
# ---------------------------------------------------------------------------


def test_pickle_policy_variants_exist():
    """All three policy variants must be accessible."""
    assert PicklePolicy.All() is not None
    assert PicklePolicy.Public() is not None
    assert getattr(PicklePolicy, "None")() is not None


def test_default_policy_is_all():
    """Default policy should be 'all' (match Python's default pickle behaviour)."""
    a = _ScalarClass(x=1, y="hi", z=1.0)
    state = a.__getstate__()
    assert set(state.keys()) == {"x", "y", "z"}


def test_explicit_policy_none():
    """pickle_policy=None excludes every member unless explicitly overridden."""
    a = _PolicyNoneClass(x=1, y=2)
    assert a.__getstate__() == {}


def test_explicit_policy_all():
    """pickle_policy=All includes every member (same as default)."""
    a = _PolicyAllClass(x=10, y=20)
    state = a.__getstate__()
    assert set(state.keys()) == {"x", "y"}


def test_explicit_policy_public():
    """pickle_policy=Public includes only non-underscore members."""
    a = _PolicyPublicClass(x=10, _private=99)
    state = a.__getstate__()
    assert "x" in state
    assert "_private" not in state


def test_explicit_policy_public_excludes_double_underscore():
    """Members starting with __ should also be excluded under 'public' policy."""
    a = _PolicyPublicDunderClass(x=5, _hidden=7)
    state = a.__getstate__()
    assert "x" in state
    assert not any(k.startswith("_") for k in state)


# ---------------------------------------------------------------------------
# Member-level pickle override beats class policy
# ---------------------------------------------------------------------------


def test_member_explicit_true_overrides_policy_none():
    """member().pickle(True) forces inclusion even when class policy is None."""
    a = _MemberOverrideTrueClass(x=1, y=2)
    state = a.__getstate__()
    assert "x" in state
    assert "y" not in state


def test_member_explicit_false_overrides_policy_all():
    """member().pickle(False) forces exclusion even when class policy is All."""
    a = _MemberOverrideFalseClass(x=1, y=2)
    state = a.__getstate__()
    assert "y" in state
    assert "x" not in state


def test_member_explicit_true_overrides_policy_public():
    """member().pickle(True) forces a private member to be included under Public."""
    a = _MemberOverrideTruePublicClass(x=10, _private=99)
    state = a.__getstate__()
    assert "x" in state
    assert "_private" in state


# ---------------------------------------------------------------------------
# __setstate__ - unknown key raises
# ---------------------------------------------------------------------------


def test_unknown_key_raises():
    """__setstate__ must raise KeyError for keys that are not known members."""
    a = _UnknownKeyClass(x=1)
    with pytest.raises(KeyError, match="unknown_key"):
        a.__setstate__({"unknown_key": 42})


def test_setstate_valid_keys_restore():
    """__setstate__ writes values directly to slots."""
    a = _SetStateClass(x=1, y="hello")
    a.__setstate__({"x": 99, "y": "world"})
    assert a.x == 99
    assert a.y == "world"


def test_setstate_bypasses_validation():
    """__setstate__ must NOT validate - invalid types should be accepted."""
    a = _SetStateBypassClass(x=1)
    # Assign a string to an int member - would fail through normal setattr
    a.__setstate__({"x": "not an int"})
    assert a.x == "not an int"


# ---------------------------------------------------------------------------
# Roundtrip: scalar members
# ---------------------------------------------------------------------------


def test_roundtrip_scalar_members():
    """Pickle/unpickle roundtrip preserves scalar member values."""
    a = _ScalarClass(x=1, y="hello", z=3.14)
    a2 = pickle.loads(pickle.dumps(a))
    assert a2.x == 1
    assert a2.y == "hello"
    assert a2.z == pytest.approx(3.14)


def test_roundtrip_unset_members_not_in_state():
    """Members that have never been set should not appear in __getstate__."""
    a = _ScalarClass(x=5, y="hi", z=1.0)
    state = a.__getstate__()
    assert "x" in state


def test_roundtrip_default_policy_excludes_none_members():
    """Excluded-by-policy members should not be set after unpickling."""
    a = _PolicyNoneWithDefaultClass(x=99)
    a2 = pickle.loads(pickle.dumps(a))
    # x was not pickled, so it falls back to its default
    assert a2.x == 42


# ---------------------------------------------------------------------------
# Roundtrip: typed list members (AtorsList)
# ---------------------------------------------------------------------------


def test_roundtrip_list_member():
    """Pickle/unpickle roundtrip preserves list content."""
    a = _ListClass(items=[1, 2, 3])
    a2 = pickle.loads(pickle.dumps(a))
    assert list(a2.items) == [1, 2, 3]


def test_list_member_validates_after_restore():
    """After unpickling, a typed list must still enforce its validator."""
    a = _ListClass(items=[10, 20])
    a2 = pickle.loads(pickle.dumps(a))
    with pytest.raises(TypeError):
        a2.items.append("not an int")


def test_list_member_append_after_restore():
    """After unpickling, valid appends to a typed list should work."""
    a = _ListClass(items=[1])
    a2 = pickle.loads(pickle.dumps(a))
    a2.items.append(2)
    assert list(a2.items) == [1, 2]


def test_container_restored_before_slot_assignment():
    """The container's metadata must be rebound BEFORE it is stored in the slot.

    Verified indirectly: after unpickling, mutations that go through the
    validator (e.g. append) work correctly, confirming that the validator
    and owner reference were set before the container was placed in the slot.
    """
    a = _ListClass(items=[5])
    a2 = pickle.loads(pickle.dumps(a))
    a2.items.append(6)
    assert list(a2.items) == [5, 6]


# ---------------------------------------------------------------------------
# Roundtrip: typed set members (AtorsSet)
# ---------------------------------------------------------------------------


def test_roundtrip_set_member():
    """Pickle/unpickle roundtrip preserves set content."""
    a = _SetClass(s={1, 2, 3})
    a2 = pickle.loads(pickle.dumps(a))
    assert set(a2.s) == {1, 2, 3}


def test_set_member_validates_after_restore():
    """After unpickling, a typed set must still enforce its validator."""
    a = _SetClass(s={1})
    a2 = pickle.loads(pickle.dumps(a))
    with pytest.raises(TypeError):
        a2.s.add("not an int")


def test_set_member_add_after_restore():
    """After unpickling, valid adds to a typed set should work."""
    a = _SetClass(s={1, 2})
    a2 = pickle.loads(pickle.dumps(a))
    a2.s.add(3)
    assert 3 in a2.s


# ---------------------------------------------------------------------------
# Roundtrip: typed dict members (AtorsDict)
# ---------------------------------------------------------------------------


def test_roundtrip_dict_member():
    """Pickle/unpickle roundtrip preserves dict content."""
    a = _DictClass(mapping={"a": 1, "b": 2})
    a2 = pickle.loads(pickle.dumps(a))
    assert dict(a2.mapping) == {"a": 1, "b": 2}


def test_dict_member_validates_after_restore():
    """After unpickling, a typed dict must still enforce its validators."""
    a = _DictClass(mapping={"x": 10})
    a2 = pickle.loads(pickle.dumps(a))
    with pytest.raises(TypeError):
        a2.mapping[123] = 99  # int key not allowed


def test_dict_member_setitem_after_restore():
    """After unpickling, valid setitem on a typed dict should work."""
    a = _DictClass(mapping={"a": 1})
    a2 = pickle.loads(pickle.dumps(a))
    a2.mapping["b"] = 2
    assert a2.mapping["b"] == 2


# ---------------------------------------------------------------------------
# Inheritance
# ---------------------------------------------------------------------------


def test_roundtrip_inherited_members():
    """Pickle/unpickle roundtrip works across inheritance."""
    c = _InheritanceChild(x=7, y="child")
    c2 = pickle.loads(pickle.dumps(c))
    assert c2.x == 7
    assert c2.y == "child"


def test_policy_inherited_by_default():
    """A subclass inherits the parent's pickle_policy when not overridden."""
    c = _PolicyNoneChild(x=1, y=2)
    state = c.__getstate__()
    # Both x and y should be excluded (policy=None inherited from parent)
    assert state == {}


def test_policy_can_be_overridden_in_subclass():
    """A subclass can override the parent's pickle_policy."""
    c = _PolicyNoneChildOverride(x=1, y=2)
    state = c.__getstate__()
    # Both x and y should be included (policy=All overridden)
    assert set(state.keys()) == {"x", "y"}


# ---------------------------------------------------------------------------
# __getnewargs__ / __getnewargs_ex__ not required
# ---------------------------------------------------------------------------


def test_getnewargs_not_required():
    """Verify that __getnewargs__ returns () and pickle still works correctly.

    AtorsBase.__new__ does not require constructor arguments; pickle uses the
    zero-argument __new__ path automatically.  No __getnewargs__ implementation
    is needed beyond the empty-tuple default provided by Ators.
    """
    a = _GetnewArgs(x=5)
    assert a.__getnewargs__() == ()
    a2 = pickle.loads(pickle.dumps(a))
    assert a2.x == 5


class _GetnewArgs(Ators):
    x: int


# ---------------------------------------------------------------------------
# Multiple pickle protocols
# ---------------------------------------------------------------------------


@pytest.mark.parametrize("protocol", range(2, pickle.HIGHEST_PROTOCOL + 1))
def test_roundtrip_multiple_protocols(protocol):
    """Roundtrip works for all supported pickle protocols."""
    a = _MultiProtocol(x=protocol, y=f"proto{protocol}")
    a2 = pickle.loads(pickle.dumps(a, protocol=protocol))
    assert a2.x == protocol
    assert a2.y == f"proto{protocol}"
