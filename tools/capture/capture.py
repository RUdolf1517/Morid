#!/usr/bin/env python3
"""Recreate the Phase 0 argv fixtures from the unmodified C++ application."""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

from canonicalise import Canonicaliser, CanonicalisationError, escape_qt_ini_value


ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "fixtures"
SCENARIOS = FIXTURES / "scenarios"
CAPTURE_TOOLS = ROOT / "tools" / "capture"
DEFAULT_RUNTIME = Path("/home/user/.cache/morid-capture/runtime")
CAPTURE_DATE = "2026-11-05"
APP_VERSION = "5.6.8"
QT_VERSION = "5.15.16"
VERSIONS = {
    "yt-dlp": "2026.10.26",
    "gallery-dl": "1.32.19",
    "wget": "1.21.3",
    "aria2c": "1.37.0",
    "you-get": "0.4.1743",
    "svtplay-dl": "4.211",
    "custom": "not applicable (argv recording wrapper)",
    "capture-custom": "1.0.0 (extension VersionArgument)",
}
URL = "https://example.com/v/fixture-01"
URL_TWO = "https://example.com/v/fixture-02"


def expand(value: Any, case_runtime: Path) -> Any:
    """Expand the {case-dir} token in a scenario definition into its real directory.

    Harness paths (a download folder, an archive or cookie file) belong to the
    case, not to the shared runtime root, so the capture can turn them into
    <case-dir>/... without inventing a second placeholder.
    """
    if isinstance(value, str):
        return value.replace("{case-dir}", str(case_runtime))
    if isinstance(value, list):
        return [expand(item, case_runtime) for item in value]
    if isinstance(value, dict):
        return {key: expand(item, case_runtime) for key, item in value.items()}
    return value


def canonicaliser(case_runtime: Path) -> Canonicaliser:
    """The substitutions this case's fixtures are written with."""
    return Canonicaliser(case_runtime, home=os.environ.get("HOME"), forbidden=(ROOT,))


def step(action: str, target: str = "", value: str = "", **extra: Any) -> dict[str, Any]:
    result = {"action": action, "target": target}
    if value or action in {"text", "plainText", "combo"}:
        result["value"] = value
    result.update(extra)
    return result


def basic(engine: str = "yt-dlp", url: str = URL, options: str = "", button: str = "pbDownload") -> list[dict[str, Any]]:
    return [
        step("tab", "tabWidget", index=0),
        step("combo", "cbEngineType", engine),
        step("text", "lineEditURL", url),
        step("text", "lineEditOptions", options),
        step("click", button),
    ]


def playlist(options: str = "", range_options: str = "") -> list[dict[str, Any]]:
    return [
        step("tab", "tabWidget", index=2),
        step("combo", "cbEngineTypePD", "yt-dlp"),
        step("text", "lineEditPLUrl", "https://example.com/playlist/fixture"),
        step("text", "lineEditPLUrlOptions", options),
        step("text", "lineEditPLDownloadRange", range_options),
        step("click", "pbPLGetList"),
    ]


def batch(options: str = "", urls: list[str] | None = None, engine: str = "yt-dlp") -> list[dict[str, Any]]:
    result = [
        step("tab", "tabWidget", index=1),
        step("combo", "cbEngineTypeBD", engine),
        step("text", "lineEditBDUrlOptions", options),
    ]
    for url in urls or [URL]:
        result.extend([step("text", "lineEditBDUrl", url), step("click", "pbBDAdd")])
    result.append(step("click", "pbBDDownload"))
    return result


def batch_row(options: str = "") -> list[dict[str, Any]]:
    return [
        step("tab", "tabWidget", index=1),
        step("combo", "cbEngineTypeBD", "yt-dlp"),
        step("text", "lineEditBDUrlOptions", options),
        step("text", "lineEditBDUrl", URL),
        step("click", "pbBDAdd"),
        step("selectRow", "tableWidgetBD", row=0),
    ]


def case(case_id: str, feature: str, backend: str, steps: list[dict[str, Any]], **extra: Any) -> dict[str, Any]:
    result = {
        "id": case_id,
        "feature": feature,
        "backend": backend,
        "steps": steps,
        "expected": 1,
    }
    result.update(extra)
    return result


