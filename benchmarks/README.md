# Ators Benchmarks

This directory contains performance benchmarks for the Ators attribute
validation library.

The benchmark suite uses shared case definitions from `benchmarks/shared/`
and two frontends:

- `pytest`/CodSpeed for CI trend tracking.
- `pyperf` for local high-fidelity microbenchmarks.

## Structure

### Shared pytest benchmark families

- `test_get_untyped.py`: untyped `__get__` reads.
- `test_set_untyped.py`: untyped `__set__` writes.
- `test_set_untyped_alternating.py`: alternating untyped writes.
- `test_get_descriptor.py`: class-level descriptor reads.
- `test_init.py`: object construction (`no_validators` and `init_coercion` groups).
- `validators/test_validation_*.py`: typed validation families.
- `containers/test_list.py`: Rust-backed list method family.
- `containers/test_set.py`: Rust-backed set method family.
- `containers/test_dict.py`: Rust-backed dict method family.

### Shared pyperf entrypoints

- `run_pyperf.py`: suite-wide case listing and execution frontend.
- `containers/bench_list.py`: convenience runner for list method family.
- `containers/bench_set.py`: convenience runner for set method family.
- `containers/bench_dict.py`: convenience runner for dict method family.

## Running Benchmarks

Activate the local environment first:

```bash
& .venv/Scripts/Activate.ps1
```

Run all shared pytest benchmark families:

```bash
pytest benchmarks/test_*.py benchmarks/validators/ \
  benchmarks/containers/test_*.py --benchmark-only
```

Run pytest grouped by benchmark marker group:

```bash
pytest benchmarks/test_*.py benchmarks/validators/ \
  benchmarks/containers/test_*.py --benchmark-only \
  --benchmark-group-by=group
```

Run one shared pytest benchmark family:

```bash
pytest benchmarks/containers/test_list.py --benchmark-only
```

List shared pyperf cases:

```bash
python benchmarks/run_pyperf.py --list
```

Run selected shared pyperf families:

```bash
python benchmarks/run_pyperf.py --family list
python benchmarks/run_pyperf.py --family validation_int
python benchmarks/run_pyperf.py --family dict --group update_dict \
  --implementation ators
```

When `rich` is installed, `run_pyperf.py --list` prints a grouped table
with summary counts.

## Adding New Benchmarks

Use this workflow to keep pytest/CodSpeed and pyperf aligned.

1. Define shared cases.
   Add or extend a registry module under `benchmarks/shared/`.
   Emit `BenchmarkCase` values with `family`, `group`, `implementation`,
   `benchmark_name`, and `operation_factory`.

2. Register case selection.
   Add your `select_*_cases(...)` function to `_registered_selectors()`
   in `benchmarks/shared/case_registry.py`.

3. Add pytest wrappers.
   Prefer thin parametrized wrappers using
   `benchmark_case_params(...)` and `run_pytest_benchmark(...)` from
   `benchmarks/shared/pytest_frontend.py`.

4. Add a pyperf wrapper when useful.

5. Verify both frontends.
   Run pytest smoke checks:
   `pytest <file> --benchmark-only -q`
   Run pyperf case discovery:
   `python benchmarks/run_pyperf.py --list --family <name>`

6. Update this README.
   Document new top-level families and new entrypoint scripts.

## Framework Labels

- `py`: plain Python baseline.
- `py_plain`: non-slotted Python object for descriptor families.
- `py_slotted`: slotted Python object for descriptor families.
- `py_typed`: Python runtime-validated container baseline.
- `property`: property-based validation baseline.
- `property_typed`: property copy-and-validate assignment baseline.
- `ators`: Ators implementation.
- `ators_frozen`: frozen Ators variant where relevant.
- `atom`: Atom implementation, included only when available.
