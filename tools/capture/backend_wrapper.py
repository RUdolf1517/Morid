#!/usr/bin/env python3
"""Record one exact QProcess argument vector and exit successfully.

The original program is pointed at this file through its extension JSON.  No
shell is involved: sys.argv[1:] is the argv that Qt passed to the backend.
"""

import json
import os
import pathlib
import sys


PROXY_VARIABLES = (
    "http_proxy",
    "https_proxy",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "all_proxy",
    "ALL_PROXY",
    "no_proxy",
    "NO_PROXY",
)

# The application probes every engine before enabling its UI. These responses
# belong only to the disposable wrapper; raw stream fixtures come from the real
# backends. Probe argv is kept separate from the scenario's golden transcript.
VERSION_OUTPUT = {
    "yt-dlp": "2026.10.26\n",
    "yt-dlp-nightly": "2026.10.26\n",
    "gallery-dl.bin": "1.32.19\n",
    "wget": "GNU Wget 1.21.3 built on linux-gnu.\n",
    "aria2c": "aria2 version 1.37.0\n",
    "you-get": "you-get: version 0.4.1743\n",
    "svtplay-dl": "svtplay-dl 4.211\n",
    "get-sauce": "get-sauce version v1.10.0\n",
    "lux": "lux version 0.24.1\n",
    "capture-custom": "1.0.0\n",
    "safaribooks.py": "1.0.0\n",
    "ffmpeg": "ffmpeg version 6.0\n",
    "ffprobe": "ffprobe version 6.0\n",
    "qjs": "QuickJS\nquickjs version 0.10.1\n",
    "qjs-linux-x86_64": "0.10.1\n",
}


def record(transcript: str) -> None:
    value = {
        "program": sys.argv[0],
        "argv": sys.argv[1:],
        "cwd": os.getcwd(),
        "env": {key: os.environ[key] for key in PROXY_VARIABLES if key in os.environ},
    }

    # One O_APPEND write keeps concurrent batch captures from interleaving.
    payload = (json.dumps(value, ensure_ascii=False, sort_keys=True) + "\n").encode("utf-8")
    fd = os.open(transcript, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    try:
        os.write(fd, payload)
    finally:
        os.close(fd)


def main() -> int:
    name = pathlib.Path(sys.argv[0]).name
    arguments = sys.argv[1:]
    is_version = name in VERSION_OUTPUT and arguments in (["--version"], ["-version"], ["-v"])
    # Cmd.Args can put substituted template arguments before VersionArgument.
    is_version = is_version or (name == "capture-custom" and arguments[-1:] == ["--version"])
    if is_version:
        probe = os.environ.get("CAPTURE_PROBE_TRANSCRIPT")
        if probe:
            record(probe)
        sys.stdout.write(VERSION_OUTPUT[name])
        return 0

    transcript = os.environ.get("CAPTURE_TRANSCRIPT")
    if not transcript:
        print("CAPTURE_TRANSCRIPT is not set", file=sys.stderr)
        return 64
    record(transcript)

    fake_name = os.environ.get("CAPTURE_FAKE_DOWNLOAD_NAME")
    if fake_name:
        # This is a temporary, empty placeholder used only to exercise the
        # original's successful-download/post-command path.  It is not a fixture.
        pathlib.Path(fake_name).write_bytes(b"")
        sys.stdout.write(f"[download] Destination: {fake_name}\n")
        sys.stdout.write("[download] 100% of 0.00B in 00:00 at 0.00B/s\n")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
