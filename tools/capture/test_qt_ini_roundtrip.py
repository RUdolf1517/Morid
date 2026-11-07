#!/usr/bin/env python3
"""Qt 5.15's INI dialect: the round trip, the fuzz, the limits, the fixture.

`escape_qt_ini_value` (see `canonicalise.py`) writes the `settings.ini` values
the capture harness hands to the original, and the original reads them back with
Qt's own parser.  This module is the proof that it can: a fixed corpus, a seeded
fuzz, and tests that make the helper's limits executable instead of prose.

It needs the standard library only, no fixtures and no arguments:

    python3 -m unittest discover                  # from the repository root
    python3 -m unittest tools.capture.test_qt_ini_roundtrip
    python3 tools/capture/test_qt_ini_roundtrip.py

`verify_fixtures.py` imports `check_all()` and `check_userdata_settings()`
from here, so there is one implementation with two callers; the reader itself
lives in `qt_ini.py`.  Qt's writer is `iniEscapedString`, its reader
`iniUnescapedStringList` and `iniChopTrailingSpaces`, all in
`qtbase/src/corelib/io/qsettings.cpp` at tag `v5.15.16-lts-lgpl`.
"""

from __future__ import annotations

import random
import sys
import tempfile
import unittest
from pathlib import Path

# Importable from the repository root (unittest discovery) and from this
# directory (direct execution, and the verifier next to it) alike.
sys.path.insert(0, str(Path(__file__).resolve().parent))

from canonicalise import escape_qt_ini_key, escape_qt_ini_value  # noqa: E402  (after sys.path)
from qt_ini import (  # noqa: E402  (after sys.path)
    qt_ini_lines,
    qt_ini_unescape,
    qt_ini_unescape_key,
    read_settings_ini,
)

# Fixed on purpose: the fuzz must give the same answer tomorrow, so both the
# seed and the iteration count live here rather than in a command line.
FUZZ_SEED = 20261105
FUZZ_ITERATIONS = 500

# What the helper is expected to produce for s20's download folder; it
# documents the bytes rather than deriving them.
S20_FOLDER_BYTES = "\\x417\\x430\\x433\\x440\\x443\\x437\\x43a\\x438"

# T9/T10's table: the values whose round trip must hold, including the ones
# that broke the helper before it learned Qt's rules.
ROUND_TRIP_VALUES = (
    "Загрузки",
    "Загрузки5",       # a digit after an escape: the reader's hex scan is greedy
    "Папка1",
    "тестa",           # a hex letter after an escape
    "é5",
    "Отпуск2024",
    "фото1",
    "фото15",          # a run of digits after an escape
    "тестa5",
    "a\\b",            # a literal backslash
    'Он сказал "да"',  # a double quote
    "загрузки ",       # a trailing space is chopped unless the value is quoted
    " загрузки",       # ... and a leading one is not, but quoting neither hurts
    "en_US",           # the plain-ASCII values of the other cases
    "/home/user/downloads/archive",
    "/home/user/Загрузки, архив",   # a comma: a QStringList unless quoted
    "output dir, второй",
    "key=value;",
)

# One value per category T12.2 asks for.  Table and categories are checked the
# same way; they are separate so that a failure names the category.
CATEGORY_VALUES = (
    "plain ascii",
    "a;b",
    "a,b",
    "a=b",
    'он сказал "да"',
    "c:\\temp\\x",
    " leading",
    "trailing ",
    "\t",
    "\n",
    "\x00\x01\x1f\x7f",
    "Загрузки",
    "\U0001F600",
)

# The fuzz alphabet: the same categories, as characters rather than values.
FUZZ_ALPHABET = (
    "abcXYZ019"        # ASCII
    ";,="              # Qt's quoting triggers
    '"\\'              # the two escaped ASCII characters
    " "                # edge spaces
    "\t\n\r"           # tab and other line-ish control characters
    "\x00\x01\x07\x1b\x7f"  # control characters
    "Загрузкиé"        # Cyrillic
    "\U0001F600"       # a non-BMP character
    "ф5a"              # hex letters and digits directly after an escape
)


