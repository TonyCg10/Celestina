#include "calcita/page_provider.h"

#include <sys/stat.h>

#include <algorithm>
#include <cmath>
#include <list>
#include <memory>
#include <utility>
#include <vector>

#include <QtCore/QAtomicInt>
#include <QtCore/QBuffer>
#include <QtCore/QByteArray>
#include <QtCore/QFile>
#include <QtCore/QMutex>
#include <QtCore/QMutexLocker>
#include <QtCore/QString>
#include <QtCore/QRunnable>
#include <QtCore/QSize>
#include <QtCore/QSizeF>
#include <QtCore/QThreadPool>
#include <QtGui/QImage>
#include <QtPdf/QPdfDocument>
#include <QtPdf/QPdfDocumentRenderOptions>
#include <QtQml/QQmlApplicationEngine>
#include <QtQuick/QQuickAsyncImageProvider>
#include <QtQuick/QQuickImageResponse>
#include <QtQuick/QQuickTextureFactory>

// Threading. QML asks an asynchronous provider from its loader threads, and
// a `QPdfDocument` must never be used from two threads at once. This
// provider therefore owns its documents outright: it never touches the
// window's `PdfDocument` (which QML's own models keep using on the Qt
// thread), opens its own `QPdfDocument` from the file, and runs every render
// on a pool of exactly one thread that never expires. That thread creates,
// uses and destroys every document it holds, so no `QPdfDocument` ever
// crosses a thread. A second render thread would buy nothing: PDFium is not
// thread-safe and QtPdf serializes every PDFium call of the process behind
// one global lock (`QPdfMutexLocker`), so renders run one at a time whatever
// the pool's size; and the Qt thread's own QtPdf calls (search, selection,
// links) only ever wait for the one render in progress.
//
// Memory. A render is bounded to `kMaxPixels` and `kMaxEdge`: a poster at
// the largest zoom is rendered smaller and scaled up by the Image item
// rather than allocating gigabytes. At most `kOpenDocuments` documents stay
// open, the most recently used first, and a document whose file changed on
// disk is opened again.
//
// Copies. While a window edits, its pages render from an in-memory copy of
// the document (the editable annotations hidden, so the window draws them
// itself). The window registers the copy's bytes under its key and a
// revision from any thread (`copies()`, behind a mutex); the render thread
// opens a `QPdfDocument` over a `QBuffer` sharing those bytes, kept with
// the open documents like a file's.
//
// Everything here is internal to this translation unit.
namespace {

// The largest page image, in pixels: an A4 page at the zoom ladder's top
// (4x) on a 2x screen, 32 megapixels (128 MB in ARGB32).
constexpr qint64 kMaxPixels = qint64(32) * 1000 * 1000;

// The longest side of a page image, whatever its area.
constexpr int kMaxEdge = 16384;

// How many documents the renderer keeps open: a few windows' worth.
constexpr std::size_t kOpenDocuments = 4;

// A version of a file: a document is opened again when either changes.
struct FileStamp
{
    qint64 bytes = -1;
    qint64 modifiedSeconds = 0;
    qint64 modifiedNanoseconds = 0;

