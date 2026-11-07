#!/usr/bin/env python3
"""Temporary: print a file as GitHub warning annotations, so it can be read back.

This environment has no Rust and cannot read job logs or artefacts, so the only
channel that carries text out of a run is the annotation API. Each step may emit
ten annotations, hence the explicit index range. Deleted before handover.
"""

import pathlib
import sys

chunk_size = 2000
first, last = int(sys.argv[2]), int(sys.argv[3])
text = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")

chunks: list[str] = []
current = ""
for line in text.splitlines(keepends=True):
    if len(current) + len(line) > chunk_size:
        chunks.append(current)
        current = ""
    current += line
chunks.append(current)

for index in range(first, min(last + 1, len(chunks))):
    message = chunks[index].replace("%", "%25").replace("\n", "\\n")
    print(f"::warning title=chunk-{index:02d}-of-{len(chunks):02d}::{message}")
