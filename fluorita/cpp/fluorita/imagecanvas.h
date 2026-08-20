// Drawing an edit onto a picture, and encoding the result.
//
// Hand-written C++ because cxx-qt-lib 0.9 exposes `QImage`, `QPainter` and
// `QPen` but not the three things this needs: `QBrush` (so a filled shape
// cannot be painted from Rust), `QTransform` (so a quarter turn cannot be
// applied) and `QImageWriter`/`QBuffer` (so nothing can be encoded to bytes).
// Reaching for a second image library instead would put a whole decoder beside
// the one Qt already is.
//
// The API is a list of orders, not a format: `fluorita-engine` walks the
// composition and calls these, so the shape of an annotation is described in
// exactly one language. A text protocol parsed on this side would be a second
// owner of that shape, and a hostile-input surface for data we generated
// ourselves.
//
// Every coordinate is in pixels of the *current* canvas, which the
// transformation calls change. Callers apply the composition's transforms
// first and its objects afterwards, which is the order the domain guarantees.
#pragma once

#include <memory>

#include <QtGui/QImage>

#include <cxx-qt-lib/qbytearray.h>
#include <cxx-qt-lib/qstring.h>
#include <rust/cxx.h>

class FluoritaCanvas
{
public:
    explicit FluoritaCanvas(QImage image);

    ::std::int32_t width() const;
    ::std::int32_t height() const;
    bool isEmpty() const;

    // Canvas transformations, in the order the document applied them.
    void rotate(::std::int32_t quarters);
    void flip(bool horizontal, bool vertical);
    void crop(float x, float y, float width, float height);
    void resize(::std::int32_t width, ::std::int32_t height);

    // Annotations. `rgba` is packed 0xRRGGBBAA, which is how the Rust side
    // already carries an `Ink`.
    void drawStroke(::rust::Slice<const float> points, float width, ::std::uint32_t rgba);
    void drawLine(float x1,
                  float y1,
                  float x2,
                  float y2,
                  float width,
                  ::std::uint32_t rgba,
                  bool arrow);
    void drawShape(bool ellipse,
                   float x,
                   float y,
                   float width,
                   float height,
                   float stroke,
                   ::std::uint32_t rgba,
                   bool filled,
                   ::std::uint32_t fillRgba);
    void drawHighlight(float x, float y, float width, float height, ::std::uint32_t rgba);
    void redact(float x, float y, float width, float height, bool blur);
    void drawText(float x,
                  float y,
                  float width,
                  float height,
                  float size,
                  ::std::uint32_t rgba,
                  bool hasBackdrop,
                  ::std::uint32_t backdropRgba,
                  ::std::int32_t quarters,
                  const QString &text);

    QByteArray encode(const QString &format, ::std::int32_t quality) const;

private:
    QImage m_image;
};

// Opens the picture named by `key` — the percent-encoded path key of ADR 0008,
// opened by descriptor so a name that is not valid UTF-8 still names its own
// file. EXIF orientation is applied on load, so the canvas is the picture as a
// person sees it, which is the frame every stored coordinate is in.
//
// Returns a canvas whose `isEmpty()` is true when the file cannot be read.
::std::unique_ptr<FluoritaCanvas> fluorita_open_canvas(const QString &key);
