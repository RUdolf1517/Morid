#!/usr/bin/env python3
"""Replace capture-host paths with fixed placeholders.

A Phase 0 fixture is only useful if it means the same thing on every machine,
so the capture writes its paths by *role*:

  <case-dir>    the scenario's disposable runtime directory.
                <runtime>/s09-download-archive/appdata/data/archiveFile-yt-dlp.txt
                becomes <case-dir>/appdata/data/archiveFile-yt-dlp.txt
  <node>        the host's JavaScript interpreter, as passed to yt-dlp:
                node:/usr/local/bin/node becomes node:<node>
  <local-url>   a loopback stream URL, used only by fixtures/streams/**/meta.json
  /home/user    the capture host's home directory (the same fixed root the
                anonymised fixtures/userdata uses)

Nothing else is rewritten.  A value is either mapped to exactly one value or
rejected: the number and order of arguments cannot change, and a path that
cannot be expressed with the vocabulary aborts the capture instead of being
written to a fixture.
"""

from __future__ import annotations

import os
import re
from pathlib import Path
from typing import Any

CASE_DIR = "<case-dir>"
NODE = "<node>"
LOCAL_URL = "<local-url>"
HOME = "/home/user"


NODE_IN_TEXT = re.compile(r"node:(/[^\s\"',]*)")


class CanonicalisationError(RuntimeError):
    """A value cannot be expressed with the documented placeholders."""


def _inside(value: str, root: str) -> str | None:
    """Return the tail of ``value`` below ``root``, or None if it is not inside."""
    if value == root:
        return ""
    if value.startswith(root + "/"):
        return value[len(root):]
    return None


class Canonicaliser:
    """Substitutions for one scenario, or for a candidate on another machine."""

    def __init__(self, case_dir: Path | str, home: str | None = None, forbidden: tuple[Path | str, ...] = ()) -> None:
        self.case_dir = str(case_dir).rstrip("/")
        self.runtime_root = str(Path(self.case_dir).parent)
        self.home = (home or os.environ.get("HOME") or HOME).rstrip("/") or "/"
        self.forbidden = tuple(str(item).rstrip("/") for item in forbidden)

    def __repr__(self) -> str:  # pragma: no cover - diagnostics only
        return f"Canonicaliser(case_dir={self.case_dir!r}, home={self.home!r})"

    # -- single values ----------------------------------------------------

    def value(self, value: str) -> str:
        if not isinstance(value, str):
            raise CanonicalisationError(f"not a string: {value!r}")
        result = self._case_dir(value)
        result = self._home(result)
        result = self._node(result)
        self._reject_residual(value, result)
        return result

    def _case_dir(self, value: str) -> str:
        if value.startswith(CASE_DIR) or value.startswith(NODE + ":") or value.startswith(LOCAL_URL):
            return value
        tail = _inside(value, self.case_dir)
        if tail is not None:
            return CASE_DIR + tail
        # A path below the runtime root but outside this case's directory has
        # no placeholder of its own: keep per-case files inside <case-dir>.
        if value == self.runtime_root or value.startswith(self.runtime_root + "/"):
            raise CanonicalisationError(
                f"{value!r} is outside the case directory {self.case_dir!r}; "
                "a fixture path must live inside its own case directory"
            )
        return value

    def _home(self, value: str) -> str:
        if value.startswith(HOME) or self.home == HOME:
            return value
        tail = _inside(value, self.home)
        if tail is None:
            return value
        return HOME + tail

    def _node(self, value: str) -> str:
        if not value.startswith("node:") or value == "node:" + NODE:
            return value
        # yt-dlp's --js-runtimes takes a comma separated list of runtime:entry.
        parts = []
        for part in value.split(","):
            if not part.startswith("node:"):
                parts.append(part)
                continue
            path = part[len("node:"):]
            if not path.startswith("/"):
                raise CanonicalisationError(f"node runtime entry is not an absolute path: {part!r}")
            parts.append("node:" + NODE)
        return ",".join(parts)

    def _reject_residual(self, original: str, result: str) -> None:
        for root in (self.case_dir, self.runtime_root) + self.forbidden:
            if root and result != root and root in result:
                raise CanonicalisationError(f"{original!r} still contains the host path {root!r} after canonicalisation")
        if self.home != HOME and self.home in result:
            raise CanonicalisationError(f"{original!r} still contains the host home {self.home!r} after canonicalisation")

    # -- transcripts ------------------------------------------------------

    def argv(self, values: list[str]) -> list[str]:
        if not isinstance(values, list):
            raise CanonicalisationError(f"argv is not a list: {values!r}")
        canonical = [self.value(item) for item in values]
        if len(canonical) != len(values):
            raise CanonicalisationError("canonicalisation changed the argument count")
        return canonical

    def record(self, record: dict[str, Any]) -> dict[str, Any]:
        canonical = dict(record)
        canonical["argv"] = self.argv(record.get("argv", []))
        canonical["program"] = self.value(record.get("program", ""))
        canonical["cwd"] = self.value(record.get("cwd", ""))
        env = record.get("env", {})
        if not isinstance(env, dict):
            raise CanonicalisationError(f"env is not an object: {env!r}")
        canonical["env"] = {key: self.value(str(item)) for key, item in env.items()}
        return canonical

    def text(self, text: str) -> str:
        """Canonicalise free-form diagnostic text (app.log) line by line."""
        lines = []
        for line in text.splitlines(keepends=True):
            body = line.rstrip("\n")
            ending = line[len(body):]
            if self.case_dir in body:
                body = body.replace(self.case_dir, CASE_DIR)
            if "node:/" in body:
                body = NODE_IN_TEXT.sub("node:" + NODE, body)
            if self.home != HOME and self.home in body:
                body = body.replace(self.home, HOME)
            self._reject_residual(line, body)
            lines.append(body + ending)
        return "".join(lines)
