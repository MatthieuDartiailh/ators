# --------------------------------------------------------------------------------------
# Copyright (c) 2025-2026, Ators contributors, see git history for details
#
# Distributed under the terms of the Modified BSD License.
#
# The full license is in the file LICENSE, distributed with this software.
# --------------------------------------------------------------------------------------
"""Benchmark dict methods implemented in Rust for Ators containers.

Compared implementations:
- Ators typed dict member
- Atom typed dict member (if available)
- Pure Python dict (no validation)
- Pure Python dict with runtime str->int validation

Run with: python benchmarks/containers/bench_dict.py
"""

import importlib.util
from typing import Any, cast

import pyperf

from ators import Ators, member

ATOM_AVAILABLE = bool(importlib.util.find_spec("atom"))

if ATOM_AVAILABLE:
    import atom.api as atom_api


INITIAL = {"a": 1, "b": 2, "c": 3, "d": 4}


def _ensure_key(key):
    if not isinstance(key, str):
        raise TypeError(f"Expected str key, got {type(key).__name__}")


def _ensure_value(value):
    if not isinstance(value, int):
        raise TypeError(f"Expected int value, got {type(value).__name__}")


class _KeysProvider:
    """Mapping-like object for update keys()/__getitem__ branch."""

    def __init__(self):
        self._data = {"x": 11, "y": 12}

    def keys(self):
        return self._data.keys()

    def __getitem__(self, key):
        return self._data[key]


class ValidatedStrIntDict(dict):
    """Python dict with str key / int value validation for mutating methods."""

    def __setitem__(self, key, value):
        _ensure_key(key)
        _ensure_value(value)
        return super().__setitem__(key, value)

    def update(self, other=(), **kwargs):
        if other:
            if hasattr(other, "keys"):
                for k in other.keys():
                    v = other[k]
                    _ensure_key(k)
                    _ensure_value(v)
            else:
                for k, v in other:
                    _ensure_key(k)
                    _ensure_value(v)
        for k, v in kwargs.items():
            _ensure_key(k)
            _ensure_value(v)
        return super().update(other, **kwargs)

    def setdefault(self, key, default=None):
        _ensure_key(key)
        if key in self:
            return self[key]
        _ensure_value(default)
        return super().setdefault(key, default)

    def __ior__(self, other):
        self.update(other)
        return self


class PyDictContainer:
    """Pure Python container without validation."""

    def __init__(self):
        self.dict_field = INITIAL.copy()


class PyValidatedDictContainer:
    """Pure Python container with validation."""

    def __init__(self):
        self.dict_field = ValidatedStrIntDict(INITIAL)


class AtorsDictContainer(Ators):
    """Ators container for dict benchmarks."""

    dict_field: dict[str, int] = member()


if ATOM_AVAILABLE:

    class AtomDictContainer(atom_api.Atom):
        """Atom container for dict benchmarks."""

        dict_field = cast(Any, atom_api.Dict)(
            cast(Any, atom_api.Str)(),
            cast(Any, atom_api.Int)(),
        )


def _build_ators():
    return AtorsDictContainer(dict_field=INITIAL.copy())


def _build_atom():
    return AtomDictContainer(dict_field=INITIAL.copy())


def _implementations():
    impls = {
        "py": PyDictContainer,
        "py_typed": PyValidatedDictContainer,
        "ators": _build_ators,
    }
    if ATOM_AVAILABLE:
        impls["atom"] = _build_atom
    return impls


def _bench_all(runner: pyperf.Runner, method: str, op_builder):
    for name, factory in _implementations().items():
        obj = factory()
        runner.bench_func(f"dict_{method}_{name}", op_builder(obj))


def bench_setitem(runner: pyperf.Runner):
    def op_builder(obj):
        next_value = [9]

        def op():
            obj.dict_field["a"] = next_value[0]
            next_value[0] = 1 if next_value[0] == 9 else 9

        return op

    _bench_all(runner, "setitem", op_builder)


def bench_update_dict(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            obj.dict_field.update({"x": 11, "y": 12})
            obj.dict_field.pop("x")
            obj.dict_field.pop("y")

        return op

    _bench_all(runner, "update_dict", op_builder)


def bench_update_keys_provider(runner: pyperf.Runner):
    def op_builder(obj):
        provider = _KeysProvider()

        def op():
            obj.dict_field.update(provider)
            obj.dict_field.pop("x")
            obj.dict_field.pop("y")

        return op

    _bench_all(runner, "update_keys_provider", op_builder)


def bench_update_pairs(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            obj.dict_field.update([("x", 11), ("y", 12)])
            obj.dict_field.pop("x")
            obj.dict_field.pop("y")

        return op

    _bench_all(runner, "update_pairs", op_builder)


def bench_setdefault_existing(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            obj.dict_field.setdefault("a", 9)

        return op

    _bench_all(runner, "setdefault_existing", op_builder)


def bench_setdefault_missing(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            obj.dict_field.setdefault("z", 26)
            obj.dict_field.pop("z")

        return op

    _bench_all(runner, "setdefault_missing", op_builder)


def bench_ior(runner: pyperf.Runner):
    def op_builder(obj):
        def op():
            obj.dict_field |= {"x": 11, "y": 12}
            obj.dict_field.pop("x")
            obj.dict_field.pop("y")

        return op

    _bench_all(runner, "ior", op_builder)


if __name__ == "__main__":
    runner = pyperf.Runner()
    bench_setitem(runner)
    bench_update_dict(runner)
    bench_update_keys_provider(runner)
    bench_update_pairs(runner)
    bench_setdefault_existing(runner)
    bench_setdefault_missing(runner)
    bench_ior(runner)
