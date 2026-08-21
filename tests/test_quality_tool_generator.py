from __future__ import annotations

import compileall
import subprocess
import sys
import tomllib
from pathlib import Path


def test_generator_builds_versioned_typescript_mutation_tool(tmp_path: Path) -> None:
    script = Path(__file__).parents[1] / "tools" / "quality_tool_generator.py"
    completed = subprocess.run(
        [sys.executable, str(script), "--repo", "mutate4ts", "--output", str(tmp_path)],
        check=False,
        text=True,
        capture_output=True,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr
    project = tomllib.loads((tmp_path / "pyproject.toml").read_text(encoding="utf-8"))
    assert project["project"]["name"] == "mutate4ts"
    assert project["project"]["version"] == "1.0.0"
    assert (tmp_path / "src" / "mutate4ts" / "core.py").exists()
    assert (tmp_path / "tests" / "fixtures" / "sample.ts").exists()
    assert compileall.compile_dir(tmp_path / "src", quiet=1)
    assert compileall.compile_dir(tmp_path / "tests", quiet=1)


def test_generator_rejects_unknown_repository(tmp_path: Path) -> None:
    script = Path(__file__).parents[1] / "tools" / "quality_tool_generator.py"
    completed = subprocess.run(
        [sys.executable, str(script), "--repo", "unknown4lang", "--output", str(tmp_path)],
        check=False,
        text=True,
        capture_output=True,
    )
    assert completed.returncode != 0
    assert "unsupported repository name" in completed.stderr
