---
name: ators-rust-python-coverage
description: Measure how much Rust code is exercised by the Python test suite in the ators project.
---

# Local Rust coverage from Python tests

Use this skill when you need to answer: "How much of the Rust implementation is covered by the Python tests in this repo?"

This is the local equivalent of the coverage job in [.github/workflows/CI.yml](.github/workflows/CI.yml).

## Prerequisites

1. Install the coverage tooling:
   - `cargo install cargo-llvm-cov`
   - `rustup component add llvm-tools-preview`
2. Sync the project environment:
   - `uv sync`
3. Ensure the Python extension is built for the current checkout:
   - `uv run -- maturin develop --uv`

## Run the coverage collection

### Linux / macOS

```bash
source <(cargo llvm-cov show-env --export-prefix)
export CARGO_TARGET_DIR=$CARGO_LLVM_COV_TARGET_DIR
export CARGO_INCREMENTAL=1
cargo llvm-cov clean --workspace
uv run -- maturin develop --uv
uv run -- pytest tests --cov --cov-report xml -v
cargo llvm-cov report --lcov --output-path coverage.lcov
```

### Windows cmd

Use the helper script instead of re-assembling the environment by hand:

```cmd
python scripts/run_rust_coverage.py
```

> On shared Windows machines, stale `CARGO_LLVM_COV*` values can recursively poison the generated `env.bat` and an uninitialized `target` directory may lack `CACHEDIR.TAG`; the helper clears the stale coverage variables and skips `cargo clean` when the target dir is not Cargo-managed.

## What this measures

- The Rust code built under the `cargo llvm-cov` instrumentation layer.
- The Python test suite exercising that Rust code through the extension module.
- The merged outputs:
  - `coverage.lcov` for Rust coverage reporting
  - `coverage.xml` for pytest coverage output

## Notes

- Keep the coverage run in the repo root so the generated files land in the working tree.
- Re-run the command after any Rust changes because the coverage instrumentation is tied to the current build.
- Use the smallest relevant test target for quick iterations, then widen to the full suite only when needed.
