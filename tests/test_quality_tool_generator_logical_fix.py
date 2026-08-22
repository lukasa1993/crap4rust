from __future__ import annotations

import compileall
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


def test_logical_fix_generator_builds_all_repositories(tmp_path: Path) -> None:
    generator = Path(__file__).parents[1] / "tools" / "quality_tool_generator_logical_fix.py"
    for repository in REPOSITORIES:
        output = tmp_path / repository
        completed = subprocess.run(
            [sys.executable, str(generator), "--repo", repository, "--output", str(output)],
            check=False,
            text=True,
            capture_output=True,
        )
        assert completed.returncode == 0, completed.stdout + completed.stderr
        assert compileall.compile_dir(output / "src", quiet=1)
        assert compileall.compile_dir(output / "tests", quiet=1)
        with (output / "pyproject.toml").open("rb") as stream:
            project = tomllib.load(stream)["project"]
        assert project["name"] == repository
        assert project["version"] == "1.0.0"
        assert (output / "tests" / "test_core.py").is_file()
        license_text = (output / "LICENSE").read_text(encoding="utf-8")
        assert license_text.startswith("MIT License\n\nCopyright (c) 2026 Luka Dodelia\n")
        assert license_text.endswith("SOFTWARE.\n")
