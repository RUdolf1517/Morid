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

from backend_wrapper import PROXY_VARIABLES
from capture import ROOT, FIXTURES, SCENARIOS, definitions
from capture_userdata import FINAL as USERDATA, history_count, settings_seed
from canonicalise import CASE_DIR, NODE, escape_qt_ini_value
from test_qt_ini_roundtrip import check_all as check_qt_ini_roundtrip


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
    "capture-custom": "capture-custom",
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

        backend = meta.get("backend")
        invocations = meta.get("backend_invocations")
        require(isinstance(invocations, int), f"{case_id}: backend_invocations is not an integer")
        if invocations == 0:
            # T4: nothing was spawned, so nothing may be named.  This also keeps
            # a null backend out of every PROGRAMS lookup below.
            require(backend is None, f"{case_id}: a case with no backend invocation must not name a spawned backend")
            require(meta.get("backend_version") is None, f"{case_id}: a case with no backend invocation must not name a backend version")
        else:
            require(backend in PROGRAMS, f"{case_id}: meta names an unknown backend {backend!r}")

        if entry["status"] == "not-captured":
            not_captured += 1
            require(entry.get("reason") and meta.get("reason"), f"{case_id}: missing not-captured reason")
            require((directory / "app.log").is_file(), f"{case_id}: no failure evidence app.log")
            if invocations:
                # An observed fallback still has to agree with the engine the
                # metadata now names, so the reason and the evidence cannot drift.
                observed = load_jsonl(directory / "log.txt")
                require(len(observed) == invocations, f"{case_id}: observed invocations differ from meta")
                require(load_json(directory / "observed-argv.json") == [item["argv"] for item in observed],
                        f"{case_id}: observed-argv differs from the transcript")
                for item in observed:
                    require(Path(item["program"]).name == PROGRAMS[backend],
                            f"{case_id}: observed program {item['program']} is not the backend named in meta.json")
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


# An absolute path is only expected where a fixture quotes it or separates it
# with whitespace; "bestvideo[ext=mp4]/bestvideo" and "https://..." are neither.
ABSOLUTE_PATH = re.compile(r'(?:(?<=[\s"\',=\[\(])|^)(/[A-Za-z0-9._~][^\s"\',;\[\]{}()<>|]*)', re.MULTILINE)
NODE_PATH = re.compile(r'node:(/[^\s"\',;\[\]{}()<>|]*)')


def verify_host_paths() -> tuple[int, int]:
    """T6: no scenario file may name a path outside the documented vocabulary."""
    files = 0
    allowed = 0
    offenders = []
    for path in sorted(SCENARIOS.rglob("*")):
        if not path.is_file():
            continue
        files += 1
        text = path.read_text(encoding="utf-8", errors="replace")
        for match in ABSOLUTE_PATH.finditer(text):
            value = match.group(1)
            # /home/user is the documented home placeholder, but the capture
            # cache lives underneath it, so it needs its own condition.
            if value.startswith("/home/user/.cache") or "/morid-capture/" in value:
                offenders.append((path, value))
            elif value == "/home/user" or value.startswith("/home/user/"):
                allowed += 1
            else:
                offenders.append((path, value))
        for match in NODE_PATH.finditer(text):
            # <node> is the only accepted spelling of the interpreter.
            offenders.append((path, "node:" + match.group(1)))
    require(not offenders, "host paths outside the fixture vocabulary (" + ", ".join(
        f"{path.relative_to(ROOT)}: {value}" for path, value in offenders[:5]) + ")")
    print(f"host_paths checked_files={files} allowed_absolute_paths={allowed} outside_vocabulary=0")
    return files, allowed


def verify_download_folder() -> None:
    """T7: settings.ini in Qt's dialect keeps s20 in its configured folder."""
    directory = SCENARIOS / "s20-output-dir-space-unicode"
    meta = load_json(directory / "meta.json")
    require(meta.get("download_folder_reset") is False,
            "s20: the original reset the download folder to the platform default")
    argv = load_json(directory / "argv.json")
    require("<case-dir>/output dir/Загрузки" in argv, "s20: the -P argument lost its cyrillic output directory")
    cwd = load_jsonl(directory / "log.txt")[0]["cwd"]
    require(cwd == "<case-dir>/output dir/Загрузки",
            f"s20: cwd is {cwd!r}, expected the configured download folder")
    require(meta.get("download_folder") == cwd,
            f"s20: meta records {meta.get('download_folder')!r}, the backend ran in {cwd!r}")
    require(escape_qt_ini_value("Загрузки") == "\\x417\\x430\\x433\\x440\\x443\\x437\\x43a\\x438",
            "the harness no longer escapes settings.ini the way Qt 5 writes it")
    print("download_folder s20 cwd=<case-dir>/output dir/Загрузки reset=false settings_ini=qt_escaped")


