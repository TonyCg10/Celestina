#include "fluorita/thumbnailprovider.h"

#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>

#include <utility>

#include <QtCore/QAtomicInt>
#include <QtCore/QByteArray>
#include <QtCore/QCryptographicHash>
#include <QtCore/QDateTime>
#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtCore/QList>
#include <QtCore/QRunnable>
#include <QtCore/QStandardPaths>
#include <QtCore/QThread>
#include <QtCore/QThreadPool>
#include <QtCore/QUrl>
#include <QtGui/QImage>
#include <QtGui/QImageReader>
#include <QtGui/QImageWriter>
#include <QtQml/QQmlApplicationEngine>
#include <QtQuick/QQuickAsyncImageProvider>
#include <QtQuick/QQuickImageResponse>
#include <QtQuick/QQuickTextureFactory>

// Everything here is internal to this translation unit. A host that links this
// crate may still carry its own provider (Siderita does, until it adopts this
// one), and an anonymous namespace is what keeps the two from ever meeting at
// link time.
namespace {

using fluorita::kThumbnailEdge;

// The largest source file handed to the reader. The same budget Fluorita's
// viewer applies before decoding a photograph (`fluorita/src/image.rs`): a
// hostile file must not make a grid read a disk into memory.
constexpr qint64 kMaxSourceBytes = qint64(256) * 1024 * 1024;

// The largest surface, in pixels, the header may claim. Also the viewer's
// budget; with the scaled read most formats never allocate it, but a format
// that cannot scale while decoding would.
constexpr qint64 kMaxSourcePixels = qint64(100) * 1000 * 1000;

// The largest cache entry read back. A 256-pixel PNG is a few hundred
// kilobytes at worst; anything past this is not a thumbnail.
constexpr qint64 kMaxCacheBytes = qint64(8) * 1024 * 1024;

// The freedesktop shared thumbnail cache root ($XDG_CACHE_HOME/thumbnails).
QString cacheRoot()
{
    return QStandardPaths::writableLocation(QStandardPaths::GenericCacheLocation) +
           QStringLiteral("/thumbnails");
}

// The extension of the last component of `pathBytes`, lowercased.
//
// Derived from the bytes rather than from `QFileInfo::suffix()`: a name that
// is not valid UTF-8 still carries an ordinary extension, and it decides
// whether this file is decoded at all.
QByteArray suffixOf(const QByteArray &pathBytes)
{
    const qsizetype slash = pathBytes.lastIndexOf('/');
    const qsizetype dot = pathBytes.lastIndexOf('.');
    if (dot < 0 || dot <= slash) {
        return QByteArray();
    }
    return pathBytes.mid(dot + 1).toLower();
}

// Whether a thumbnail is *generated* here for this file — only raster images,
// which Qt decodes with no extra dependency. Video and audio need a media
// stack, so their entries are only ever read from the cache the engine fills.
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
// Every Qt file API takes a QString, and a QString cannot hold a name that is
// not valid UTF-8; `open` on the bytes is the one call that names such a file
// exactly.
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

// What `stat` says about a source: its modification time and size, or an
// invalid time when it is not a regular file this process may read.
struct SourceStat
{
    QDateTime modified;
    qint64 bytes = 0;
};

// `stat` on the raw bytes rather than QFileInfo, for the same reason the whole
// data path here is a QByteArray. Checked before anything is opened, too:
// `open` on a FIFO blocks.
SourceStat statSource(const QByteArray &pathBytes)
{
    // A published key is always absolute; a relative one would stat against
    // this process's working directory and key the cache on another URI.
    if (!pathBytes.startsWith('/')) {
        return {};
    }
    struct stat source = {};
    if (::stat(pathBytes.constData(), &source) != 0 || !S_ISREG(source.st_mode)) {
        return {};
    }
    return {QDateTime::fromSecsSinceEpoch(source.st_mtime), qint64(source.st_size)};
}

// Decodes a thumbnail of the image at `pathBytes`, or a null image when it is
// not a source this provider generates from or a budget refuses it.
//
// QImageReader decodes at a reduced size where the format allows (cheap for
// JPEG) and honours EXIF orientation. It reads a descriptor opened on the raw
// bytes, so it never has to spell the source path.
QImage decodeImage(const QByteArray &pathBytes, const SourceStat &source)
{
    if (!source.modified.isValid() || source.bytes > kMaxSourceBytes ||
        !generatableImage(suffixOf(pathBytes))) {
        return QImage();
    }
    ReadDescriptor descriptor(pathBytes);
    QFile file;
    if (!descriptor.adoptInto(file)) {
        return QImage();
    }
    QImageReader reader(&file);
    reader.setAutoTransform(true);
    // The header's claim is hostile until measured: an unknown size is
    // refused rather than guessed, and a claimed surface past the budget is
    // never allocated.
    const QSize original = reader.size();
    if (!original.isValid() ||
        qint64(original.width()) * qint64(original.height()) > kMaxSourcePixels) {
        return QImage();
    }
    if (original.width() > kThumbnailEdge || original.height() > kThumbnailEdge) {
        reader.setScaledSize(original.scaled(kThumbnailEdge, kThumbnailEdge, Qt::KeepAspectRatio));
    }
    QImage image = reader.read();
    if (image.isNull()) {
        return QImage();
    }
    // A format that ignored the scaled size still leaves here at thumbnail
    // size, so the cache never holds a full-resolution copy.
    if (image.width() > kThumbnailEdge || image.height() > kThumbnailEdge) {
        image = image.scaled(kThumbnailEdge, kThumbnailEdge, Qt::KeepAspectRatio,
                             Qt::SmoothTransformation);
    }
    return image;
}

// Caches a produced thumbnail: write to a temporary sibling, owner-only, then
// rename, so a reader never sees a half-written PNG. Failure to cache is not
// fatal — the thumbnail still shows this session.
void writeCache(const QString &largeDir, const QString &cachePath, const QImage &image,
                const QByteArray &uri, const QDateTime &sourceMtime)
{
    if (!QDir().mkpath(largeDir)) {
        return;
    }
    // The process id as well as the thread: two hosts sharing the cache
    // could otherwise stage into the same temporary name.
    const QString temp =
        cachePath + QStringLiteral(".tmp-") + QString::number(::getpid()) + QLatin1Char('-') +
        QString::number(reinterpret_cast<quintptr>(QThread::currentThreadId()), 16);
    // The spec's two keys go on the image, not on the writer: the writer
    // folds its text into one "key: value" description that the PNG handler
    // splits again at the first colon, so "Thumb::URI" arrived as a key
    // "Thumb" and the URI was lost. The image's own text map is written key
    // by key.
    QImage keyed = image;
    keyed.setText(QStringLiteral("Thumb::URI"), QString::fromLatin1(uri));
    keyed.setText(QStringLiteral("Thumb::MTime"),
                  QString::number(sourceMtime.toSecsSinceEpoch()));
    QImageWriter writer(temp, "png");
    if (writer.write(keyed)) {
        QFile::setPermissions(temp, QFile::ReadOwner | QFile::WriteOwner);
        QFile::remove(cachePath);
        if (!QFile::rename(temp, cachePath)) {
            QFile::remove(temp);
        }
    } else {
        QFile::remove(temp);
    }
}

// The provider's own pool, bounded.
//
// On the global pool, scrolling past a folder of 5 000 images queued 5 000
// decodes, and reads blocked on a slow mount held the very threads QML's own
// asynchronous loaders use. This pool has a few threads of its own, and a
// request the view no longer needs is taken back out of its queue
// (`ThumbnailResponse::cancel`).
//
// Deliberately never destroyed: a QThreadPool's destructor waits for every
// running task without a bound, and a read blocked on a mount that stopped
// answering would then hold the quit open for ever. Quitting drains it
// instead, with a bound (`fluorita_thumbnail_shutdown`), and a task still
// blocked after that is left to the process's exit.
QThreadPool &thumbnailPool()
{
    static QThreadPool *const pool = [] {
        auto *created = new QThreadPool();
        created->setMaxThreadCount(qBound(2, QThread::idealThreadCount() / 2, 4));
        return created;
    }();
    return *pool;
}

// Loads a thumbnail for the file named by `pathBytes`: a current one from the
// shared cache, else what the host's hook answers, else a freshly generated and
// cached one. A null image for anything that is none of these — the delegate
// then keeps its glyph. Runs off-thread.
QImage loadThumbnail(const QByteArray &pathBytes, const fluorita::ThumbnailOwnPicture &ownPicture)
{
    const SourceStat source = statSource(pathBytes);
    if (!source.modified.isValid()) {
        return QImage();
    }

    // The spec keys the cache on the canonical file:// URI; hashing the same
    // URI other managers do lets this reuse — and contribute to — their cache.
    const QByteArray uri = fluorita_thumbnail_cache_uri(pathBytes);
    const QString digest =
        QString::fromLatin1(QCryptographicHash::hash(uri, QCryptographicHash::Md5).toHex());
    const QString largeDir = cacheRoot() + QStringLiteral("/large");
    const QString cachePath = largeDir + QLatin1Char('/') + digest + QStringLiteral(".png");

    // Reuse a cached thumbnail while it is at least as new as the file it
    // depicts — a thumbnail is always written after its source, so an edit
    // (which bumps the source mtime past the entry) is what forces a
    // regenerate. This keys off the filesystem, not the PNG's `Thumb::MTime`,
    // so it also honours entries other managers wrote. The entry's name is a
    // hexadecimal digest, so it is addressable as a QString even when its
    // source is not.
    {
        const QFileInfo cacheInfo(cachePath);
        if (cacheInfo.exists() && cacheInfo.size() <= kMaxCacheBytes &&
            cacheInfo.lastModified() >= source.modified) {
            const QImage cached(cachePath);
            if (!cached.isNull()) {
                return cached;
            }
        }
    }

    if (ownPicture) {
        fluorita::OwnPicture own = ownPicture(pathBytes);
        if (!own.image.isNull()) {
            if (own.image.width() > kThumbnailEdge || own.image.height() > kThumbnailEdge) {
                own.image = own.image.scaled(kThumbnailEdge, kThumbnailEdge, Qt::KeepAspectRatio,
                                             Qt::SmoothTransformation);
            }
            if (own.cacheable) {
                writeCache(largeDir, cachePath, own.image, uri, source.modified);
            }
            return own.image;
        }
    }

    QImage image = decodeImage(pathBytes, source);
    if (image.isNull()) {
        return QImage();
    }
    writeCache(largeDir, cachePath, image, uri, source.modified);
    return image;
}

// The raw path bytes an image-provider id names.
//
// `toUtf8`, and the distinction is not cosmetic. Qt does not pass the key
// through: it derives this id with `url.toString(RemoveScheme |
// RemoveAuthority)`, whose PrettyDecoded formatting has already turned every
// escape that spells valid UTF-8 back into its character. An accented name
// arrives decoded, and `toLatin1` would flatten each of its characters to one
// byte where the file holds two. An escape Qt could not decode — exactly the
// not-valid-UTF-8 case a key exists for — is still `%XX` here and is decoded
// below. From then on the path travels as bytes, never as a QString.
QByteArray pathBytesForId(const QString &id)
{
    return QByteArray::fromPercentEncoding(id.toUtf8());
}

// One asynchronous request: the work runs on the provider's bounded pool and
// the image is handed back when done.
//
// The engine owns the response and deletes it after `finished`; the pool never
// does (`setAutoDelete(false)`), so a response taken back out of the queue by
// `cancel` is simply never run.
class ThumbnailResponse : public QQuickImageResponse, public QRunnable
{
public:
    ThumbnailResponse(QByteArray pathBytes, fluorita::ThumbnailOwnPicture ownPicture)
        : m_pathBytes(std::move(pathBytes))
        , m_ownPicture(std::move(ownPicture))
    {
        setAutoDelete(false);
        thumbnailPool().start(this);
    }

