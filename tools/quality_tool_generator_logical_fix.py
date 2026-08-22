#!/usr/bin/env python3
from __future__ import annotations

import base64
import importlib
import zlib
from pathlib import Path


def _payload(module_name: str, part_name: str) -> str:
    module = importlib.import_module(module_name)
    for name in (part_name, "DATA"):
        value = getattr(module, name, None)
        if isinstance(value, str):
            return value
    raise RuntimeError(f"{module_name} does not define {part_name} or DATA")


payload = (
    _payload("_quality_tool_generator_data_1", "DATA_PART_1")
    + _payload("_quality_tool_generator_data_2", "DATA_PART_2")
    + _payload("_quality_tool_generator_data_3", "DATA_PART_3")
)
source = zlib.decompress(base64.b85decode(payload)).decode("utf-8")
old = '''def _operator_count(node, source):
    if node.type not in BINARY_TYPES:
        return 0
    count = 0
'''
new = '''def _operator_count(node, source):
    # Tree-sitter grammars use different parent node names for logical
    # expressions. Count only direct operator children. Each direct && or ||
    # token then contributes exactly once.
    count = 0
'''
if old not in source:
    raise RuntimeError("the reviewed generator no longer contains the expected operator counter")
source = source.replace(old, new, 1)
namespace = {
    "__name__": "__main__",
    "__file__": str(Path(__file__).resolve()),
}
exec(compile(source, namespace["__file__"], "exec"), namespace)
