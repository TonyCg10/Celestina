#include "siderita/thumbnailprovider.h"

#include <QtCore/QCryptographicHash>
#include <QtCore/QDateTime>
#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtCore/QRunnable>
#include <QtCore/QStandardPaths>
#include <QtCore/QThreadPool>
#include <QtCore/QUrl>
#include <QtGui/QImage>
#include <QtGui/QImageReader>
#include <QtGui/QImageWriter>
#include <QtQml/QQmlApplicationEngine>
#include <QtQuick/QQuickAsyncImageProvider>
#include <QtQuick/QQuickImageResponse>
#include <QtQuick/QQuickTextureFactory>

namespace {

// The freedesktop shared thumbnail cache root ($XDG_CACHE_HOME/thumbnails).
QString cacheRoot()
{
    return QStandardPaths::writableLocation(QStandardPaths::GenericCacheLocation) +
           QStringLiteral("/thumbnails");
}

// The "large" (256 px) thumbnail size the spec defines; big enough for the grid
// at a comfortable zoom, and the size most desktops already cache.
constexpr int kThumbMax = 256;

// Loads a thumbnail for `path`: a valid cached one from the shared cache, else a
// freshly generated + cached one. Returns a null image for anything that is not
// a loadable image (the delegate then keeps its generic glyph). Runs off-thread.
QImage loadThumbnail(const QString &path)
{
    const QFileInfo info(path);
    if (!info.exists() || info.isDir()) {
        return QImage();
    }

    const QString absolute = info.absoluteFilePath();
    // The spec keys the cache on the canonical file:// URI; hashing the same URI
    // other managers do lets us reuse (and contribute to) their cache.
    const QByteArray uri = QUrl::fromLocalFile(absolute).toEncoded();
    const QString digest =
        QString::fromLatin1(QCryptographicHash::hash(uri, QCryptographicHash::Md5).toHex());
    const QString largeDir = cacheRoot() + QStringLiteral("/large");
    const QString cachePath = largeDir + QLatin1Char('/') + digest + QStringLiteral(".png");
    const QDateTime sourceMtime = info.lastModified();

    // Reuse a cached thumbnail while it is at least as new as the file it depicts
    // — a thumbnail is always written after its source, so an edit (which bumps
    // the source mtime past the cache) is what forces a regenerate. This keys off
    // the filesystem, not the PNG's embedded `Thumb::MTime` (Qt mangles that key
    // on write), and so also honours thumbnails other managers produced.
    {
        const QFileInfo cacheInfo(cachePath);
        if (cacheInfo.exists() && cacheInfo.lastModified() >= sourceMtime) {
            const QImage cached(cachePath);
            if (!cached.isNull()) {
                return cached;
            }
        }
    }

    // Generate. QImageReader decodes at a reduced size where the format allows
    // (cheap for JPEG) and honours EXIF orientation.
    QImageReader reader(absolute);
    reader.setAutoTransform(true);
    const QSize original = reader.size();
    if (original.isValid() && (original.width() > kThumbMax || original.height() > kThumbMax)) {
        reader.setScaledSize(original.scaled(kThumbMax, kThumbMax, Qt::KeepAspectRatio));
    }
    QImage image = reader.read();
    if (image.isNull()) {
        return QImage(); // not a decodable image
    }
    if (image.width() > kThumbMax || image.height() > kThumbMax) {
        image = image.scaled(kThumbMax, kThumbMax, Qt::KeepAspectRatio, Qt::SmoothTransformation);
    }

    // Cache it: write to a temp sibling then rename, so a reader never sees a
    // half-written PNG. Failure to cache is non-fatal — the thumbnail still
    // shows this session.
    QDir().mkpath(largeDir);
    const QString temp =
        cachePath + QStringLiteral(".tmp-") +
        QString::number(reinterpret_cast<quintptr>(QThread::currentThreadId()), 16);
    QImageWriter writer(temp, "png");
    writer.setText(QStringLiteral("Thumb::URI"), QString::fromUtf8(uri));
    writer.setText(QStringLiteral("Thumb::MTime"),
                   QString::number(sourceMtime.toSecsSinceEpoch()));
    if (writer.write(image)) {
        QFile::setPermissions(temp, QFile::ReadOwner | QFile::WriteOwner);
        QFile::remove(cachePath);
        if (!QFile::rename(temp, cachePath)) {
            QFile::remove(temp);
        }
    } else {
        QFile::remove(temp);
    }

    return image;
}

// One async request: does the work on the global thread pool and hands back the
// image when done.
class ThumbnailResponse : public QQuickImageResponse, public QRunnable
{
public:
    explicit ThumbnailResponse(const QString &path)
        : m_path(path)
    {
        setAutoDelete(false);
        QThreadPool::globalInstance()->start(this);
    }

    QQuickTextureFactory *textureFactory() const override
    {
        return QQuickTextureFactory::textureFactoryForImage(m_image);
    }

    void run() override
    {
        m_image = loadThumbnail(m_path);
        Q_EMIT finished();
    }

private:
    QString m_path;
    QImage m_image;
};

class ThumbnailProvider : public QQuickAsyncImageProvider
{
public:
    QQuickImageResponse *requestImageResponse(const QString &id, const QSize &) override
    {
        // The id is the file path (percent-encoded by the delegate); decode it.
        return new ThumbnailResponse(QUrl::fromPercentEncoding(id.toUtf8()));
    }
};

} // namespace

void register_siderita_thumbnail_provider(QQmlApplicationEngine &engine)
{
    // The engine takes ownership of the provider.
    engine.addImageProvider(QStringLiteral("thumb"), new ThumbnailProvider());
}
