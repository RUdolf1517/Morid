#!/usr/bin/env python3
"""Qt 5.15's INI dialect: the round trip, the fuzz and the limits.

`escape_qt_ini_value` (see `canonicalise.py`) writes the `settings.ini` values
the capture harness hands to the original, and the original reads them back with
Qt's own parser.  This module is the proof that it can: a port of Qt's reader, a
fixed corpus, a seeded fuzz, and tests that make the helper's limits executable
instead of prose.

It needs the standard library only, no fixtures and no arguments:

    python3 -m unittest discover                  # from the repository root
    python3 -m unittest tools.capture.test_qt_ini_roundtrip
    python3 tools/capture/test_qt_ini_roundtrip.py

`verify_fixtures.py` imports `check_all()` from here, so there is one
implementation with two callers.  Qt's writer is `iniEscapedString`, its reader
`iniUnescapedStringList` and `iniChopTrailingSpaces`, all in
`qtbase/src/corelib/io/qsettings.cpp` at tag `v5.15.16-lts-lgpl`.
"""

from __future__ import annotations

import random
import sys
import unittest
from pathlib import Path

# Importable from the repository root (unittest discovery) and from this
# directory (direct execution, and the verifier next to it) alike.
sys.path.insert(0, str(Path(__file__).resolve().parent))

from canonicalise import escape_qt_ini_value  # noqa: E402  (after sys.path)

# Fixed on purpose: the fuzz must give the same answer tomorrow, so both the
# seed and the iteration count live here rather than in a command line.
FUZZ_SEED = 20261105
FUZZ_ITERATIONS = 500

# The single-value escapes Qt's reader knows (escapeCodes in qsettings.cpp).
# A backslash before anything else *drops* that character; see the port below.
QT_INI_ESCAPES = {
    "a": "\a", "b": "\b", "f": "\f", "n": "\n", "r": "\r", "t": "\t", "v": "\v",
    '"': '"', "?": "?", "'": "'", "\\": "\\",
}

HEX_DIGITS = "0123456789abcdefABCDEF"

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


def _decode_units(units: list[int]) -> str:
    """Turn UTF-16 code units (and code points) into text, as QString does."""
    raw = b"".join(chr(unit).encode("utf-16-be", "surrogatepass") for unit in units)
    return raw.decode("utf-16-be", "surrogatepass")


def qt_ini_unescape(text: str) -> str | list[str]:
    """Read one INI value the way Qt 5.15.16 does, transcribed from its source.

    Mirrors `QSettingsPrivate::iniUnescapedStringList` for a single value:
    leading blanks are skipped, a double quote toggles quoting, a ``\\x`` escape
    scans *all* following hex digits greedily on its own (any length, upper or
    lower case), a backslash plus an unknown character drops that character, an
    unquoted trailing space is chopped by `iniChopTrailingSpaces`, and a comma
    outside quotes starts a QStringList -- which is returned as a list, so a
    caller comparing against a string sees that the value did not survive.

    Deliberately not modelled: the `@ByteArray(`, `@Variant(` and `@DateTime(`
    value prefixes, which Qt's reader turns into other types.  The helper does
    not claim those either; see `QtIniLimit`.
    """
    units: list[int] = []
    items: list[list[int]] = []
    quoted_value = False
    in_quotes = False
    i, size = 0, len(text)

    def chop() -> None:
        while units and units[-1] in (0x20, 0x09):
            units.pop()

    while i < size and text[i] in " \t":
        i += 1
    while i < size:
        char = text[i]
        if char == "\\":
            i += 1
            if i >= size:
                break
            char = text[i]
            i += 1
            if char in QT_INI_ESCAPES:
                units.append(ord(QT_INI_ESCAPES[char]))
            elif char == "x":
                j = i
                while j < size and text[j] in HEX_DIGITS:
                    j += 1
                if j > i:
                    units.append(int(text[i:j], 16) & 0xFFFF)
                    i = j
            elif char in "01234567":
                j = i
                while j < size and text[j] in "01234567":
                    j += 1
                units.append(int(char + text[i:j], 8) & 0xFFFF)
                i = j
            elif char in "\n\r":
                if i < size and text[i] in "\n\r" and text[i] != char:
                    i += 1
            # any other escaped character is dropped, exactly as Qt drops it
            continue
        if char == '"':
            quoted_value = True
            in_quotes = not in_quotes
            i += 1
            if not in_quotes:
                while i < size and text[i] in " \t":
                    i += 1
            continue
        if char == "," and not in_quotes:
            if not quoted_value:
                chop()
            items.append(units)
            units = []
            quoted_value = False
            i += 1
            while i < size and text[i] in " \t":
                i += 1
            continue
        units.append(ord(char))
        i += 1
    if not quoted_value:
        chop()
    if not items:
        return _decode_units(units)
    items.append(units)
    return [_decode_units(item) for item in items]


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

    fuzz = round_trip(escape, fuzz_values())
    return {"values": values, "quoted": quoted, "fuzz": fuzz, "seed": FUZZ_SEED}


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
