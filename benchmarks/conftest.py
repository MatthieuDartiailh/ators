# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Shared fixtures and base classes for benchmarks."""

from typing import Any, Literal, Optional

import pytest

from ators import Ators, freeze, member
from ators.behaviors import ValueValidator
from benchmarks.shared.runtime import atom_benchmarks_available

ATOM_AVAILABLE = atom_benchmarks_available()


# ============================================================================
# Custom Classes
# ============================================================================


class CustomClass:
    """Custom class for forward reference testing."""

    pass


class ForwardRefClass:
    """Class used with forward reference."""

    pass


# ============================================================================
# Python Reference Implementations
# ============================================================================


class PySlottedClass:
    """Reference implementation using Python __slots__."""

    __slots__ = ("_field",)

    def __init__(self):
        self._field = 0


class PyPlainClass:
    """Python class without __slots__ for comparison."""

    def __init__(self):
        self._field = 0


# ============================================================================
# Untyped Implementations
# ============================================================================


class AtorsUntypedClass(Ators):
    """Ators class with untyped field (Any annotation)."""

    field: Any = member()


class AtorsValidatedClass(Ators):
    """Ators class with comprehensive type validators."""

    # Basic type validators
    int_field: int = member()
    float_field: float = member()
    str_field: str = member()
    bool_field: bool = member()
    complex_field: complex = member()
    bytes_field: bytes = member()

    # Container validators (typed)
    set_field: set[int] = member()
    dict_field: dict[str, int] = member()
    list_field: list[int] = member()
    tuple_field: tuple[int, ...] = member()
    fixed_tuple_field: tuple[int, int, str] = member()
    frozen_set_field: frozenset[int] = member()

    # Optional type
    optional_int_field: Optional[int] = member()

    # Enum-like using Literal
    enum_like_field: Literal[1, 2, 3] = member()

    # Constrained int using enum values
    constrained_int_field: int = member().append_value_validator(
        ValueValidator.Values({0, 25, 50, 75, 100})
    )

    # Custom class validation
    custom_class_field: CustomClass = member()


class AtorsForwardRefClass(Ators):
    """Ators class using forward references."""

    int_field: int = member()
    custom_field: ForwardRefClass = member()


# ============================================================================
# Atom Implementations (if available)
# ============================================================================


if ATOM_AVAILABLE:
    from atom.api import (
        Atom,
        Bool,
        Bytes,
        Dict,
        Enum,
        FixedTuple,
        Float,
        Int,
        List,
        Set,
        Str,
        Tuple,
        Typed,
        Value,
    )

    class AtomUntypedClass(Atom):
        """Atom class with untyped field (Value descriptor)."""

        field = Value()

    class AtomSimpleTypes(Atom):
        """Atom class with basic type validators."""

        int_field = Int()
        float_field = Float()
        str_field = Str()
        bool_field = Bool()

    class AtomContainerTypes(Atom):
        """Atom class with container type validators."""

        set_field = Set()
        dict_field = Dict()

    class AtomValidatedClass(Atom):
        """Atom class with comprehensive type validators matching AtorsValidatedClass."""

        # Basic type validators
        int_field = Int()
        float_field = Float()
        str_field = Str()
        bool_field = Bool()
        bytes_field = Bytes()

        # Container validators (typed)
        list_field = List(Int())
        set_field = Set(Int())
        dict_field = Dict(Str(), Int())
        tuple_field = Tuple(Int())
        fixed_tuple_field = FixedTuple(Int(), Int(), Str())

        # Optional type - Atom doesn't have direct optional support
        optional_int_field = Value()  # Using Value for optional
        # Literal - Atom has Enum
        enum_like_field = Enum(1, 2, 3)
        # Custom class - Atom can use Value() for any type
        custom_class_field = Typed(CustomClass)


# ============================================================================
# Fixtures - Untyped (GET/SET benchmarks)
# ============================================================================


@pytest.fixture
def py_slotted_untyped():
    """Fresh PySlottedClass instance for each benchmark."""
    obj = PySlottedClass()
    obj._field = 42
    return obj


@pytest.fixture
def py_plain_untyped():
    """Fresh PyPlainClass instance for each benchmark."""
    obj = PyPlainClass()
    obj._field = 42
    return obj


