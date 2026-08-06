#include "siderita/clipboard.h"

#include <QtCore/QByteArray>
#include <QtCore/QList>
#include <QtCore/QMimeData>
#include <QtCore/QUrl>
#include <QtGui/QClipboard>
#include <QtGui/QGuiApplication>

namespace {
// The GNOME/Nautilus file-clipboard format: a first line of "copy" or "cut",
// then one URI per line. Honoured by most freedesktop file managers.
const QString kGnomeFormat = QStringLiteral("x-special/gnome-copied-files");
} // namespace

void siderita_set_clipboard_uris(const QStringList& uris, bool cut)
{
    QClipboard* clipboard = QGuiApplication::clipboard();
    if (clipboard == nullptr) {
        return;
    }

    // QClipboard takes ownership of the QMimeData.
    auto* mime = new QMimeData();
    QList<QUrl> urls;
    urls.reserve(uris.size());
    QByteArray gnome = cut ? QByteArrayLiteral("cut") : QByteArrayLiteral("copy");
    for (const QString& uri : uris) {
        // `fromEncoded`, not `fromLocalFile`: the escapes are already the ones
        // Rust chose, and re-deriving them from a path would need a QString
        // spelling of a name that may not have one.
        const QUrl url = QUrl::fromEncoded(uri.toLatin1());
        if (!url.isValid()) {
            continue;
        }
        urls.append(url);
        gnome.append('\n');
        gnome.append(url.toEncoded());
    }
    mime->setUrls(urls); // populates text/uri-list
    mime->setData(kGnomeFormat, gnome);
    clipboard->setMimeData(mime);
}

QStringList siderita_read_clipboard_uris()
{
    QStringList out;
    QClipboard* clipboard = QGuiApplication::clipboard();
    if (clipboard == nullptr) {
        return out;
    }
    const QMimeData* mime = clipboard->mimeData();
    if (mime == nullptr || !mime->hasUrls()) {
        return out;
    }
    const QList<QUrl> urls = mime->urls();
    for (const QUrl& url : urls) {
        if (url.isLocalFile()) {
            // `toEncoded`, not `toLocalFile`: the second one has to produce a
            // QString path, and that is exactly where a byte with no UTF-8
            // meaning used to be replaced. Rust decodes the URI instead.
            out.append(QString::fromLatin1(url.toEncoded()));
        }
    }
    return out;
}

bool siderita_clipboard_is_cut()
{
    QClipboard* clipboard = QGuiApplication::clipboard();
    if (clipboard == nullptr) {
        return false;
    }
    const QMimeData* mime = clipboard->mimeData();
    if (mime == nullptr || !mime->hasFormat(kGnomeFormat)) {
        return false;
    }
    return mime->data(kGnomeFormat).startsWith("cut");
}

bool siderita_clipboard_has_uris()
{
    QClipboard* clipboard = QGuiApplication::clipboard();
    if (clipboard == nullptr) {
        return false;
    }
    const QMimeData* mime = clipboard->mimeData();
    return mime != nullptr && mime->hasUrls();
}

void siderita_clear_clipboard()
{
    QClipboard* clipboard = QGuiApplication::clipboard();
    if (clipboard != nullptr) {
        clipboard->clear();
    }
}
