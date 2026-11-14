"""Offline release-adapter acceptance; all backends are local fixtures, no HTTP."""
import json
import hashlib
import queue
import threading
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest

BINARY = Path(sys.argv.pop(1)).resolve()
REPO = Path(__file__).resolve().parents[1]

class Cli(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="morid-cli-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / BINARY.name
        shutil.copy2(BINARY, self.bin)
        self.data = self.root / "data"
        self.extensions = self.data / "extensions"
        self.extensions.mkdir(parents=True)
        self.destination = self.root / "output dir Загрузки"
        document = {
            "Name": "fixture", "Version": "1", "LikeYoutubeDl": False,
            "Cmd": {platform: {"amd64": {"Name": sys.executable, "Args": [sys.executable, str(REPO / "tools/cli_fixture_backend.py"), str(self.root)]}} for platform in ["Generic", "MacOS", "Windows"]},
            "DefaultDownLoadCmdOptions": [], "DefaultListCmdOptions": ["--fixture-list"],
            "SplitLinesBy": ["\n"], "RemoveText": [], "SkipLineWithText": [],
            "ControlJsonStructure": {"lhs": {"contains": "downloaded_bytes:"}},
        }
        (self.extensions / "fixture.json").write_text(json.dumps(document), encoding="utf-8")
        self.settings = self.root / "settings.json"
        self.settings.write_text(json.dumps({"engine": "fixture", "destination": str(self.destination), "internal_archive": False}), encoding="utf-8")
        self.base = [str(self.bin), "--config", str(self.data)]

    def run_cli(self, *args):
        result = subprocess.run(self.base + list(args), cwd=self.root, capture_output=True, text=True, encoding="utf-8", timeout=30)
        print("CLI", args, "exit", result.returncode, "stderr", result.stderr)
        return result

    def command(self, verb, *args):
        return self.run_cli(verb, "--settings", str(self.settings), *args)

    def records(self, result):
        return [json.loads(line) for line in result.stdout.splitlines()]

    def test_dry_run_never_spawns_and_flags_override_json(self):
        result = self.command("download", "https://fixture.test/one", "--dry-run", "--to", str(self.root / "never-created"))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)[-1], "https://fixture.test/one")
        self.assertFalse((self.root / "spawns.jsonl").exists())
        self.assertFalse((self.root / "never-created").exists())

    def test_download_jsonl_first_last_fields_and_file(self):
        result = self.command("download", "https://fixture.test/one", "--json")
        self.assertEqual(result.returncode, 0, result.stderr)
        rows = self.records(result)
        self.assertEqual(rows[0]["type"], "started")
        self.assertEqual(rows[-1]["type"], "finished")
        self.assertEqual(rows[-1]["state"], "success")
        self.assertTrue(all(row["v"] == 1 for row in rows))
        self.assertTrue(any(row["type"] == "progress" and row["downloaded"] == 15 for row in rows))
        self.assertEqual(rows[-1]["files"], [str(self.destination / "fixture-one.bin")])
        self.assertEqual((self.destination / "fixture-one.bin").read_bytes(), b"fixture content")
        print("JSONL first", json.dumps(rows[0]), "last", json.dumps(rows[-1]), "wc-l", len(rows))

    def test_batch_one_process_per_url(self):
        urls = self.root / "urls.txt"
        urls.write_text("https://fixture.test/one\nhttps://fixture.test/two\n")
        result = self.command("download", "--batch", str(urls), "--json")
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = [json.loads(line) for line in (self.root / "spawns.jsonl").read_text().splitlines()]
        self.assertEqual(len(calls), 2)
        self.assertEqual(sorted(row[-1] for row in calls), ["https://fixture.test/one", "https://fixture.test/two"])
        self.assertEqual(len([r for r in self.records(result) if r["type"] == "finished"]), 2)

    def test_list_metadata(self):
        result = self.command("list", "https://fixture.test/one", "--json")
        self.assertEqual(result.returncode, 0, result.stderr)
        row = next(row for row in self.records(result) if row["type"] == "metadata")
        self.assertEqual(row["formats"][0]["id"], "22")

    def test_lang_rejection_is_explicit_and_never_spawns(self):
        for language in ["ru", "de"]:
            result = self.command("download", "https://fixture.test/one", "--lang", language, "--json")
            self.assertEqual(result.returncode, 1)
            self.assertEqual(result.stdout, "")
            self.assertIn("only --lang en is supported in Phase 3", result.stderr)
            self.assertFalse((self.root / "spawns.jsonl").exists())
            self.assertFalse(self.destination.exists())
            print("LANGUAGE REJECTION", language, "exit", result.returncode, "stdout empty; stderr:", result.stderr.strip())

    def test_captured_list_matches_all_formats_and_printed_table(self):
        capture = REPO / "fixtures/streams/yt-dlp/list.json"
        data = capture.read_bytes()
        original = json.loads(data)
        expected_formats = [dict(row, id=row.get("format_id")) for row in original["formats"]]
        result = self.command("list", "https://fixture.test/captured", "--json", "--lang", "en")
        self.assertEqual(result.returncode, 0, result.stderr)
        records = self.records(result)
        metadata = [row for row in records if row["type"] == "metadata"]
        self.assertEqual(len(metadata), 1)
        self.assertEqual(metadata[0]["title"], original.get("title"))
        self.assertEqual(metadata[0]["duration"], original.get("duration"))
        self.assertEqual(metadata[0]["thumbnail"], original.get("thumbnail"))
        self.assertEqual(metadata[0]["formats"], expected_formats)
        self.assertIsNone(records[0]["engine_version"])
        human = self.command("list", "https://fixture.test/captured", "--lang", "en")
        self.assertEqual(human.returncode, 0, human.stderr)
        lines = human.stdout.splitlines()
        header = lines.index("ID\tEXT\tRESOLUTION\tFILESIZE")
        expected_rows = ["\t".join([
            row.get("format_id") or "?", row.get("ext") or "?",
            row.get("resolution") or "?", json.dumps(row.get("filesize")),
        ]) for row in original["formats"]]
        actual_rows = lines[header + 1: header + 1 + len(expected_rows)]
        self.assertEqual(actual_rows, expected_rows)
        self.assertTrue(lines[header + 1 + len(expected_rows)].startswith("finished "))
        self.assertIn("title " + original["title"], lines)
        calls = [json.loads(line) for line in (self.root / "spawns.jsonl").read_text().splitlines()]
        self.assertEqual(len(calls), 2, "one backend process per list; no hidden --version probe")
        self.assertTrue(all("--version" not in call for call in calls))
        print("CAPTURED LIST INPUT", capture, "SHA256", hashlib.sha256(data).hexdigest())
        print("CAPTURED FORMATS", len(expected_formats), "JSON exact match, original format_id retained")
        print("ENGINE VERSION unknown: null; schema Version is not executable version; no --version probes")
        print("TABLE EXPECTED", repr(expected_rows), "ACTUAL", repr(actual_rows))

    def test_cancelled_batch_json_with_queue_remainder(self):
        config = json.loads(self.settings.read_text(encoding="utf-8"))
        config["concurrency"] = "1"
        self.settings.write_text(json.dumps(config), encoding="utf-8")
        urls = ["https://fixture.test/cancel-first", "https://fixture.test/queued-two", "https://fixture.test/queued-three"]
        batch = self.root / "cancel-batch.txt"
        batch.write_text("\n".join(urls) + "\n")
        flags = subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0
        process = subprocess.Popen(self.base + ["download", "--batch", str(batch), "--settings", str(self.settings), "--json"], cwd=self.root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8", creationflags=flags)
        self.addCleanup(lambda: process.kill() if process.poll() is None else None)
        # started is emitted only after the CLI received ALL enqueue replies.
        # Observing it proves the two remaining jobs have actually been queued;
        # readiness of the backend alone would not establish that causal gate.
        first_line = queue.Queue()
        threading.Thread(target=lambda: first_line.put(process.stdout.readline()), daemon=True).start()
        first = first_line.get(timeout=15)
        self.assertEqual(json.loads(first)["type"], "started")
        deadline = time.monotonic() + 15
        while not (self.root / "ready").exists():
            self.assertLess(time.monotonic(), deadline)
            time.sleep(0.01)
        process.send_signal(signal.CTRL_BREAK_EVENT if os.name == "nt" else signal.SIGINT)
        out, err = process.communicate(timeout=20)
        self.assertEqual(process.returncode, 5, (first + out, err))
        rows = [json.loads(line) for line in (first + out).splitlines()]
        self.assertTrue(all(row["v"] == 1 for row in rows))
        started = [row for row in rows if row["type"] == "started"]
        finished = [row for row in rows if row["type"] == "finished"]
        self.assertEqual(len(started), 1)
        self.assertEqual(len(finished), 3)
        self.assertEqual({row["job"] for row in finished}, {0, 1, 2})
        self.assertTrue(all(row["state"] == "cancelled" and row["files"] == [] for row in finished))
        terminals = set()
        for row in rows:
            self.assertNotIn(row["job"], terminals, "event after terminal")
            if row["type"] == "finished":
                terminals.add(row["job"])
        calls = [json.loads(line) for line in (self.root / "spawns.jsonl").read_text().splitlines()]
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][-1], urls[0])
        self.assertFalse(list(self.destination.glob("fixture-*.bin")))
        pid = int((self.root / "ready").read_text())
        if os.name == "nt":
            powershell = Path(os.environ["SystemRoot"]) / "System32/WindowsPowerShell/v1.0/powershell.exe"
            table = subprocess.run([str(powershell), "-NoProfile", "-NonInteractive", "-Command", f"if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ 'alive' }} else {{ 'gone' }}"], capture_output=True, text=True, timeout=10, check=True)
            self.assertEqual(table.stdout.strip(), "gone")
        else:
            table = subprocess.run(["ps", "-o", "stat=", "-p", str(pid)], capture_output=True, text=True, timeout=10)
            self.assertIn(table.returncode, [0, 1])
            self.assertFalse(table.stdout.strip(), "owned backend must already be reaped")
        print("CANCELLED BATCH JSONL", json.dumps(rows), "exit", process.returncode, "spawn count", len(calls), "post-terminal pid", pid, "gone")

    def test_reachable_exit_codes(self):
        self.assertEqual(self.command("download", "--wat").returncode, 1)
        self.assertEqual(self.command("download", "not-a-url").returncode, 2)
        refused = self.command("download", "https://fixture.test/unsupported", "--json")
        self.assertEqual(refused.returncode, 2, refused.stderr)
        self.assertEqual(next(row["code"] for row in self.records(refused) if row["type"] == "error"), 2)
        self.assertEqual(self.command("download", "https://fixture.test/fail", "--json").returncode, 3)
        self.assertEqual(self.command("download", "https://fixture.test/one", "--engine", "absent").returncode, 4)
        blocked = self.root / "blocked"
        blocked.write_text("file")
        result = self.command("download", "https://fixture.test/one", "--to", str(blocked / "child"), "--json")
        self.assertEqual(result.returncode, 6, result.stderr)

    def test_interrupt_cancels_and_exits_five(self):
        flags = subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0
        process = subprocess.Popen(self.base + ["download", "https://fixture.test/cancel", "--settings", str(self.settings), "--json"], cwd=self.root, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8", creationflags=flags)
        self.addCleanup(lambda: process.kill() if process.poll() is None else None)
        deadline = time.monotonic() + 15
        while not (self.root / "ready").exists():
            self.assertLess(time.monotonic(), deadline)
            time.sleep(0.01)
        process.send_signal(signal.CTRL_BREAK_EVENT if os.name == "nt" else signal.SIGINT)
        out, err = process.communicate(timeout=20)
        self.assertEqual(process.returncode, 5, (out, err))
        self.assertEqual(json.loads(out.splitlines()[-1])["state"], "cancelled")

    def test_release_search_outside_checkout(self):
        result = self.run_cli("engines", "--json")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(str(self.extensions), result.stderr)
        adjacent = self.root / "extensions"
        shutil.copytree(REPO / "extensions", adjacent)
        result = self.run_cli("engines", "--json")
        self.assertIn(result.returncode, [0, 4])
        rows = self.records(result)
        self.assertEqual(len(rows), 17)
        self.assertTrue(all(row["resolved"] for row in rows))
        print("OUTSIDE CHECKOUT SEVENTEEN ENGINES", result.stdout)
        shutil.rmtree(adjacent)
        shutil.rmtree(self.extensions)
        result = self.run_cli("engines")
        self.assertEqual(result.returncode, 4)
        self.assertIn("searched", result.stderr)

if __name__ == "__main__":
    unittest.main(verbosity=2)
