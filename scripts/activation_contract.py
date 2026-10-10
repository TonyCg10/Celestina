#!/usr/bin/env python3
"""The activation guard: one owner requests an application's bus name.

Every first-party application keeps to one window through the suite's shared
claim-first hand-off, `celestina-rs/crates/celestina-core/src/activation.rs`
(CONV-1-A), which also owns the suite's `org.celestina.<App>` names. In every
registered application's Rust sources this guard refuses:

  claim    a bus-name request: `request_name(...)`,
           `request_name_with_flags(...)`, or `.name(...)` on a zbus
           connection builder (a call may span lines)
  literal  a string literal naming a suite bus name or interface
           (`"org.celestina.` followed by a capital letter); the names live in
           `celestina_core::activation`

Each file is scanned as one text, comments removed. The allowlists below are
keyed by (file, argument) and (file, literal), each with its reason, so an
allowed file cannot claim a second name unnoticed.

    activation_contract.py LABEL=APPLICATION_ROOT ...
"""

from __future__ import annotations

import pathlib
import re
import sys

CLAIM = re.compile(r"\brequest_name(?:_with_flags)?\s*\(\s*([^,)]*?)\s*[,)]")
BUILDER = re.compile(r"connection\s*::\s*Builder\s*::")
BUILDER_NAME = re.compile(r"\.\s*name\s*\(\s*([^,)]*?)\s*[,)]")
BUILDER_VARIABLE_NAME = re.compile(r"\bbuilder\s*\.\s*name\s*\(\s*([^,)]*?)\s*[,)]")
LITERAL = re.compile(r'"(org\.celestina\.[A-Z][^"]*)"')
SKIPPED_DIRS = {"target", "build", ".git"}

# (file, first argument as written) -> why that file may claim that name.
ALLOWED_CLAIMS = {
    ("siderita/src/dbus.rs", '"org.freedesktop.FileManager1"'):
        "the freedesktop 'show in file manager' service, which Siderita serves "
        "for other programs",
    ("siderita/src/portal.rs", '"org.freedesktop.impl.portal.desktop.celestina"'):
        "the xdg-desktop-portal FileChooser backend the portal front-end "
        "activates Siderita for",
    ("fluorita/src/mpris.rs", "BUS_NAME"):
        "MPRIS2 (`org.mpris.MediaPlayer2.*`), a separate connection beside "
        "Fluorita's activation claim",
}

# (file, literal) -> why that file may spell a suite bus name.
ALLOWED_LITERALS = {
    ("siderita/src/devices.rs", "org.celestina.Magnetita"):
        "client of the Magnetita daemon's name, which magnetitad owns",
    ("siderita/src/devices.rs", "org.celestina.Devices1"):
        "client of the Magnetita daemon's interface",
    ("magnetita/src/devices.rs", "org.celestina.Magnetita"):
        "client of the Magnetita daemon's name, which magnetitad owns",
    ("magnetita/src/devices.rs", "org.celestina.Devices1"):
        "client of the Magnetita daemon's interface",
    ("magnetita/src/devices.rs", "org.celestina.Mirror1"):
        "the mirror interface Magnetita serves to its daemon, not a name",
    ("magnetita/src/main.rs", "org.celestina.Magnetita"):
        "Magnetita's freedesktop ID; Magnetita has not adopted the shared "
        "activation (not part of CONV-1-A)",
    ("selenita/src/activation.rs", "org.celestina.Selenita1"):
        "Selenita's own capture interface (`Capture`, `ToggleRecording`), "
        "served beside the shared one on the activation connection for the "
        "niri key bindings; an interface, not a name",
    ("grafita/src/syntax.rs", "org.celestina.Grafita.desktop"):
        "a desktop-entry file name in a syntax-detection test, not a bus name",
}


RAW_STRING = re.compile(r'b?r(#*)"')


