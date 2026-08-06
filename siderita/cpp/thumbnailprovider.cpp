#include "siderita/thumbnailprovider.h"

#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

#include <QtCore/QByteArray>
#include <QtCore/QCryptographicHash>
#include <QtCore/QDateTime>
#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtCore/QRunnable>
#include <QtCore/QStandardPaths>
#include <QtCore/QThread>
#include <QtCore/QThreadPool>
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

// The extension of the last component of `pathBytes`, lowercased.
//
// Derived from the bytes rather than from `QFileInfo::suffix()` because the
// source path here is not a QString: a name that is not valid UTF-8 still
// carries a perfectly ordinary extension, and it decides whether this file can
// be decoded at all.
QByteArray suffixOf(const QByteArray &pathBytes)
{
    const qsizetype slash = pathBytes.lastIndexOf('/');
    const qsizetype dot = pathBytes.lastIndexOf('.');
    if (dot < 0 || dot <= slash) {
        return QByteArray();
    }
    return pathBytes.mid(dot + 1).toLower();
}

// Whether Siderita will *generate* a thumbnail for this file itself — only raster
// images, which Qt decodes with no extra dependency. Video / audio thumbnails
// (first frames, embedded covers) need a media stack, so those are only ever
// reused from the shared cache when something else — the system, or Celestina's
// media app — produced them; Siderita never decodes them here.
bool generatableImage(const QByteArray &suffix)
{
    static const QList<QByteArray> kImage = {
        QByteArrayLiteral("png"),  QByteArrayLiteral("jpg"),  QByteArrayLiteral("jpeg"),
        QByteArrayLiteral("gif"),  QByteArrayLiteral("webp"), QByteArrayLiteral("bmp"),
        QByteArrayLiteral("ico"),  QByteArrayLiteral("tif"),  QByteArrayLiteral("tiff"),
        QByteArrayLiteral("avif"), QByteArrayLiteral("jxl"),  QByteArrayLiteral("heic"),
        QByteArrayLiteral("heif"),
    };
    return kImage.contains(suffix);
}

// A read-only descriptor opened from raw path bytes, closed when this goes out
// of scope unless a QFile has taken over the closing.
//
// The source file is addressed by descriptor because every Qt file API takes a
// QString, and a QString cannot hold a name that is not valid UTF-8. `open` on
// the bytes is the one call that can name such a file exactly.
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

    bool isValid() const { return m_descriptor >= 0; }

    // Hands the descriptor to `file`, which closes it from here on. Returns
    // false — and keeps ownership — when the QFile refuses it.
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

// The modification time of the regular file at `pathBytes`, or an invalid
// QDateTime when it is not a regular file this process may read.
//
// `stat` on the raw bytes rather than QFileInfo for the same reason the whole
// data path here is a QByteArray: a QString cannot spell a name that is not
// valid UTF-8, so QFileInfo would report a near miss as absent.
QDateTime sourceModified(const QByteArray &pathBytes)
{
    // A published key is always absolute; a relative one would stat against
    // this process's working directory and key the cache on a different URI.
    if (!pathBytes.startsWith('/')) {
        return QDateTime();
    }
    struct stat source = {};
    if (::stat(pathBytes.constData(), &source) != 0 || !S_ISREG(source.st_mode)) {
        return QDateTime();
    }
    return QDateTime::fromSecsSinceEpoch(source.st_mtime);
}

// The image at `pathBytes`, opened by descriptor and ready to decode, or a
// closed file when it is not a source this process generates from.
bool openGeneratableSource(const QByteArray &pathBytes, QFile &file, ReadDescriptor &descriptor)
{
    return sourceModified(pathBytes).isValid() && generatableImage(suffixOf(pathBytes)) &&
           descriptor.adoptInto(file);
}