# The reader itself lives in `qt_ini.py` so that the verifier and the userdata
# capture can read the same dialect without importing a test module; the tests
# below are what prove the writer against it.

# The committed userdata file: written by the original, so it is Qt dialect by
# construction, and G10 is the finding that nothing checked it.
USERDATA_SETTINGS = (
    Path(__file__).resolve().parents[2]
    / "fixtures" / "userdata" / "media-downloader" / "settings" / "settings.ini"
)


# Keys are a different dialect from values (data-compat §3.0a): `%XX` for
# Latin-1, `%UXXXX` for the rest, `\\` for the group separator.  Nothing in
# fixtures/ uses one; real Windows settings.ini files do.
KEY_CORPUS = (
    "DownloadFolder",               # ASCII, the ordinary case
    "CookieBrowser_yt-dlp-aria2c",  # every character Qt keeps literal
    "Загрузки",                     # %UXXXX
    "é",                            # %XX, Latin-1
    "My/Group/Key",                 # a nested key: a backslash in the file
    "50%",                          # a literal percent, which is escaped
    "\U0001F600",                   # non-BMP: two escapes, as Qt writes it
    "a b",                          # a space, escaped
)


def check_keys(escape_key=escape_qt_ini_key, unescape_key=qt_ini_unescape_key) -> int:
    """Check that a key survives its own escape, which no fixture exercises."""
    for key in KEY_CORPUS:
        escaped = escape_key(key)
        _require(all(ord(char) < 0x80 for char in escaped),
                 f"key {key!r} escaped to non-ASCII {escaped!r}")
        _require(unescape_key(escaped) == key,
                 f"key {key!r} is written as {escaped!r}, which Qt reads back as "
                 f"{unescape_key(escaped)!r}")
    return len(KEY_CORPUS)


def check_userdata_settings(path: Path | str = USERDATA_SETTINGS) -> dict[str, int]:
    """T13/G10: the committed userdata `settings.ini` is Qt's own output.

    The original wrote this file, so it is Qt dialect by construction -- but
    `configparser` read it as UTF-8 and would accept anything, which is what G10
    is.  Here the file is read the way Qt reads it (Latin-1 bytes, then the
    reader) and every value is written back with the helper: if the two forms
    differ, the file is not in the dialect Qt produces.  Nothing is repaired --
    the check reports and the caller decides.
    """
    path = Path(path)
    lines = qt_ini_lines(path)
    non_ascii = sum(1 for byte in path.read_bytes() if byte >= 0x80)
    lists = 0
    for raw_key, raw in lines:
        key = qt_ini_unescape_key(raw_key)
        _require(escape_qt_ini_key(key) == raw_key,
                 f"{path.name}: key {raw_key!r} decodes to {key!r}, which would be written "
                 f"as {escape_qt_ini_key(key)!r}")
        decoded = qt_ini_unescape(raw)
        if isinstance(decoded, list):
            lists += 1
            reencoded = ", ".join(escape_qt_ini_value(item) for item in decoded)
        else:
            reencoded = escape_qt_ini_value(decoded)
        _require(reencoded == raw,
                 f"{path.name}: {key} is written as {raw!r}, but the value Qt reads back "
                 f"({decoded!r}) would be written as {reencoded!r}")
    return {"keys": len(lines), "lists": lists, "non_ascii": non_ascii}


def fuzz_values(seed: int = FUZZ_SEED, iterations: int = FUZZ_ITERATIONS):
    """Yield the seeded corpus: same seed, same values, every time."""
    rng = random.Random(seed)
    pool = "".join(FUZZ_ALPHABET)
    for _ in range(iterations):
        yield "".join(rng.choice(pool) for _ in range(rng.randrange(0, 13)))


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def expected_after_qt(value: str, escaped: str) -> str:
    """What Qt's reader gives back for ``value``, given ``escaped``.

    Everything round-trips exactly, with one documented exception: an unquoted
    trailing U+0009 is chopped by `iniChopTrailingSpaces` *even when it arrived
    as an escape*, and Qt's writer does not quote for a tab (only for a space).
    Qt itself loses that character, so the helper does too; `QtIniLimit` pins it.
    """
    if escaped.startswith('"'):
        return value
    return value.rstrip("\t")


