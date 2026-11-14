"""Read exported data with the real legacy application; never invokes capture.

Linux-only: Qt5 legacy build + readback probe are supplied by CI. The original
runs as the caller in a kernel network namespace with no network interfaces.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

REPO = Path(__file__).resolve().parents[1]


def hashes(root):
    return {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob("*") if p.is_file()}


def main():
    binary, original, probe = map(lambda p: str(Path(p).resolve()), sys.argv[1:])
    fixture = REPO / "fixtures/userdata/media-downloader"
    before = hashes(fixture)
    with tempfile.TemporaryDirectory(prefix="morid-original-readback-") as temp:
        root = Path(temp)
        config, exported = root / "morid", root / "exported"
        for args, expected in [(["import", str(fixture)], 8), (["export", str(exported)], 0)]:
            result = subprocess.run([binary, "--config", str(config), *args], text=True, capture_output=True, timeout=30)
            print("MORID", args[0], "exit", result.returncode, result.stdout, result.stderr, flush=True)
            assert result.returncode == expected
        print("EXPORTED FILE LIST", json.dumps(sorted(hashes(exported))), flush=True)
        assert not (exported / ".morid").exists()
        assert len(json.loads((config / "data/downloadHistory.json").read_bytes())) == 20
        assert (exported / "data/downloadHistory.json").read_bytes() == (fixture / "data/downloadHistory.json").read_bytes()
        # Test-only version responders, not installation or invocation of any
        # real backend. They are isolated scaffolding added AFTER export.
        bin_dir, engine_dir = exported / "bin", exported / "engines.v1"
        bin_dir.mkdir()
        shutil.copytree(REPO / "extensions", engine_dir)
        for name in ["yt-dlp", "gallery-dl.bin", "wget", "stdbuf", "aria2c", "you-get", "svtplay-dl", "get-sauce", "qjs", "qjs-linux-x86_64", "ffmpeg", "ffprobe", "safaribooks.py"]:
            path = bin_dir / name
            shutil.copy2(REPO / "tools/capture/backend_wrapper.py", path)
            path.chmod(0o755)
        overrides = {
            "MediaDownloaderAppDataPath": str(exported),
            "QT_QPA_PLATFORM": "minimal", "LD_PRELOAD": probe,
            "CAPTURE_TRANSCRIPT": str(root / "unexpected-backend-calls.jsonl"),
            "CAPTURE_PROBE_TRANSCRIPT": str(root / "version-probes.jsonl"),
            "PATH": str(bin_dir) + ":" + os.environ["PATH"],
            "HOME": os.environ["HOME"], "LANG": "C.UTF-8",
        }
        command = ["sudo", "unshare", "--net", "--", "runuser", "-u", os.environ["USER"], "--", "env"]
        command.extend(key + "=" + value for key, value in overrides.items())
        command.extend([original, "--debug", "-s"])
        result = subprocess.run(command, cwd=exported, capture_output=True, text=True, timeout=150)
        print("=== LIVE ORIGINAL STDOUT ===\n" + result.stdout, flush=True)
        print("=== LIVE ORIGINAL STDERR ===\n" + result.stderr, flush=True)
        assert result.returncode == 0, f"original readback exit {result.returncode}"
        assert "ORIGINAL_ROUNDTRIP_OK" in result.stdout
        assert hashes(fixture) == before, "captured source mutated"
        print("ORIGINAL_SOURCE_HASHES_UNCHANGED", json.dumps(before, sort_keys=True), flush=True)


if __name__ == "__main__":
    main()
