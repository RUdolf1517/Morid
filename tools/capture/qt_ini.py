#!/usr/bin/env python3
"""Qt 5.15's INI dialect: the reader side, and how Qt reads a file.

`canonicalise.escape_qt_ini_value` writes the dialect; this module reads it.  It
is a transcription of `QSettingsPrivate::iniUnescapedStringList`,
`iniChopTrailingSpaces`, `QConfFileSettingsPrivate::readIniLine` and
`readIniSection` from `qtbase/src/corelib/io/qsettings.cpp` at tag
`v5.15.16-lts-lgpl`, reduced to the pieces a single-value settings file needs.

Two callers: `test_qt_ini_roundtrip.py` proves the writer with it, and
`verify_fixtures.py` / `capture_userdata.py` use it to read the committed
`fixtures/userdata/.../settings.ini`, which `configparser` reads as UTF-8 — the
one thing Qt does not do (see D-016 and data-compat §3.0).
"""

from __future__ import annotations

from pathlib import Path

# The single-value escapes Qt's reader knows (escapeCodes in qsettings.cpp).
# A backslash before anything else *drops* that character.
QT_INI_ESCAPES = {
    "a": "\a", "b": "\b", "f": "\f", "n": "\n", "r": "\r", "t": "\t", "v": "\v",
    '"': '"', "?": "?", "'": "'", "\\": "\\",
}

HEX_DIGITS = "0123456789abcdefABCDEF"


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
    value prefixes, which `stringToVariant` turns into other types.  The writer
    does not claim those either; see `QtIniLimit` in the test module.
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


def _split_line(line: str) -> tuple[str, str | None]:
    """Split at the first '=' outside quotes, as `readIniLine` reports it."""
    in_quotes = False
    for index, char in enumerate(line):
        if char == '"':
            in_quotes = not in_quotes
        elif char == "=" and not in_quotes:
            return line[:index], line[index + 1:]
    return line, None


def qt_ini_unescape_key(text: str) -> str:
    """Decode an INI key: `%XX`, `%UXXXX`, and `\\` for the group separator.

    Mirrors `QSettingsPrivate::iniUnescapedKey` with one deliberate difference:
    Qt appends a literal '%' and carries on when an escape is truncated or not
    hexadecimal, while this raises.  A key that silently decodes to something
    else is exactly the failure this port exists to prevent -- the caller then
    looks for a setting that is not there and reports a missing key rather than
    a decoding problem -- and no file Qt wrote contains a malformed escape.

    Qt's `%U` marker is an uppercase 'U' only; the four hex digits after it are
    case-insensitive, as `QString::toInt(&ok, 16)` is.  Qt's `lowercaseOnly`
    return value is not modelled: it does not change the key's text, it selects
    `Qt::CaseSensitive` versus the platform's `IniCaseSensitivity` for lookups,
    which is `Qt::CaseSensitive` on Linux, where these fixtures were captured.
    """
    units: list[int] = []
    i, size = 0, len(text)
    while i < size:
        char = text[i]
        if char == "\\":
            units.append(ord("/"))
            i += 1
            continue
        if char != "%":
            units.append(ord(char))
            i += 1
            continue
        wide = text[i + 1:i + 2] == "U"
        digits = 4 if wide else 2
        start = i + 2 if wide else i + 1
        chunk = text[start:start + digits]
        if len(chunk) != digits or any(digit not in HEX_DIGITS for digit in chunk):
            raise ValueError(
                f"malformed %-escape in the key {text!r} at offset {i}: "
                f"{text[i:i + 1 + digits]!r} is not a {'%UXXXX' if wide or text[i + 1:i + 2] == 'U' else '%XX'} escape"
            )
        units.append(int(chunk, 16))
        i = start + digits
    return _decode_units(units)


def qt_ini_lines(path: Path | str) -> list[tuple[str, str]]:
    """Return `[(raw key, raw value text)]` for a file written the way Qt does.

    The bytes are decoded as Latin-1, not UTF-8, because that is what Qt 5 does
    without `QSettings::setIniCodec`: a byte above 0x7f becomes the code point
    of the same number.  The text is left escaped, so a caller can compare it
    with what the writers produce.

    Lines follow `readIniLine`/`readIniSection`: leading blanks are skipped, a
    line starting with '[' is a section header, a ';' line without '=' is a
    comment, and the key ends at the first '=' outside quotes (with trailing
    spaces and tabs trimmed).  Anything else is an error here rather than a
    guess: only the `[General]` section the original writes is accepted, and a
    line without '=' that is not a comment raises.
    """
    text = Path(path).read_bytes().decode("latin-1")
    lines: list[tuple[str, str]] = []
    section = ""
    for number, line in enumerate(text.splitlines(), start=1):
        line = line.lstrip(" \t")
        if not line:
            continue
        if line.startswith("["):
            end = line.find("]")
            if end == -1:
                raise ValueError(f"{path}:{number}: unterminated section header")
            section = line[1:end].strip()
            if qt_ini_unescape_key(section).lower() != "general":
                raise ValueError(f"{path}:{number}: unexpected section [{section}]")
            continue
        key, raw = _split_line(line)
        if raw is None:
            if line.startswith(";"):
                continue
            raise ValueError(f"{path}:{number}: line without '=': {line!r}")
        key = key.rstrip(" \t")
        if not key:
            raise ValueError(f"{path}:{number}: empty key")
        if section.lower() != "general":
            raise ValueError(f"{path}:{number}: key outside [General]: {key!r}")
        lines.append((key, raw))
    return lines


def read_settings_ini(path: Path | str) -> dict[str, str]:
    """Return `{decoded key: raw value text}` for a file Qt wrote.

    The key is decoded (`%XX`, `%UXXXX`, `\\` for '/'), the value is left as
    text so that `qt_ini_unescape` can be applied by a caller that wants the
    settings value or by one that wants to re-encode it and compare bytes.
    """
    return {qt_ini_unescape_key(key): raw for key, raw in qt_ini_lines(path)}
