"""Offline two-pipe fixture: ACKs establish write order, never timing sleeps.

The Python already required by repository verification is test-only. A watchdog
bounds even blocked writes in the large-pipe test. No network is used.
"""
import os
from pathlib import Path
import sys
import threading
import time

mode, folder = sys.argv[1:]
root = Path(folder)
watchdog = threading.Timer(30, lambda: os._exit(80))
watchdog.daemon = True
watchdog.start()


def emit(fd, data):
    view = memoryview(data)
    while view:
        view = view[os.write(fd, view):]


def ack(index):
    deadline = time.monotonic() + 10
    while not (root / f"ack-{index}").exists():
        if time.monotonic() >= deadline:
            raise TimeoutError(f"missing ACK {index}")
        time.sleep(0.005)


if mode == "ordered":
    emit(2, b"err-first\n")
    ack(0)
    for byte in "out-é-終\n".encode():
        emit(1, bytes([byte]))
    ack(1)
    emit(2, b"[download] downloaded_bytes:1024 ETA:86 total_bytes_estimate:NA total_bytes:3006209 progress.speed:34781.85 filename:movie.mp4\n")
    ack(2)
    emit(1, b"\x1b[K[download]  25.0% of    2.00MiB at    4.94MiB/s ETA 00:02\r")
    ack(3)
    emit(2, b"err-tail")
    os.close(2)
    ack(4)
    emit(1, b"out-tail")
    os.close(1)
    ack(5)
    os._exit(0)
elif mode == "burst":
    for fd, prefix in [(2, "E"), (1, "O")]:
        for index in range(512):
            emit(fd, (f"{prefix}:{index}:" + "x" * 4090 + "\n").encode())
elif mode == "idle":
    emit(2, b"ready\n")
    ack("never")
else:
    raise ValueError(mode)
