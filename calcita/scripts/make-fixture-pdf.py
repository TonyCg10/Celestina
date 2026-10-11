#!/usr/bin/env python3
"""Writes Calcita's test fixtures, byte-for-byte the same on every run (no
dates, no IDs):

- tests/fixtures/three-pages.pdf: three empty A4 pages;
- tests/fixtures/outline.pdf: three A4 pages reading «Page one», «Page two»
  and «Page three» in Helvetica, an outline of two bookmarks (Introduction →
  page 1, Chapter two → page 2) and, on page 1, an internal link to page 3
  and an external link to https://example.org/guide;
- tests/fixtures/many-pages.pdf: 300 empty A4 pages, for the page view's
  cost with a long document;
- tests/fixtures/annotated.pdf: one A4 page reading «Highlighted words» and
  «Visit the guide», with an annotation of each kind Calcita edits, all
  written by «Another program» with their appearance streams (a FreeText
  «Hello Calcita» in 16 pt dark blue Helvetica, a yellow Highlight over the
  first line blended with Multiply, a red Ink zigzag) and a Link to
  https://example.org/guide over the second line;
- tests/fixtures/rotated.pdf: four pages with /Rotate 0, 90, 180 and 270,
  each with the box [50 100 645 942] (a /MediaBox on the first three, a
  /CropBox inside a larger /MediaBox on the fourth) and a yellow Highlight
  at [120 852 230 886] in user space.

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


def form_stream(bbox: str, resources: str, text: str) -> bytes:
    """An appearance stream: a form XObject over `bbox` drawing `text`."""
    body = text.encode()
    return (
        f"<< /Type /XObject /Subtype /Form /BBox [{bbox}] /Resources << {resources} >> "
        f"/Length {len(body)} >>\nstream\n".encode()
        + body
        + b"\nendstream"
    )


def build_annotated() -> bytes:
    """Objects: 1 catalog, 2 pages, 3 the page, 4 the font, 5 the page
    content, 6-7 the FreeText and its appearance, 8-9 the Highlight and its
    appearance, 10-11 the Ink and its appearance, 12 the Link."""
    author = b"/T (Another program) /F 4 /P 3 0 R"
    zigzag = "310 500 360 620 410 500 460 620 510 500"
    path = zigzag.split()
    ink_path = f"{path[0]} {path[1]} m " + " ".join(
        f"{path[i]} {path[i + 1]} l" for i in range(2, len(path), 2)
    )
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] "
        b"/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R "
        b"/Annots [6 0 R 8 0 R 10 0 R 12 0 R] >>",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        stream(
            "BT /F1 24 Tf 72 760 Td (Highlighted words) Tj ET\n"
            "BT /F1 14 Tf 72 610 Td (Visit the guide) Tj ET"
        ),
        b"<< /Type /Annot /Subtype /FreeText /Rect [300 700 520 760] "
        b"/Contents (Hello Calcita) /DA (/Helv 16 Tf 0 0 0.6 rg) /NM (fixture-text) "
        + author
        + b" /AP << /N 7 0 R >> >>",
        form_stream(
            "0 0 220 60",
            "/Font << /Helv 4 0 R >>",
            "BT /Helv 16 Tf 0 0 0.6 rg 8 22 Td (Hello Calcita) Tj ET",
        ),
        b"<< /Type /Annot /Subtype /Highlight /Rect [70 752 290 786] "
        b"/QuadPoints [70 786 290 786 70 752 290 752] /C [1 1 0] /NM (fixture-highlight) "
        + author
        + b" /AP << /N 9 0 R >> >>",
        form_stream(
            "0 0 220 34",
            "/ExtGState << /GS0 << /Type /ExtGState /BM /Multiply >> >>",
            "/GS0 gs 1 1 0 rg 0 0 220 34 re f",
        ),
        b"<< /Type /Annot /Subtype /Ink /Rect [300 480 520 640] "
        b"/InkList [[" + zigzag.encode() + b"]] /C [1 0 0] /BS << /W 4 >> /NM (fixture-ink) "
        + author
        + b" /AP << /N 11 0 R >> >>",
        form_stream("300 480 520 640", "", f"1 0 0 RG 4 w 1 J 1 j {ink_path} S"),
        b"<< /Type /Annot /Subtype /Link /Rect [70 600 260 630] /Border [0 0 0] "
        b"/A << /S /URI /URI (https://example.org/guide) >> >>",
    ]
    return serialize(objects)


def build_rotated() -> bytes:
    """Objects: 1 catalog, 2 pages, 3-6 the pages, 7 the font, 8-11 their
    contents, 12-15 their highlights, 16 the highlights' appearance."""
    box = "50 100 645 942"
    boxes = [f"/MediaBox [{box}]"] * 3 + [f"/MediaBox [0 0 800 1000] /CropBox [{box}]"]
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R 4 0 R 5 0 R 6 0 R] /Count 4 >>",
    ]
    for index, rotate in enumerate((0, 90, 180, 270)):
        objects.append(
            f"<< /Type /Page /Parent 2 0 R {boxes[index]} /Rotate {rotate} "
            f"/Resources << /Font << /F1 7 0 R >> >> /Contents {8 + index} 0 R "
            f"/Annots [{12 + index} 0 R] >>".encode()
        )
    objects.append(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
    for rotate in (0, 90, 180, 270):
        objects.append(stream(f"BT /F1 24 Tf 122 860 Td (Rotate {rotate}) Tj ET"))
    for index in range(4):
        objects.append(
            f"<< /Type /Annot /Subtype /Highlight /Rect [120 852 230 886] "
            f"/QuadPoints [120 886 230 886 120 852 230 852] /C [1 1 0] /F 4 "
            f"/P {3 + index} 0 R /AP << /N 16 0 R >> >>".encode()
        )
    objects.append(
        form_stream(
            "120 852 230 886",
            "/ExtGState << /GS0 << /Type /ExtGState /BM /Multiply >> >>",
            "/GS0 gs 1 1 0 rg 120 852 110 34 re f",
        )
    )
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
    (fixtures / "annotated.pdf").write_bytes(build_annotated())
    (fixtures / "rotated.pdf").write_bytes(build_rotated())
    return 0


if __name__ == "__main__":
    sys.exit(main())