def verify_settings_ini_roundtrip() -> None:
    """T12: the round trip runs here and in `python3 -m unittest discover`.

    The proof itself lives in `test_qt_ini_roundtrip.py`, next to this file:
    this function only calls it, so there is one implementation and two
    callers, and a reviewer reading either one sees the same checks.
    """
    try:
        summary = check_qt_ini_roundtrip()
    except AssertionError as error:
        raise RuntimeError(f"settings.ini: {error}") from error
    print(f"settings_ini_roundtrip values={summary['values']} fuzz={summary['fuzz']} "
          f"seed={summary['seed']} quoted={summary['quoted']} digit_after_escape=ok ascii_verbatim=3")


def verify_metadata() -> None:
    """T4: the metadata that must describe what actually ran, or nothing at all."""
    cases = {item["id"]: item for item in load_json(FIXTURES / "manifest.json")["cases"]}

    update = load_json(SCENARIOS / "s28-auto-update-false" / "meta.json")
    require(cases["s28-auto-update-false"]["status"] == "not-captured", "s28: status changed")
    require(update.get("backend", "missing") is None, "s28: backend still names an engine that never ran")
    require(update.get("backend_version", "missing") is None, "s28: backend_version still names a version")
    require(update.get("subject") == "AutoUpdate / versionInfo", "s28: subject is not the update path")
    require(update.get("backend_invocations") == 0, "s28: recorded invocations for a case that spawns nothing")

    unknown = load_json(SCENARIOS / "s35-unknown-extension" / "meta.json")
    require(unknown.get("backend") == "yt-dlp", "s35: backend is not the engine that actually ran")
    require(unknown.get("requested_extension") == "extension-does-not-exist", "s35: requested_extension is not the setting under test")
    require("fell back" in unknown.get("observed_behaviour", ""), "s35: observed_behaviour does not record the fallback")
    require(cases["s35-unknown-extension"]["status"] == "not-captured", "s35: status changed")

    missing = load_json(SCENARIOS / "s36-missing-required-key" / "meta.json")
    require(missing.get("backend") == "capture-custom", "s36: backend is not the program that actually ran")
    require("requested_extension" in missing and missing["requested_extension"] is None,
            "s36: requested_extension must be present and null (a file name is not a Name)")
    require(missing.get("extension_file") == "missing-name.json", "s36: extension_file is not the invalid extension")
    require("no fallback occurred" in missing.get("reason", ""), "s36: reason still claims a fallback")
    require(cases["s36-missing-required-key"]["status"] == "not-captured", "s36: status changed")

    return None


def verify_proxy_env(captured: int) -> int:
    """T3: proxy evidence lives in the transcripts, not in duplicate env.json files."""
    duplicates = sorted(path.relative_to(ROOT) for path in SCENARIOS.rglob("env.json"))
    require(not duplicates, f"env.json duplicates the transcript env field: {duplicates}")

    checked = 0
    for directory in sorted(SCENARIOS.iterdir()):
        meta = load_json(directory / "meta.json")
        if meta.get("status") != "captured":
            continue
        env = load_jsonl(directory / "log.txt")[0]["env"]
        require(isinstance(env, dict), f"{directory.name}: transcript env is not an object")
        require(all(isinstance(key, str) and isinstance(value, str) for key, value in env.items()),
                f"{directory.name}: transcript env is not an object of strings")
        require(set(env) <= set(PROXY_VARIABLES), f"{directory.name}: transcript env has a key the wrapper does not record")
        checked += 1

    manual = load_json(SCENARIOS / "s14-proxy-manual" / "argv.json")
    manual_env = load_jsonl(SCENARIOS / "s14-proxy-manual" / "log.txt")[0]["env"]
    env_mode = load_json(SCENARIOS / "s15-proxy-env" / "argv.json")
    env_mode_env = load_jsonl(SCENARIOS / "s15-proxy-env" / "log.txt")[0]["env"]
    require(manual_env == {}, "s14: manual proxy mode must leave the child environment empty")
    require(env_mode_env == {"http_proxy": "http://user:pass@host:8080", "https_proxy": "http://user:pass@host:8080"},
            "s15: env proxy mode must export the proxy variables to the child")
    require(manual == env_mode, "s14/s15: the two proxy sources are expected to produce the same wget argv")
    for name, argv in (("s14", manual), ("s15", env_mode)):
        require("--proxy-user=user" in argv and "--proxy-password=pass" in argv, f"{name}: proxy credentials are not passed as arguments")
        require(not any("user:pass@" in item for item in argv), f"{name}: proxy credentials were interpolated into an argument")
    print(f"proxy_env checked={checked}/{captured} env_json_files=0 "
          f"s14_env=empty s15_env={sorted(env_mode_env)} credentials=arguments")
    return checked


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