def custom_extension(name: str, args: list[str], **values: Any) -> dict[str, Any]:
    result: dict[str, Any] = {
        "Name": name,
        "Version": "4",
        "RequiredMinimumVersionOfMediaDownloader": "2.2.0",
        "AutoUpdate": False,
        "BackendPath": "${default}",
        "CanDownloadPlaylist": False,
        "LikeYoutubeDl": False,
        "OptionsArgument": "",
        "DefaultDownLoadCmdOptions": [],
        "DefaultListCmdOptions": [],
        "ReplaceOutputWithProgressReport": False,
        "VersionArgument": "--version",
        "VersionStringLine": 0,
        "VersionStringPosition": 0,
        "Cmd": {"Generic": {"amd64": {"Name": args[0], "Args": args}}},
    }
    result.update(values)
    return result


def definitions() -> list[dict[str, Any]]:
    # presetOptions rewrites the bundled height selector to format_note at load.
    preset = "-f bestvideo[format_note*=144p][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[format_note*=144p]+bestaudio"
    substitutions = custom_extension(
        "custom-template",
        ["capture-custom", "${BackendPath}/${CommandName}"],
        DefaultDownLoadCmdOptions=[
            "--data-path",
            "{MediaDownloaderDataPath}/result.json",
            "--download-path",
            "{MediaDownloaderDefaultDownloadPath}/result.bin",
            "--cwd",
            "{MediaDownloaderCWD}/working",
        ],
    )
    current_os = custom_extension("custom-current-os", ["capture-custom"], DefaultDownLoadCmdOptions=["--current-os"])
    other_arch = custom_extension("custom-other-arch", ["capture-custom"])
    other_arch["Cmd"] = {"Generic": {"aarch64": {"Name": "capture-custom", "Args": ["capture-custom"]}}}
    extra = custom_extension(
        "custom-extra-options",
        ["capture-custom"],
        DefaultDownLoadCmdOptions=["--engine-default"],
        ExtraOptions=["--extra-from-extension", "two words"],
    )
    packaging = custom_extension(
        "custom-packaging",
        ["capture-custom"],
        DefaultDownLoadCmdOptions=["--engine-default"],
        ExtraOptions=["--ordinary-extra"],
        ExtraOptionsFlatpakAppImage=["--packaging-extra", "flatpak appimage"],
    )

    return [
        case("s01-plain-yt-dlp", "F-03", "yt-dlp", basic()),
        case("s02-bundled-preset", "F-06", "yt-dlp", basic(options=preset)),
        case("s03-user-format-22", "F-02", "yt-dlp", basic(options="-f 22")),
        case("s04-conflicting-user-options", "F-02", "yt-dlp", basic(options="-f 22 -f 18 --no-playlist --yes-playlist")),
        case("s05-format-list", "F-01", "yt-dlp", basic(button="pbList")),
        case("s06-playlist-whole", "F-20", "yt-dlp", playlist()),
        case("s07-playlist-item-range", "F-21", "yt-dlp", playlist(range_options="--playlist-items 2-3")),
        case("s08-playlist-no-playlist-last", "F-21", "yt-dlp", playlist(options="--yes-playlist --no-playlist")),
        case("s09-download-archive", "F-23", "yt-dlp", basic(options="--download-archive \"{case-dir}/archive folder/archive.txt\""), settings={"UseInternalArchiveFile": False}),
        case("s10-batch-two-urls", "F-10", "yt-dlp", batch(urls=[URL, URL_TWO]), expected=2, settings={"ConcurrentDownloading": False, "MaxConcurrentDownloads": 1, "ShowMetaDataInBatchDownloader": False}),
        case("s11-cookies-browser", "F-55", "yt-dlp", basic(), settings={"CookieSourceSetToBrowerName": True, "CookieBrowser_yt-dlp": "fixture-browser"}),
        case("s12-cookies-file", "F-55", "yt-dlp", basic(), settings={"CookieSourceSetToBrowerName": False, "CookieTextFilePath_yt-dlp": "{case-dir}/cookie files/fixture-cookies.txt"}),
        case("s13-proxy-none", "F-54", "wget", basic(engine="wget"), settings={"ProxySettingsType": "None", "BasicDownloaderDefaultEngine": "wget"}),
        case("s14-proxy-manual", "F-54", "wget", basic(engine="wget"), settings={"ProxySettingsType": "Manual", "ProxySettingsCustomSource": "http://user:pass@host:8080", "BasicDownloaderDefaultEngine": "wget"}),
        case("s15-proxy-env", "F-54", "wget", basic(engine="wget"), settings={"ProxySettingsType": "Env", "BasicDownloaderDefaultEngine": "wget"}, environment={"http_proxy": "http://user:pass@host:8080", "https_proxy": "http://user:pass@host:8080"}),
        case("s16-subtitles-list", "F-08", "yt-dlp", batch_row() + [step("contextMenu", "tableWidgetBD", "Show Subtitles")], settings={"ShowMetaDataInBatchDownloader": False}),
        case("s17-subtitle-languages", "F-08", "yt-dlp", basic(options="--write-subs --sub-langs \"en,pl\"")),
        case("s18-comments", "F-09", "yt-dlp", batch_row() + [step("contextMenu", "tableWidgetBD", "Show Comments")], settings={"ShowMetaDataInBatchDownloader": False}),
        case("s19-split-chapters", "F-15", "yt-dlp", batch_row(options="-o \"chapter:%(title)s-%(section_number)02d.%(ext)s\"") + [
            step("contextMenu", "tableWidgetBD", "Download Media Part"),
            step("text", "lineEditChapters", "00:00:00-00:00:01,00:00:01-00:00:02"),
            step("check", "cbSplitByChapters", checked=True),
            step("click", "pbSetTimeIntervals"),
            step("click", "pbBDDownload"),
        ], settings={"ShowMetaDataInBatchDownloader": False, "AutoSetDefaultEngineAndOptions": True}),
        case("s20-output-dir-space-unicode", "F-50", "yt-dlp", basic(options="-P \"{case-dir}/output dir/Загрузки\""), download_dir="{case-dir}/output dir/Загрузки", record_download_folder_reset=True),
        case("s21-user-output-template", "F-50", "yt-dlp", basic(options="-o \"%(title)s from user.%(ext)s\"")),
        case("s22-url-metacharacters", "F-50", "yt-dlp", basic(url="https://example.com/watch?a=1&b=two#frag%20x")),
        case("s23-url-manager", "F-52", "gallery-dl", basic(url="https://route.example/v/fixture"), data_files={"downloadDefaultOptions.json": [{"url": "route.example", "engine": "gallery-dl", "downloadOption": "--route-option \"two words\""}]}),
        case("s24-custom-extension-substitution", "F-39", "custom", basic(engine="custom-template"), extensions={"custom-template.json": substitutions}, settings={"BasicDownloaderDefaultEngine": "custom-template"}),
        case("s25-cmd-current-os", "F-31", "custom", basic(engine="custom-current-os"), extensions={"custom-current-os.json": current_os}, settings={"BasicDownloaderDefaultEngine": "custom-current-os"}),
        case("s26-cmd-other-architecture", "F-31", "custom", [step("tab", "tabWidget", index=0), step("text", "lineEditURL", URL), step("text", "lineEditOptions", ""), step("click", "pbDownload")], extensions={"custom-other-arch.json": other_arch}, settings={"BasicDownloaderDefaultEngine": "custom-other-arch"}, observation="x86_64 has no matching Cmd; no engine is selected manually, so the wrapper records the original's actual fallback"),
        case("s27-extra-options", "F-30", "custom", basic(engine="custom-extra-options"), extensions={"custom-extra-options.json": extra}, settings={"BasicDownloaderDefaultEngine": "custom-extra-options"}),
        case("s28-auto-update-false", "F-34", None, [], subject="AutoUpdate / versionInfo", impossible="AutoUpdate is handled by QNetworkAccess/versionInfo, not by spawning a backend update command; there is no external update argv for the wrapper to capture"),
        case("s29-gallery-dl-default", "F-31", "gallery-dl", basic(engine="gallery-dl"), settings={"BasicDownloaderDefaultEngine": "gallery-dl"}),
        case("s30-gallery-dl-engine-default", "F-02", "gallery-dl", basic(engine="gallery-dl"), settings={"BasicDownloaderDefaultEngine": "gallery-dl"}, data_files={"downloadEngineDefaultOptions.json": [{"engineName": "gallery-dl", "options": "--per-engine-default \"two words\"", "default": "yes"}]}),
        case("s31-wget", "F-31", "wget", basic(engine="wget"), settings={"BasicDownloaderDefaultEngine": "wget"}),
        case("s32-aria2c", "F-31", "aria2c", basic(engine="aria2c"), settings={"BasicDownloaderDefaultEngine": "aria2c"}),
        case("s33-you-get", "F-31", "you-get", basic(engine="you-get"), settings={"BasicDownloaderDefaultEngine": "you-get"}, require_interpreter=True),
        case("s34-svtplay-dl", "F-31", "svtplay-dl", basic(engine="svtplay-dl"), settings={"BasicDownloaderDefaultEngine": "svtplay-dl"}),
        case("s35-unknown-extension", "F-30", "yt-dlp", [step("tab", "tabWidget", index=0), step("text", "lineEditURL", URL), step("click", "pbDownload")], settings={"BasicDownloaderDefaultEngine": "extension-does-not-exist"}, expect_empty=True, requested_extension="extension-does-not-exist", observed_behaviour="unknown extension name rejected; fell back to the default engine"),
        case("s36-missing-required-key", "F-30", "capture-custom", [step("tab", "tabWidget", index=0), step("text", "lineEditURL", URL), step("click", "pbDownload")], extensions={"missing-name.json": custom_extension("temporary", ["capture-custom"])}, missing_name=True, settings={"BasicDownloaderDefaultEngine": ""}, expect_empty=True, requested_extension=None, extension_file="missing-name.json", observed_behaviour="extension with Name removed still loaded and executed; argv was the URL alone; no fallback", reason="the extension with Name removed was still loaded and executed; the captured argv is the URL alone and no fallback occurred, so the task's expected empty argv is not the reference behaviour"),
        case("s37-audio-extraction", "F-02", "yt-dlp", basic(options="-x --audio-format mp3 -f bestaudio")),
        case("s38-command-after-download", "F-60", "yt-dlp", basic(), post_command=True, fake_download="fixture-output.bin"),
        case("s39-non-utf8-encoding", "F-56", "yt-dlp", basic(), settings={"TextEncoding_yt-dlp": "ISO-8859-1"}),
        case("s40-flatpak-appimage-extra", "F-30", "custom", basic(engine="custom-packaging"), extensions={"custom-packaging.json": packaging}, settings={"BasicDownloaderDefaultEngine": "custom-packaging"}, environment={"APPIMAGE": "/home/user/.cache/morid-capture/runtime/Morid.AppImage"}),
    ]


