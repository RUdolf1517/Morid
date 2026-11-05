#!/usr/bin/env python3
"""Verify the committed Phase 0 fixtures without modifying them."""

from __future__ import annotations

import configparser
import hashlib
import json
import re
import subprocess
import tempfile
from pathlib import Path

from capture import ROOT, FIXTURES, SCENARIOS, definitions
from capture_userdata import FINAL as USERDATA, history_count, settings_seed


REQUIRED_STREAM_BACKENDS = {
    "yt-dlp", "gallery-dl", "wget", "aria2c", "you-get", "svtplay-dl", "get-sauce",
}
PROGRAMS = {
    "yt-dlp": "yt-dlp",
    "gallery-dl": "gallery-dl.bin",
    "wget": "wget",
    "aria2c": "aria2c",
    "you-get": "you-get",
    "svtplay-dl": "svtplay-dl",
    "custom": "capture-custom",
}
USERDATA_FILES = {
    "settings/settings.ini",
    "data/presetOptions.json",
    "data/downloadDefaultOptions.json",
    "data/downloadEngineDefaultOptions.json",
    "data/subscriptions.json",
    "data/autoSavedList.json",
    "data/downloadHistory.json",
    "data/archiveFile-yt-dlp.txt",
    "data/themes/Dark.json",
}


def load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def load_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def directory_size(path: Path) -> int:
    return sum(item.stat().st_size for item in path.rglob("*") if item.is_file())


def verify_scenarios() -> tuple[int, int]:
    manifest = load_json(FIXTURES / "manifest.json")
    cases = manifest.get("cases", [])
    expected = definitions()
    expected_ids = [item["id"] for item in expected]
    require(len(cases) == 40, f"manifest has {len(cases)} cases, expected 40")
    require([item.get("id") for item in cases] == expected_ids, "manifest case IDs/order differ from the 40 definitions")
    require([int(item["id"][1:3]) for item in cases] == list(range(1, 41)), "case numbering is not s01..s40")

    captured = 0
    not_captured = 0
    for entry, definition in zip(cases, expected):
        case_id = entry["id"]
        directory = SCENARIOS / case_id
        require(entry.get("feature") == definition["feature"], f"{case_id}: wrong feature ID")
        require(entry.get("status") in {"captured", "not-captured"}, f"{case_id}: bad status")
        require(entry.get("method") in {"log", "wrapper", "other"}, f"{case_id}: bad method")
        require(entry.get("files"), f"{case_id}: no manifest files")
        for name in entry["files"]:
            require((ROOT / name).is_file(), f"{case_id}: missing manifest file {name}")
        actual_files = {
            f"fixtures/scenarios/{case_id}/{item.name}" for item in directory.iterdir() if item.is_file()
        }
        require(set(entry["files"]) == actual_files, f"{case_id}: manifest files differ from directory")

        meta = load_json(directory / "meta.json")
        require(meta.get("case_id") == case_id, f"{case_id}: wrong meta case ID")
        require(meta.get("status") == entry["status"], f"{case_id}: meta/manifest status mismatch")
        require(meta.get("qt_version") == "5.15.16", f"{case_id}: wrong Qt version")
        require(re.fullmatch(r"[0-9a-f]{40}", meta.get("reference_commit", "")) is not None, f"{case_id}: no commit pin")

        if entry["status"] == "not-captured":
            not_captured += 1
            require(entry.get("reason") and meta.get("reason"), f"{case_id}: missing not-captured reason")
            require((directory / "app.log").is_file(), f"{case_id}: no failure evidence app.log")
            continue

        captured += 1
        argv = load_json(directory / "argv.json")
        require(isinstance(argv, list) and all(isinstance(item, str) for item in argv), f"{case_id}: invalid argv")
        records = load_jsonl(directory / "log.txt")
        require(records and records[0]["argv"] == argv, f"{case_id}: argv differs from wrapper transcript")
        require(len(records) == meta.get("backend_invocations"), f"{case_id}: invocation count mismatch")
        if len(records) > 1:
            require(load_json(directory / "invocations.json") == [item["argv"] for item in records], f"{case_id}: invocations mismatch")
            require(load_json(directory / "argv-2.json") == records[1]["argv"], f"{case_id}: argv-2 mismatch")

        # s26 deliberately records the original's yt-dlp fallback. Every other
        # captured case must invoke the backend named by its scenario metadata.
        expected_program = "yt-dlp" if case_id == "s26-cmd-other-architecture" else PROGRAMS[meta["backend"]]
        for record in records:
            require(Path(record["program"]).name == expected_program, f"{case_id}: expected {expected_program}, got {record['program']}")
        if definition.get("post_command"):
            post = load_jsonl(directory / "post-log.txt")
            require(len(post) == 1 and post[0]["argv"] == load_json(directory / "post-argv.json"), f"{case_id}: post-command mismatch")

    require(captured >= 35, f"only {captured}/40 cases captured")
    require(captured + not_captured == 40, "status counts do not total 40")
    return captured, not_captured


