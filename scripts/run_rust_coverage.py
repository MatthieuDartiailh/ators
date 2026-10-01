#!/usr/bin/env python3
"""Run local Rust coverage for the ators Python test suite."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
WINDOWS_COVERAGE_ENV_KEYS = (
    "CARGO_LLVM_COV_TARGET_DIR",
    "CARGO_LLVM_COV_BUILD_DIR",
    "CARGO_LLVM_COV",
    "CARGO_LLVM_COV_SHOW_ENV",
    "__CARGO_LLVM_COV_RUSTC_WRAPPER",
    "__CARGO_LLVM_COV_RUSTC_WRAPPER_RUSTFLAGS",
    "__CARGO_LLVM_COV_RUSTC_WRAPPER_CRATE_NAMES",
    "__CARGO_LLVM_COV_RUSTC_WRAPPER_PRE_EXISTING",
    "LLVM_PROFILE_FILE",
    "RUSTC_WRAPPER",
    "RUSTFLAGS",
    "RUSTDOCFLAGS",
)


def sanitize_windows_coverage_env(env: dict[str, str]) -> dict[str, str]:
    cleaned = dict(env)
    for key in WINDOWS_COVERAGE_ENV_KEYS:
        cleaned.pop(key, None)
    return cleaned


CARGO_CACHE_TAG = (
    "Signature: 8a477f597d28d172789f06886806bc55\n"
    "# This file is a cache directory tag created by cargo.\n"
    "# For information about cache directory tags see https://bford.info/cachedir/\n"
)


def cargo_target_is_cleanable(target_dir: Path) -> bool:
    if not target_dir.exists():
        return True
    return (target_dir / "CACHEDIR.TAG").exists()


def ensure_cargo_target_cache_tag(target_dir: Path) -> None:
    target_dir.mkdir(parents=True, exist_ok=True)
    tag_path = target_dir / "CACHEDIR.TAG"
    if not tag_path.exists():
        tag_path.write_text(CARGO_CACHE_TAG, encoding="utf-8")


def stale_llvm_cov_artifacts(target_dir: Path) -> list[Path]:
    if not target_dir.exists():
        return []
    artifacts: list[Path] = []
    for pattern in ("*.profraw", "*.profdata", "*profraw-list"):
        artifacts.extend(target_dir.glob(pattern))
    llvm_cov_target = target_dir / "llvm-cov-target"
    if llvm_cov_target.exists():
        artifacts.append(llvm_cov_target)
    return sorted(set(artifacts), key=lambda path: str(path))


def clear_stale_llvm_cov_artifacts(target_dir: Path) -> None:
    for artifact in stale_llvm_cov_artifacts(target_dir):
        if artifact.is_dir():
            shutil.rmtree(artifact)
        else:
            artifact.unlink(missing_ok=True)


def normalize_windows_llvm_cov_value(value: str) -> str:
    """cargo-llvm-cov emits a unit-separator marker between flags on Windows."""
    value = value.replace("\r", "").strip()
    if "-Cinstrument-coverage--cfg=coverage" in value:
        value = value.replace(
            "-Cinstrument-coverage--cfg=coverage",
            "-Cinstrument-coverage\x1f--cfg=coverage",
        )
    return value


def parse_llvm_cov_env(raw_output: str, env: dict[str, str]) -> dict[str, str]:
    parsed = dict(env)
    for line in raw_output.splitlines():
        line = line.strip()
        if not line.startswith("set "):
            continue
        assignment = line[4:]
        if "=" not in assignment:
            continue
        key, value = assignment.split("=", 1)
        key = key.strip()
        value = normalize_windows_llvm_cov_value(value.strip().strip('"'))
        if key:
            parsed[key] = value
    return parsed


def ensure_tool(name: str, install_hint: str) -> None:
    if shutil.which(name):
        return
    raise SystemExit(
        f"{name} is not installed or not on PATH. Install it with: {install_hint}"
    )


def add_repo_to_pythonpath(env: dict[str, str]) -> dict[str, str]:
    paths = [str(REPO_ROOT)]
    existing = env.get("PYTHONPATH")
    if existing:
        paths.append(existing)
    env["PYTHONPATH"] = os.pathsep.join(filter(None, paths))
    return env


def active_python_executable() -> str:
    local_venv = REPO_ROOT / ".venv"
    candidate = local_venv / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
    if candidate.exists():
        return str(candidate)

    if os.environ.get("VIRTUAL_ENV"):
        venv_root = Path(os.environ["VIRTUAL_ENV"])
        candidate = venv_root / (
            "Scripts/python.exe" if os.name == "nt" else "bin/python"
        )
        if candidate.exists():
            return str(candidate)
    return sys.executable


def maturin_develop_command(python_exe: str) -> list[str]:
    return [python_exe, "-m", "maturin", "develop"]


def run_local_coverage() -> None:
    ensure_tool("cargo", "Install Rust from https://rustup.rs/")
    ensure_tool("uv", "pip install uv")
    ensure_tool(
        "cargo-llvm-cov",
        "cargo install cargo-llvm-cov; rustup component add llvm-tools-preview",
    )

    clean_env = sanitize_windows_coverage_env(os.environ.copy())
    clean_env = add_repo_to_pythonpath(clean_env)
    if os.name == "nt":
        show_env = subprocess.run(
            ["cargo", "llvm-cov", "show-env", "--cmd"],
            check=True,
            capture_output=True,
            text=True,
            cwd=REPO_ROOT,
            env=clean_env,
        ).stdout
        coverage_env = parse_llvm_cov_env(show_env, clean_env)
        coverage_env = add_repo_to_pythonpath(coverage_env)
        coverage_env["CARGO_TARGET_DIR"] = coverage_env.get(
            "CARGO_LLVM_COV_TARGET_DIR", str(REPO_ROOT / "target")
        )
        coverage_env["CARGO_INCREMENTAL"] = "1"
        target_dir = REPO_ROOT / "target"
        has_profraws = any(target_dir.glob("*.profraw"))
        if not cargo_target_is_cleanable(target_dir):
            ensure_cargo_target_cache_tag(target_dir)
        if (
            cargo_target_is_cleanable(target_dir)
            or stale_llvm_cov_artifacts(target_dir)
            or not has_profraws
        ):
            clear_stale_llvm_cov_artifacts(target_dir)
            subprocess.run(
                ["cargo", "llvm-cov", "clean", "--workspace"],
                cwd=REPO_ROOT,
                check=True,
                env=coverage_env,
            )
        else:
            print(
                "Skipping cargo clean: target dir is missing CACHEDIR.TAG and has no stale llvm-cov artifacts"
            )
        python_exe = active_python_executable()
        subprocess.run(
            maturin_develop_command(python_exe),
            cwd=REPO_ROOT,
            check=True,
            env=coverage_env,
        )
        subprocess.run(
            [
                python_exe,
                "-m",
                "pytest",
                "tests",
                "--cov",
                "--cov-report",
                "xml",
                "-v",
            ],
            cwd=REPO_ROOT,
            check=True,
            env=coverage_env,
        )
        subprocess.run(
            ["cargo", "llvm-cov", "report", "--lcov", "--output-path", "coverage.lcov"],
            cwd=REPO_ROOT,
            check=True,
            env=coverage_env,
        )
    else:
        clean_env = add_repo_to_pythonpath(os.environ.copy())
        target_dir = REPO_ROOT / "target"
        if not cargo_target_is_cleanable(target_dir):
            ensure_cargo_target_cache_tag(target_dir)
        has_profraws = any(target_dir.glob("*.profraw"))
        if (
            cargo_target_is_cleanable(target_dir)
            or stale_llvm_cov_artifacts(target_dir)
            or not has_profraws
        ):
            clean_step = "cargo llvm-cov clean --workspace && "
        else:
            clean_step = 'echo "Skipping cargo clean: target dir is missing CACHEDIR.TAG and has no stale llvm-cov artifacts" && '
        python_exe = active_python_executable()
        command = (
            "source <(cargo llvm-cov show-env --export-prefix) && "
            "export CARGO_TARGET_DIR=$CARGO_LLVM_COV_TARGET_DIR && "
            "export CARGO_INCREMENTAL=1 && "
            "export PYTHONPATH=${PYTHONPATH:+$PYTHONPATH:}$PWD && "
            f"{clean_step}"
            f"{python_exe} -m maturin develop && "
            f"{python_exe} -m pytest tests --cov --cov-report xml -v && "
            "cargo llvm-cov report --lcov --output-path coverage.lcov"
        )
        subprocess.run(
            ["bash", "-lc", command], cwd=REPO_ROOT, check=True, env=clean_env
        )

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