// Loads a thumbnail for the file named by `pathBytes`: a valid cached one from
// the shared cache, else a freshly generated + cached one. Returns a null image
// for anything that is not a loadable image (the delegate then keeps its generic
// glyph). Runs off-thread.
QImage loadThumbnail(const QByteArray &pathBytes)
{
    const QDateTime sourceMtime = sourceModified(pathBytes);
    if (!sourceMtime.isValid()) {
        return QImage();
    }

    // The spec keys the cache on the canonical file:// URI; hashing the same URI
    // other managers do lets us reuse (and contribute to) their cache.
    const QByteArray uri = siderita_thumbnail_cache_uri(pathBytes);
    const QString digest =
        QString::fromLatin1(QCryptographicHash::hash(uri, QCryptographicHash::Md5).toHex());
    const QString largeDir = cacheRoot() + QStringLiteral("/large");
    const QString cachePath = largeDir + QLatin1Char('/') + digest + QStringLiteral(".png");

    // Reuse a cached thumbnail while it is at least as new as the file it depicts
    // — a thumbnail is always written after its source, so an edit (which bumps
    // the source mtime past the cache) is what forces a regenerate. This keys off
    // the filesystem, not the PNG's embedded `Thumb::MTime` (Qt mangles that key
    // on write), and so also honours thumbnails other managers produced. The
    // cache file itself is a hexadecimal digest, so it is addressable as a
    // QString even when its source is not.
    {
        const QFileInfo cacheInfo(cachePath);
        if (cacheInfo.exists() && cacheInfo.lastModified() >= sourceMtime) {
            const QImage cached(cachePath);
            if (!cached.isNull()) {
                return cached;
            }
        }
    }

    // Past here we would generate — but only for images. A video / audio file
    // with no cached thumbnail keeps its themed glyph until a media producer
    // fills the cache.
    //
    // QImageReader decodes at a reduced size where the format allows (cheap for
    // JPEG) and honours EXIF orientation. It reads a descriptor opened on the
    // raw bytes, so it never has to spell the source path.
    ReadDescriptor descriptor(pathBytes);
    QFile file;
    if (!openGeneratableSource(pathBytes, file, descriptor)) {
        return QImage();
    }
    QImageReader reader(&file);
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
    writer.setText(QStringLiteral("Thumb::URI"), QString::fromLatin1(uri));
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
    explicit ThumbnailResponse(const QByteArray &pathBytes)
        : m_pathBytes(pathBytes)
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
        m_image = loadThumbnail(m_pathBytes);
        Q_EMIT finished();
    }

private:
    QByteArray m_pathBytes;
    QImage m_image;
};

class ThumbnailProvider : public QQuickAsyncImageProvider
{
public:
    QQuickImageResponse *requestImageResponse(const QString &id, const QSize &) override
    {
        // The id is the entry's path key (ADR 0008), handed over verbatim: the
        // delegate must not re-encode it, or this would decode one layer and
        // look for a file literally named "%FF".
        //
        // The decoded path travels as QByteArray, not QString, for the whole
        // data path below. A key is ASCII by construction, but what it decodes
        // to is a raw byte string that a QString cannot hold: the very names
        // this seam exists for would lose a byte here and address nothing.
        return new ThumbnailResponse(QByteArray::fromPercentEncoding(id.toLatin1()));
    }
};

} // namespace

QByteArray siderita_thumbnail_cache_uri(const QByteArray &pathBytes)
{
    return QByteArrayLiteral("file://") + pathBytes.toPercentEncoding("!$&'()*+,;=:@/");
}

QSize siderita_thumbnail_source_size(const QByteArray &pathBytes)
{
    ReadDescriptor descriptor(pathBytes);
    QFile file;
    if (!openGeneratableSource(pathBytes, file, descriptor)) {
        return QSize();
    }
    return QImageReader(&file).size();
}

void register_siderita_thumbnail_provider(QQmlApplicationEngine &engine)
{
    // The engine takes ownership of the provider.
    engine.addImageProvider(QStringLiteral("thumb"), new ThumbnailProvider());
}
