#!/usr/bin/env python3
from __future__ import annotations

import argparse
import compileall
import json
import os
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

OWNER = "lukasa1993"
REPOSITORIES = (
    "crap4ts", "mutate4ts", "dry4ts",
    "crap4python", "mutate4python", "dry4python",
    "crap4rust", "mutate4rust", "dry4rust",
    "crap4swift", "mutate4swift", "dry4swift",
    "crap4objc", "mutate4objc", "dry4objc",
    "crap4bash", "mutate4bash", "dry4bash",
    "crap4c", "mutate4c", "dry4c",
    "crap4cpp", "mutate4cpp", "dry4cpp",
)
ROOT_FILES = (".gitignore", "README.md", "SKILL.md", "pyproject.toml")
TREE_ROOTS = ("src", "tests")
LEGACY_PATHS = (
    "package.json", "package-lock.json", "tsconfig.json",
    "src/core.ts", "src/index.ts", "src/node-shims.d.ts", "test",
    ".github/workflows/bootstrap-production.yml",
)
ALLOWED_EXTRA_TESTS = {
    "crap4rust": {
        "tests/test_quality_tool_generator.py",
        "tests/test_quality_tool_generator_logical_fix.py",
    }
}


def run(command: list[str], *, cwd: Path | None = None, env: dict[str, str] | None = None, timeout: int = 240) -> str:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        timeout=timeout,
        check=False,
    )
    if completed.returncode != 0:
        joined = " ".join(command)
        raise RuntimeError(f"command failed ({completed.returncode}): {joined}\n{completed.stdout}")
    return completed.stdout


def files_under(root: Path, relative: str) -> set[str]:
    base = root / relative
    if not base.exists():
        return set()
    if base.is_file():
        return {relative}
    return {
        path.relative_to(root).as_posix()
        for path in base.rglob("*")
        if path.is_file()
        and "__pycache__" not in path.parts
        and ".pytest_cache" not in path.parts
        and not path.name.endswith((".pyc", ".pyo"))
    }


def compare_generated_tree(repository: str, actual: Path, expected: Path) -> None:
    for relative in ROOT_FILES:
        actual_file = actual / relative
        expected_file = expected / relative
        if not actual_file.is_file():
            raise RuntimeError(f"{repository}: missing {relative}")
        if actual_file.read_bytes() != expected_file.read_bytes():
            raise RuntimeError(f"{repository}: {relative} does not match the reviewed generator")

    workflow = ".github/workflows/ci.yml"
    if (actual / workflow).read_bytes() != (expected / workflow).read_bytes():
        raise RuntimeError(f"{repository}: {workflow} does not match the reviewed generator")

    allowed = ALLOWED_EXTRA_TESTS.get(repository, set())
    for tree_root in TREE_ROOTS:
        expected_files = files_under(expected, tree_root)
        actual_files = files_under(actual, tree_root)
        extras = actual_files - expected_files - allowed
        missing = expected_files - actual_files
        if missing:
            raise RuntimeError(f"{repository}: missing generated files: {sorted(missing)}")
        if extras:
            raise RuntimeError(f"{repository}: unexpected files remain: {sorted(extras)}")
        for relative in sorted(expected_files):
            if (actual / relative).read_bytes() != (expected / relative).read_bytes():
                raise RuntimeError(f"{repository}: {relative} does not match the reviewed generator")

    for relative in LEGACY_PATHS:
        if (actual / relative).exists():
            raise RuntimeError(f"{repository}: legacy or bootstrap path remains: {relative}")


def audit_repository(repository: str, workspace: Path, generator: Path) -> dict[str, object]:
    actual = workspace / "actual" / repository
    expected = workspace / "expected" / repository
    wheel_dir = workspace / "wheels" / repository
    actual.parent.mkdir(parents=True, exist_ok=True)
    expected.parent.mkdir(parents=True, exist_ok=True)
    wheel_dir.mkdir(parents=True, exist_ok=True)

    run(["git", "clone", "--depth", "1", f"https://github.com/{OWNER}/{repository}.git", str(actual)], timeout=180)
    run([sys.executable, str(generator), "--repo", repository, "--output", str(expected)])
    compare_generated_tree(repository, actual, expected)

    with (actual / "pyproject.toml").open("rb") as stream:
        project = tomllib.load(stream)["project"]
    if project["name"] != repository:
        raise RuntimeError(f"{repository}: package name is {project['name']!r}")

    if not compileall.compile_dir(actual / "src", quiet=1):
        raise RuntimeError(f"{repository}: Python compilation failed")

    run([sys.executable, "-m", "build", "--wheel", "--outdir", str(wheel_dir)], cwd=actual)
    environment = os.environ.copy()
    environment["PYTHONPATH"] = str(actual / "src")
    run([sys.executable, "-m", "pytest", "-q"], cwd=actual, env=environment, timeout=300)
    run([sys.executable, "-m", repository, "--version"], cwd=actual, env=environment)
    run([sys.executable, "-m", repository, "--help"], cwd=actual, env=environment)

    commit = run(["git", "rev-parse", "HEAD"], cwd=actual).strip()
    return {
        "repository": f"{OWNER}/{repository}",
        "commit": commit,
        "package_version": project["version"],
        "generated_tree": "match",
        "wheel": "pass",
        "tests": "pass",
        "cli": "pass",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Audit all production quality-tool repositories.")
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    generator = Path(__file__).with_name("quality_tool_generator_logical_fix.py")
    if not generator.is_file():
        raise SystemExit(f"missing reviewed generator: {generator}")

    results: list[dict[str, object]] = []
    with tempfile.TemporaryDirectory(prefix="quality-portfolio-audit-") as temporary:
        workspace = Path(temporary)
        for repository in REPOSITORIES:
            print(f"AUDIT {repository}", flush=True)
            results.append(audit_repository(repository, workspace, generator))

    report = {
        "schema_version": 1,
        "owner": OWNER,
        "repository_count": len(results),
        "status": "pass",
        "repositories": results,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"PASS {len(results)} repositories", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