def round_trip(escape, values) -> int:
    """Check that Qt's reader gives back every value the writer produced."""
    checked = 0
    for value in values:
        escaped = escape(value)
        _require(all(ord(char) < 0x80 for char in escaped),
                 f"{value!r} escaped to non-ASCII {escaped!r}")
        _require("\n" not in escaped and "\r" not in escaped,
                 f"{value!r} escaped to a value that breaks the line: {escaped!r}")
        decoded = qt_ini_unescape(escaped)
        expected = expected_after_qt(value, escaped)
        _require(decoded == expected,
                 f"{value!r} is written as {escaped!r}, which Qt reads back as {decoded!r}")
        checked += 1
    return checked


def check_digit_flag(escape) -> None:
    """A hex digit directly after an escape must be escaped in turn (T9)."""
    _require(escape("Загрузки5").endswith("\\x438\\x35"),
             f"a digit after an escape is not escaped: {escape('Загрузки5')!r}")
    _require(escape("фото15").endswith("\\x31\\x35"),
             f"a run of digits after an escape is not escaped: {escape('фото15')!r}")
    _require(escape("тестa") == "\\x442\\x435\\x441\\x442\\x61",
             f"a hex letter after an escape is not escaped: {escape('тестa')!r}")


def check_quoting(escape) -> int:
    """Exactly Qt's five quoting conditions, and no others (T10)."""
    quoted = 0
    for value in ("a;b", "a,b", "a=b", " leading", "trailing "):
        _require(escape(value).startswith('"'), f"{value!r} must be quoted, is {escape(value)!r}")
        quoted += 1
    for value in ("two words", "/home/user/output dir/Загрузки", "en_US", "a-b", ""):
        _require(not escape(value).startswith('"'), f"{value!r} must not be quoted, is {escape(value)!r}")
    comma = "/home/user/Загрузки, архив"
    escaped = escape(comma)
    _require(escaped.startswith('"') and escaped.endswith('"'),
             f"a comma does not quote the value: {escaped!r}")
    _require(qt_ini_unescape(escaped) == comma,
             f"the reader does not strip the quotes: {qt_ini_unescape(escaped)!r}")
    return quoted


def check_all(escape=escape_qt_ini_value) -> dict[str, int]:
    """Run every check; raise AssertionError on the first failure.

    The `escape` argument exists so that the negative-control test below can
    hand it a deliberately broken writer: a check that cannot fail proves
    nothing.
    """
    values = round_trip(escape, ROUND_TRIP_VALUES)
    values += round_trip(escape, CATEGORY_VALUES)

    _require(escape("Загрузки") == S20_FOLDER_BYTES,
             f"s20's folder no longer escapes to {S20_FOLDER_BYTES!r}: {escape('Загрузки')!r}")
    check_digit_flag(escape)
    quoted = check_quoting(escape)
    for value in ("en_US", "/home/user/downloads/archive", "yt-dlp"):
        _require(escape(value) == value, f"{value!r} is no longer written verbatim: {escape(value)!r}")

    keys = check_keys()
    fuzz = round_trip(escape, fuzz_values())
    return {"values": values, "keys": keys, "quoted": quoted, "fuzz": fuzz, "seed": FUZZ_SEED}