@pytest.fixture
def ators_untyped():
    """Fresh AtorsUntypedClass instance for each benchmark."""
    return AtorsUntypedClass(field=42)


@pytest.fixture
def ators_frozen_untyped():
    """Fresh frozen AtorsUntypedClass instance for each benchmark."""
    obj = AtorsUntypedClass(field=42)
    freeze(obj)
    return obj


@pytest.fixture
def atom_untyped():
    """Fresh AtomUntypedClass instance for each benchmark (if available)."""
    if not ATOM_AVAILABLE:
        pytest.skip("Atom not available")
    return AtomUntypedClass(field=42)


@pytest.fixture
def property_untyped():
    """Fresh PropertyUntypedClass instance for each benchmark."""
    return PropertyUntypedClass()


# ============================================================================
# Fixtures - Typed (Validation benchmarks)
# ============================================================================


@pytest.fixture
def py_slotted_typed():
    """Fresh PySlottedClass for typed benchmarks (using _field for int type)."""
    obj = PySlottedClass()
    obj._field = 0
    return obj


@pytest.fixture
def ators_typed():
    """Fresh AtorsValidatedClass for validation benchmarks."""
    return AtorsValidatedClass(
        int_field=0,
        float_field=0.0,
        str_field="",
        bool_field=False,
        complex_field=0j,
        bytes_field=b"",
        set_field=set(),
        dict_field={},
        list_field=[],
        tuple_field=(),
        fixed_tuple_field=(0, 0, ""),
        frozen_set_field=frozenset(),
        optional_int_field=None,
        enum_like_field=1,
        constrained_int_field=0,
        custom_class_field=CustomClass(),
    )


@pytest.fixture
def atom_typed():
    """Fresh AtomValidatedClass for validation benchmarks."""
    if not ATOM_AVAILABLE:
        pytest.skip("Atom not available")
    return AtomValidatedClass(
        int_field=0,
        float_field=0.0,
        str_field="",
        bool_field=False,
        bytes_field=b"",
        set_field=set(),
        dict_field={},
        tuple_field=(),
        fixed_tuple_field=(0, 0, ""),
        optional_int_field=None,
        enum_like_field=1,
        custom_class_field=CustomClass(),
    )


# ============================================================================
# Property-based Implementations (with validation)
# ============================================================================


class PropertyUntypedClass:
    """Python class with simple properties (no validation) for untyped benchmarks."""

    def __init__(self):
        self._field = 0

    @property
    def field(self):
        return self._field

    @field.setter
    def field(self, value):
        self._field = value


