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
REPO = Path(__file__).resolve().parents[1]

class Lifecycle(unittest.TestCase):
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
