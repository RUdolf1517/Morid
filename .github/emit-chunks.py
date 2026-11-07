#!/usr/bin/env python3
"""Temporary: print a file as GitHub warning annotations, so it can be read back.

This environment has no Rust and cannot read job logs or artefacts, so the only
channel that carries text out of a run is the annotation API. A check run keeps
at most fifty annotations, hence the compression and the explicit index range.
Deleted before handover.
"""

import base64
import pathlib
import sys
import zlib

chunk_size = 3000
path, first, last = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
compress = len(sys.argv) > 4 and sys.argv[4] == "gzip"

data = pathlib.Path(path).read_bytes()
if compress:
    text = base64.b64encode(zlib.compress(data, 9)).decode("ascii")
else:
    text = data.decode("utf-8")

chunks: list[str] = [text[i : i + chunk_size] for i in range(0, len(text), chunk_size)]
for index in range(first, min(last + 1, len(chunks))):
    message = chunks[index].replace("%", "%25")
    print(f"::warning title=chunk-{index:02d}-of-{len(chunks):02d}::{message}")
