#!/usr/bin/env python3
"""Capture raw stdout/stderr bytes from real backends against a local HTTP server.

The generated response bodies are disposable test inputs, never fixture output.
Every fixture output file is written directly by the backend's stdout/stderr.
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import pty
import select
import shutil
import signal
import struct
import subprocess
import sys
import termios
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[2]
DESTINATION = ROOT / "fixtures" / "streams"
WORK = Path("/home/user/.cache/morid-capture/stream-work")
VENV = Path("/home/user/.cache/morid-capture/venv")
PORT = 18080
BASE = f"http://127.0.0.1:{PORT}"
CAPTURE_DATE = "2026-11-04"
VERSIONS = {
    "yt-dlp": "2026.10.26",
    "gallery-dl": "1.32.19",
    "wget": "1.21.3",
    "aria2c": "1.37.0",
    "you-get": "0.4.1743",
    "svtplay-dl": "4.211",
    "get-sauce": "1.10.0",
}


def body(size: int, seed: int = 0) -> bytes:
    block = bytes((index * 31 + seed * 17) % 256 for index in range(16384))
    return (block * (size // len(block) + 1))[:size]


OBJECTS = {
    "/video.mp4": ("video/mp4", body(2 * 1024 * 1024, 1)),
    "/slow.mp4": ("video/mp4", body(8 * 1024 * 1024, 2)),
    "/image.jpg": ("image/jpeg", body(512 * 1024, 3)),
    "/Café.mp4": ("video/mp4", body(256 * 1024, 4)),
}
for index in range(8):
    OBJECTS[f"/segment-{index}.ts"] = ("video/mp2t", body(128 * 1024, 10 + index))
PLAYLIST = (
    "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:4\n"
    + "".join(f"#EXTINF:4,\n/segment-{index}.ts\n" for index in range(8))
    + "#EXT-X-ENDLIST\n"
).encode("ascii")


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, _format: str, *_args: Any) -> None:
        pass

    def do_HEAD(self) -> None:
        self.serve(head=True)

    def do_GET(self) -> None:
        self.serve(head=False)

    def serve(self, head: bool) -> None:
        path = unquote(urlsplit(self.path).path)
        if path == "/stream.m3u8":
            content_type, data = "application/vnd.apple.mpegurl", PLAYLIST
        elif path in OBJECTS:
            content_type, data = OBJECTS[path]
        else:
            data = b"fixture: not found\n"
            self.send_response(404)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Connection", "close")
            self.end_headers()
            if not head:
                self.wfile.write(data)
            return

        start, end = 0, len(data) - 1
        range_header = self.headers.get("Range")
        partial = False
        if range_header and range_header.startswith("bytes="):
            try:
                left, right = range_header[6:].split("-", 1)
                if left:
                    start = int(left)
                if right:
                    end = min(int(right), len(data) - 1)
                if start > end or start >= len(data):
                    raise ValueError("invalid range")
                partial = True
            except ValueError:
                self.send_response(416)
                self.send_header("Content-Range", f"bytes */{len(data)}")
                self.send_header("Content-Length", "0")
                self.end_headers()
                return
        length = end - start + 1
        self.send_response(206 if partial else 200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(length))
        self.send_header("Accept-Ranges", "bytes")
        if partial:
            self.send_header("Content-Range", f"bytes {start}-{end}/{len(data)}")
        self.end_headers()
        if head:
            return
        try:
            position = start
            while position <= end:
                chunk = data[position : min(position + 16384, end + 1)]
                self.wfile.write(chunk)
                self.wfile.flush()
                position += len(chunk)
                if path == "/slow.mp4":
                    time.sleep(0.04)
                elif path != "/stream.m3u8":
                    time.sleep(0.003)
        except (BrokenPipeError, ConnectionResetError):
            pass


class Capture:
    def __init__(self, force: bool) -> None:
        self.force = force
        self.home = WORK / "home"
        self.home.mkdir(parents=True, exist_ok=True)

    def environment(self, extra: dict[str, str] | None = None) -> dict[str, str]:
        environment = os.environ.copy()
        for key in list(environment):
            if key.lower() in {"http_proxy", "https_proxy", "all_proxy", "no_proxy"}:
                environment.pop(key)
        environment.update({
            "HOME": str(self.home),
            "XDG_CONFIG_HOME": str(self.home / ".config"),
            "XDG_CACHE_HOME": str(self.home / ".cache"),
            "LC_ALL": "C",
            "LANG": "C",
            "TERM": "dumb",
            "COLUMNS": "100",
            "NO_COLOR": "1",
            "PYTHONIOENCODING": "utf-8",
            "no_proxy": "127.0.0.1,localhost",
        })
        environment.update(extra or {})
        return environment

    def run(
        self,
        backend: str,
        name: str,
        command: list[str],
        url_class: str,
        *,
        environment: dict[str, str] | None = None,
        interrupt_after: float | None = None,
        use_pty: bool = False,
        timeout: float = 45,
    ) -> dict[str, Any]:
        directory = DESTINATION / backend
        directory.mkdir(parents=True, exist_ok=True)
        target = directory / name
        if target.exists() and not self.force:
            print(f"keep {backend}/{name} (use --force to recapture)")
            metadata_path = directory / "meta.json"
            if metadata_path.exists():
                metadata = json.loads(metadata_path.read_text(encoding="utf-8"))
                for item in metadata.get("captures", []):
                    if item.get("file") == name:
                        return item
            return {
                "file": name,
                "url_class": url_class,
                "command": ["previously captured; delete this file or use --force to rerun"],
                "exit_code": None,
                "bytes": target.stat().st_size,
                "sha256": hashlib.sha256(target.read_bytes()).hexdigest(),
            }

        cwd = WORK / backend / name.replace(".", "_")
        if cwd.exists():
            shutil.rmtree(cwd)
        cwd.mkdir(parents=True)
        printable = ["<work-dir>" if item == str(cwd) else item for item in command]
        printable = ["<local-url>" if item.startswith(BASE) else item for item in printable]
        print(f"capture {backend}/{name}: exit pending", flush=True)
        with target.open("wb") as output:
            if use_pty:
                master, slave = pty.openpty()
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
                process = subprocess.Popen(
                    command,
                    cwd=cwd,
                    env=self.environment(environment),
                    stdin=subprocess.DEVNULL,
                    stdout=slave,
                    stderr=slave,
                )
                os.close(slave)
                deadline = time.monotonic() + timeout
                while True:
                    ready, _, _ = select.select([master], [], [], 0.1)
                    if ready:
                        try:
                            data = os.read(master, 65536)
                        except OSError:
                            data = b""
                        if data:
                            output.write(data)
                            output.flush()
                        elif process.poll() is not None:
                            break
                    if process.poll() is not None and not ready:
                        break
                    if time.monotonic() > deadline:
                        process.kill()
                returncode = process.wait()
                os.close(master)
            else:
                process = subprocess.Popen(
                    command,
                    cwd=cwd,
                    env=self.environment(environment),
                    stdin=subprocess.DEVNULL,
                    stdout=output,
                    stderr=subprocess.STDOUT,
                )
                try:
                    if interrupt_after is None:
                        returncode = process.wait(timeout=timeout)
                    else:
                        deadline = time.monotonic() + timeout
                        while time.monotonic() < deadline and process.poll() is None:
                            data = target.read_bytes() if target.exists() else b""
                            if b"[download] Destination:" in data and b"\r" in data:
                                time.sleep(0.1)
                                break
                            time.sleep(0.05)
                        process.kill()
                        returncode = process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    returncode = process.wait()
        print(f"  exit {returncode}, {target.stat().st_size} bytes", flush=True)
        return {
            "file": name,
            "url_class": url_class,
            "command": printable,
            "transport": "pty" if use_pty else "pipe",
            "exit_code": returncode,
            "bytes": target.stat().st_size,
            "sha256": hashlib.sha256(target.read_bytes()).hexdigest(),
        }


def executable(name: str) -> str:
    if name == "wget":
        return "/usr/bin/wget"
    return str(VENV / "bin" / name)


def capture_all(force: bool) -> dict[str, Any]:
    capture = Capture(force)
    records: dict[str, list[dict[str, Any]]] = {name: [] for name in VERSIONS}

    def add(backend: str, name: str, command: list[str], url_class: str, **kwargs: Any) -> None:
        records[backend].append(capture.run(backend, name, command, url_class, **kwargs))

    local_video = "local deterministic HTTP object with a video file extension, small"
    local_image = "local deterministic HTTP object with an image file extension, small"
    local_hls = "local deterministic HLS playlist with eight finite segments"
    missing = "local HTTP object returning 404"
    interrupted = "local deterministic HTTP object interrupted during transfer"

    add("yt-dlp", "version.txt", [executable("yt-dlp"), "--version"], "no URL")
    yt_base = [executable("yt-dlp"), "--no-config", "--no-cache-dir", "--no-colors", "--no-warnings"]
    add("yt-dlp", "progress.txt", yt_base + ["--progress", "--progress-delta", "0.1", "-o", "fixture.%(ext)s", f"{BASE}/video.mp4"], local_video)
    add("yt-dlp", "destination.txt", yt_base + ["--newline", "-o", "fixture.%(ext)s", f"{BASE}/video.mp4"], local_video)
    add("yt-dlp", "complete.txt", yt_base + ["--newline", "-o", "fixture.%(ext)s", f"{BASE}/video.mp4"], local_video)
    add("yt-dlp", "failure.txt", yt_base + ["--retries", "0", "--socket-timeout", "2", f"{BASE}/missing"], missing)
    add("yt-dlp", "interrupted.txt", yt_base + ["--progress", "-o", "fixture.%(ext)s", f"{BASE}/slow.mp4"], interrupted, interrupt_after=1.5)
    add("yt-dlp", "list.json", yt_base + ["--dump-single-json", "--skip-download", f"{BASE}/video.mp4"], local_video)
    add("yt-dlp", "cr.bin", yt_base + ["--progress", "--progress-delta", "0", "-o", "fixture.%(ext)s", f"{BASE}/video.mp4"], local_video + "; CR progress")
    add("yt-dlp", "partial-line.bin", yt_base + ["--progress", "--progress-delta", "0", "-o", "fixture.%(ext)s", f"{BASE}/slow.mp4"], interrupted + "; final partial line", interrupt_after=1.2)
    add("yt-dlp", "non-utf8.bin", yt_base + ["--newline", "-o", "%(title)s.%(ext)s", f"{BASE}/Caf%C3%A9.mp4"], local_video + "; Latin-1 process output", environment={"PYTHONIOENCODING": "latin-1"})

    add("gallery-dl", "version.txt", [executable("gallery-dl"), "--version"], "no URL")
    gallery_base = [executable("gallery-dl"), "--no-colors", "--no-input", "-R", "0", "-d", "."]
    add("gallery-dl", "progress.txt", gallery_base + ["-r", "128k", "-f", "fixture.{extension}", f"{BASE}/image.jpg"], local_image, use_pty=True)
    add("gallery-dl", "destination.txt", gallery_base + ["-f", "fixture.{extension}", f"{BASE}/image.jpg"], local_image)
    add("gallery-dl", "complete.txt", gallery_base + ["-f", "fixture.{extension}", f"{BASE}/image.jpg"], local_image)
    add("gallery-dl", "failure.txt", gallery_base + [f"{BASE}/missing.jpg"], missing)
    add("gallery-dl", "list.json", [executable("gallery-dl"), "--no-colors", "--dump-json", f"{BASE}/image.jpg"], local_image)

    add("wget", "version.txt", [executable("wget"), "--version"], "no URL")
    add("wget", "progress.txt", [executable("wget"), "--progress=bar:force", "--limit-rate=128k", "-O", "fixture.bin", f"{BASE}/video.mp4"], local_video)
    add("wget", "destination.txt", [executable("wget"), "-O", "fixture.bin", f"{BASE}/video.mp4"], local_video)
    add("wget", "complete.txt", [executable("wget"), "-O", "fixture.bin", f"{BASE}/video.mp4"], local_video)
    add("wget", "failure.txt", [executable("wget"), "--tries=1", "--timeout=2", "-O", "fixture.bin", f"{BASE}/missing"], missing)

    add("aria2c", "version.txt", [executable("aria2c"), "--version"], "no URL")
    aria_base = [executable("aria2c"), "--dir=.", "--out=fixture.bin", "--allow-overwrite=true", "--auto-file-renaming=false", "--summary-interval=1"]
    add("aria2c", "progress.txt", aria_base + ["--max-download-limit=128K", f"{BASE}/video.mp4"], local_video)
    add("aria2c", "destination.txt", aria_base + [f"{BASE}/video.mp4"], local_video)
    add("aria2c", "complete.txt", aria_base + [f"{BASE}/video.mp4"], local_video)
    add("aria2c", "failure.txt", aria_base + ["--max-tries=1", "--retry-wait=0", f"{BASE}/missing"], missing)

    add("you-get", "version.txt", [executable("you-get"), "--version"], "no URL")
    add("you-get", "progress.txt", [executable("you-get"), "--no-merge", "--force", "-O", "fixture", "-o", ".", f"{BASE}/video.mp4"], local_video)
    add("you-get", "destination.txt", [executable("you-get"), "--no-merge", "--force", "-O", "fixture", "-o", ".", f"{BASE}/video.mp4"], local_video)
    add("you-get", "complete.txt", [executable("you-get"), "--no-merge", "--force", "-O", "fixture", "-o", ".", f"{BASE}/video.mp4"], local_video)
    add("you-get", "failure.txt", [executable("you-get"), "--no-merge", "--force", "-O", "fixture", "-o", ".", f"{BASE}/missing.mp4"], missing)
    add("you-get", "list.json", [executable("you-get"), "--json", f"{BASE}/video.mp4"], local_video)

    add("svtplay-dl", "version.txt", [executable("svtplay-dl"), "--version"], "no URL")
    svt_base = [executable("svtplay-dl"), "--no-remux", "--no-merge", "--no-postprocess", "-f", "-o", "fixture.ts"]
    add("svtplay-dl", "progress.txt", svt_base + [f"{BASE}/stream.m3u8"], local_hls)
    add("svtplay-dl", "destination.txt", svt_base + [f"{BASE}/stream.m3u8"], local_hls)
    add("svtplay-dl", "complete.txt", svt_base + [f"{BASE}/stream.m3u8"], local_hls)
    add("svtplay-dl", "failure.txt", svt_base + [f"{BASE}/missing.m3u8"], missing)
    add("svtplay-dl", "list.json", [executable("svtplay-dl"), "--list-quality", f"{BASE}/stream.m3u8"], local_hls)

    sauce = "/home/user/.cache/morid-capture/bin/get-sauce"
    add("get-sauce", "version.txt", [sauce, "-v"], "no URL")
    sauce_base = [sauce, "-m", "none", "-t", "-O", "./", "-o", "fixture"]
    add("get-sauce", "progress.txt", sauce_base + [f"{BASE}/video.mp4"], local_video)
    add("get-sauce", "destination.txt", sauce_base + [f"{BASE}/video.mp4"], local_video)
    add("get-sauce", "complete.txt", sauce_base + [f"{BASE}/video.mp4"], local_video)
    add("get-sauce", "failure.txt", sauce_base + [f"{BASE}/missing.mp4"], missing)
    add("get-sauce", "list.json", [sauce, "-j", f"{BASE}/video.mp4"], local_video)

    return records


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--force", action="store_true", help="replace raw captures (timing-sensitive bytes can change)")
    args = parser.parse_args()

    server = ThreadingHTTPServer(("127.0.0.1", PORT), Handler)
    server.daemon_threads = True
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        records = capture_all(args.force)
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)

    for backend, items in records.items():
        metadata = {
            "backend": backend,
            "version": VERSIONS[backend],
            "capture_date": CAPTURE_DATE,
            "platform": "Linux x86_64",
            "source": "real subprocess stdout and stderr merged directly to each file; local deterministic HTTP input",
            "captures": items,
        }
        path = DESTINATION / backend / "meta.json"
        path.write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    manifest_path = ROOT / "fixtures" / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8")) if manifest_path.exists() else {"schema_version": 1}
    manifest["streams"] = {
        backend: {
            "status": "captured" if all((DESTINATION / backend / name).exists() for name in ("version.txt", "progress.txt", "failure.txt")) else "not-captured",
            "version": VERSIONS[backend],
            "files": [f"fixtures/streams/{backend}/{item['file']}" for item in items] + [f"fixtures/streams/{backend}/meta.json"],
        }
        for backend in VERSIONS
    }
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
