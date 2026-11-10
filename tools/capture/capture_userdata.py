#!/usr/bin/env python3
"""Build and verify an anonymized data directory with the original application.

A disposable settings seed supplies defaults for older/read-only keys. The C++
application itself creates the presets, URL rules, engine defaults, subscription,
auto-saved list, download history, window state, and themes. Only disposable
paths are anonymized in the resulting copy. Existing fixtures are not rewritten
unless --force is supplied.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import threading
from pathlib import Path
from typing import Any

from capture_streams import Handler, PORT, BASE, VENV
from qt_ini import read_settings_ini


ROOT = Path(__file__).resolve().parents[2]
FINAL_ROOT = ROOT / "fixtures" / "userdata"
FINAL = FINAL_ROOT / "media-downloader"
WORK = Path("/home/user/.cache/morid-capture/userdata-work")
VERIFY = Path("/home/user/.cache/morid-capture/userdata-verify")
LEGACY = Path("/home/user/.cache/morid-capture/legacy-build/media-downloader")
DRIVER = Path("/home/user/.cache/morid-capture/qt-driver.so")
WRAPPER = ROOT / "tools" / "capture" / "backend_wrapper.py"
CAPTURE_DATE = "2026-11-05"


def settings_seed() -> dict[str, Any]:
    # Includes the current dynamic families and legacy keys that 5.6.8 reads.
    # Values are application defaults, except harmless example user choices.
    result: dict[str, Any] = {
        "AutoDownload": False,
        "AutoDownloadWhenAddedInBatchDownloader": False,
        "AutoHideDownloadWhenCompleted": False,
        "AutoSavePlaylistOnExit": True,
        "AutoSetDefaultEngineAndOptions": False,
        "BackgroundClipboardMonitor": False,
        "BasicDownloaderDefaultEngine": "yt-dlp",
        "BatchDownloaderDefaultEngine": "yt-dlp",
        "PlaylistDownloaderDefaultEngine": "yt-dlp",
        "BasicDownloaderMonitorClipboadUrl": False,
        "BatchDownloaderMonitorClipboadUrl": False,
        "PlaylistDownloaderMonitorClipboadUrl": False,
        "BasicDownloaderOptionsHistory-yt-dlp": "--history-option",
        "BatchDownloaderOptionsHistory-yt-dlp": "--batch-option",
        "PlaylistDownloaderOptionsHistory-yt-dlp": "--flat-playlist",
        "PlaylistDownloaderSaveHistory": True,
        "CheckForEnginesUpdates": False,
        "CheckForUpdates": False,
        "CommandOnSuccessfulDownload": "",
        "CommandWhenAllFinished": "",
        "ConcurrentDownloading": False,
        "MaxConcurrentDownloads": 1,
        "CookieBrowser_yt-dlp": "fixture-browser",
        "CookieTextFilePath_yt-dlp": "",
        "CookieSourceSetToBrowerName": True,
        "DeleteFilesOnCanceledDownload": False,
        "DenoEnableAutoDownloading": False,
        "DesktopNotificationTimeOut": 10000,
        "DesktopNotifyOnAllDownloadComplete": False,
        "DesktopNotifyOnDownloadComplete": False,
        "DownloadFolder": "/home/user",
        "DownloadOptionsAsLast": False,
        "EnableLibraryTab": True,
        "Env": "",
        "FlatpackUseDenoRuntime": False,
        "HighDpiScalingFactorInterval": 0.05,
        "HighDpiScalingFactorValue": 1.0,
        "HistorySize": 30,
        "Language": "en_US",
        "LastUsedOptionBasicTab_yt-dlp": "--history-option",
        "LastUsedOptionBatchTab_yt-dlp": "--batch-option",
        "LastUsedOptionPlayListTab_yt-dlp": "--flat-playlist",
        "LibraryArrangeAscending": True,
        "LibraryArrangeByDate": True,
        "LibraryDownloadFolder": "/home/user",
        "LibraryShowFolderFirst": True,
        "MainTableTextAlignment": "center",
        "MainWindowDimensions": "0 0 1024 768",
        "Manual": "",
        "MaxLoggerProcesses": 0,
        "MediaDownloaderAppDataPath": "/home/user",
        "NetworkTimeOutInSeconds": 30,
        "OpenWith": "",
        "PlaylistRangeHistory-yt-dlp": "--break-on-existing",
        "PlaylistUrlHistory-yt-dlp": "https://example.com/playlist/fixture",
        "playlistRangeHistoryLastUsed-yt-dlp": "--break-on-existing",
        "ProxySettingsCustomSource": "",
        "ProxySettingsType": "None",
        "SaveDownloadHistory": True,
        "SaveHistory": True,
        "ShowLocalAndLatestVersionInformation": False,
        "ShowLocalVersionInformationOnly": False,
        "ShowMetaDataInBatchDownloader": False,
        "ShowTrayIcon": False,
        "ShowVersionInfoAndAutoDownloadUpdates": False,
        "SingleInstance": False,
        "StringTruncationSize": 100,
        "TabNumber": 0,
        "TextEncoding_yt-dlp": "ISO-8859-1",
        "ThemeName": "Dark",
        "ThumbnailHeight_batch": 72,
        "ThumbnailHeight_playlist": 72,
        "ThumbnailWidth_batch": 128,
        "ThumbnailWidth_playlist": 128,
        "TimeOutWaitingForClipboardData": 5000,
        "UseInternalArchiveFile": True,
        "UseSystemEngine": True,
        "UseSystemSupportingEngine": True,
        "UseSystemProvidedVersionIfAvailable": True,
        "UseSystemProvidingEngineIfAvailable": True,
        "WindowsUpdateChannel": "release",
        "WindowDimensions_LogWindow": "0-0-800-600",
        "denoInFlatpakUpdated": False,
    }
    return result


def step(action: str, target: str = "", value: str = "", **extra: Any) -> dict[str, Any]:
    item: dict[str, Any] = {"action": action, "target": target}
    if value or action in {"text", "plainText", "combo"}:
        item["value"] = value
    item.update(extra)
    return item


def steps() -> list[dict[str, Any]]:
    result = [
        step("tab", "tabWidget", index=4),
        step("sleep", ms=500),
        step("text", "lineEditConfigureDownloadPath", "/home/user"),
        step("combo", "cbConfigureEngines", "yt-dlp"),
        step("text", "lineEditConfigureTextEncoding", "ISO-8859-1"),
        step("text", "lineEditConfigureCookieBrowserName", "fixture-browser"),
        step("text", "lineEditAddDefaultDownloadOption", "--engine-default \"two words\""),
        step("click", "pbAddDefaultDownloadOption"),
        step("combo", "cbConfigureEnginesUrlManager", "gallery-dl"),
        step("text", "lineEditConfigureManageUrl", "route.example"),
        step("text", "lineEditConfigureManageOptions", "--url-manager \"two words\""),
        step("click", "pbConfigureManageUrl"),
        step("click", "pbConfigureSave"),
        step("tab", "tabWidget", index=2),
        step("clickMenu", "pbPlSubscription", "Manage Subscriptions"),
        step("text", "lineEditPlSubscriptionUiName", "Example subscription"),
        step("text", "lineEditPlSubscriptionUrl", "https://example.com/playlist/subscription"),
        step("text", "lineEditPlSubscriptionGetListOptions", "--flat-playlist"),
        step("click", "pbPlSubscriptionAdd"),
        step("click", "pbPlSubscriptionDone"),
        step("text", "lineEditPLUrl", "https://example.com/playlist/fixture"),
        step("text", "lineEditPLUrlOptions", "--flat-playlist"),
        step("click", "pbPLGetList"),
        step("sleep", ms=700),
        step("tab", "tabWidget", index=1),
        step("text", "lineEditBDUrlOptions", "--batch-option"),
        step("text", "lineEditBDUrl", "https://example.com/batch/01"),
        step("click", "pbBDAdd"),
        step("text", "lineEditBDUrl", "https://example.com/batch/02"),
        step("click", "pbBDAdd"),
        step("tab", "tabWidget", index=0),
    ]
    for index in range(1, 21):
        result.extend([
            step("text", "lineEditOptions", f"--history-option {index:02d}"),
            step("text", "lineEditURL", f"https://example.com/history/{index:02d}"),
            step("click", "pbDownload"),
            step("sleep", ms=450),
        ])
    result.extend([
        step("sleep", ms=2000),
        step("tab", "tabWidget", index=4),
        step("click", "pbConfigureQuit"),
    ])
    return result


def write_seed(path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="\n") as output:
        output.write("[General]\n")
        for key, value in settings_seed().items():
            if isinstance(value, bool):
                value = "true" if value else "false"
            output.write(f"{key}={value}\n")


def install_scaffolding(appdata: Path) -> None:
    for directory in (appdata / "bin", appdata / "engines.v1", appdata / "data"):
        directory.mkdir(parents=True, exist_ok=True)
    for source in (ROOT / "extensions").glob("*.json"):
        shutil.copy2(source, appdata / "engines.v1" / source.name)
    for name in ("yt-dlp", "gallery-dl.bin", "wget", "stdbuf", "aria2c", "you-get", "svtplay-dl", "get-sauce", "qjs", "qjs-linux-x86_64", "ffmpeg", "ffprobe", "safaribooks.py"):
        (appdata / "bin" / name).symlink_to(WRAPPER)


def run_original(appdata: Path, capture_steps: list[dict[str, Any]], log_path: Path, fake_download: bool = False) -> tuple[int, str]:
    steps_path = appdata / "capture-steps.json"
    steps_path.write_text(json.dumps(capture_steps, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    environment = os.environ.copy()
    for key in list(environment):
        if key.lower() in {"http_proxy", "https_proxy", "all_proxy", "no_proxy"}:
            environment.pop(key)
    environment.update({
        "MediaDownloaderAppDataPath": str(appdata),
        "CAPTURE_STEPS": str(steps_path),
        "CAPTURE_TRANSCRIPT": str(appdata / "capture-transcript.jsonl"),
        "CAPTURE_PROBE_TRANSCRIPT": str(appdata / "capture-probes.jsonl"),
        "CAPTURE_UI_LOG": str(appdata / "capture-ui.log"),
        "CAPTURE_DRIVER_ERROR": str(appdata / "capture-driver-error.txt"),
        "CAPTURE_SETTLE_MS": "1500",
        "QT_QPA_PLATFORM": "minimal",
        "LD_LIBRARY_PATH": str(Path(os.environ["QT_PREFIX"]) / "lib"),
        "LD_PRELOAD": str(DRIVER),
        "no_proxy": "127.0.0.1,localhost",
    })
    if fake_download:
        environment["CAPTURE_FAKE_DOWNLOAD_NAME"] = "fixture-output.bin"
    with log_path.open("w", encoding="utf-8") as output:
        process = subprocess.run(
            [str(LEGACY), "--debug", "-s"],
            cwd=appdata,
            env=environment,
            stdout=output,
            stderr=subprocess.STDOUT,
            timeout=90,
            check=False,
        )
    error_path = appdata / "capture-driver-error.txt"
    error = error_path.read_text(encoding="utf-8").strip() if error_path.exists() else ""
    return process.returncode, error


def create_archive(data_dir: Path) -> tuple[int, str]:
    from http.server import ThreadingHTTPServer

    server = ThreadingHTTPServer(("127.0.0.1", PORT), Handler)
    server.daemon_threads = True
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    archive = data_dir / "archiveFile-yt-dlp.txt"
    log = WORK / "archive-capture.log"
    try:
        with log.open("wb") as output:
            process = subprocess.run(
                [
                    str(VENV / "bin" / "yt-dlp"),
                    "--no-config",
                    "--no-cache-dir",
                    "--force-write-archive",
                    "--skip-download",
                    "--download-archive",
                    str(archive),
                    f"{BASE}/video.mp4",
                ],
                cwd=WORK,
                stdout=output,
                stderr=subprocess.STDOUT,
                timeout=30,
                check=False,
            )
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
    return process.returncode, log.read_text(encoding="utf-8", errors="replace")


def copy_and_anonymize(source: Path, destination: Path) -> None:
    if destination.exists():
        shutil.rmtree(destination)
    destination.mkdir(parents=True)
    selected = [
        "settings/settings.ini",
        "data/presetOptions.json",
        "data/downloadDefaultOptions.json",
        "data/downloadEngineDefaultOptions.json",
        "data/subscriptions.json",
        "data/autoSavedList.json",
        "data/downloadHistory.json",
        "data/archiveFile-yt-dlp.txt",
        "data/themes/Dark.json",
        "data/themes/Pure Dark.json",
    ]
    for relative in selected:
        src = source / relative
        if not src.exists():
            raise RuntimeError(f"original did not create {relative}")
        dst = destination / relative
        dst.parent.mkdir(parents=True, exist_ok=True)
        data = src.read_bytes()
        data = data.replace(str(source).encode(), b"/home/user")
        data = data.replace(str(WORK).encode(), b"/home/user")
        dst.write_bytes(data)


def history_count(path: Path) -> int:
    text = path.read_text(encoding="utf-8")
    decoder = json.JSONDecoder()
    position = 0
    count = 0
    while position < len(text):
        while position < len(text) and text[position].isspace():
            position += 1
        if position >= len(text):
            break
        _, position = decoder.raw_decode(text, position)
        count += 1
    return count


def validate_fixture() -> dict[str, Any]:
    required = settings_seed().keys()
    # The original wrote this file, so it is read with Qt's reader (qt_ini.py);
    # configparser would parse it as UTF-8, which Qt never does.
    actual = set(read_settings_ini(FINAL / "settings" / "settings.ini"))
    missing = sorted(set(required) - actual)
    if missing:
        raise RuntimeError(f"settings keys missing: {', '.join(missing)}")
    count = history_count(FINAL / "data" / "downloadHistory.json")
    if count < 20:
        raise RuntimeError(f"only {count} history objects")
    for name in ("presetOptions.json", "downloadDefaultOptions.json", "downloadEngineDefaultOptions.json", "subscriptions.json", "autoSavedList.json"):
        value = json.loads((FINAL / "data" / name).read_text(encoding="utf-8"))
        if not isinstance(value, list) or not value:
            raise RuntimeError(f"{name} is not a non-empty JSON array")
    return {"settings_keys": len(actual), "history_entries": count}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--force", action="store_true", help="recreate the fixture instead of keeping it unchanged")
    parser.add_argument("--verify-only", action="store_true", help="only load the existing fixture in a disposable copy")
    args = parser.parse_args()

    if "QT_PREFIX" not in os.environ:
        parser.error("set QT_PREFIX")
    if not LEGACY.exists() or not DRIVER.exists():
        parser.error("build the original and qt_driver first")

    if FINAL.exists() and not args.force:
        if not args.verify_only:
            print("keep existing userdata (use --force to recapture)")
    else:
        if WORK.exists():
            shutil.rmtree(WORK)
        WORK.mkdir(parents=True)
        WRAPPER.chmod(0o755)
        write_seed(WORK / "settings" / "settings.ini")
        install_scaffolding(WORK)
        returncode, error = run_original(WORK, steps(), WORK / "original-capture.log", fake_download=True)
        if returncode != 0 or error:
            raise RuntimeError(f"capture failed: exit {returncode}; {error}; see {WORK / 'original-capture.log'}")
        archive_code, archive_log = create_archive(WORK / "data")
        if archive_code != 0:
            raise RuntimeError(f"archive capture failed: exit {archive_code}\n{archive_log}")
        copy_and_anonymize(WORK, FINAL)

    validation = validate_fixture()

    if VERIFY.exists():
        shutil.rmtree(VERIFY)
    VERIFY.mkdir(parents=True)
    shutil.copytree(FINAL, VERIFY, dirs_exist_ok=True)
    install_scaffolding(VERIFY)
    verification_steps = [
        step("tab", "tabWidget", index=4),
        step("sleep", ms=400),
        step("tab", "tabWidget", index=2),
        step("clickMenu", "pbPlSubscription", "Manage Subscriptions"),
        step("sleep", ms=300),
        step("click", "pbPlSubscriptionDone"),
        step("tab", "tabWidget", index=1),
        step("sleep", ms=300),
        step("tab", "tabWidget", index=0),
    ]
    returncode, error = run_original(VERIFY, verification_steps, WORK / "original-verify.log")
    if returncode != 0 or error:
        raise RuntimeError(f"verification failed: exit {returncode}; {error}; see {WORK / 'original-verify.log'}")

    metadata = {
        "status": "captured",
        "app_version": "5.6.8",
        "qt_version": "5.15.16",
        "platform": "Linux x86_64",
        "capture_date": CAPTURE_DATE,
        "method": "original C++ UI and backend writes, disposable-path anonymization, original C++ reload",
        "verification": {"exit_code": returncode, **validation},
        "files": [str(path.relative_to(FINAL_ROOT)) for path in sorted(FINAL.rglob("*")) if path.is_file()],
    }
    (FINAL_ROOT / "meta.json").write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    manifest_path = ROOT / "fixtures" / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8")) if manifest_path.exists() else {"schema_version": 1}
    manifest["userdata"] = {
        "status": "captured",
        "method": "original-ui-and-backend-writes",
        "files": ["fixtures/userdata/meta.json"] + [f"fixtures/userdata/{name}" for name in metadata["files"]],
    }
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"userdata_load_exit": returncode, **validation}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