def verify_streams() -> int:
    manifest = load_json(FIXTURES / "manifest.json")
    require(set(manifest.get("streams", {})) == REQUIRED_STREAM_BACKENDS, "stream backend set mismatch")
    count = 0
    for backend in sorted(REQUIRED_STREAM_BACKENDS):
        directory = FIXTURES / "streams" / backend
        for name in ("version.txt", "progress.txt", "failure.txt"):
            require((directory / name).stat().st_size > 0, f"{backend}/{name} is empty")
        metadata = load_json(directory / "meta.json")
        require(metadata.get("status", "captured") == "captured", f"{backend}: stream not captured")
        require(metadata.get("capture_date") == "2026-11-04", f"{backend}: wrong capture date")
        for item in metadata["captures"]:
            path = directory / item["file"]
            data = path.read_bytes()
            require(len(data) == item["bytes"], f"{backend}/{item['file']}: byte count mismatch")
            require(hashlib.sha256(data).hexdigest() == item["sha256"], f"{backend}/{item['file']}: SHA-256 mismatch")
            require(item.get("url_class"), f"{backend}/{item['file']}: no URL class")
            require("127.0.0.1" not in item["url_class"], f"{backend}/{item['file']}: URL leaked into class")
            count += 1

    yt = FIXTURES / "streams" / "yt-dlp"
    require(b"\r" in (yt / "cr.bin").read_bytes(), "yt-dlp cr.bin has no CR")
    partial = (yt / "partial-line.bin").read_bytes()
    require(partial and not partial.endswith(b"\n"), "yt-dlp partial-line.bin has a final newline")
    try:
        (yt / "non-utf8.bin").read_bytes().decode("utf-8")
    except UnicodeDecodeError:
        pass
    else:
        raise RuntimeError("yt-dlp non-utf8.bin is valid UTF-8")
    require((yt / "interrupted.txt").stat().st_size > 0, "yt-dlp interrupted output is empty")
    return count


def verify_userdata() -> tuple[int, int]:
    for relative in USERDATA_FILES:
        require((USERDATA / relative).is_file(), f"userdata missing {relative}")
    parser = configparser.ConfigParser(interpolation=None, strict=False)
    parser.optionxform = str
    parser.read(USERDATA / "settings" / "settings.ini", encoding="utf-8")
    require("General" in parser, "userdata settings has no General section")
    keys = set(parser["General"])
    require(not (set(settings_seed()) - keys), "userdata settings keys missing")
    count = history_count(USERDATA / "data" / "downloadHistory.json")
    require(count >= 20, f"userdata has only {count} history entries")
    for name in ("presetOptions.json", "downloadDefaultOptions.json", "downloadEngineDefaultOptions.json", "subscriptions.json", "autoSavedList.json"):
        value = load_json(USERDATA / "data" / name)
        require(isinstance(value, list) and value, f"userdata {name} is empty/not an array")

    text = "\n".join(path.read_text(encoding="utf-8") for path in USERDATA.rglob("*") if path.is_file())
    hosts = set(re.findall(r"https?://([^/\s\"']+)", text))
    require(hosts <= {"example.com"}, f"userdata contains non-anonymized hosts: {sorted(hosts)}")
    home_users = set(re.findall(r"/home/([^/\s]+)", text))
    require(home_users <= {"user"}, f"userdata contains a non-anonymized home path: {sorted(home_users)}")
    require("/home/user/.cache" not in text and "/home/user/Morid" not in text, "userdata contains workspace paths")
    require(not re.search(r"[A-Za-z0-9._%+-]+@(?!example\.com)[A-Za-z0-9.-]+", text), "userdata contains an email-like value")
    require("http://user:pass" not in text and "REDACTED" not in text, "userdata contains proxy credentials or placeholder residue")
    require(not list(USERDATA.rglob("*cookie*")), "userdata contains a cookie file")
    metadata = load_json(FIXTURES / "userdata" / "meta.json")
    require(metadata["verification"]["exit_code"] == 0, "original-program userdata reload failed")
    require(metadata["verification"]["history_entries"] == count, "userdata history count differs from reload metadata")
    return len(keys), count


