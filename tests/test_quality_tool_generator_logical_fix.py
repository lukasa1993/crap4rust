from __future__ import annotations

import compileall
import os
import subprocess
import sys
import tomllib
from pathlib import Path


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


def test_logical_fix_generator_builds_and_tests_all_repositories(tmp_path: Path) -> None:
    generator = Path(__file__).parents[1] / "tools" / "quality_tool_generator_logical_fix.py"
    for repository in REPOSITORIES:
        output = tmp_path / repository
        generated = subprocess.run(
            [sys.executable, str(generator), "--repo", repository, "--output", str(output)],
            check=False,
            text=True,
            capture_output=True,
        )
        assert generated.returncode == 0, generated.stdout + generated.stderr
        assert compileall.compile_dir(output / "src", quiet=1)
        assert compileall.compile_dir(output / "tests", quiet=1)
        with (output / "pyproject.toml").open("rb") as stream:
            project = tomllib.load(stream)["project"]
        assert project["name"] == repository
        assert project["version"] == "1.0.0"
        assert (output / "tests" / "test_core.py").is_file()

        environment = os.environ.copy()
        environment["PYTHONPATH"] = str(output / "src")
        tested = subprocess.run(
            [sys.executable, "-m", "pytest", "-q", str(output / "tests")],
            cwd=output,
            env=environment,
            check=False,
            text=True,
            capture_output=True,
        )
        assert tested.returncode == 0, f"{repository}\n{tested.stdout}\n{tested.stderr}"
