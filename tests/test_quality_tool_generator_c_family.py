from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


REPOSITORIES = ("crap4c", "crap4cpp")


def test_c_family_generated_analyzers_pass_their_tests(tmp_path: Path) -> None:
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
        environment = dict(os.environ)
        environment["PYTHONPATH"] = str(output / "src")
        tested = subprocess.run(
            [sys.executable, "-m", "pytest", "-q", "tests/test_core.py"],
            cwd=output,
            env=environment,
            check=False,
            text=True,
            capture_output=True,
            timeout=120,
        )
        assert tested.returncode == 0, tested.stdout + tested.stderr
