#!/usr/bin/env python3
from __future__ import annotations

import base64
import sys
import zlib
from pathlib import Path

from _quality_tool_generator_data_1 import DATA as DATA_1
from _quality_tool_generator_data_2 import DATA as DATA_2
from _quality_tool_generator_data_3 import DATA as DATA_3


def _argument(name: str) -> str:
    try:
        index = sys.argv.index(name)
        return sys.argv[index + 1]
    except (ValueError, IndexError) as error:
        raise RuntimeError(f"{name} is required") from error


payload = DATA_1 + DATA_2 + DATA_3
source = zlib.decompress(base64.b85decode(payload)).decode("utf-8")
namespace = {
    "__name__": "quality_tool_generator_embedded",
    "__file__": str(Path(__file__).resolve()),
}
exec(compile(source, namespace["__file__"], "exec"), namespace)
main = namespace.get("main")
if not callable(main):
    raise RuntimeError("the reviewed generator does not define main()")
result = main()
if result not in (None, 0):
    raise SystemExit(result)

repository = _argument("--repo")
output = Path(_argument("--output")).resolve()
core = output / "src" / repository / "core.py"
if core.is_file():
    text = core.read_text(encoding="utf-8")
    old = "    if node.type not in BINARY_TYPES:\n        return 0\n"
    if old in text:
        new = (
            "    # Tree-sitter grammars use different parent node names for logical\n"
            "    # expressions. Count direct && and || operator children once.\n"
        )
        core.write_text(text.replace(old, new, 1), encoding="utf-8")
    elif "def _operator_count" in text:
        raise RuntimeError("generated operator counter has an unknown guard form")

raise SystemExit(0)