def json_write(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def write_records(path: Path, records: list[dict[str, Any]]) -> None:
    """Write wrapper JSONL in the exact byte form backend_wrapper.py writes."""
    text = "".join(json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n" for record in records)
    path.write_text(text, encoding="utf-8")


def load_records(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        return []
    records = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            records.append(json.loads(line))
    return records


def reference_commit() -> str:
    # origin/main, not the possibly stale local main branch, identifies the
    # unmodified legacy tree. It stays fixed when capture helpers are committed.
    result = subprocess.run(["git", "merge-base", "HEAD", "origin/main"], cwd=ROOT, text=True, capture_output=True, check=False)
    return result.stdout.strip() if result.returncode == 0 else "17c0104b8824bdad7620b2e5127484ca42dde98e"


def prepare_case(definition: dict[str, Any], runtime: Path) -> dict[str, Any]:
    case_runtime = runtime / definition["id"]
    if case_runtime.exists():
        shutil.rmtree(case_runtime)
    definition = expand(definition, case_runtime)
    app_data = case_runtime / "appdata"
    bin_dir = app_data / "bin"
    engines_dir = app_data / "engines.v1"
    data_dir = app_data / "data"
    download_dir = Path(definition.get("download_dir", str(case_runtime / "downloads")))
    for directory in (bin_dir, engines_dir, data_dir, download_dir):
        directory.mkdir(parents=True, exist_ok=True)

    for source in (ROOT / "extensions").glob("*.json"):
        shutil.copy2(source, engines_dir / source.name)
    for name, value in definition.get("extensions", {}).items():
        if definition.get("missing_name"):
            value = dict(value)
            value.pop("Name", None)
        (engines_dir / name).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    wrapper = CAPTURE_TOOLS / "backend_wrapper.py"
    wrapper.chmod(0o755)
    command_names = {
        "yt-dlp", "yt-dlp-nightly", "gallery-dl.bin", "wget", "stdbuf", "aria2c",
        "you-get", "svtplay-dl", "get-sauce", "lux", "capture-custom", "qjs", "qjs-linux-x86_64", "ffmpeg", "ffprobe", "safaribooks.py",
    }
    for name in command_names:
        (bin_dir / name).symlink_to(wrapper)

    settings: dict[str, Any] = {
        "BasicDownloaderDefaultEngine": "yt-dlp",
        "BatchDownloaderDefaultEngine": "yt-dlp",
        "PlaylistDownloaderDefaultEngine": "yt-dlp",
        "DownloadFolder": str(download_dir),
        "LibraryDownloadFolder": str(download_dir),
        "UseSystemEngine": True,
        "UseSystemSupportingEngine": True,
        "SingleInstance": False,
        "ShowTrayIcon": False,
        "ShowVersionInfoAndAutoDownloadUpdates": False,
        "ShowLocalAndLatestVersionInformation": False,
        "ShowLocalVersionInformationOnly": False,
        "CheckForUpdates": False,
        "CheckForEnginesUpdates": False,
        "SaveDownloadHistory": False,
        "AutoSavePlaylistOnExit": False,
        "PlaylistDownloaderSaveHistory": False,
        "ProxySettingsType": "None",
        "Language": "en_US",
        "ConcurrentDownloading": False,
        "MaxConcurrentDownloads": 1,
    }
    settings.update(definition.get("settings", {}))
    settings_path = app_data / "settings" / "settings.ini"
    settings_path.parent.mkdir(parents=True, exist_ok=True)
    with settings_path.open("w", encoding="utf-8", newline="\n") as output:
        output.write("[General]\n")
        for key, value in settings.items():
            if isinstance(value, bool):
                value = "true" if value else "false"
            # Qt 5 writes ini values it owns as escaped ASCII and reads a raw
            # UTF-8 file back as Latin-1; s20's cyrillic download folder only
            # survives the round trip in Qt's dialect.
            output.write(f"{key}={escape_qt_ini_value(str(value))}\n")

    for name, value in definition.get("data_files", {}).items():
        json_write(data_dir / name, value)

    steps_path = case_runtime / "steps.json"
    json_write(steps_path, definition["steps"])
    transcript = case_runtime / "transcript.jsonl"
    post_transcript = case_runtime / "post-transcript.jsonl"
    post_hook = bin_dir / "post-hook"
    if definition.get("post_command"):
        # CAPTURE_PROGRAM records the program the original started (the hook)
        # rather than the wrapper that replaces it, so post-log.txt does not
        # depend on where this repository happens to live.
        post_hook.write_text(
            "#!/bin/sh\n"
            f"CAPTURE_TRANSCRIPT='{post_transcript}' CAPTURE_PROGRAM=\"$0\" exec '{wrapper}' \"$@\"\n",
            encoding="utf-8",
        )
        post_hook.chmod(0o755)
        with settings_path.open("a", encoding="utf-8") as output:
            output.write(f'CommandOnSuccessfulDownload="{post_hook} --label \\"two words\\""\n')

    environment = os.environ.copy()
    for key in ("http_proxy", "https_proxy", "HTTP_PROXY", "HTTPS_PROXY", "all_proxy", "ALL_PROXY", "no_proxy", "NO_PROXY"):
        environment.pop(key, None)
    environment.update(definition.get("environment", {}))
    environment.update({
        "MediaDownloaderAppDataPath": str(app_data),
        "CAPTURE_STEPS": str(steps_path),
        "CAPTURE_TRANSCRIPT": str(transcript),
        "CAPTURE_PROBE_TRANSCRIPT": str(case_runtime / "probe-transcript.jsonl"),
        "CAPTURE_UI_LOG": str(case_runtime / "ui.log"),
        "CAPTURE_DRIVER_ERROR": str(case_runtime / "driver-error.txt"),
        "CAPTURE_SETTLE_MS": "1800",
        "QT_QPA_PLATFORM": "minimal",
        "LD_LIBRARY_PATH": str(Path(os.environ["QT_PREFIX"]) / "lib"),
        "LD_PRELOAD": os.environ["CAPTURE_DRIVER"],
    })
    if definition.get("fake_download"):
        environment["CAPTURE_FAKE_DOWNLOAD_NAME"] = definition["fake_download"]

    return {
        "runtime": case_runtime,
        "transcript": transcript,
        "post_transcript": post_transcript,
        "download_dir": str(download_dir),
        "environment": environment,
    }


def run_case(definition: dict[str, Any], prepared: dict[str, Any], legacy: Path) -> tuple[int, str]:
    app_log = prepared["runtime"] / "app.log"
    with app_log.open("w", encoding="utf-8") as output:
        result = subprocess.run(
            [str(legacy), "--debug", "-s"],
            cwd=prepared["runtime"],
            env=prepared["environment"],
            stdout=output,
            stderr=subprocess.STDOUT,
            timeout=45,
            check=False,
        )
    error_path = prepared["runtime"] / "driver-error.txt"
    error = error_path.read_text(encoding="utf-8").strip() if error_path.exists() else ""
    return result.returncode, error


def finalize_case(definition: dict[str, Any], prepared: dict[str, Any], returncode: int, driver_error: str) -> dict[str, Any]:
    case_id = definition["id"]
    # Canonicalise first: a path that cannot be expressed with the documented
    # placeholders must abort the capture, not leave a half-written fixture.
    transform = canonicaliser(prepared["runtime"])
    records = [transform.record(record) for record in load_records(prepared["transcript"])]
    post_records = [transform.record(record) for record in load_records(prepared["post_transcript"])]

    destination = SCENARIOS / case_id
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True, exist_ok=True)
    expected = definition["expected"]
    reason = ""
    status = "captured"

    if definition.get("impossible"):
        status, reason = "not-captured", definition["impossible"]
    elif driver_error:
        status, reason = "not-captured", f"Qt driver: {driver_error}; app exit {returncode}; see app.log"
    elif returncode != 0:
        status, reason = f"not-captured", f"legacy exited {returncode}; see app.log"
    elif definition.get("require_interpreter"):
        if not records or not ({"-u", "--format"} & set(records[0]["argv"])) or "-u" not in records[0]["argv"]:
            status = "not-captured"
            reason = "the shipped Linux Generic Cmd invokes you-get directly; the interpreter invocation exists only in the Windows Cmd branch, unavailable on this Linux x86_64 capture host"
    elif definition.get("expect_empty"):
        if records:
            status = "not-captured"
            reason = definition.get("reason") or definition.get("expect_empty_reason") or (
                "the original did not produce the task's expected empty argv; "
                "it logged the extension error and fell back to another engine (observed argv is preserved)"
            )
    elif len(records) != expected:
        status = "not-captured"
        reason = f"expected {expected} backend invocation(s), captured {len(records)}; see app.log"
    if definition.get("post_command") and len(post_records) != 1 and status == "captured":
        status = "not-captured"
        reason = f"expected one post-download command, captured {len(post_records)}; see app.log"

    files: list[str] = []
    method = "wrapper" if records else "log"
    # Startup UI logs contain asynchronously ordered engine probes and are not
    # scenario evidence. Keep the deterministic driver/command log for gaps.
    source = prepared["runtime"] / "app.log"
    if source.exists() and status != "captured":
        (destination / "app.log").write_text(transform.text(source.read_text(encoding="utf-8")), encoding="utf-8")
        files.append("app.log")

    if records:
        write_records(destination / "log.txt", records)
        files.append("log.txt")
        if status == "captured":
            json_write(destination / "argv.json", records[0]["argv"])
            files.append("argv.json")
            if len(records) > 1:
                json_write(destination / "invocations.json", [record["argv"] for record in records])
                json_write(destination / "argv-2.json", records[1]["argv"])
                files.extend(["invocations.json", "argv-2.json"])
        else:
            json_write(destination / "observed-argv.json", [record["argv"] for record in records])
            files.append("observed-argv.json")
    elif status == "captured" and definition.get("expect_empty"):
        json_write(destination / "argv.json", [])
        files.append("argv.json")
        source = prepared["runtime"] / "app.log"
        if source.exists():
            (destination / "app.log").write_text(transform.text(source.read_text(encoding="utf-8")), encoding="utf-8")
            if "app.log" not in files:
                files.append("app.log")

    if post_records:
        write_records(destination / "post-log.txt", post_records)
        json_write(destination / "post-argv.json", post_records[0]["argv"])
        files.extend(["post-log.txt", "post-argv.json"])

    backend = definition["backend"]
    meta = {
        "case_id": case_id,
        "backend": backend,
        # A case that spawns nothing (s28) names no engine and no version.
        "backend_version": VERSIONS[backend] if backend else None,
        "app_version": APP_VERSION,
        "qt_version": QT_VERSION,
        "os": "Linux",
        "arch": platform.machine(),
        "capture_date": CAPTURE_DATE,
        "method": method,
        "reference_commit": reference_commit(),
        "backend_invocations": len(records),
        "status": status,
    }
    if definition.get("record_download_folder_reset"):
        # The reset is only observable through where the backend actually ran:
        # settings::downloadFolder() passes a default-constructed logger, so its
        # "Resetting download folder to default" line never reaches stdout.
        configured = transform.value(str(prepared["download_dir"]))
        meta["download_folder"] = configured
        meta["download_folder_reset"] = bool(records) and records[0]["cwd"] != configured
    if definition.get("subject"):
        meta["subject"] = definition["subject"]
    for name in ("requested_extension", "extension_file", "observed_behaviour"):
        if name in definition:
            meta[name] = definition[name]
    if definition.get("observation"):
        meta["observation"] = definition["observation"]
    if reason:
        meta["reason"] = reason
    json_write(destination / "meta.json", meta)
    files.append("meta.json")

    manifest_entry = {
        "id": case_id,
        "feature": definition["feature"],
        "status": status,
        "method": method,
        "files": [f"fixtures/scenarios/{case_id}/{name}" for name in sorted(set(files))],
    }
    if reason:
        manifest_entry["reason"] = reason
    return manifest_entry


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--legacy", type=Path, default=Path("/home/user/.cache/morid-capture/legacy-build/media-downloader"))
    parser.add_argument("--runtime", type=Path, default=DEFAULT_RUNTIME)
    parser.add_argument("--only", action="append", help="capture only this case ID (repeatable)")
    args = parser.parse_args()

    if "QT_PREFIX" not in os.environ or "CAPTURE_DRIVER" not in os.environ:
        parser.error("set QT_PREFIX and CAPTURE_DRIVER; see fixtures/README.md")
    if not args.legacy.is_file():
        parser.error(f"legacy executable not found: {args.legacy}")

    selected = definitions()
    if args.only:
        selected = [item for item in selected if item["id"] in args.only]
    if not selected:
        parser.error("no matching cases")

    entries = []
    for definition in selected:
        print(f"capture {definition['id']}", flush=True)
        prepared = prepare_case(definition, args.runtime)
        try:
            returncode, driver_error = run_case(definition, prepared, args.legacy)
        except subprocess.TimeoutExpired:
            returncode, driver_error = 124, "legacy exceeded the 45 second capture timeout"
        try:
            entry = finalize_case(definition, prepared, returncode, driver_error)
        except CanonicalisationError as error:
            print(f"CANONICALISATION_FAILED {definition['id']}: {error}", file=sys.stderr, flush=True)
            return 1
        print(f"  {entry['status']}: {entry.get('reason', 'ok')}", flush=True)
        entries.append(entry)

    if not args.only:
        manifest_path = FIXTURES / "manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8")) if manifest_path.exists() else {}
        manifest.update({
            "schema_version": 1,
            "reference": {"app_version": APP_VERSION, "qt_version": QT_VERSION, "commit": reference_commit()},
            "cases": entries,
        })
        json_write(manifest_path, manifest)

    captured = sum(entry["status"] == "captured" for entry in entries)
    print(f"captured {captured}/{len(entries)}")
    return 0 if (args.only or captured >= 35) else 1


if __name__ == "__main__":
    raise SystemExit(main())