class QtIniRoundTrip(unittest.TestCase):
    """The writer must survive Qt's reader for every value a fixture can need."""

    def test_round_trip_table(self):
        self.assertEqual(round_trip(escape_qt_ini_value, ROUND_TRIP_VALUES), len(ROUND_TRIP_VALUES))

    def test_round_trip_categories(self):
        self.assertEqual(round_trip(escape_qt_ini_value, CATEGORY_VALUES), len(CATEGORY_VALUES))

    def test_fuzz_is_reproducible(self):
        self.assertEqual(list(fuzz_values()), list(fuzz_values()))
        self.assertEqual(len(list(fuzz_values())), FUZZ_ITERATIONS)

    def test_fuzz_seeded_corpus(self):
        self.assertEqual(round_trip(escape_qt_ini_value, fuzz_values()), FUZZ_ITERATIONS)

    def test_digit_after_escape(self):
        check_digit_flag(escape_qt_ini_value)

    def test_s20_folder_bytes(self):
        self.assertEqual(escape_qt_ini_value("Загрузки"), S20_FOLDER_BYTES)

    def test_quoting_matches_qt(self):
        self.assertEqual(check_quoting(escape_qt_ini_value), 5)

    def test_ascii_is_verbatim(self):
        for value in ("en_US", "/home/user/downloads/archive", "yt-dlp", "output dir/Загрузки"):
            if value.isascii():
                self.assertEqual(escape_qt_ini_value(value), value)

    def test_reader_port_keeps_its_own_rules(self):
        """The port is Qt's reader, not a copy of the writer's assumptions."""
        self.assertEqual(qt_ini_unescape("\\x4385"), "\u4385")   # greedy: one code point, not U+0438 + "5"
        self.assertEqual(qt_ini_unescape("a,b"), ["a", "b"])
        self.assertEqual(qt_ini_unescape("a\\,b"), "ab")     # an unknown escape is dropped
        self.assertEqual(qt_ini_unescape("value  "), "value")  # an unquoted trailing space is chopped
        self.assertEqual(qt_ini_unescape('"value  "'), "value  ")         # ... unless quoted
        self.assertEqual(qt_ini_unescape('\\"quoted\\"'), '"quoted"')     # \x22 is not quoting

    def test_check_all(self):
        summary = check_all()
        self.assertEqual(summary["values"], len(ROUND_TRIP_VALUES) + len(CATEGORY_VALUES))
        self.assertEqual(summary["fuzz"], FUZZ_ITERATIONS)
        self.assertEqual(summary["quoted"], 5)
        self.assertEqual(summary["seed"], FUZZ_SEED)
        self.assertEqual(summary["keys"], len(KEY_CORPUS))

    def test_a_broken_writer_is_caught(self):
        """Proof that the checks above are not vacuous: drop the digit flag."""
        def naive(value: str) -> str:
            units = value.encode("utf-16-be")
            out = []
            for index in range(0, len(units), 2):
                unit = (units[index] << 8) | units[index + 1]
                if unit == 0x5C:
                    out.append("\\\\")
                elif unit == 0x22:
                    out.append('\\"')
                elif 0x20 <= unit <= 0x7E:
                    out.append(chr(unit))
                else:
                    out.append("\\x%x" % unit)
            return "".join(out)

        with self.assertRaises(AssertionError) as caught:
            check_all(naive)
        self.assertIn("Загрузки5", str(caught.exception))


