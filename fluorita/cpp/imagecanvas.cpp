#include "fluorita/imagecanvas.h"

#include <fcntl.h>
#include <unistd.h>

#include <QtCore/QBuffer>
#include <QtCore/QByteArray>
#include <QtCore/QFile>
#include <QtCore/QRectF>
#include <QtGui/QBrush>
#include <QtGui/QColor>
#include <QtGui/QFont>
#include <QtGui/QImageReader>
#include <QtGui/QImageWriter>
#include <QtGui/QPainter>
#include <QtGui/QPainterPath>
#include <QtGui/QPen>
#include <QtGui/QPolygonF>
#include <QtGui/QTransform>

#include <cmath>

namespace {

QColor colourFrom(::std::uint32_t rgba)
{
    return QColor(static_cast<int>((rgba >> 24) & 0xFF),
                  static_cast<int>((rgba >> 16) & 0xFF),
                  static_cast<int>((rgba >> 8) & 0xFF),
                  static_cast<int>(rgba & 0xFF));
}

QRectF rectangleFrom(float x, float y, float width, float height)
{
    return QRectF(static_cast<qreal>(x),
                  static_cast<qreal>(y),
                  static_cast<qreal>(width),
                  static_cast<qreal>(height));
}

// A pen that survives being scaled: a width of zero is Qt's "cosmetic" pen and
// would draw a hairline whatever the picture's size, which on a 4000-pixel
// photograph is invisible.
QPen penFor(::std::uint32_t rgba, float width)
{
    QPen pen(colourFrom(rgba));
    pen.setWidthF(std::max(1.0, static_cast<qreal>(width)));
    pen.setCapStyle(Qt::RoundCap);
    pen.setJoinStyle(Qt::RoundJoin);
    return pen;
}

// Blurring by shrinking and growing again. Qt has no blur outside the graphics
// effects framework, which needs a scene; a smooth downscale followed by a
// smooth upscale is what that framework does to approximate one anyway, and it
// is genuinely irreversible, which is the property a redaction needs.
QImage blurred(const QImage &area)
{
    const int width = std::max(1, area.width() / 24);
    const int height = std::max(1, area.height() / 24);
    return area.scaled(width, height, Qt::IgnoreAspectRatio, Qt::SmoothTransformation)
        .scaled(area.size(), Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
}

// Pixelating by shrinking smoothly and growing with nearest-neighbour, which is
// what produces visible blocks rather than a soft smear.
QImage pixelated(const QImage &area)
{
    const int width = std::max(1, area.width() / 16);
    const int height = std::max(1, area.height() / 16);
    return area.scaled(width, height, Qt::IgnoreAspectRatio, Qt::SmoothTransformation)
        .scaled(area.size(), Qt::IgnoreAspectRatio, Qt::FastTransformation);
}

// A read-only descriptor closed on scope exit unless a QFile adopted it. The
// same seam `imageprobe.cpp` uses, for the same reason: `open` on raw bytes is
// the only call that names a file exactly, whatever its name spells.
class ReadDescriptor
{
public:
    explicit ReadDescriptor(const QByteArray &pathBytes)
        : m_descriptor(::open(pathBytes.constData(), O_RDONLY | O_CLOEXEC))
    {
    }

    ReadDescriptor(const ReadDescriptor &) = delete;
    ReadDescriptor &operator=(const ReadDescriptor &) = delete;
    ReadDescriptor(ReadDescriptor &&) = delete;
    ReadDescriptor &operator=(ReadDescriptor &&) = delete;

    ~ReadDescriptor()
    {
        if (m_descriptor >= 0) {
            ::close(m_descriptor);
        }
    }

    bool adoptInto(QFile &file)
    {
        if (m_descriptor < 0 ||
            !file.open(m_descriptor, QIODevice::ReadOnly, QFileDevice::AutoCloseHandle)) {
            return false;
        }
        m_descriptor = -1;
        return true;
    }

private:
    int m_descriptor;
};

} // namespace

FluoritaCanvas::FluoritaCanvas(QImage image)
    : m_image(std::move(image))
{
    // Every drawing path below composites with alpha, and a paletted or
    // grayscale source cannot hold the result.
    if (!m_image.isNull() && m_image.format() != QImage::Format_ARGB32_Premultiplied) {
        m_image = m_image.convertToFormat(QImage::Format_ARGB32_Premultiplied);
    }
}

::std::int32_t FluoritaCanvas::width() const
{
    return m_image.width();
}

::std::int32_t FluoritaCanvas::height() const
{
    return m_image.height();
}

bool FluoritaCanvas::isEmpty() const
{
    return m_image.isNull();
}

void FluoritaCanvas::rotate(::std::int32_t quarters)
{
    const int turns = ((quarters % 4) + 4) % 4;
    if (turns == 0 || m_image.isNull()) {
        return;
    }
    QTransform transform;
    transform.rotate(90.0 * turns);
    m_image = m_image.transformed(transform, Qt::SmoothTransformation);
}

void FluoritaCanvas::flip(bool horizontal, bool vertical)
{
    if (m_image.isNull() || (!horizontal && !vertical)) {
        return;
    }
    Qt::Orientations axes;
    if (horizontal) {
        axes |= Qt::Horizontal;
    }
    if (vertical) {
        axes |= Qt::Vertical;
    }
    m_image = m_image.flipped(axes);
}

void FluoritaCanvas::crop(float x, float y, float width, float height)
{
    if (m_image.isNull()) {
        return;
    }
    const QRect area = rectangleFrom(x, y, width, height).toRect().intersected(m_image.rect());
    if (area.isEmpty()) {
        return;
    }
    m_image = m_image.copy(area);
}

void FluoritaCanvas::resize(::std::int32_t width, ::std::int32_t height)
{
    if (m_image.isNull() || width <= 0 || height <= 0) {
        return;
    }
    m_image = m_image.scaled(width, height, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
}

void FluoritaCanvas::drawStroke(::rust::Slice<const float> points,
                                float width,
                                ::std::uint32_t rgba)
{
    if (m_image.isNull() || points.size() < 4) {
        return;
    }
    QPainterPath path;
    path.moveTo(static_cast<qreal>(points[0]), static_cast<qreal>(points[1]));
    for (::std::size_t index = 2; index + 1 < points.size(); index += 2) {
        path.lineTo(static_cast<qreal>(points[index]), static_cast<qreal>(points[index + 1]));
    }

    QPainter painter(&m_image);
    painter.setRenderHint(QPainter::Antialiasing, true);
    painter.strokePath(path, penFor(rgba, width));
}

void FluoritaCanvas::drawLine(float x1,
                              float y1,
                              float x2,
                              float y2,
                              float width,
                              ::std::uint32_t rgba,
                              bool arrow)
{
    if (m_image.isNull()) {
        return;
    }
    QPainter painter(&m_image);
    painter.setRenderHint(QPainter::Antialiasing, true);
    const QPen pen = penFor(rgba, width);
    painter.setPen(pen);
    painter.drawLine(QPointF(x1, y1), QPointF(x2, y2));

    if (!arrow) {
        return;
    }
    // The head is sized from the line's own width, so a thick arrow does not
    // end in a pinhead.
    const qreal head = std::max<qreal>(8.0, pen.widthF() * 4.0);
    const qreal angle = std::atan2(static_cast<qreal>(y2 - y1), static_cast<qreal>(x2 - x1));
    const qreal spread = 0.5;
    QPolygonF wings;
    wings << QPointF(x2, y2)
          << QPointF(x2 - head * std::cos(angle - spread), y2 - head * std::sin(angle - spread))
          << QPointF(x2 - head * std::cos(angle + spread), y2 - head * std::sin(angle + spread));
    painter.setPen(Qt::NoPen);
    painter.setBrush(QBrush(colourFrom(rgba)));
    painter.drawPolygon(wings);
}

void FluoritaCanvas::drawShape(bool ellipse,
                               float x,
                               float y,
                               float width,
                               float height,
                               float stroke,
                               ::std::uint32_t rgba,
                               bool filled,
                               ::std::uint32_t fillRgba)
{
    if (m_image.isNull()) {
        return;
    }
    const QRectF area = rectangleFrom(x, y, width, height);
    QPainter painter(&m_image);
    painter.setRenderHint(QPainter::Antialiasing, true);
    painter.setPen(penFor(rgba, stroke));
    painter.setBrush(filled ? QBrush(colourFrom(fillRgba)) : QBrush(Qt::NoBrush));
    if (ellipse) {
        painter.drawEllipse(area);
    } else {
        painter.drawRect(area);
    }
}

void FluoritaCanvas::drawHighlight(float x,
                                   float y,
                                   float width,
                                   float height,
                                   ::std::uint32_t rgba)
{
    if (m_image.isNull()) {
        return;
    }
    QPainter painter(&m_image);
    // Multiply is what a real highlighter does to paper: it darkens towards its
    // own colour instead of covering what is under it.
    painter.setCompositionMode(QPainter::CompositionMode_Multiply);
    painter.fillRect(rectangleFrom(x, y, width, height), colourFrom(rgba));
}

void FluoritaCanvas::redact(float x, float y, float width, float height, bool blur)
{
    if (m_image.isNull()) {
        return;
    }
    const QRect area = rectangleFrom(x, y, width, height).toRect().intersected(m_image.rect());
    if (area.isEmpty()) {
        return;
    }
    const QImage patch = m_image.copy(area);
    const QImage hidden = blur ? blurred(patch) : pixelated(patch);

    QPainter painter(&m_image);
    painter.drawImage(area, hidden);
}

void FluoritaCanvas::drawText(float x,
                              float y,
                              float width,
                              float height,
                              float size,
                              ::std::uint32_t rgba,
                              bool hasBackdrop,
                              ::std::uint32_t backdropRgba,
                              ::std::int32_t quarters,
                              const QString &text)
{
    if (m_image.isNull() || text.isEmpty()) {
        return;
    }
    QPainter painter(&m_image);
    painter.setRenderHint(QPainter::Antialiasing, true);
    painter.setRenderHint(QPainter::TextAntialiasing, true);

    // The box is stored axis-aligned in canvas coordinates; the turns it
    // accumulated from canvas rotations are applied around its own centre, so
    // the word reads the way it did when it was written.
    const QRectF box = rectangleFrom(x, y, width, height);
    const int turns = ((quarters % 4) + 4) % 4;
    painter.translate(box.center());
    painter.rotate(90.0 * turns);
    const QRectF local = (turns % 2 == 0)
        ? QRectF(-box.width() / 2.0, -box.height() / 2.0, box.width(), box.height())
        : QRectF(-box.height() / 2.0, -box.width() / 2.0, box.height(), box.width());

    if (hasBackdrop) {
        painter.fillRect(local, colourFrom(backdropRgba));
    }

    QFont font = painter.font();
    font.setPixelSize(std::max(1, static_cast<int>(std::lround(size))));
    painter.setFont(font);
    painter.setPen(QPen(colourFrom(rgba)));
    painter.drawText(local, Qt::AlignLeft | Qt::AlignTop | Qt::TextWordWrap, text);
}

QByteArray FluoritaCanvas::encode(const QString &format, ::std::int32_t quality) const
{
    if (m_image.isNull()) {
        return QByteArray();
    }
    QByteArray bytes;
    QBuffer buffer(&bytes);
    if (!buffer.open(QIODevice::WriteOnly)) {
        return QByteArray();
    }
    QImageWriter writer(&buffer, format.toLatin1());
    if (quality >= 0) {
        writer.setQuality(quality);
    }
    QImage output = m_image;
    // A format with no alpha channel would otherwise composite the picture
    // against black wherever an annotation left it translucent.
    if (!writer.supportsOption(QImageIOHandler::ImageFormat) || format.toLower() == QStringLiteral("jpg")
        || format.toLower() == QStringLiteral("jpeg")) {
        output = output.convertToFormat(QImage::Format_RGB32);
    }
    if (!writer.write(output)) {
        return QByteArray();
    }
    buffer.close();
    return bytes;
}

::std::unique_ptr<FluoritaCanvas> fluorita_open_canvas(const QString &key)
{
    const QByteArray pathBytes = QByteArray::fromPercentEncoding(key.toLatin1());
    if (pathBytes.isEmpty() || !pathBytes.startsWith('/')) {
        return ::std::make_unique<FluoritaCanvas>(QImage());
    }

    ReadDescriptor descriptor(pathBytes);
    QFile file;
    if (!descriptor.adoptInto(file)) {
        return ::std::make_unique<FluoritaCanvas>(QImage());
    }

    QImageReader reader(&file);
    // The canvas is the picture as a person sees it, which is what every
    // stored coordinate refers to. `imageprobe.cpp` measures with the same
    // setting, so the budget and the canvas agree.
    reader.setAutoTransform(true);
    QImage image = reader.read();
    return ::std::make_unique<FluoritaCanvas>(std::move(image));
}
