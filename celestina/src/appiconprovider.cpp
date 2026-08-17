#include "appiconprovider.h"

#include "trayicons.h"

#include <QIcon>
#include <QMutexLocker>
#include <QUrl>

namespace {
// What a tile draws at. Asking the theme for this size rather than for whatever
// a delegate happens to request keeps the cache to one entry per application
// instead of one per pixel width a layout passed through while settling.
constexpr int iconSize = 32;

// An application id is another program's string. It is used only as a theme
// lookup key, never as a path, but a name carrying separators would let a theme
// engine walk somewhere this shell never meant to look.
bool isUsableName(const QString &name)
{
    if (name.isEmpty() || name.size() > 128)
        return false;

    return !name.contains(u'/') && !name.contains(u'\\') && !name.startsWith(u'.');
}
} // namespace

AppIconProvider::AppIconProvider()
    : QQuickImageProvider(QQuickImageProvider::Image)
{
    // Without this Qt resolves nothing: a shell with no platform theme has an
    // empty theme name and one search path into its own resources. The tray
    // installs the same configuration when it starts, and the call is written to
    // be repeatable — it de-duplicates its paths and keeps a theme name that is
    // already set — so this provider does not quietly depend on a tray having
    // run first.
    configureForeignIconThemes();
}

QImage AppIconProvider::requestImage(
    const QString &id,
    QSize *size,
    const QSize &requested
)
{
    const QString name = QUrl::fromPercentEncoding(id.toUtf8());
    if (!isUsableName(name))
        return QImage();

    QImage image;
    bool resolved = false;
    {
        const QMutexLocker locked(&m_lock);
        const auto cached = m_cache.constFind(name);
        if (cached != m_cache.constEnd()) {
            image = *cached;
            resolved = true;
        }
    }

    if (!resolved) {
        // A theme that has never heard of this application is the ordinary
        // case, not an error: plenty of programs ship no icon under the id they
        // report. The null image is cached too, so a miss is not looked up again
        // on every frame the map is drawn.
        //
        // Whether the answer was already found is what decides that, and it has
        // to be — asking whether the *image* is null cannot tell a miss that was
        // cached from a name never looked up, so every miss resolved again on
        // every frame. `QIcon::fromTheme` walks each installed theme's
        // directories before it can conclude nothing is there, on the GUI
        // thread, and a workspace map holding two unknown application ids paid
        // that walk twice per frame for as long as it was open. Measured as the
        // ids `com.anthropic.Claude` and `Chatgpt` on the author's session.
        image = QIcon::fromTheme(name).pixmap(iconSize, iconSize).toImage();
        const QMutexLocker locked(&m_lock);
        m_cache.insert(name, image);
        ++m_resolutions;
    }

    if (image.isNull())
        return image;

    if (requested.isValid() && !requested.isEmpty()) {
        image = image.scaled(
            requested,
            Qt::KeepAspectRatio,
            Qt::SmoothTransformation
        );
    }
    if (size)
        *size = image.size();
    return image;
}

int AppIconProvider::resolutionCount() const
{
    const QMutexLocker locked(&m_lock);
    return m_resolutions;
}