class QtIniLimit(unittest.TestCase):
    """What the helper deliberately does *not* claim, as executable statements.

    These are not tests of Qt; they pin the boundary so that a later change
    cannot quietly widen the promise without widening the checks.
    """

    def test_control_characters_are_not_written_like_qt(self):
        """Qt writes \\n, \\t, \\0; the helper writes \\xa, \\x9, \\x0.

        Qt reads both back as the same character -- `QtIniRoundTrip` proves that
        -- but the bytes are not Qt's, and nothing here claims they are.
        """
        self.assertEqual(escape_qt_ini_value("\n"), "\\xa")
        self.assertEqual(escape_qt_ini_value("\t"), "\\x9")
        self.assertEqual(escape_qt_ini_value("\x00"), "\\x0")

    def test_variant_prefixes_are_out_of_scope(self):
        """Qt reads `@ByteArray(...)` as a QByteArray, not as this string."""
        for prefix in ("@ByteArray(", "@Variant(", "@DateTime("):
            value = prefix + "x)"
            self.assertEqual(escape_qt_ini_value(value), value)   # written verbatim
            self.assertEqual(qt_ini_unescape(value), value)       # the port does not model it either

    def test_a_trailing_tab_is_chopped_by_qt_itself(self):
        """Qt chops it too; the helper matches Qt rather than fixing it.

        `iniChopTrailingSpaces` runs from the length the value had before its
        last escape, so an escaped trailing tab is removed exactly like a
        literal one, and `iniEscapedString` sets `needsQuotes` for a space but
        not for a tab.  A value that ends in U+0009 therefore loses that
        character in Qt's own writer/reader pair as well.
        """
        self.assertEqual(escape_qt_ini_value("a\t"), "a\\x9")
        self.assertEqual(qt_ini_unescape("a\\x9"), "a")    # what Qt does with our bytes
        self.assertEqual(qt_ini_unescape("a\\t"), "a")     # ... and with its own
        self.assertEqual(qt_ini_unescape('"a\t"'), "a\t")  # quoting is what would have kept it

    def test_qstringlist_splitting_is_not_emitted(self):
        """A comma makes Qt read a QStringList; the helper always quotes it."""
        self.assertTrue(escape_qt_ini_value("a,b").startswith('"'))
        self.assertEqual(qt_ini_unescape("a,b"), ["a", "b"])


