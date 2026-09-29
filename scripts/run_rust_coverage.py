#!/usr/bin/env python3
"""Run local Rust coverage for the ators Python test suite."""

from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]


def ensure_tool(name: str, install_hint: str) -> None:
    if shutil.which(name):
        return
    raise SystemExit(
        f"{name} is not installed or not on PATH. Install it with: {install_hint}"
    )


def run_local_coverage() -> None:
    ensure_tool("cargo", "Install Rust from https://rustup.rs/")
    ensure_tool("uv", "pip install uv")
    ensure_tool(
        "cargo-llvm-cov",
        "cargo install cargo-llvm-cov; rustup component add llvm-tools-preview",
    )

    if os.name == "nt":
        env_file = REPO_ROOT / "env.bat"
        with env_file.open("w", encoding="utf-8") as handle:
            subprocess.run(
                ["cargo", "llvm-cov", "show-env", "--cmd"],
                check=True,
                stdout=handle,
                cwd=REPO_ROOT,
            )
        command = (
            'call "{env_file}" && '
            "set CARGO_TARGET_DIR=%CARGO_LLVM_COV_TARGET_DIR% && "
            "set CARGO_INCREMENTAL=1 && "
            "cargo llvm-cov clean --workspace && "
            "uv run -- maturin develop --uv && "
            "uv run -- pytest tests --cov --cov-report xml -v && "
            "cargo llvm-cov report --lcov --output-path coverage.lcov"
        ).format(env_file=env_file)
        subprocess.run(
            ["cmd", "/D", "/V:OFF", "/C", command], cwd=REPO_ROOT, check=True
        )
    else:
        command = (
            "source <(cargo llvm-cov show-env --export-prefix) && "
            "export CARGO_TARGET_DIR=$CARGO_LLVM_COV_TARGET_DIR && "
            "export CARGO_INCREMENTAL=1 && "
            "cargo llvm-cov clean --workspace && "
            "uv run -- maturin develop --uv && "
            "uv run -- pytest tests --cov --cov-report xml -v && "
            "cargo llvm-cov report --lcov --output-path coverage.lcov"
        )
        subprocess.run(["bash", "-lc", command], cwd=REPO_ROOT, check=True)

    print("Coverage report written to:")
    print(f"  - {REPO_ROOT / 'coverage.lcov'}")
    print(f"  - {REPO_ROOT / 'coverage.xml'}")


if __name__ == "__main__":
    try:
        run_local_coverage()
    except subprocess.CalledProcessError as exc:
        raise SystemExit(
            f"Coverage command failed with exit code {exc.returncode}."
        ) from exc