def strip_comments(text: str) -> str:
    """`text` with Rust comments blanked, string and char literals kept.

    A small tokenizer: a `//` or `/*` inside `"file://"`, `"image/*"`, a raw
    string or a char literal is not a comment. Newlines inside comments are
    kept so line numbers stay true; block comments nest, as in Rust.
    """
    out: list[str] = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c == "/" and text.startswith("//", i):
            end = text.find("\n", i)
            i = n if end < 0 else end
            continue
        if c == "/" and text.startswith("/*", i):
            depth, i = 1, i + 2
            while i < n and depth:
                if text.startswith("/*", i):
                    depth, i = depth + 1, i + 2
                elif text.startswith("*/", i):
                    depth, i = depth - 1, i + 2
                else:
                    if text[i] == "\n":
                        out.append("\n")
                    i += 1
            continue
        raw = RAW_STRING.match(text, i) if c in "br" else None
        if raw and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
            closing = '"' + raw.group(1)
            end = text.find(closing, raw.end())
            end = n if end < 0 else end + len(closing)
            out.append(text[i:end])
            i = end
            continue
        if c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            out.append(text[i:j + 1])
            i = j + 1
            continue
        if c == "'":
            # A char literal ('x', '\n', '\u{..}'); otherwise a lifetime.
            match = re.match(r"'(?:\\(?:u\{[0-9a-fA-F]+\}|x[0-9a-fA-F]{2}|.)|[^\\'\n])'", text[i:])
            if match:
                out.append(match.group(0))
                i += len(match.group(0))
                continue
        out.append(c)
        i += 1
    return "".join(out)


def line_of(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def sources(root: pathlib.Path):
    for path in sorted(root.rglob("*.rs")):
        if SKIPPED_DIRS.intersection(path.relative_to(root).parts):
            continue
        yield path


def claims(text: str):
    """(offset, argument) of every bus-name request in `text`."""
    for match in CLAIM.finditer(text):
        yield match.start(), match.group(1)
    seen = set()
    for builder in BUILDER.finditer(text):
        end = text.find(";", builder.end())
        span_end = len(text) if end < 0 else end
        for match in BUILDER_NAME.finditer(text, builder.end(), span_end):
            seen.add(match.start())
            yield match.start(), match.group(1)
    for match in BUILDER_VARIABLE_NAME.finditer(text):
        dot = text.index(".", match.start())
        if dot not in seen and match.start() not in seen:
            yield match.start(), match.group(1)


def scan(root: pathlib.Path) -> list[str]:
    findings: list[str] = []
    for path in sources(root):
        key = path.as_posix()
        text = strip_comments(path.read_text(encoding="utf-8", errors="replace"))
        reported = set()
        for offset, argument in claims(text):
            line = line_of(text, offset)
            if (key, argument) in ALLOWED_CLAIMS or (line, argument) in reported:
                continue
            reported.add((line, argument))
            findings.append(
                f"{key}:{line}: claim: requests the bus name {argument} itself; "
                "claim through celestina_core::activation instead")
        for match in LITERAL.finditer(text):
            literal = match.group(1)
            if (key, literal) in ALLOWED_LITERALS:
                continue
            findings.append(
                f"{key}:{line_of(text, match.start())}: literal: spells "
                f"\"{literal}\"; use the name from celestina_core::activation")
    return findings


def main(argv: list[str]) -> int:
    if not argv:
        print("usage: activation_contract.py LABEL=APPLICATION_ROOT ...", file=sys.stderr)
        return 2
    findings: list[str] = []
    for argument in argv:
        label, separator, root = argument.partition("=")
        if not separator or not root:
            print(f"activation: ERROR: bad argument {argument!r}", file=sys.stderr)
            return 2
        directory = pathlib.Path(root)
        if not directory.is_dir():
            print(f"activation: ERROR: {label}: {root} is not a directory", file=sys.stderr)
            return 2
        findings.extend(scan(directory))
    for finding in findings:
        print(f"activation: ERROR: {finding}", file=sys.stderr)
    if not findings:
        print("Activation contract: OK")
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
