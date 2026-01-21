"""Shared fixtures and base classes for benchmarks."""

import pytest
from typing import Optional, Literal, Any
from ators import Ators, member
from ators.behaviors import ValueValidator

try:
    from atom.api import Atom, Int, Float, Str, Bool, Set, Dict, Value

    ATOM_AVAILABLE = True
except ImportError:
    ATOM_AVAILABLE = False


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

    # Container validators (typed)
    set_field: set[int] = member()
    dict_field: dict[str, int] = member()
    tuple_field: tuple[int, ...] = member()
    fixed_tuple_field: tuple[int, int, str] = member()

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
def atom_untyped():
    """Fresh AtomUntypedClass instance for each benchmark (if available)."""
    if not ATOM_AVAILABLE:
        pytest.skip("Atom not available")
    return AtomUntypedClass(field=42)


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
        set_field=set(),
        dict_field={},
        tuple_field=(),
        fixed_tuple_field=(0, 0, ""),
        optional_int_field=None,
        enum_like_field=1,
        constrained_int_field=0,
        custom_class_field=CustomClass(),
    )


@pytest.fixture
def atom_typed():
    """Fresh AtomSimpleTypes for validation benchmarks."""
    if not ATOM_AVAILABLE:
        pytest.skip("Atom not available")
    return AtomSimpleTypes(int_field=0, float_field=0.0, str_field="", bool_field=False)
