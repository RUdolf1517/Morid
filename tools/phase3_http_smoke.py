"""Separate real yt-dlp/loopback-HTTP acceptance, NOT part of offline tests.
Run using a Python environment containing yt-dlp. No remote media dependency.
"""
import functools
import http.server
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import threading

repo = Path(__file__).resolve().parents[1]
binary = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="morid-http-") as temp:
    root = Path(temp)
    source = root / "source"
    source.mkdir()
    payload = bytes(range(256)) * 4096
    (source / "video.mp4").write_bytes(payload)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(source)))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        install = root / "install"
        install.mkdir()
        app = install / binary.name
        shutil.copy2(binary, app)
        extensions = install / "extensions"
        extensions.mkdir()
        extension = json.loads((repo / "extensions/yt-dlp.json").read_text())
        extension["Cmd"] = {platform: {"amd64": {"Name": sys.executable, "Args": [sys.executable, "-m", "yt_dlp"]}} for platform in ["Generic", "MacOS", "Windows"]}
        (extensions / "yt-dlp.json").write_text(json.dumps(extension))
        destination = root / "download dir"
        url = f"http://127.0.0.1:{server.server_port}/video.mp4"
        command = [str(app), "download", url, "--to", str(destination), "--no-archive", "--options", "-f best --fixup never", "--json"]
        result = subprocess.run(command, cwd=root, capture_output=True, text=True, encoding="utf-8", timeout=60)
        print("REAL HTTP COMMAND", json.dumps(command))
        print(result.stdout)
        print(result.stderr, file=sys.stderr)
        assert result.returncode == 0, result.returncode
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        files = list(destination.glob("*.mp4"))
        assert len(files) == 1, files
        assert files[0].read_bytes() == payload
        assert rows[0]["type"] == "started" and rows[-1]["state"] == "success"
        assert str(files[0]) in rows[-1]["files"], rows[-1]
        print("JSONL FIRST", json.dumps(rows[0]))
        print("JSONL LAST", json.dumps(rows[-1]))
        print("JSONL WC-L", len(rows))
        print("REAL HTTP FILE", files[0], "BYTES", files[0].stat().st_size)
    finally:
        server.shutdown()
        thread.join()
        server.server_close()
