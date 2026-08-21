#!/usr/bin/env python3
from __future__ import annotations

import base64
import zlib

from _quality_tool_generator_data_1 import DATA as DATA_1
from _quality_tool_generator_data_2 import DATA as DATA_2
from _quality_tool_generator_data_3 import DATA as DATA_3

exec(compile(zlib.decompress(base64.b85decode(DATA_1 + DATA_2 + DATA_3)), __file__, "exec"))