class UserdataSettingsIni(unittest.TestCase):
    """T13/G10: the third Qt-dialect file is read with Qt's reader, not UTF-8."""

    @unittest.skipUnless(USERDATA_SETTINGS.is_file(),
                         "fixtures/userdata is not part of this checkout")
    def test_committed_userdata_is_qt_dialect(self):
        summary = check_userdata_settings()
        self.assertGreaterEqual(summary["keys"], 90)   # the fixture has 94
        self.assertEqual(summary["non_ascii"], 0,
                         "the original writes ASCII; a non-ASCII byte is Latin-1 on read")
        self.assertGreaterEqual(summary["lists"], 1)   # the options-history keys

    def test_a_utf8_value_is_reported(self):
        """The check must fail on what `configparser` accepted without comment."""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "settings.ini"
            path.write_bytes("[General]\nDownloadFolder=/tmp/Загрузки\n".encode("utf-8"))
            with self.assertRaises(AssertionError) as caught:
                check_userdata_settings(path)
        self.assertIn("DownloadFolder", str(caught.exception))

    def test_a_malformed_line_is_reported(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "settings.ini"
            path.write_bytes(b"[General]\nno-equals-sign\n")
            with self.assertRaises(ValueError):
                read_settings_ini(path)


class LoneSurrogate(unittest.TestCase):
    """T13.2: the behaviour is chosen, not a UnicodeEncodeError in passing."""

    def test_a_lone_surrogate_is_rejected_with_a_message(self):
        with self.assertRaises(ValueError) as caught:
            escape_qt_ini_value("ok\ud800bad")
        self.assertIn("surrogate", str(caught.exception))
        self.assertIn("U+d800", str(caught.exception))

    def test_a_real_non_bmp_character_is_still_written(self):
        """The rejection is about lone surrogates, not about astral characters."""
        self.assertEqual(escape_qt_ini_value("\U0001F600"), "\\xd83d\\xde00")
        self.assertEqual(qt_ini_unescape("\\xd83d\\xde00"), "\U0001F600")


SETTINGS_INI_SAMPLE = (
    "[General]\n"
    "AutoDownload=false\n"
    "%U0417%U0430%U0433%U0440%U0443%U0437%U043A%U0438=\\x417\\x430\\x433\\x440\\x443\\x437\\x43a\\x438\n"
    "%E9=50%25\n"
    "My\\Group\\Key=two words\n"
    "%UD83D%UDE00=\\xd83d\\xde00\n"
    "50%25=value\n"
)


class QtIniKeys(unittest.TestCase):
    """T14/G11: the reader understands keys, not only values.

    Qt escapes a key with a dialect of its own -- `%XX` for Latin-1, `%UXXXX`
    above it, `\\` for the group separator -- and `iniUnescapedKey` is what
    reverses it.  No fixture in this tree has such a key; real settings.ini
    files from Windows users do.
    """

    def test_a_wide_escape(self):
        self.assertEqual(qt_ini_unescape_key("%U0417%U0430"), "За")
        self.assertEqual(escape_qt_ini_key("Загрузки"),
                         "%U0417%U0430%U0433%U0440%U0443%U0437%U043A%U0438")

    def test_a_latin1_escape(self):
        self.assertEqual(qt_ini_unescape_key("%D0%BF"), "Ð¿")   # a byte, not a UTF-8 pair
        self.assertEqual(escape_qt_ini_key("é"), "%E9")         # uppercase hex on the way out
        self.assertEqual(qt_ini_unescape_key("%e9"), "é")       # ... and case-insensitive on the way in

    def test_a_nested_key(self):
        self.assertEqual(escape_qt_ini_key("My/Group/Key"), "My\\Group\\Key")
        self.assertEqual(qt_ini_unescape_key("My\\Group\\Key"), "My/Group/Key")

    def test_a_literal_percent(self):
        """A '%' in a key is always an escape in the file, so `50%` is `50%25`."""
        self.assertEqual(escape_qt_ini_key("50%"), "50%25")
        self.assertEqual(qt_ini_unescape_key("50%25"), "50%")

    def test_a_non_bmp_key_is_two_escapes(self):
        self.assertEqual(escape_qt_ini_key("\U0001F600"), "%UD83D%UDE00")
        self.assertEqual(qt_ini_unescape_key("%UD83D%UDE00"), "\U0001F600")

    def test_the_key_corpus_round_trips(self):
        self.assertEqual(check_keys(), len(KEY_CORPUS))

    def test_a_malformed_escape_is_refused(self):
        """Qt appends a literal '%' and carries on; we refuse instead.

        A key that silently decodes to something else is the failure this port
        exists to prevent: the caller looks for a setting that is not there.
        No file Qt wrote contains a malformed escape.
        """
        for bad in ("%U417", "%zz", "%u0417", "%U04", "abc%"):
            with self.assertRaises(ValueError, msg=bad):
                qt_ini_unescape_key(bad)

    def test_the_file_reader_refuses_a_malformed_key(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "settings.ini"
            path.write_text("[General]\n%zz=1\n", encoding="ascii")
            with self.assertRaises(ValueError):
                read_settings_ini(path)

    def test_a_whole_settings_file_round_trips_byte_for_byte(self):
        """One file with every case, decoded and written back by the writers."""
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "settings.ini"
            path.write_text(SETTINGS_INI_SAMPLE, encoding="ascii")

            keys = read_settings_ini(path)
            self.assertIn("Загрузки", keys)
            self.assertIn("My/Group/Key", keys)
            self.assertIn("\U0001F600", keys)

            rebuilt = ["[General]"]
            for raw_key, raw_value in qt_ini_lines(path):
                rebuilt.append(
                    f"{escape_qt_ini_key(qt_ini_unescape_key(raw_key))}="
                    f"{escape_qt_ini_value(qt_ini_unescape(raw_value))}"
                )
            self.assertEqual("\n".join(rebuilt) + "\n", SETTINGS_INI_SAMPLE)


class OneWriterForSettingsIni(unittest.TestCase):
    """T12.3: `settings.ini` has exactly one writer, and it is the helper."""

    def test_capture_has_no_second_inline_writer(self):
        source = (Path(__file__).resolve().parent / "capture.py").read_text(encoding="utf-8")
        # No `Key=value` line written by hand: every value, the post-download
        # hook's command line included, goes through the settings loop.
        self.assertNotIn("CommandOnSuccessfulDownload=", source)
        self.assertEqual(source.count("escape_qt_ini_value("), 1)


if __name__ == "__main__":
    unittest.main()
