"""Audit the exact release table against shipped JSON (no I/O beyond files)."""
import json
from pathlib import Path
import re

root = Path(__file__).resolve().parents[1]
expected = set()
for file in sorted((root / "extensions").glob("*.json")):
    document = json.loads(file.read_text(encoding="utf-8"))
    for key in ("DownloadUrl", "DownloadUrlMAC"):
        value = document.get(key)
        if value:
            expected.add(value)
            print(f"DERIVED_ENDPOINT source=extensions/{file.name}::{key} url={value}")
exception = "https://api.github.com/repos/mhogomchungu/packages/releases/latest"
assert exception not in expected
text = (root / "crates/morid-net/src/endpoints.rs").read_text(encoding="utf-8")
actual = re.findall(r'url:\s*"([^"]+)"', text)
assert len(actual) == len(set(actual)), "duplicate table rows"
assert set(actual) == expected | {exception}, (set(actual) - expected, expected - set(actual))
print(f"HANDWRITTEN_ENDPOINT D-021 source=ffmpeg.cpp:96,python.cpp:130 url={exception}")
print(f"EXACT_ENDPOINT_TABLE_OK derived={len(expected)} handwritten=1 total={len(actual)}")
