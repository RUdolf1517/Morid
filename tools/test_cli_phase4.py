"""Offline Phase 4 CLI acceptance; never changes fixtures or runs capture tools."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

BINARY = Path(sys.argv.pop(1)).resolve()
REPO = Path(__file__).resolve().parents[1]
FIXTURE = REPO / "fixtures/userdata/media-downloader"


def snapshot(root):
    return {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob("*")) if p.is_file()}


class UserData(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="morid-user-data-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        # Isolate the executable from CI's release-adjacent bundled extensions.
        # Production correctly prefers adjacent resources over --config ones.
        self.binary = self.root / "cli" / BINARY.name
        self.binary.parent.mkdir()
        shutil.copy2(BINARY, self.binary)
        self.source = self.root / "source"
        shutil.copytree(FIXTURE, self.source)
        self.config = self.root / "morid"
        self.before = snapshot(self.source)

    def cli(self, *args, config=None, env=None):
        result = subprocess.run([str(self.binary), "--config", str(config or self.config), *map(str, args)],
                                cwd=self.root, env=env, capture_output=True, text=True,
                                encoding="utf-8", timeout=30)
        print("CLI", args, "exit", result.returncode, "stdout", result.stdout, "stderr", result.stderr)
        return result

    def test_source_not_recognized_exit_7(self):
        self.assertEqual(self.cli("import", self.root / "absent").returncode, 7)
        self.assertFalse(self.config.exists())

    def test_import_export_reports_source_unchanged_idempotence_and_backup(self):
        report = self.root / "import-report.json"
        result = self.cli("import", self.source, "--report", report)
        self.assertEqual(result.returncode, 8, result.stderr)  # D-013's three deliberate exclusions
        data = json.loads(report.read_text())
        self.assertEqual(set(data["dropped_keys"]), {"WindowsUpdateChannel", "CheckForUpdates", "ShowVersionInfoAndAutoDownloadUpdates"})
        self.assertIn("dropped self-update key", result.stdout)
        self.assertEqual(snapshot(self.source), self.before)
        normalized = json.loads((self.config / "data/downloadHistory.json").read_bytes())
        self.assertEqual(len(normalized), 20)
        self.assertEqual((self.config / ".morid/original-downloadHistory.json").read_bytes(),
                         (self.source / "data/downloadHistory.json").read_bytes())
        print("IMPORT JSON REPORT", json.dumps(data, sort_keys=True))
        stored = snapshot(self.config)
        second = self.cli("import", self.source)
        self.assertEqual(second.returncode, 8)
        self.assertIn("unchanged:", second.stdout)
        self.assertEqual(snapshot(self.config), stored)
        exported = self.root / "exported"
        self.assertEqual(self.cli("export", exported).returncode, 0)
        self.assertFalse((exported / ".morid").exists())
        for name in self.before:
            if name != "settings/settings.ini":
                self.assertEqual((exported / name).read_bytes(), (self.source / name).read_bytes(), name)
        settings = (self.config / "settings/settings.ini")
        original = settings.read_bytes()
        settings.write_bytes(original.replace(b"MaxConcurrentDownloads=1", b"MaxConcurrentDownloads=3"))
        changed = snapshot(self.config)
        conflict = self.cli("import", self.source)
        self.assertEqual(conflict.returncode, 8)
        self.assertIn("different existing content", conflict.stdout)
        self.assertEqual(snapshot(self.config), changed)
        forced_report = self.root / "force.json"
        self.assertEqual(self.cli("import", self.source, "--force", "--report", forced_report).returncode, 8)
        backup = Path(json.loads(forced_report.read_text())["backup"])
        self.assertEqual(snapshot(backup), changed)
        self.assertEqual(snapshot(self.source), self.before)
        print("SOURCE BYTE HASHES", json.dumps(self.before, sort_keys=True))
        print("EXPORTED FILE LIST", json.dumps(sorted(snapshot(exported))))

    def test_simple_import_exit_zero_and_unknown_list_preserved(self):
        settings = self.source / "settings/settings.ini"
        settings.write_bytes(b"[General]\nDownloadFolder=/tmp\nNovelFutureKey=first, second\n")
        report = self.root / "unknown.json"
        (self.source / "data/future-user-data.bin").write_bytes(b"\x00\xfffuture\r\n")
        result = self.cli("import", self.source, "--report", report)
        self.assertEqual(result.returncode, 0)
        self.assertIn("unknown item preserved:", result.stdout)
        self.assertEqual([name.replace("\\", "/") for name in json.loads(report.read_bytes())["unknown_items"]], ["data/future-user-data.bin"])
        self.assertEqual(json.loads(report.read_text())["unknown_keys"], ["NovelFutureKey"])
        stored = (self.config / "settings/settings.ini").read_bytes()
        self.assertIn(b"[imported]\nNovelFutureKey=first, second", stored)
        exported = self.root / "exported"
        self.assertEqual(self.cli("export", exported).returncode, 0)
        self.assertEqual((exported / "settings/settings.ini").read_bytes(), settings.read_bytes())
        self.assertEqual((exported / "data/future-user-data.bin").read_bytes(), b"\x00\xfffuture\r\n")

    def test_config_fixture_ini_reaches_actual_dry_run_and_json_overlay(self):
        extensions = self.source / "extensions"
        shutil.copytree(REPO / "extensions", extensions)
        result = self.cli("download", "https://example.test/video", "--dry-run", config=self.source)
        self.assertEqual(result.returncode, 0, result.stderr)
        argv = json.loads(result.stdout)
        self.assertEqual(argv[0], "yt-dlp.exe" if os.name == "nt" else "yt-dlp_macos" if sys.platform == "darwin" else "yt-dlp")
        self.assertEqual(argv[argv.index("--encoding") + 1], "ISO-8859-1")
        self.assertEqual(Path(argv[argv.index("--download-archive") + 1]), self.source / "data/archiveFile-yt-dlp.txt")
        self.assertEqual(argv[argv.index("--cookies-from-browser") + 1], "fixture-browser")
        overrides = self.root / "override.json"
        overrides.write_text(json.dumps({"text_encoding": "UTF-8", "internal_archive": False}))
        result = self.cli("download", "https://example.test/video", "--dry-run", "--settings", overrides, config=self.source)
        self.assertEqual(result.returncode, 0, result.stderr)
        argv = json.loads(result.stdout)
        self.assertEqual(argv[argv.index("--encoding") + 1], "UTF-8")
        self.assertNotIn("--download-archive", argv)
        print("CONFIG INI DRY-RUN AND JSON OVERLAY VERIFIED")

    def test_history_normalization_keeps_original_and_export_does_not_drop_new_rows(self):
        raw = (self.source / "data/downloadHistory.json").read_bytes()
        self.assertEqual(self.cli("import", self.source).returncode, 8)
        history = self.config / "data/downloadHistory.json"
        rows = json.loads(history.read_bytes())
        self.assertEqual(len(rows), 20)
        self.assertEqual((self.config / ".morid/original-downloadHistory.json").read_bytes(), raw)
        rows.append({"Url": "https://example.test/new-history", "UnknownFutureField": [1, 2]})
        history.write_text(json.dumps(rows), encoding="utf-8")
        exported = self.root / "edited-export"
        self.assertEqual(self.cli("export", exported).returncode, 0)
        self.assertEqual((exported / "data/downloadHistory.json").read_bytes(), history.read_bytes())
        self.assertEqual(len(json.loads((exported / "data/downloadHistory.json").read_bytes())), 21)
        self.assertFalse((exported / ".morid").exists())
        self.assertEqual((self.source / "data/downloadHistory.json").read_bytes(), raw)

    def test_corrupt_history_fragments_are_reported_as_raw_bytes(self):
        history = self.source / "data/downloadHistory.json"
        raw = bytearray(history.read_bytes())
        offset = raw.index(b"example.com/history")
        raw[offset] = 0xff
        history.write_bytes(raw)
        report = self.root / "history-report.json"
        self.assertEqual(self.cli("import", self.source, "--report", report).returncode, 8)
        data = json.loads(report.read_bytes())
        self.assertTrue(any(255 in item["bytes"] for item in data["history_fragments"]))
        self.assertEqual(len(json.loads((self.config / "data/downloadHistory.json").read_bytes())), 19)
        self.assertEqual((self.config / ".morid/original-downloadHistory.json").read_bytes(), raw)
        self.assertEqual(history.read_bytes(), raw)
        exported = self.root / "corrupt-history-export"
        self.assertEqual(self.cli("export", exported).returncode, 0)
        self.assertEqual((exported / "data/downloadHistory.json").read_bytes(), raw)
        self.assertFalse((exported / ".morid").exists())

    def test_tab_engine_and_overrides_select_their_own_ini_fields(self):
        shutil.copytree(REPO / "extensions", self.source / "extensions")
        settings = self.source / "settings/settings.ini"
        raw = settings.read_bytes().replace(b"BatchDownloaderDefaultEngine=yt-dlp\n",
                                            b"BatchDownloaderDefaultEngine=yt-dlp-nightly\n")
        settings.write_bytes(raw + b"TextEncoding_yt-dlp-nightly=KOI8-R\nCookieBrowser_yt-dlp-nightly=nightly-browser\n")
        batch = self.root / "batch.txt"
        batch.write_text("https://example.test/video\n")
        result = self.cli("download", "--batch", batch, "--dry-run", config=self.source)
        self.assertEqual(result.returncode, 0, result.stderr)
        argv = json.loads(result.stdout)
        self.assertIn("nightly", argv[0])
        self.assertEqual(argv[argv.index("--encoding") + 1], "KOI8-R")
        self.assertEqual(argv[argv.index("--cookies-from-browser") + 1], "nightly-browser")
        overrides = self.root / "selected.json"
        overrides.write_text(json.dumps({"engine": "yt-dlp-nightly"}))
        result = self.cli("list", "https://example.test/video", "--dry-run", "--settings", overrides, config=self.source)
        self.assertEqual(result.returncode, 0, result.stderr)
        argv = json.loads(result.stdout)
        self.assertEqual(argv[argv.index("--encoding") + 1], "KOI8-R")
        result = self.cli("list", "https://example.test/video", "--dry-run", "--settings", overrides,
                          "--engine", "yt-dlp", config=self.source)
        self.assertEqual(result.returncode, 0, result.stderr)
        argv = json.loads(result.stdout)
        self.assertNotIn("nightly", argv[0])
        self.assertEqual(argv[argv.index("--encoding") + 1], "ISO-8859-1")
        self.assertEqual(argv[argv.index("--cookies-from-browser") + 1], "fixture-browser")
        overrides.write_text(json.dumps({"engine": "yt-dlp-nightly", "text_encoding": "UTF-8", "cookie_browser": None}))
        result = self.cli("download", "https://example.test/video", "--dry-run", "--settings", overrides, config=self.source)
        self.assertEqual(result.returncode, 0, result.stderr)
        argv = json.loads(result.stdout)
        self.assertEqual(argv[argv.index("--encoding") + 1], "UTF-8")
        self.assertNotIn("--cookies-from-browser", argv)

    def test_missing_cookie_source_uses_the_original_browser_default(self):
        shutil.copytree(REPO / "extensions", self.source / "extensions")
        settings = self.source / "settings/settings.ini"
        settings.write_bytes(settings.read_bytes().replace(b"CookieSourceSetToBrowerName=true\n", b""))
        result = self.cli("download", "https://example.test/video", "--dry-run", config=self.source)
        self.assertEqual(result.returncode, 0, result.stderr)
        argv = json.loads(result.stdout)
        self.assertEqual(argv[argv.index("--cookies-from-browser") + 1], "fixture-browser")

    def test_option_payload_is_not_reparsed_as_an_adapter_flag(self):
        shutil.copytree(REPO / "extensions", self.source / "extensions")
        for payload in ["--engine", "--no-archive", "--settings"]:
            result = self.cli("download", "https://example.test/video", "--dry-run", "--options", payload, config=self.source)
            self.assertEqual(result.returncode, 0, result.stderr)
            argv = json.loads(result.stdout)
            self.assertIn(payload, argv)
            self.assertIn("--download-archive", argv)
            self.assertNotIn("nightly", argv[0])

    def test_archive_is_never_rewritten(self):
        archive_name = "data/archiveFile-yt-dlp.txt"
        original = (self.source / archive_name).read_bytes()
        self.assertEqual(self.cli("import", self.source).returncode, 8)
        archive = self.config / archive_name
        self.assertEqual(archive.read_bytes(), original)
        extensions = self.config / "extensions"
        extensions.mkdir()
        document = {
            "Name": "fixture", "Version": "1", "LikeYoutubeDl": False,
            "Cmd": {platform: {"amd64": {"Name": sys.executable,
                "Args": [sys.executable, str(REPO / "tools/cli_fixture_backend.py"), str(self.root)]}}
                for platform in ["Generic", "MacOS", "Windows"]},
            "DefaultDownLoadCmdOptions": [], "SplitLinesBy": ["\n"],
            "RemoveText": [], "SkipLineWithText": [],
        }
        (extensions / "fixture.json").write_text(json.dumps(document))
        overrides = self.root / "fixture-settings.json"
        overrides.write_text(json.dumps({"engine": "fixture", "destination": str(self.root / "downloads"), "internal_archive": False}))
        # Explicit raw backend option is preserved shell-free even for a generic
        # engine; the fixture process opens that exact path and returns normally.
        result = self.cli("download", "https://fixture.test/archive-check", "--settings", overrides,
                          "--options", '--download-archive "' + str(archive) + '"', "--json")
        self.assertEqual(result.returncode, 0, result.stderr)
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(rows[0]["type"], "started")
        self.assertEqual(rows[-1]["state"], "success")
        self.assertEqual((self.root / "archive-observed.bin").read_bytes(), original)
        self.assertEqual(archive.read_bytes(), original)
        exported = self.root / "exported"
        self.assertEqual(self.cli("export", exported).returncode, 0)
        self.assertEqual((exported / archive_name).read_bytes(), original)
        self.assertEqual((self.source / archive_name).read_bytes(), original)
        print("ARCHIVE BYTES VERIFIED import -> offline fixture-backend download cycle -> export", repr(original))

    def test_non_ascii_ini_is_reported_without_partial_publication(self):
        (self.source / "settings/settings.ini").write_text("[General]\nDownloadFolder=Привет\n", encoding="utf-8")
        report = self.root / "encoding.json"
        result = self.cli("import", self.source, "--report", report)
        self.assertEqual(result.returncode, 8)
        self.assertIn("non-ASCII UTF-8", result.stdout)
        self.assertFalse(self.config.exists())


if __name__ == "__main__":
    unittest.main(verbosity=2)
