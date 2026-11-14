"""Offline, byte-deterministic CLI backend. Never used in production."""
import json
import os
from pathlib import Path
import signal
import sys
import time

sys.stdout.reconfigure(newline="\n")
root = Path(sys.argv[1])
url = sys.argv[-1]
with (root / "spawns.jsonl").open("a", encoding="utf-8") as output:
    output.write(json.dumps(sys.argv[2:]) + "\n")
if url.endswith("/archive-check"):
    # Phase 4: exercise the actual argv's archive path without rewriting it.
    # This is an offline fixture backend, not a claim about yt-dlp networking.
    archive = Path(sys.argv[sys.argv.index("--download-archive") + 1])
    (root / "archive-observed.bin").write_bytes(archive.read_bytes())
    print("archive observed read-only")
    sys.exit(0)
if "--fixture-list" in sys.argv and url.endswith("/captured"):
    # Replay captured bytes verbatim, not a reduced/reconstructed JSON document.
    capture = Path(__file__).resolve().parents[1] / "fixtures/streams/yt-dlp/list.json"
    sys.stdout.buffer.write(capture.read_bytes())
    sys.stdout.buffer.flush()
    sys.exit(0)
if "--fixture-list" in sys.argv:
    print(json.dumps({"title": "fixture", "duration": 12, "formats": [{"format_id": "22", "ext": "mp4"}]}))
    sys.exit(0)
if "cancel" in url:
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    (root / "ready").write_text(str(os.getpid()))
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        time.sleep(0.01)
    sys.exit(1)
if "unsupported" in url:
    print("ERROR: Unsupported URL: " + url, file=sys.stderr)
    sys.exit(1)
if "fail" in url:
    print("fixture error", file=sys.stderr)
    sys.exit(7)
name = "fixture-" + url.rsplit("/", 1)[-1] + ".bin"
Path(name).write_bytes(b"fixture content")
print("[download] Destination: " + name)
print("[download] downloaded_bytes:15 ETA:0 total_bytes:15 progress.speed:15 filename:" + name)
