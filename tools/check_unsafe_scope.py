#!/usr/bin/env python3
"""D-019 source guard; checks both OS modules even on a Linux runner.

This is a lexical scope/lint check, not a proof of FFI invariants or handle aliases.
Rust compilation and review of each SAFETY comment remain necessary.
"""
from pathlib import Path
import re
import sys
import tomllib

ALLOWED = {
    "crates/morid-engine/src/process_group.rs",
    "crates/morid-engine/src/job_object.rs",
}
ENGINE = "crates/morid-engine/src/lib.rs"
GUI = "crates/morid-gui/src/main.rs"  # Existing Phase 1 Slint exception, not D-019.
EXCLUDED = {".git", ".cache", ".arena", ".venv", "node_modules", "target", "legacy"}


def code_only(text):
    """Mask strings/chars and nested comments, retaining offsets and newlines."""
    result = list(text)
    i = 0

    def mask(start, end):
        for pos in range(start, end):
            if result[pos] not in "\r\n":
                result[pos] = " "

    while i < len(text):
        start = i
        if text.startswith("//", i):
            end = text.find("\n", i)
            i = len(text) if end < 0 else end
        elif text.startswith("/*", i):
            depth = 1
            i += 2
            while i < len(text) and depth:
                if text.startswith("/*", i):
                    depth += 1
                    i += 2
                elif text.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    i += 1
        else:
            raw = re.match(r'(?:br|cr|r)(#{0,255})"', text[i:])
            if raw and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
                delimiter = '"' + raw.group(1)
                end = text.find(delimiter, i + raw.end())
                i = len(text) if end < 0 else end + len(delimiter)
            elif text[i] == '"':
                i += 1
                while i < len(text):
                    if text[i] == "\\":
                        i += 2
                    elif text[i] == '"':
                        i += 1
                        break
                    else:
                        i += 1
                i = min(i, len(text))
            elif text[i] == "'":
                char = re.match(r"'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]+\}|[^\n])|[^'\\\n])'", text[i:])
                if not char:  # A lifetime, not a character literal.
                    i += 1
                    continue
                i += char.end()
            else:
                i += 1
                continue
        mask(start, i)
    return "".join(result)


def crate_roots(root):
    manifest = tomllib.loads((root / "Cargo.toml").read_text())
    members = list(manifest["workspace"]["members"])
    if (root / "xtask/Cargo.toml").is_file():
        members.append("xtask")
    paths = set()
    for member in members:
        directory = root / member
        data = tomllib.loads((directory / "Cargo.toml").read_text())
        for default in ["src/lib.rs", "src/main.rs"]:
            path = directory / default
            if path.is_file():
                paths.add(path)
        if "lib" in data and "path" in data["lib"]:
            paths.add(directory / data["lib"]["path"])
        for target in data.get("bin", []):
            if "path" in target:
                paths.add(directory / target["path"])
    return paths


def safety_comment(text, offset):
    before = text[:text.rfind("\n", 0, offset) + 1].splitlines()
    for line in reversed(before):
        if not line.strip():
            continue
        if not line.lstrip().startswith("//"):
            break
        if re.match(r"\s*//\s*SAFETY:\s*\S", line):
            return True
    return False


def check(root):
    root = Path(root)
    errors = []
    counts = {path: 0 for path in sorted(ALLOWED)}
    sources = {}
    for path in sorted(root.rglob("*.rs")):
        relative = path.relative_to(root)
        if any(part in EXCLUDED for part in relative.parts):
            continue
        name = relative.as_posix()
        text = path.read_text(encoding="utf-8")
        code = code_only(text)
        sources[name] = code
        for attribute in re.finditer(r"#\s*!?\s*\[[^\]]*\b(?:allow|expect)\s*\([^)]*\bunsafe_code\b[^\]]*\]", code):
            exact = re.fullmatch(r"#!\[allow\(unsafe_code\)\]", re.sub(r"\s", "", attribute.group()))
            if name not in ALLOWED or not exact:
                errors.append(f"{name}: unapproved unsafe_code exemption")
        for token in re.finditer(r"\bunsafe\b", code):
            line = text.count("\n", 0, token.start()) + 1
            if name not in ALLOWED:
                errors.append(f"{name}:{line}: unsafe outside the two platform modules")
                continue
            if not code[token.end():].lstrip().startswith("{"):
                errors.append(f"{name}:{line}: only unsafe blocks allowed, not fn/impl/trait/extern")
                continue
            counts[name] += 1
            if not safety_comment(text, token.start()):
                errors.append(f"{name}:{line}: missing adjacent SAFETY invariant comment")
        # Deliberately conservative check of literal public signatures/types;
        # aliases/re-exports still require review, not a false claim of AST proof.
        if name in ALLOWED:
            for public in re.finditer(r"\bpub\s+(?:unsafe\s+)?(?:fn|type|const|static)\b[^;{]*", code):
                if re.search(r"\bisize\b|\*\s*(?:mut|const)\b|\b(?:HANDLE|RawHandle)\b", public.group()):
                    errors.append(f"{name}: raw platform type in public API")
    for name in sorted(ALLOWED):
        if name not in sources or not re.match(r"\s*#!\[allow\(unsafe_code\)\]", sources[name]):
            errors.append(f"{name}: expected module-level allow(unsafe_code)")
    for path in sorted(crate_roots(root)):
        name = path.relative_to(root).as_posix()
        expected = "deny" if name in {ENGINE, GUI} else "forbid"
        if not re.match(rf"\s*#!\[{expected}\(unsafe_code\)\]", sources.get(name, "")):
            errors.append(f"{name}: expected crate-level {expected}(unsafe_code)")
    return errors, counts


def main():
    errors, counts = check(Path(__file__).resolve().parents[1])
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        print("unsafe_stays_in_the_two_platform_modules: FAIL", file=sys.stderr)
        return 1
    print("unsafe_stays_in_the_two_platform_modules: OK")
    for path, count in counts.items():
        print(f"{path}: {count} unsafe blocks")
    print("Existing morid-gui binary deny exception (Slint): preserved")
    return 0


if __name__ == "__main__":
    sys.exit(main())
