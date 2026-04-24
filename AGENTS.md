# Ators Agent Rules

This file is the repository-level contract for coding agents working in
`ators`, whether the session runs locally in an editor or remotely.

## Session startup

1. Work from the `ators` repository root.
2. If a `.venv/` directory is present, activate it before Python commands
   using the shell-appropriate activation command.
3. Install or refresh the extension module with `maturin develop` before
   relying on Python-side behavior after Rust changes.
4. If `rtk` is available, prefer `rtk <command>` for shell commands that may
   produce large output.

## Repository facts

- Python target is 3.14+.
- The package is built with `maturin` and the extension module lives under
  `python/ators`.
- Performance-sensitive behavior is typically implemented in Rust under `src/`.
- Benchmark coverage is split between shared `pytest` benchmark families and
  local `pyperf` runners under `benchmarks/`.

## Change strategy

1. Prefer the smallest change that fixes the root cause.
2. When behavior is performance-sensitive, change the Rust path that owns the
   hot operation instead of adding a Python-side workaround.
3. Preserve the existing split between:
   - core implementation in `src/`
   - Python exposure in `python/ators/`
   - behavior checks in `tests/`
   - performance checks in `benchmarks/`
4. Avoid broad refactors unless the task explicitly asks for them.

## Validation rules

After edits, run the narrowest check that can falsify the change:

- Rust-only logic change: `cargo test` or a narrower Rust-targeted check if one
  exists.
- Python-visible behavior change: `maturin develop`, then targeted `pytest`.
- Typing or lint-only change: targeted `ruff` or `mypy`.
- Benchmark harness change: run the smallest relevant benchmark family first.

Prefer targeted commands such as:

- `pytest tests/<target>.py -q`
- `pytest benchmarks/<target>.py --benchmark-only -q`
- `python benchmarks/run_pyperf.py --list --family <name>`
- `python benchmarks/run_pyperf.py --family <name>`

Do not run the full benchmark suite or broad repository-wide test commands
unless the task needs that coverage.

## Benchmark discipline

Performance work in `ators` must be backed by benchmark evidence.

1. Read `BENCHMARKS.md` and `benchmarks/README.md` before changing benchmark
   structure.
2. Before running benchmarks, install the package with `pip install .` or
   `uv pip install .`.
3. Do not use `maturin develop` as the benchmark install step: it builds a
   debug extension, which is not suitable for benchmarking.
4. Keep `pytest`/CodSpeed and `pyperf` coverage aligned when adding a new
   benchmark family.
5. For descriptor write-path work, distinguish constant-write and alternating
   write cases. Do not draw conclusions from constant-write results alone.
6. Treat no-op assignment and refcount churn as first-class costs when touching
   set paths.
7. Prefer local, behavior-specific measurements over broad benchmark reruns.

## Documentation and session notes

1. Update nearby documentation when a workflow, benchmark entrypoint, or public
   behavior changes.
2. If a session produces a durable performance or implementation lesson,
   capture it in a short repo note instead of leaving it only in chat history.
3. Keep benchmark documentation synchronized with actual file names and command
   examples.

## Scope guardrails

- Do not revert unrelated user changes.
- Do not change generated artifacts or `target/` output unless the task
  explicitly requires it.
- Do not introduce new dependencies without a clear need.
- Do not widen a focused task into unrelated cleanup.

## Useful commands

- Install/update extension locally: `maturin develop`
- Install for benchmarking: `pip install .` or `uv pip install .`
- Run Python tests: `pytest tests -q`
- Run one benchmark family with pytest: `pytest benchmarks/test_get_untyped.py --benchmark-only -q`
- List pyperf cases: `python benchmarks/run_pyperf.py --list`
- Run one pyperf family: `python benchmarks/run_pyperf.py --family get_untyped`

## Key references

- `benchmarks/README.md`: current benchmark entrypoints and workflow