    bool valid() const { return bytes >= 0; }
    bool operator==(const FileStamp& other) const
    {
        return bytes == other.bytes && modifiedSeconds == other.modifiedSeconds &&
               modifiedNanoseconds == other.modifiedNanoseconds;
    }
};

// `stat` on the raw path bytes: a regular file's size and modification time,
// or an invalid stamp. A published key is always absolute.
FileStamp stampOf(const QByteArray& pathBytes)
{
    if (!pathBytes.startsWith('/')) {
        return {};
    }
    struct stat file = {};
    if (::stat(pathBytes.constData(), &file) != 0 || !S_ISREG(file.st_mode)) {
        return {};
    }
    return {qint64(file.st_size), qint64(file.st_mtim.tv_sec), qint64(file.st_mtim.tv_nsec)};
}

// A copy registered for a document's key at one revision.
struct RegisteredCopy
{
    QByteArray pathBytes;
    int revision = 0;
    QByteArray bytes;
};

// The registered copies; written from any thread, read from the render
// thread, always under `copiesMutex()`. Never destroyed.
std::vector<RegisteredCopy>& copies()
{
    static auto* const registered = new std::vector<RegisteredCopy>();
    return *registered;
}

QMutex& copiesMutex()
{
    static auto* const mutex = new QMutex();
    return *mutex;
}

// The bytes registered for `pathBytes` at `revision`, or a null array. The
// array shares the registered bytes (implicitly shared, so the copy out is
// cheap and safe across threads).
QByteArray registeredCopy(const QByteArray& pathBytes, int revision)
{
    QMutexLocker locker(&copiesMutex());
    for (const auto& copy : copies()) {
        if (copy.revision == revision && copy.pathBytes == pathBytes) {
            return copy.bytes;
        }
    }
    return {};
}

// The path bytes a path key names (its percent-encoding undone).
QByteArray pathBytesOfKey(const QString& key)
{
    return QByteArray::fromPercentEncoding(key.toUtf8());
}

struct OpenDocument
{
    QByteArray pathBytes;
    // The copy's revision, or -1 for the file itself.
    int revision = -1;
    FileStamp stamp;
    // A copy's bytes, which the document reads; declared before it, so it
    // outlives it.
    std::unique_ptr<QBuffer> buffer;
    std::unique_ptr<QPdfDocument> document;
};

// The open documents, most recently used first. Only the render thread
// touches the list (see the threading note above). Never destroyed: the
// shutdown empties it on that thread instead.
std::list<OpenDocument>& openDocuments()
{
    static auto* const documents = new std::list<OpenDocument>();
    return *documents;
}

// Keeps `opened` first among the open documents, within the bound.
QPdfDocument* keep(OpenDocument opened)
{
    auto& documents = openDocuments();
    documents.push_front(std::move(opened));
    while (documents.size() > kOpenDocuments) {
        documents.pop_back();
    }
    return documents.front().document.get();
}

// The copy of `pathBytes` registered at `revision`, opened or reused; null
// when none is registered or QtPdf cannot load it.
QPdfDocument* copyFor(const QByteArray& pathBytes, int revision)
{
    const QByteArray bytes = registeredCopy(pathBytes, revision);
    if (bytes.isNull()) {
        return nullptr;
    }
    auto& documents = openDocuments();
    for (auto it = documents.begin(); it != documents.end(); ++it) {
        if (it->revision == revision && it->pathBytes == pathBytes) {
            documents.splice(documents.begin(), documents, it);
            return documents.front().document.get();
        }
    }
    OpenDocument opened;
    opened.pathBytes = pathBytes;
    opened.revision = revision;
    opened.buffer = std::make_unique<QBuffer>();
    opened.buffer->setData(bytes);
    if (!opened.buffer->open(QIODevice::ReadOnly)) {
        return nullptr;
    }
    opened.document = std::make_unique<QPdfDocument>();
    // A random-access device loads at once, on this thread.
    opened.document->load(opened.buffer.get());
    if (opened.document->status() != QPdfDocument::Status::Ready) {
        return nullptr;
    }
    return keep(std::move(opened));
}

// The document at `pathBytes`, opened or reused; null when it is not a
// regular file or QtPdf cannot load it (a password-protected document
// included, as in the window).
QPdfDocument* fileFor(const QByteArray& pathBytes)
{
    const FileStamp stamp = stampOf(pathBytes);
    if (!stamp.valid()) {
        return nullptr;
    }
    auto& documents = openDocuments();
    for (auto it = documents.begin(); it != documents.end(); ++it) {
        if (it->revision != -1 || it->pathBytes != pathBytes) {
            continue;
        }
        if (it->stamp == stamp) {
            documents.splice(documents.begin(), documents, it);
            return documents.front().document.get();
        }
        documents.erase(it);
        break;
    }
    // QPdfDocument takes a QString name, as the window's PdfDocument does
    // through its URL, so the two open the same file the same way.
    OpenDocument opened;
    opened.pathBytes = pathBytes;
    opened.stamp = stamp;
    opened.document = std::make_unique<QPdfDocument>();
    if (opened.document->load(QFile::decodeName(pathBytes)) != QPdfDocument::Error::None) {
        return nullptr;
    }
    return keep(std::move(opened));
}

// What the pages of `pathBytes` at `revision` render from: the copy
// registered there, else the file.
QPdfDocument* documentFor(const QByteArray& pathBytes, int revision)
{
    if (QPdfDocument* copy = copyFor(pathBytes, revision)) {
        return copy;
    }
    return fileFor(pathBytes);
}

// The pixel size to render a page of `points` at for a `requested` source
// size: one side given keeps the proportions, none gives one pixel per
// point; then bounded by the budgets, keeping the proportions.
QSize renderSize(const QSizeF& points, const QSize& requested)
{
    if (points.width() <= 0 || points.height() <= 0) {
        return {};
    }
    double width = points.width();
    double height = points.height();
    if (requested.width() > 0 && requested.height() > 0) {
        width = requested.width();
        height = requested.height();
    } else if (requested.width() > 0) {
        width = requested.width();
        height = width * points.height() / points.width();
    } else if (requested.height() > 0) {
        height = requested.height();
        width = height * points.width() / points.height();
    }
    double shrink = 1.0;
    const double area = width * height;
    if (area > double(kMaxPixels)) {
        shrink = std::sqrt(double(kMaxPixels) / area);
    }
    const double longest = std::max(width, height) * shrink;
    if (longest > kMaxEdge) {
        shrink *= kMaxEdge / longest;
    }
    const int pixelsWide = std::max(1, int(std::floor(width * shrink)));
    const int pixelsHigh = std::max(1, int(std::floor(height * shrink)));
    return {pixelsWide, pixelsHigh};
}

// Renders `page` of the document at `pathBytes` with its annotations, on the
// render thread; a null image for a page that does not exist or a document
// that does not open. The background stays clear, as QtPdf renders it: the
// page view lays the paper under it.
QImage renderPage(const QByteArray& pathBytes, int revision, int page, const QSize& requested)
{
    QPdfDocument* document = documentFor(pathBytes, revision);
    if (document == nullptr || page < 0 || page >= document->pageCount()) {
        return {};
    }
    const QSize size = renderSize(document->pagePointSize(page), requested);
    if (size.isEmpty()) {
        return {};
    }
    QPdfDocumentRenderOptions options;
    options.setRenderFlags(QPdfDocumentRenderOptions::RenderFlag::Annotations);
    return document->render(page, size, options);
}

// The render pool: one thread, kept for the life of the process so the
// documents it opened stay on the thread that opened them. Never destroyed:
// a QThreadPool's destructor waits for the running render without a bound;
// the shutdown drains it with one instead.
QThreadPool& renderPool()
{
    static QThreadPool* const pool = [] {
        auto* created = new QThreadPool();
        created->setMaxThreadCount(1);
        created->setExpiryTimeout(-1);
        return created;
    }();
    return *pool;
}

// The page, the revision and the raw path bytes an image id names,
// `<page>/<revision>/<key>`.
//
// Qt derives the id with PrettyDecoded formatting, which has already turned
// every escape of the key that spells valid UTF-8 back into its character,
// and leaves the rest (a byte that is not UTF-8, `%25` itself) escaped; so
// the id is spelled back to UTF-8 and only then percent-decoded, and the
// path travels as bytes from there on, as the suite's thumbnail provider
// does with the same keys.
bool parseId(const QString& id, int* page, int* revision, QByteArray* pathBytes)
{
    const qsizetype first = id.indexOf(QLatin1Char('/'));
    const qsizetype second = first > 0 ? id.indexOf(QLatin1Char('/'), first + 1) : -1;
    if (first <= 0 || second <= first + 1) {
        return false;
    }
    bool pageNumber = false;
    bool revisionNumber = false;
    *page = id.left(first).toInt(&pageNumber);
    *revision = id.mid(first + 1, second - first - 1).toInt(&revisionNumber);
    *pathBytes = pathBytesOfKey(id.mid(second + 1));
    return pageNumber && revisionNumber && *page >= 0 && !pathBytes->isEmpty();
}

// One request: rendered on the pool and handed back when done.
//
// The engine owns the response and deletes it after `finished`; the pool
// never does (`setAutoDelete(false)`), so a response taken back out of the
// queue by `cancel` is simply never run.
class PageResponse : public QQuickImageResponse, public QRunnable
{
public:
    PageResponse(QByteArray pathBytes, int revision, int page, QSize requested)
        : m_pathBytes(std::move(pathBytes))
        , m_revision(revision)
        , m_page(page)
        , m_requested(requested)
    {
        setAutoDelete(false);
        renderPool().start(this);
    }

