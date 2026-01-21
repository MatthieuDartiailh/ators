# Ators Benchmarks

This directory contains performance benchmarks for the Ators attribute validation library.

## Structure

Benchmarks are organized into focused test files, each measuring different aspects of performance:

- **`bench_get_untyped.py`**: Benchmark `__get__` (read) performance with untyped fields
  - Compares: Python (slotted), Python (plain), Ators (Any), Atom (if available)
  - Measures pure attribute access overhead

- **`bench_set_untyped.py`**: Benchmark `__set__` (write) performance with untyped fields
  - Compares: Python (slotted), Python (plain), Ators (Any), Atom (if available)
  - Measures pure attribute assignment overhead (no validation)

- **`bench_validation.py`**: Benchmark validation performance with typed fields
  - Compares: Python baseline (no validation), Ators (typed validators), Atom (typed validators)
  - Measures the cost of type validation for various types:
    - Basic types: int, float, str, bool
    - Optional types: Optional[int]
    - Constrained types: Literal, custom validators
    - Container types: set[T], dict[K, V], tuple[T, ...]

## Running Benchmarks

Run all benchmarks:
```bash
pytest benchmarks/bench_*.py --benchmark-only
```

Run specific benchmark group:
```bash
pytest benchmarks/bench_*.py --benchmark-only --benchmark-group-by=group
```

Run specific benchmark file:
```bash
pytest benchmarks/bench_get_untyped.py --benchmark-only
```

Run with detailed output:
```bash
pytest benchmarks/bench_*.py --benchmark-only -v
```

Save benchmark results:
```bash
pytest benchmarks/bench_*.py --benchmark-only --benchmark-json=results.json
```

## Interpreting Results

Each benchmark provides:
- **Min**: Minimum execution time observed
- **Max**: Maximum execution time observed
- **Mean**: Average execution time
- **StdDev**: Standard deviation of measurements
- **Median**: Median execution time
- **IQR**: Interquartile range
- **OPS**: Operations per second
- **Rounds**: Number of measurement rounds
- **Iterations**: Number of iterations per round

## Framework Comparison

- **Python (slotted)**: Baseline with `__slots__` for minimal overhead
- **Python (plain)**: Baseline without `__slots__` to measure dict overhead
- **Ators**: Type validation library (subject of benchmarks)
- **Atom**: Alternative framework for comparison (when available)

## Key Metrics

1. **Access Overhead** (get_untyped): How much slower is Ators vs pure Python?
2. **Assignment Overhead** (set_untyped): How much slower is Ators vs pure Python (untyped)?
3. **Validation Cost** (validation_*): Additional cost of type validation per type