    QQuickTextureFactory *textureFactory() const override
    {
        return QQuickTextureFactory::textureFactoryForImage(m_image);
    }

    // The view scrolled past, or the delegate went away. A request still
    // queued is taken out and never decoded; one already running skips the
    // work if it has not reached it yet. Either way `finished` is emitted
    // exactly once, which is what lets the engine clean the response up.
    void cancel() override
    {
        m_cancelled.storeRelease(1);
        if (thumbnailPool().tryTake(this)) {
            Q_EMIT finished();
        }
    }

    void run() override
    {
        if (!m_cancelled.loadAcquire()) {
            m_image = loadThumbnail(m_pathBytes, m_ownPicture);
        }
        Q_EMIT finished();
    }

private:
    QByteArray m_pathBytes;
    fluorita::ThumbnailOwnPicture m_ownPicture;
    QImage m_image;
    QAtomicInt m_cancelled;
};

class ThumbnailProvider : public QQuickAsyncImageProvider
{
public:
    explicit ThumbnailProvider(fluorita::ThumbnailOwnPicture ownPicture)
        : m_ownPicture(std::move(ownPicture))
    {
    }

    QQuickImageResponse *requestImageResponse(const QString &id, const QSize &) override
    {
        // The id is the item's path key, handed over verbatim: a delegate must
        // not re-encode it, or this would decode one layer and look for a file
        // literally named "%FF".
        return new ThumbnailResponse(pathBytesForId(id), m_ownPicture);
    }

private:
    fluorita::ThumbnailOwnPicture m_ownPicture;
};

} // namespace