    QQuickTextureFactory* textureFactory() const override
    {
        return QQuickTextureFactory::textureFactoryForImage(m_image);
    }

    // The page left the near range or the window closed: a render still
    // queued is taken out; one running finishes. Either way `finished` is
    // emitted exactly once.
    void cancel() override
    {
        m_cancelled.storeRelease(1);
        if (renderPool().tryTake(this)) {
            Q_EMIT finished();
        }
    }

    void run() override
    {
        if (!m_cancelled.loadAcquire()) {
            m_image = renderPage(m_pathBytes, m_revision, m_page, m_requested);
        }
        Q_EMIT finished();
    }

private:
    QByteArray m_pathBytes;
    int m_revision;
    int m_page;
    QSize m_requested;
    QImage m_image;
    QAtomicInt m_cancelled;
};

class PageProvider : public QQuickAsyncImageProvider
{
public:
    QQuickImageResponse* requestImageResponse(const QString& id, const QSize& requestedSize) override
    {
        int page = -1;
        int revision = 0;
        QByteArray pathBytes;
        if (!parseId(id, &page, &revision, &pathBytes)) {
            // An id that names no page renders nothing: an empty path opens
            // no document, and the Image item reports the null image.
            return new PageResponse(QByteArray(), 0, -1, requestedSize);
        }
        return new PageResponse(std::move(pathBytes), revision, page, requestedSize);
    }
};

} // namespace

