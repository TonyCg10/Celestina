#!/usr/bin/env python3
"""Writes tests/fixtures/three-pages.pdf: a minimal valid PDF with three empty
A4 pages, byte-for-byte the same on every run (no dates, no IDs).

The fixture is committed; this script records how it was made.
"""

import pathlib
import sys

PAGES = 3


def build() -> bytes:
    kids = " ".join(f"{3 + index} 0 R" for index in range(PAGES))
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        f"<< /Type /Pages /Kids [{kids}] /Count {PAGES} >>".encode(),
    ]
    for _ in range(PAGES):
        objects.append(b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>")

    out = bytearray(b"%PDF-1.4\n")
    offsets = []
    for number, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += f"{number} 0 obj\n".encode() + body + b"\nendobj\n"
    xref = len(out)
    out += f"xref\n0 {len(objects) + 1}\n".encode()
    out += b"0000000000 65535 f \n"
    for offset in offsets:
        out += f"{offset:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\n".encode()
    out += f"startxref\n{xref}\n%%EOF\n".encode()
    return bytes(out)


def main() -> int:
    target = pathlib.Path(__file__).resolve().parent.parent / "tests/fixtures/three-pages.pdf"
    target.write_bytes(build())
    return 0


if __name__ == "__main__":
    sys.exit(main())
