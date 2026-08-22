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
ROOT_FILES = (".gitignore", "LICENSE", "README.md", "SKILL.md", "pyproject.toml")
TREE_ROOTS = ("src", "tests")
LEGACY_PATHS = (
    "package.json", "package-lock.json", "tsconfig.json",
    "src/core.ts", "src/index.ts", "src/node-shims.d.ts", "test",
    ".github/workflows/bootstrap-production.yml",
    ".github/workflows/generate-production.yml",
    ".github/workflows/production-hardening.yml",
    ".github/workflows/verify-production.yml",
)
ALLOWED_EXTRA_TESTS = {
    "crap4rust": {
        "tests/test_quality_tool_generator.py",
        "tests/test_quality_tool_generator_logical_fix.py",
        "tests/test_quality_tool_generator_c_family.py",
    }
}


class AuditError(RuntimeError):
    pass


def run(
    command: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    timeout: int = 300,
) -> str:
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
        raise AuditError(f"command failed ({completed.returncode}): {joined}\n{completed.stdout}")
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


def compare_file(repository: str, actual: Path, expected: Path, relative: str) -> None:
    actual_file = actual / relative
    expected_file = expected / relative
    if not actual_file.is_file():
        raise AuditError(f"{repository}: missing {relative}")
    if not expected_file.is_file():
        raise AuditError(f"generator did not create {relative} for {repository}")
    if actual_file.read_bytes() != expected_file.read_bytes():
        raise AuditError(f"{repository}: {relative} does not match the reviewed generator")


def compare_generated_tree(repository: str, actual: Path, expected: Path) -> None:
    for relative in ROOT_FILES:
        compare_file(repository, actual, expected, relative)

    compare_file(repository, actual, expected, ".github/workflows/ci.yml")

    allowed = ALLOWED_EXTRA_TESTS.get(repository, set())
    for tree_root in TREE_ROOTS:
        expected_files = files_under(expected, tree_root)
        actual_files = files_under(actual, tree_root)
        extras = actual_files - expected_files - allowed
        missing = expected_files - actual_files
        if missing:
            raise AuditError(f"{repository}: missing generated files: {sorted(missing)}")
        if extras:
            raise AuditError(f"{repository}: unexpected files remain: {sorted(extras)}")
        for relative in sorted(expected_files):
            compare_file(repository, actual, expected, relative)

    for relative in LEGACY_PATHS:
        if (actual / relative).exists():
            raise AuditError(f"{repository}: legacy or bootstrap path remains: {relative}")


def check_project_metadata(repository: str, actual: Path) -> dict[str, object]:
    with (actual / "pyproject.toml").open("rb") as stream:
        project = tomllib.load(stream)["project"]
    if project.get("name") != repository:
        raise AuditError(f"{repository}: package name is {project.get('name')!r}")
    if project.get("version") != "1.0.0":
        raise AuditError(f"{repository}: package version is {project.get('version')!r}, expected '1.0.0'")
    classifiers = set(project.get("classifiers", []))
    if "Development Status :: 5 - Production/Stable" not in classifiers:
        raise AuditError(f"{repository}: package is not marked Production/Stable")
    return project


def install_and_test(
    repository: str,
    actual: Path,
    wheel_dir: Path,
    python: Path,
    scripts: Path,
) -> None:
    if not compileall.compile_dir(actual / "src", quiet=1):
        raise AuditError(f"{repository}: Python compilation failed")
    if not compileall.compile_dir(actual / "tests", quiet=1):
        raise AuditError(f"{repository}: test compilation failed")

    run([sys.executable, "-m", "build", "--wheel", "--outdir", str(wheel_dir)], cwd=actual, timeout=600)
    wheels = sorted(wheel_dir.glob("*.whl"))
    if len(wheels) != 1:
        raise AuditError(f"{repository}: expected one wheel, found {len(wheels)}")
    run([str(python), "-m", "pip", "install", "--force-reinstall", str(wheels[0])], timeout=600)
    run([str(python), "-m", "pip", "check"], timeout=180)

    environment = os.environ.copy()
    environment.pop("PYTHONPATH", None)
    run([str(python), "-m", "pytest", "-q"], cwd=actual, env=environment, timeout=600)
    run([str(python), "-m", repository, "--version"], cwd=actual, env=environment)
    run([str(python), "-m", repository, "--help"], cwd=actual, env=environment)

    command = scripts / repository
    if not command.is_file():
        raise AuditError(f"{repository}: installed console command is missing: {command}")
    run([str(command), "--version"], cwd=actual, env=environment)
    run([str(command), "--help"], cwd=actual, env=environment)


def audit_repository(
    repository: str,
    workspace: Path,
    generator: Path,
    python: Path,
    scripts: Path,
) -> dict[str, object]:
    actual = workspace / "actual" / repository
    expected = workspace / "expected" / repository
    wheel_dir = workspace / "wheels" / repository
    actual.parent.mkdir(parents=True, exist_ok=True)
    expected.parent.mkdir(parents=True, exist_ok=True)
    wheel_dir.mkdir(parents=True, exist_ok=True)

    run(["git", "clone", "--depth", "1", f"https://github.com/{OWNER}/{repository}.git", str(actual)], timeout=240)
    run([sys.executable, str(generator), "--repo", repository, "--output", str(expected)], timeout=180)
    compare_generated_tree(repository, actual, expected)
    project = check_project_metadata(repository, actual)
    install_and_test(repository, actual, wheel_dir, python, scripts)

    commit = run(["git", "rev-parse", "HEAD"], cwd=actual).strip()
    return {
        "repository": f"{OWNER}/{repository}",
        "commit": commit,
        "package_version": project["version"],
        "generated_tree": "match",
        "wheel_build": "pass",
        "wheel_install": "pass",
        "dependency_check": "pass",
        "tests": "pass",
        "module_cli": "pass",
        "console_cli": "pass",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Audit all production quality-tool repositories.")
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()

    generator = Path(__file__).with_name("quality_tool_generator_logical_fix.py")
    if not generator.is_file():
        raise SystemExit(f"missing reviewed generator: {generator}")

    results: list[dict[str, object]] = []
    failure: str | None = None
    with tempfile.TemporaryDirectory(prefix="quality-portfolio-audit-") as temporary:
        workspace = Path(temporary)
        environment = workspace / "venv"
        run([sys.executable, "-m", "venv", str(environment)], timeout=180)
        scripts = environment / ("Scripts" if os.name == "nt" else "bin")
        python = scripts / ("python.exe" if os.name == "nt" else "python")
        run([str(python), "-m", "pip", "install", "--upgrade", "pip", "pytest"], timeout=600)

        for repository in REPOSITORIES:
            print(f"AUDIT {repository}", flush=True)
            try:
                results.append(audit_repository(repository, workspace, generator, python, scripts))
            except Exception as error:  # The report must preserve the first exact failure.
                failure = f"{repository}: {error}"
                print(f"FAIL {failure}", file=sys.stderr, flush=True)
                break

    report = {
        "schema_version": 2,
        "owner": OWNER,
        "expected_repository_count": len(REPOSITORIES),
        "audited_repository_count": len(results),
        "status": "pass" if failure is None and len(results) == len(REPOSITORIES) else "fail",
        "failure": failure,
        "repositories": results,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    if report["status"] != "pass":
        return 1
    print(f"PASS {len(results)} repositories", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