class PropertyValidatedClass:
    """Python class with properties providing the same validation as Ators."""

    def __init__(self):
        self._int_field = 0
        self._float_field = 0.0
        self._str_field = ""
        self._bool_field = False
        self._complex_field = 0j
        self._bytes_field = b""
        self._set_field = set()
        self._list_field = []
        self._dict_field = {}
        self._tuple_field = ()
        self._fixed_tuple_field = (0, 0, "")
        self._optional_int_field = None
        self._enum_like_field = 1
        self._constrained_int_field = 0
        self._frozen_set_field = frozenset()
        self._custom_class_field = CustomClass()

    @property
    def int_field(self):
        return self._int_field

    @int_field.setter
    def int_field(self, value):
        if not isinstance(value, int):
            raise TypeError(f"Expected int, got {type(value)}")
        self._int_field = value

    @property
    def float_field(self):
        return self._float_field

    @float_field.setter
    def float_field(self, value):
        if not isinstance(value, float):
            raise TypeError(f"Expected float, got {type(value)}")
        self._float_field = value

    @property
    def str_field(self):
        return self._str_field

    @str_field.setter
    def str_field(self, value):
        if not isinstance(value, str):
            raise TypeError(f"Expected str, got {type(value)}")
        self._str_field = value

    @property
    def bool_field(self):
        return self._bool_field

    @bool_field.setter
    def bool_field(self, value):
        if not isinstance(value, bool):
            raise TypeError(f"Expected bool, got {type(value)}")
        self._bool_field = value

    @property
    def complex_field(self):
        return self._complex_field

    @complex_field.setter
    def complex_field(self, value):
        if not isinstance(value, complex):
            raise TypeError(f"Expected complex, got {type(value)}")
        self._complex_field = value

    @property
    def bytes_field(self):
        return self._bytes_field

    @bytes_field.setter
    def bytes_field(self, value):
        if not isinstance(value, bytes):
            raise TypeError(f"Expected bytes, got {type(value)}")
        self._bytes_field = value

    @property
    def set_field(self):
        return self._set_field

    @set_field.setter
    def set_field(self, value):
        if not isinstance(value, set):
            raise TypeError(f"Expected set, got {type(value)}")
        if not all(isinstance(item, int) for item in value):
            raise TypeError("All set items must be int")
        self._set_field = value

    @property
    def list_field(self):
        return self._list_field

    @list_field.setter
    def list_field(self, value):
        if not isinstance(value, list):
            raise TypeError(f"Expected list, got {type(value)}")
        if not all(isinstance(item, int) for item in value):
            raise TypeError("All list items must be int")
        self._list_field = value

    @property
    def dict_field(self):
        return self._dict_field

    @dict_field.setter
    def dict_field(self, value):
        if not isinstance(value, dict):
            raise TypeError(f"Expected dict, got {type(value)}")
        if not all(isinstance(k, str) and isinstance(v, int) for k, v in value.items()):
            raise TypeError("Dict keys must be str, values must be int")
        self._dict_field = value

    @property
    def tuple_field(self):
        return self._tuple_field

    @tuple_field.setter
    def tuple_field(self, value):
        if not isinstance(value, tuple):
            raise TypeError(f"Expected tuple, got {type(value)}")
        if not all(isinstance(item, int) for item in value):
            raise TypeError("All tuple items must be int")
        self._tuple_field = value

    @property
    def fixed_tuple_field(self):
        return self._fixed_tuple_field

    @fixed_tuple_field.setter
    def fixed_tuple_field(self, value):
        if not isinstance(value, tuple) or len(value) != 3:
            raise TypeError(f"Expected tuple of length 3, got {type(value)}")
        if not (
            isinstance(value[0], int)
            and isinstance(value[1], int)
            and isinstance(value[2], str)
        ):
            raise TypeError("Expected (int, int, str)")
        self._fixed_tuple_field = value

    @property
    def optional_int_field(self):
        return self._optional_int_field

    @optional_int_field.setter
    def optional_int_field(self, value):
        if value is not None and not isinstance(value, int):
            raise TypeError(f"Expected int or None, got {type(value)}")
        self._optional_int_field = value

    @property
    def enum_like_field(self):
        return self._enum_like_field

    @enum_like_field.setter
    def enum_like_field(self, value):
        if value not in (1, 2, 3):
            raise TypeError(f"Expected 1, 2, or 3, got {value}")
        self._enum_like_field = value

    @property
    def constrained_int_field(self):
        return self._constrained_int_field

    @constrained_int_field.setter
    def constrained_int_field(self, value):
        if not isinstance(value, int):
            raise TypeError(f"Expected int, got {type(value)}")
        if value not in {0, 25, 50, 75, 100}:
            raise ValueError(f"Value {value} not in allowed set")
        self._constrained_int_field = value

    @property
    def frozen_set_field(self):
        return self._frozen_set_field

    @frozen_set_field.setter
    def frozen_set_field(self, value):
        if not isinstance(value, frozenset):
            raise TypeError(f"Expected frozenset, got {type(value)}")
        if not all(isinstance(item, int) for item in value):
            raise TypeError("All frozenset items must be int")
        self._frozen_set_field = value

    @property
    def custom_class_field(self):
        return self._custom_class_field

    @custom_class_field.setter
    def custom_class_field(self, value):
        if not isinstance(value, CustomClass):
            raise TypeError(f"Expected CustomClass, got {type(value)}")
        self._custom_class_field = value


# ============================================================================
# Fixtures - Property-based (Validation benchmarks)
# ============================================================================


@pytest.fixture
def property_typed():
    """Fresh PropertyValidatedClass for validation benchmarks."""
    return PropertyValidatedClass()


@pytest.fixture
def custom_class_instance():
    """Fresh CustomClass instance for benchmarks."""
    return CustomClass()