void calcita_register_page_provider(QQmlApplicationEngine& engine)
{
    // The engine takes ownership of the provider.
    engine.addImageProvider(QStringLiteral("calcitapage"), new PageProvider());
}

void calcita_page_provider_show_copy(const QString& key, ::std::int32_t revision,
                                     ::rust::Slice<const ::std::uint8_t> bytes)
{
    RegisteredCopy copy;
    copy.pathBytes = pathBytesOfKey(key);
    copy.revision = revision;
    copy.bytes = QByteArray(reinterpret_cast<const char*>(bytes.data()), qsizetype(bytes.size()));
    QMutexLocker locker(&copiesMutex());
    auto& registered = copies();
    for (auto& existing : registered) {
        if (existing.revision == revision && existing.pathBytes == copy.pathBytes) {
            existing.bytes = std::move(copy.bytes);
            return;
        }
    }
    registered.push_back(std::move(copy));
}

void calcita_page_provider_drop_copy(const QString& key, ::std::int32_t revision)
{
    const QByteArray pathBytes = pathBytesOfKey(key);
    QMutexLocker locker(&copiesMutex());
    auto& registered = copies();
    registered.erase(std::remove_if(registered.begin(), registered.end(),
                                    [&](const RegisteredCopy& copy) {
                                        return copy.revision == revision &&
                                               copy.pathBytes == pathBytes;
                                    }),
                     registered.end());
}

void calcita_page_provider_shutdown(::std::int32_t milliseconds)
{
    renderPool().clear();
    // The documents are closed on the thread that opened them.
    renderPool().start([] { openDocuments().clear(); });
    renderPool().waitForDone(milliseconds);
}
