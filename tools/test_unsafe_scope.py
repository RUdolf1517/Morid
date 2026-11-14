"""Mutation tests for the D-019 CI guard; no Rust binaries or network needed."""
from pathlib import Path
import tempfile
import unittest

from tools.check_unsafe_scope import ALLOWED, check, code_only


class UnsafeScopeTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        members = ["crates/morid-engine", "crates/morid-core", "crates/morid-gui"]
        self.write("Cargo.toml", "[workspace]\nmembers = " + repr(members).replace("'", '"'))
        for member in members:
            self.write(member + "/Cargo.toml", '[package]\nname = "example"\nversion = "0.1.0"\n')
            lint = "deny" if member.endswith("morid-engine") else "forbid"
            self.write(member + "/src/lib.rs", f"#![{lint}(unsafe_code)]\n")
        self.write("crates/morid-gui/src/main.rs", "#![deny(unsafe_code)]\nfn main() {}\n")
        for path in ALLOWED:
            self.write(path, "#![allow(unsafe_code)]\nfn primitive() {\n// SAFETY: no operation in this synthetic guard fixture.\nunsafe {}\n}\n")

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def test_unsafe_stays_in_the_two_platform_modules(self):
        errors, counts = check(self.root)
        self.assertEqual(errors, [])
        self.assertEqual(sorted(counts.values()), [1, 1])

    def test_third_file_is_rejected(self):
        self.write("crates/morid-engine/src/third.rs", "#![allow(unsafe_code)]\nfn extra() { unsafe {} }\n")
        errors, _ = check(self.root)
        self.assertTrue(any("third.rs" in error and "outside" in error for error in errors))

    def test_other_crate_losing_forbid_is_rejected(self):
        self.write("crates/morid-core/src/lib.rs", "#![deny(unsafe_code)]\n")
        errors, _ = check(self.root)
        self.assertTrue(any("morid-core" in error and "forbid" in error for error in errors))

    def test_missing_safety_comment_is_rejected(self):
        self.write("crates/morid-engine/src/job_object.rs", "#![allow(unsafe_code)]\nfn f() { unsafe {} }\n")
        errors, _ = check(self.root)
        self.assertTrue(any("missing adjacent SAFETY" in error for error in errors))

    def test_unsafe_function_and_raw_public_signature_are_rejected(self):
        self.write("crates/morid-engine/src/job_object.rs", "#![allow(unsafe_code)]\npub unsafe fn leak() -> *mut u8 { loop {} }\n")
        errors, _ = check(self.root)
        self.assertTrue(any("only unsafe blocks" in error for error in errors))
        self.assertTrue(any("raw platform type" in error for error in errors))

    def test_exemption_in_another_crate_is_rejected_even_without_blocks(self):
        self.write("crates/morid-core/src/helper.rs", "#![allow(unsafe_code)]\n")
        errors, _ = check(self.root)
        self.assertTrue(any("unapproved" in error for error in errors))

    def test_comments_raw_strings_characters_and_lifetimes_are_not_code(self):
        sample = '''// unsafe {}
/* outer /* unsafe {} */ comment */
const WORD: &str = r###"unsafe { \"quoted\" }"###;
const BYTE: u8 = b'"';
fn borrowed<'a>(value: &'a str) -> &'a str { value }
'''
        self.assertNotIn("unsafe", code_only(sample))
        self.write("crates/morid-core/src/helper.rs", sample)
        self.assertEqual(check(self.root)[0], [])


if __name__ == "__main__":
    unittest.main()