def remote_candidate(golden: list, case_id: str) -> tuple[list, str]:
    """Rewrite a golden as a candidate from another machine would print it."""
    case_dir = f"/home/ci/morid-runtime/{case_id}"
    return [
        item.replace(CASE_DIR, case_dir).replace("node:" + NODE, "node:/opt/node/bin/node")
        for item in golden
    ], case_dir


def verify_comparator_canonicalised(captured: int) -> tuple[int, int]:
    """T1: a candidate that prints real paths still has to match the golden."""
    comparator = ROOT / "tools" / "compare-argv"
    checked = 0
    inferred = 0
    negative = None
    for directory in sorted(SCENARIOS.iterdir()):
        golden_path = directory / "argv.json"
        if not golden_path.exists() or load_json(directory / "meta.json")["status"] != "captured":
            continue
        golden = load_json(golden_path)
        candidate, case_dir = remote_candidate(golden, directory.name)
        carries_case_dir = any(item == CASE_DIR or item.startswith(CASE_DIR + "/") for item in golden)
        with tempfile.TemporaryDirectory(prefix="morid-canonicalise-") as temporary:
            path = Path(temporary) / "candidate.json"
            path.write_text(json.dumps(candidate), encoding="utf-8")
            command = [str(comparator), directory.name, "--candidate", str(path), "--canonicalise"]
            if not carries_case_dir:
                command.extend(["--case-dir", case_dir])
            result = subprocess.run(command, text=True, capture_output=True, check=False)
            require(result.returncode == 0,
                    f"canonicalising comparator failed for {directory.name}:\n{result.stdout}\n{result.stderr}")
            require("canonicalise: case-dir=" in result.stdout, f"{directory.name}: comparator did not report canonicalisation")
        inferred += carries_case_dir
        checked += 1
        if negative is None and len(candidate) >= 2:
            negative = (directory.name, case_dir, carries_case_dir)
            swapped = list(candidate)
            swapped[0], swapped[1] = swapped[1], swapped[0]
            with tempfile.TemporaryDirectory(prefix="morid-canonicalise-") as temporary:
                path = Path(temporary) / "candidate.json"
                path.write_text(json.dumps(swapped), encoding="utf-8")
                command = [str(comparator), directory.name, "--candidate", str(path), "--canonicalise"]
                if not carries_case_dir:
                    command.extend(["--case-dir", case_dir])
                result = subprocess.run(command, text=True, capture_output=True, check=False)
            require(result.returncode == 1, f"canonicalised negative comparator returned {result.returncode}, expected 1")
            require("DIFFERENT" in result.stdout, "canonicalised negative comparator has no DIFFERENT output")
    require(checked == captured, f"canonicalising comparator checked {checked}, expected {captured}")
    require(negative is not None, "no candidate available for the canonicalised negative test")
    print(f"comparator_canonicalised={checked}/{captured} exit=0 case_dir_inferred={inferred} "
          f"case_dir_explicit={checked - inferred} negative={negative[0]} exit=1")
    return checked, inferred


def main() -> int:
    captured, not_captured = verify_scenarios()
    print(f"manifest_cases=40 captured={captured} not_captured={not_captured}")
    print(f"argv_transcripts_verified={captured}")
    stream_count = verify_streams()
    print(f"stream_backends=7 stream_captures={stream_count} awkward=CR,partial-line,non-UTF-8")
    settings_count, history = verify_userdata()
    print(f"userdata_load_exit=0 settings_keys={settings_count} history_entries={history} anonymized_hosts=example.com")
    verify_comparator(captured)
    verify_comparator_canonicalised(captured)
    verify_host_paths()
    verify_download_folder()
    verify_settings_ini_roundtrip()
    verify_metadata()
    print("metadata checks=s28_backend_null,s28_subject,s35_requested_extension,s35_fallback,s36_no_fallback")
    proxy_env = verify_proxy_env(captured)
    require(proxy_env == captured, "proxy env check did not cover every captured case")
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
