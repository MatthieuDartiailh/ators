# Benchmark Reorganization Summary

## Overview

The benchmark suite has been reorganized into a dedicated top-level `/benchmarks` directory with focused test files for different performance aspects.

## New Structure

```
benchmarks/
├── __init__.py                  # Package marker
├── conftest.py                  # Shared fixtures and base classes
├── README.md                    # Documentation
├── bench_get_untyped.py        # __get__ performance benchmarks (read-only)
├── bench_set_untyped.py        # __set__ performance benchmarks (write-only, untyped)
└── bench_validation.py          # Validation performance benchmarks (write with type checking)
```

## Test Files Overview

### 1. `bench_get_untyped.py` (4 tests)
**Focus**: Pure attribute read performance with untyped fields

Tests:
- `test_benchmark_get_py_slotted`: Python baseline (with __slots__)
- `test_benchmark_get_py_plain`: Python baseline (without __slots__)
- `test_benchmark_get_ators`: Ators with Any annotation
- `test_benchmark_get_atom`: Atom with Value descriptor (if available)

**Use case**: Measure the overhead of Ators' `__get__` mechanism compared to pure Python attribute access.

### 2. `bench_set_untyped.py` (4 tests)
**Focus**: Pure attribute write performance with untyped fields (NO validation)

Tests:
- `test_benchmark_set_py_slotted`: Python baseline (with __slots__)
- `test_benchmark_set_py_plain`: Python baseline (without __slots__)
- `test_benchmark_set_ators`: Ators with Any annotation
- `test_benchmark_set_atom`: Atom with Value descriptor (if available)

**Use case**: Measure the overhead of Ators' `__set__` mechanism without validation.

### 3. `bench_validation.py` (26 tests across 10 validator groups)
**Focus**: Type validation performance impact

Validator groups tested:
- `validation_int`: Basic int type validation
- `validation_float`: Basic float type validation
- `validation_str`: Basic string type validation
- `validation_bool`: Basic bool type validation
- `validation_optional_int`: Optional[int] validation
- `validation_literal`: Literal type validation
- `validation_set`: set[int] container validation
- `validation_dict`: dict[str, int] container validation
- `validation_constrained_int`: Custom constrained validator
- `validation_tuple`: tuple[int, ...] validation
- `validation_fixed_tuple`: tuple[int, int, str] fixed validation

Each group compares:
- Python baseline (no validation)
- Ators with typed validator
- Atom with typed validator (if available)

**Use case**: Measure the performance cost of type validation for various validator types.

### 4. `conftest.py` (Shared Infrastructure)
**Provides**:
- Reference implementations:
  - `PySlottedClass`: Python with __slots__
  - `PyPlainClass`: Python without __slots__
  - `AtorsUntypedClass`: Ators with Any annotation
  - `AtorsValidatedClass`: Ators with typed validators
  - `AtomUntypedClass`: Atom with Value descriptor
  - `AtomSimpleTypes`: Atom with basic type validators

- Fixtures for untyped benchmarks:
  - `py_slotted_untyped`: Fresh slotted Python instance
  - `py_plain_untyped`: Fresh plain Python instance
  - `ators_untyped`: Fresh untyped Ators instance
  - `atom_untyped`: Fresh untyped Atom instance

- Fixtures for validation benchmarks:
  - `py_slotted_typed`: Typed Python baseline
  - `ators_typed`: Ators with all validator types
  - `atom_typed`: Atom with basic type validators

## Running the Benchmarks

### Run all benchmarks (with explicit glob pattern):
```bash
pytest benchmarks/bench_*.py --benchmark-only
```

### Run specific test file:
```bash
pytest benchmarks/bench_get_untyped.py --benchmark-only -v
pytest benchmarks/bench_set_untyped.py --benchmark-only -v
pytest benchmarks/bench_validation.py --benchmark-only -v
```

### Run specific benchmark group:
```bash
pytest benchmarks/bench_*.py --benchmark-only --benchmark-group-by=group
```

### Save results to JSON:
```bash
pytest benchmarks/bench_*.py --benchmark-only --benchmark-json=results.json
```

## Framework Comparison

All benchmarks compare across:
- **Python baseline** (slotted and plain) - no validation, pure attribute access
- **Ators** - subject framework with type validation
- **Atom** - alternative framework (when available)

## Total Benchmarks

- **GET (untyped)**: 4 tests
- **SET (untyped)**: 4 tests
- **VALIDATION**: 26 tests across 10 validator types
- **TOTAL**: 34 comprehensive benchmarks

## Key Performance Insights

The three-part benchmark structure allows you to measure:

1. **Access Overhead** (from bench_get_untyped):
   - Raw `__get__` performance without any type operations
   - Shows the minimum overhead of Ators

2. **Write Overhead** (from bench_set_untyped):
   - Raw `__set__` performance without validation
   - Shows assignment mechanism efficiency

3. **Validation Overhead** (from bench_validation):
   - Cost of each specific type validator
   - Which validator types are expensive?
   - Comparison with Atom's approach

## Legacy Tests

The original comprehensive benchmark file is still available at:
- `tests/test_benchmark_attr_access.py` (35 tests)

This contains the full validator coverage but with mixed GET/SET/validation operations combined.

## Notes

- Benchmarks require `pytest-benchmark` plugin (already installed)
- Atom framework is optional; tests using Atom are skipped if not available
- Each benchmark uses fresh object instances to avoid state contamination
- Results include statistical analysis (min, max, mean, stddev, median, IQR, OPS)
