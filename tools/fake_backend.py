"""Offline service-test backend; descendants inherit the backend's containment.

Uses the Python already required by repository verification, never production.
It ignores polite termination and starts its grandchild immediately, without a
cooperative containment gate. Every process has a bounded fail-safe lifetime.
"""
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

root = Path(sys.argv[2])
signal.signal(signal.SIGTERM, signal.SIG_IGN)
child = None
if sys.argv[1] == "backend":
    child = subprocess.Popen(
        [sys.executable, __file__, "grandchild", str(root)],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
else:
    temporary = root / "grandchild.tmp"
    temporary.write_text(str(os.getpid()))
    os.replace(temporary, root / "grandchild.pid")
try:
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline and not (root / "stop").exists():
        time.sleep(0.01)
finally:
    if child is not None:
        child.kill()
        child.wait()
