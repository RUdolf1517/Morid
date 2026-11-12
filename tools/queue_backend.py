"""Offline queue fixture. Files gate completion; no timing-based start ordering."""
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

# Byte-identical framing on Windows too: the queue rules deliberately select LF,
# so Python's default CRLF translation must not change the fixture protocol.
sys.stdout.reconfigure(newline="\n")
sys.stderr.reconfigure(newline="\n")
mode, folder, job = sys.argv[1:]
root = Path(folder)
signal.signal(signal.SIGTERM, signal.SIG_IGN)
(root / f"cwd-{job}").write_text(os.getcwd())
(root / f"pid-{job}").write_text(str(os.getpid()))
if mode == "grandchild":
    deadline = time.monotonic() + 45
    while not (root / "stop").exists() and time.monotonic() < deadline:
        time.sleep(0.01)
    sys.exit(0)
child = None
if mode in ("background", "tree-gate"):
    child = subprocess.Popen(
        [sys.executable, __file__, "grandchild", str(root), f"child-{job}"],
        stdin=subprocess.DEVNULL,
    )
    deadline = time.monotonic() + 10
    while not (root / f"pid-child-{job}").exists():
        if time.monotonic() >= deadline:
            raise TimeoutError("grandchild readiness")
        time.sleep(0.005)
    if mode == "background":
        os._exit(0)
print(f"out-{job}", flush=True)
print(f"err-{job}", file=sys.stderr, flush=True)
print("[download] downloaded_bytes:50 ETA:1 total_bytes:100 progress.speed:12 filename:test.mp4", flush=True)
if mode == "flood":
    deadline = time.monotonic() + 30
    sequence = 0
    while not (root / f"release-{job}").exists():
        if time.monotonic() >= deadline:
            raise TimeoutError("flood completion gate")
        sys.stdout.write("".join(f"flood-{sequence + index}\n" for index in range(256)))
        sys.stdout.flush()
        sequence += 256
if mode in ("gate", "tree-gate"):
    deadline = time.monotonic() + 30
    while not (root / f"release-{job}").exists() and not (root / "stop").exists():
        if time.monotonic() >= deadline:
            raise TimeoutError("completion gate")
        time.sleep(0.005)
if child:
    child.kill()
    child.wait()
sys.stdout.write(f"stdout-tail-{job}")
sys.stdout.flush()
sys.stderr.write(f"tail-{job}")
sys.stderr.flush()
sys.exit(7 if mode == "fail" else 0)
