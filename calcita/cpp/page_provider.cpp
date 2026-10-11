#include "calcita/page_provider.h"

#include <sys/stat.h>

#include <algorithm>
#include <cmath>
#include <list>
#include <memory>
#include <utility>

#include <QtCore/QAtomicInt>
#include <QtCore/QByteArray>
#include <QtCore/QFile>
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

struct OpenDocument
{
    QByteArray pathBytes;
    FileStamp stamp;
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

// The document at `pathBytes`, opened or reused; null when it is not a
// regular file or QtPdf cannot load it (a password-protected document
// included, as in the window).
QPdfDocument* documentFor(const QByteArray& pathBytes)
{
    const FileStamp stamp = stampOf(pathBytes);
    if (!stamp.valid()) {
        return nullptr;
    }
    auto& documents = openDocuments();
    for (auto it = documents.begin(); it != documents.end(); ++it) {
        if (it->pathBytes != pathBytes) {
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
    auto document = std::make_unique<QPdfDocument>();
    if (document->load(QFile::decodeName(pathBytes)) != QPdfDocument::Error::None) {
        return nullptr;
    }
    documents.push_front({pathBytes, stamp, std::move(document)});
    while (documents.size() > kOpenDocuments) {
        documents.pop_back();
    }
    return documents.front().document.get();
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
QImage renderPage(const QByteArray& pathBytes, int page, const QSize& requested)
{
    QPdfDocument* document = documentFor(pathBytes);
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

// The page and the raw path bytes an image id names, `<page>/<key>`.
//
// Qt derives the id with PrettyDecoded formatting, which has already turned
// every escape of the key that spells valid UTF-8 back into its character,
// and leaves the rest (a byte that is not UTF-8, `%25` itself) escaped; so
// the id is spelled back to UTF-8 and only then percent-decoded, and the
// path travels as bytes from there on, as the suite's thumbnail provider
// does with the same keys.
bool parseId(const QString& id, int* page, QByteArray* pathBytes)
{
    const qsizetype slash = id.indexOf(QLatin1Char('/'));
    if (slash <= 0) {
        return false;
    }
    bool number = false;
    *page = id.left(slash).toInt(&number);
    *pathBytes = QByteArray::fromPercentEncoding(id.mid(slash + 1).toUtf8());
    return number && *page >= 0 && !pathBytes->isEmpty();
}

// One request: rendered on the pool and handed back when done.
//
// The engine owns the response and deletes it after `finished`; the pool
// never does (`setAutoDelete(false)`), so a response taken back out of the
// queue by `cancel` is simply never run.
class PageResponse : public QQuickImageResponse, public QRunnable
{
public:
    PageResponse(QByteArray pathBytes, int page, QSize requested)
        : m_pathBytes(std::move(pathBytes))
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
            m_image = renderPage(m_pathBytes, m_page, m_requested);
        }
        Q_EMIT finished();
    }

private:
    QByteArray m_pathBytes;
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
        QByteArray pathBytes;
        if (!parseId(id, &page, &pathBytes)) {
            // An id that names no page renders nothing: an empty path opens
            // no document, and the Image item reports the null image.
            return new PageResponse(QByteArray(), -1, requestedSize);
        }
        return new PageResponse(std::move(pathBytes), page, requestedSize);
    }
};

} // namespace

void calcita_register_page_provider(QQmlApplicationEngine& engine)
{
    // The engine takes ownership of the provider.
    engine.addImageProvider(QStringLiteral("calcitapage"), new PageProvider());
}

void calcita_page_provider_shutdown(::std::int32_t milliseconds)
{
    renderPool().clear();
    // The documents are closed on the thread that opened them.
    renderPool().start([] { openDocuments().clear(); });
    renderPool().waitForDone(milliseconds);
}
