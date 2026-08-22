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


def _replace_once(text: str, old: str, new: str, description: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"generated {description} has an unknown form: expected one match, found {count}")
    return text.replace(old, new, 1)


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

    logical_guard = "    if node.type not in BINARY_TYPES:\n        return 0\n"
    if logical_guard in text:
        text = _replace_once(
            text,
            logical_guard,
            (
                "    # Tree-sitter grammars use different parent node names for logical\n"
                "    # expressions. Count direct && and || operator children once.\n"
            ),
            "logical-operator counter",
        )
    elif "def _operator_count" in text:
        raise RuntimeError("generated operator counter has an unknown guard form")

    if repository in {"crap4c", "crap4cpp"}:
        old_declarator = '''    declarator = node.child_by_field_name("declarator")
    if declarator is not None:
        candidates = [child for child in _walk(declarator) if child.type in NAME_TYPES]
        if candidates:
            return candidates[-1]
'''
        new_declarator = '''    declarator = node.child_by_field_name("declarator")
    if declarator is not None:
        # In C-family trees, the outer declarator also contains parameter names.
        # Read the name from the function declarator's own declarator field first.
        function_declarators = [child for child in _walk(declarator) if child.type == "function_declarator"]
        for function_declarator in function_declarators:
            function_name_declarator = function_declarator.child_by_field_name("declarator")
            if function_name_declarator is None:
                continue
            name_candidates = [
                child for child in _walk(function_name_declarator) if child.type in NAME_TYPES
            ]
            if name_candidates:
                return name_candidates[-1]
        candidates = [child for child in _walk(declarator) if child.type in NAME_TYPES]
        if candidates:
            return candidates[0]
'''
        text = _replace_once(text, old_declarator, new_declarator, "C-family declarator parser")

    core.write_text(text, encoding="utf-8")

raise SystemExit(0)
