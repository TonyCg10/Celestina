#include "siderita/thumbnailprovider.h"

#include "fluorita/thumbnailprovider.h"

// The Rust side of the same crate: the parsers that read a picture out of a
// program, a music tag or a package. Declared through the generated header so
// the signature stays the bridge's, not a hand-written copy of it.
#include "siderita/src/embedded.cxx.h"

#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

#include <QtCore/QByteArray>
#include <QtCore/QFile>
#include <QtGui/QImage>
#include <QtGui/QImageReader>
#include <QtQml/QQmlApplicationEngine>

namespace {

using fluorita::kThumbnailEdge;

// The icon file at `pathBytes` — one a launcher names, found in an icon theme —
// decoded at thumbnail size. A scalable icon is rendered at that size rather
// than at its nominal one, so a grid cell gets a crisp picture instead of a
// 48-pixel stamp scaled up; a raster one is only ever scaled down.
//
// Opened by descriptor on the raw bytes, because a QString cannot spell a name
// that is not valid UTF-8; checked with `stat` first, because `open` on a FIFO
// blocks.
QImage loadIconFile(const QByteArray &pathBytes)
{
    struct stat icon = {};
    if (!pathBytes.startsWith('/') || ::stat(pathBytes.constData(), &icon) != 0 ||
        !S_ISREG(icon.st_mode)) {
        return QImage();
    }
    const int descriptor = ::open(pathBytes.constData(), O_RDONLY | O_CLOEXEC);
    if (descriptor < 0) {
        return QImage();
    }
    QFile file;
    if (!file.open(descriptor, QIODevice::ReadOnly, QFileDevice::AutoCloseHandle)) {
        ::close(descriptor);
        return QImage();
    }
    QImageReader reader(&file);
    const QSize natural = reader.size();
    const QByteArray format = reader.format();
    const bool scalable = format == "svg" || format == "svgz";
    if (natural.isValid() &&
        (scalable || natural.width() > kThumbnailEdge || natural.height() > kThumbnailEdge)) {
        reader.setScaledSize(natural.scaled(kThumbnailEdge, kThumbnailEdge, Qt::KeepAspectRatio));
    }
    return reader.read();
}

// The hook: the picture a file carries or names inside itself. The shared
// provider asks it on its pool, after the cache missed and before the image
// reader — none of these files *is* an image, so the reader would refuse them,
// and reading one costs a parse the cache spares.
fluorita::OwnPicture siderita_own_picture(const QByteArray &pathBytes)
{
    const ::rust::Slice<const ::std::uint8_t> raw(
        reinterpret_cast<const ::std::uint8_t *>(pathBytes.constData()),
        static_cast<::std::size_t>(pathBytes.size()));

    // A launcher does not hold its picture, it names one: the icon file an
    // installed theme provides. Resolving that name reads the launcher and
    // searches every theme directory, which is why it happens here, on the
    // pool, and not in the delegate's binding on the Qt thread.
    //
    // Not cacheable: the picture belongs to the icon theme, not to the
    // launcher file, so a cached copy would outlive a theme change — and the
    // cache is shared with every other application, which draws launchers its
    // own way.
    const ::rust::Vec<::std::uint8_t> named = siderita_own_icon_path(raw);
    if (!named.empty()) {
        QImage icon = loadIconFile(QByteArray(reinterpret_cast<const char *>(named.data()),
                                              static_cast<qsizetype>(named.size())));
        if (!icon.isNull()) {
            return {icon, false};
        }
    }

    // A program's icon, an album cover, an app's launcher art: the file's own
    // bytes, so the entry is keyed on it and cached like any thumbnail. The
    // shared provider scales it to thumbnail size before caching.
    const ::rust::Vec<::std::uint8_t> carried = siderita_embedded_image(raw);
    if (!carried.empty()) {
        const QImage embedded = QImage::fromData(QByteArray(
            reinterpret_cast<const char *>(carried.data()), static_cast<qsizetype>(carried.size())));
        if (!embedded.isNull()) {
            return {embedded, true};
        }
    }
    return {};
}

} // namespace

void register_siderita_thumbnail_provider(QQmlApplicationEngine &engine)
{
    register_fluorita_thumbnail_provider_with(engine, &siderita_own_picture);
}
