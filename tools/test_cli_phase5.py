"""Phase 5 offline CLI evidence. No release capture or real backend is used."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

BINARY = Path(sys.argv.pop(1)).resolve()
VERSION_FIXTURE = Path(sys.argv.pop(1)).resolve()
REPO = Path(__file__).resolve().parents[1]

class Lifecycle(unittest.TestCase):
    def test_engines_reads_versions_by_running_native_double(self):
        with tempfile.TemporaryDirectory(prefix="morid-version-cli-") as temporary:
            root = Path(temporary)
            binary = root / "cli" / BINARY.name
            binary.parent.mkdir()
            shutil.copy2(BINARY, binary)
            data = root / "data"
            (data / "extensions").mkdir(parents=True)
            (data / "bin").mkdir()
            (data / "settings").mkdir()
            (data / "settings/settings.ini").write_text("[General]\nUseSystemSupportingEngine=false\n", encoding="ascii")
            executable = "version-double.exe" if os.name == "nt" else "version-double"
            shutil.copy2(VERSION_FIXTURE, data / "bin" / executable)
            expected = {
                "yt-dlp": "2026.10.26", "gallery-dl": "1.32.19", "aria2c": "1.37.0",
                "wget": "1.21.3", "svtplay-dl": "4.211", "you-get": "0.4.1743,", "getsauce": "v1.10.0",
            }
            for filename in ["yt-dlp", "gallery-dl", "aria2c", "wget", "svtplay-dl", "you-get", "getsauce", "deno", "lux", "quickjs", "quickjs_ng", "safaribooks", "ytdl-patched"]:
                document = json.loads((REPO / f"extensions/{filename}.json").read_text())
                replay = "get-sauce" if filename == "getsauce" else document["Name"]
                for platform in document["Cmd"].values():
                    for command in platform.values():
                        command["Name"] = executable
                        command["Args"] = [executable, replay]
                # Avoid thin-file expansion: these are complete test documents.
                (data / "extensions" / f"version-{filename}.json").write_text(json.dumps(document), encoding="utf-8")
            for helper in (["deno.exe", "qjs-linux-x86"] if os.name == "nt" else ["deno", "qjs-linux-x86"]):
                shutil.copy2(VERSION_FIXTURE, data / "bin" / helper)
            environment = dict(os.environ, PATH="")
            result = subprocess.run([str(binary), "--config", str(data), "engines", "--json"], capture_output=True, text=True, encoding="utf-8", timeout=60, cwd=root, env=environment)
            print("CLI_VERSIONS exit", result.returncode, "stdout", result.stdout, "stderr", result.stderr)
            self.assertEqual(result.returncode, 0)
            rows = {row["name"]: row for row in map(json.loads, result.stdout.splitlines())}
            for name, value in expected.items():
                self.assertEqual(rows[name]["version"], value)
                print("CLI_VERSION_CAPTURED", name, "printed", repr(value))
            for name in ["deno", "lux", "quickjs", "quickjs-ng", "safaribooks", "ytdl-patched"]:
                if name == "quickjs-ng" and name not in rows:
                    print("CLI_VERSION_NO_CAPTURE quickjs-ng helper not selected on this target (D-020); direct execution tested separately")
                    continue
                self.assertEqual(rows[name]["version"], "unavailable")
                print("CLI_VERSION_NO_CAPTURE", name, "synthetic_empty_output display=unavailable reason=" + rows[name]["version_status"])
            self.assertEqual(rows["deno"]["section"], "supporting")
            self.assertTrue(all(row["section"] == "downloaders" for name, row in rows.items() if name in expected))

    def test_remove_twice_retains_json_and_refuses_bundled_and_unknown(self):
        with tempfile.TemporaryDirectory(prefix="morid-phase5-") as temporary:
            root = Path(temporary)
            binary = root / "cli" / BINARY.name
            binary.parent.mkdir()
            shutil.copy2(BINARY, binary)
            data = root / "data"
            (data / "extensions").mkdir(parents=True)
            (data / "bin").mkdir()
            custom = json.loads((REPO / "extensions/yt-dlp.json").read_text())
            custom["Name"] = "custom-tool"
            executable = "custom-tool.exe" if os.name == "nt" else "custom-tool"
            for platform in custom["Cmd"].values():
                for command in platform.values():
                    command["Name"] = executable
                    command["Args"] = [executable]
            description = data / "extensions/custom-tool.json"
            description.write_text(json.dumps(custom), encoding="utf-8")
            original_json = description.read_bytes()
            installed = data / "bin" / executable
            installed.write_bytes(b"private old executable stand-in, never executed")
            for name, expected in [("custom-tool", 0), ("custom-tool", 0), ("yt-dlp", 1), ("missing", 4)]:
                command = [str(binary), "--config", str(data), "engines", "--remove", name]
                result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", timeout=30, cwd=root)
                print("CLI_REMOVE", name, "exit", result.returncode, "stdout", repr(result.stdout), "stderr", repr(result.stderr))
                self.assertEqual(result.returncode, expected)
                self.assertEqual(description.read_bytes(), original_json)
                self.assertFalse(installed.exists())
                if name == "yt-dlp":
                    self.assertIn("cannot remove bundled backend", result.stderr)
            print("REMOVE_IDEMPOTENT_AND_JSON_BYTE_IDENTICAL_OK")

if __name__ == "__main__":
    unittest.main(verbosity=2)