QByteArray fluorita_thumbnail_cache_uri(const QByteArray &pathBytes)
{
    return QByteArrayLiteral("file://") + pathBytes.toPercentEncoding("!$&'()*+,;=:@/");
}

QByteArray fluorita_thumbnail_resolved_path(const QByteArray &key)
{
    // Exactly what a delegate writes, and exactly what Qt does with it before
    // calling `requestImageResponse`. A key is ASCII, so spelling it back into
    // a QString here loses nothing.
    const QUrl url(QStringLiteral("image://thumb/") + QString::fromUtf8(key));
    const QString id = url.toString(QUrl::RemoveScheme | QUrl::RemoveAuthority).mid(1);
    return pathBytesForId(id);
}

QSize fluorita_thumbnail_generated_size(const QByteArray &pathBytes)
{
    return decodeImage(pathBytes, statSource(pathBytes)).size();
}

void fluorita_thumbnail_shutdown(::std::int32_t milliseconds)
{
    // Requests nobody started yet are dropped; the ones running get a bounded
    // wait. Their responses still emit `finished`, into an engine that no
    // longer listens.
    thumbnailPool().clear();
    thumbnailPool().waitForDone(milliseconds);
}

void register_fluorita_thumbnail_provider(QQmlApplicationEngine &engine)
{
    register_fluorita_thumbnail_provider_with(engine, fluorita::ThumbnailOwnPicture());
}

void register_fluorita_thumbnail_provider_with(QQmlApplicationEngine &engine,
                                               fluorita::ThumbnailOwnPicture ownPicture)
{
    // The engine takes ownership of the provider.
    engine.addImageProvider(QStringLiteral("thumb"), new ThumbnailProvider(std::move(ownPicture)));
}
