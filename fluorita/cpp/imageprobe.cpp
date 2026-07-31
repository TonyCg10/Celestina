#include "fluorita/imageprobe.h"

#include <QtGui/QImageReader>

QSize fluorita_probe_image(const QString &path)
{
    QImageReader reader(path);
    // EXIF orientation swaps width and height for a quarter turn, and the
    // budget must be judged on what would actually be allocated.
    reader.setAutoTransform(true);
    if (!reader.canRead()) {
        return QSize();
    }
    const QSize size = reader.size();
    // Some formats do not answer before decoding; an unknown size is reported
    // as empty so the caller refuses rather than guesses a budget.
    return size.isValid() ? size : QSize();
}
