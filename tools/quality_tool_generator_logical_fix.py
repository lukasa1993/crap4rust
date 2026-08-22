#!/usr/bin/env python3
from __future__ import annotations

import base64
import zlib
from pathlib import Path

from _quality_tool_generator_data_1 import DATA as DATA_1
from _quality_tool_generator_data_2 import DATA as DATA_2
from _quality_tool_generator_data_3 import DATA as DATA_3


payload = DATA_1 + DATA_2 + DATA_3
source = zlib.decompress(base64.b85decode(payload)).decode("utf-8")
old = '''    if node.type not in BINARY_TYPES:
        return 0
'''
new = '''    # Tree-sitter grammars use different parent node names for logical
    # expressions. Count only direct operator children. Each direct && or ||
    # token then contributes exactly once.
'''
if source.count(old) != 1:
    raise RuntimeError("the reviewed generator does not contain exactly one logical-operator guard")
source = source.replace(old, new, 1)
namespace = {
    "__name__": "__main__",
    "__file__": str(Path(__file__).resolve()),
}
exec(compile(source, namespace["__file__"], "exec"), namespace)
