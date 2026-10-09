#!/usr/bin/env python3
"""Writes Calcita's test fixtures, byte-for-byte the same on every run (no
dates, no IDs):

- tests/fixtures/three-pages.pdf: three empty A4 pages;
- tests/fixtures/outline.pdf: three A4 pages reading «Page one», «Page two»
  and «Page three» in Helvetica, an outline of two bookmarks (Introduction →
  page 1, Chapter two → page 2) and, on page 1, an internal link to page 3
  and an external link to https://example.org/guide;
- tests/fixtures/many-pages.pdf: 300 empty A4 pages, for the page view's
  cost with a long document.

The fixtures are committed; this script records how they were made.
"""

import pathlib
import sys

PAGES = 3
MANY_PAGES = 300


def build(pages: int = PAGES) -> bytes:
    kids = " ".join(f"{3 + index} 0 R" for index in range(pages))
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        f"<< /Type /Pages /Kids [{kids}] /Count {pages} >>".encode(),
    ]
    for _ in range(pages):
        objects.append(b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >>")
    return serialize(objects)


def stream(text: str) -> bytes:
    body = text.encode()
    return f"<< /Length {len(body)} >>\nstream\n".encode() + body + b"\nendstream"


def build_outline() -> bytes:
    """Objects: 1 catalog, 2 pages, 3 outline root, 4-6 the pages, 7-8 the
    bookmarks, 9 the font, 10-12 the page contents, 13-14 page 1's links."""
    resources = b"/Resources << /Font << /F1 9 0 R >> >>"
    page = b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] " + resources
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R /Outlines 3 0 R >>",
        b"<< /Type /Pages /Kids [4 0 R 5 0 R 6 0 R] /Count 3 >>",
        b"<< /Type /Outlines /First 7 0 R /Last 8 0 R /Count 2 >>",
        page + b" /Contents 10 0 R /Annots [13 0 R 14 0 R] >>",
        page + b" /Contents 11 0 R >>",
        page + b" /Contents 12 0 R >>",
        b"<< /Title (Introduction) /Parent 3 0 R /Next 8 0 R /Dest [4 0 R /XYZ 0 842 0] >>",
        b"<< /Title (Chapter two) /Parent 3 0 R /Prev 7 0 R /Dest [5 0 R /XYZ 0 842 0] >>",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        stream(
            "BT /F1 24 Tf 72 760 Td (Page one) Tj ET\n"
            "BT /F1 14 Tf 72 700 Td (Go to the end) Tj ET\n"
            "BT /F1 14 Tf 72 660 Td (Visit the guide) Tj ET"
        ),
        stream("BT /F1 24 Tf 72 760 Td (Page two) Tj ET"),
        stream("BT /F1 24 Tf 72 760 Td (Page three) Tj ET"),
        b"<< /Type /Annot /Subtype /Link /Rect [70 690 260 720] /Border [0 0 0] "
        b"/Dest [6 0 R /XYZ 0 842 0] >>",
        b"<< /Type /Annot /Subtype /Link /Rect [70 650 260 680] /Border [0 0 0] "
        b"/A << /S /URI /URI (https://example.org/guide) >> >>",
    ]
    return serialize(objects)


def serialize(objects: list) -> bytes:
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
    fixtures = pathlib.Path(__file__).resolve().parent.parent / "tests/fixtures"
    (fixtures / "three-pages.pdf").write_bytes(build())
    (fixtures / "outline.pdf").write_bytes(build_outline())
    (fixtures / "many-pages.pdf").write_bytes(build(MANY_PAGES))
    return 0


if __name__ == "__main__":
    sys.exit(main())