def verify_comparator(captured: int) -> None:
    comparator = ROOT / "tools" / "compare-argv"
    passed = 0
    first = None
    for directory in sorted(SCENARIOS.iterdir()):
        golden = directory / "argv.json"
        meta = directory / "meta.json"
        if not golden.exists() or load_json(meta)["status"] != "captured":
            continue
        result = subprocess.run([str(comparator), directory.name, "--candidate", str(golden)], text=True, capture_output=True, check=False)
        require(result.returncode == 0, f"comparator positive failed for {directory.name}:\n{result.stdout}\n{result.stderr}")
        passed += 1
        if first is None and len(load_json(golden)) >= 2:
            first = directory
    require(passed == captured, f"comparator tested {passed}, expected {captured}")
    require(first is not None, "no captured argv available for negative comparator test")

    swapped = load_json(first / "argv.json")
    swapped[0], swapped[1] = swapped[1], swapped[0]
    with tempfile.TemporaryDirectory(prefix="morid-comparator-") as temporary:
        candidate = Path(temporary) / "candidate.json"
        candidate.write_text(json.dumps(swapped), encoding="utf-8")
        result = subprocess.run([str(comparator), first.name, "--candidate", str(candidate)], text=True, capture_output=True, check=False)
    require(result.returncode == 1, f"comparator negative returned {result.returncode}, expected 1")
    require("DIFFERENT" in result.stdout, "comparator negative has no DIFFERENT output")
    print(f"comparator_positive={passed}/{captured} exit=0")
    print(f"comparator_negative={first.name} exit={result.returncode} {result.stdout.splitlines()[-1]}")


def main() -> int:
    captured, not_captured = verify_scenarios()
    print(f"manifest_cases=40 captured={captured} not_captured={not_captured}")
    print(f"argv_transcripts_verified={captured}")
    stream_count = verify_streams()
    print(f"stream_backends=7 stream_captures={stream_count} awkward=CR,partial-line,non-UTF-8")
    settings_count, history = verify_userdata()
    print(f"userdata_load_exit=0 settings_keys={settings_count} history_entries={history} anonymized_hosts=example.com")
    verify_comparator(captured)
    fixture_bytes = directory_size(FIXTURES)
    userdata_bytes = directory_size(FIXTURES / "userdata")
    require(fixture_bytes < 2 * 1024 * 1024, f"fixtures are too large: {fixture_bytes}")
    require(userdata_bytes < 200 * 1024, f"userdata is too large: {userdata_bytes}")
    forbidden = [path for path in FIXTURES.rglob("*") if path.is_file() and path.suffix.lower() in {".mp4", ".mp3", ".mkv", ".webm", ".jpg", ".jpeg", ".png", ".gif", ".exe", ".dll", ".so"}]
    require(not forbidden, f"binary/media artifacts found: {forbidden}")
    print(f"fixture_bytes={fixture_bytes} userdata_bytes={userdata_bytes} binary_media_artifacts=0")
    print("VERIFY_OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
