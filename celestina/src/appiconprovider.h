#pragma once

#include <QHash>
#include <QImage>
#include <QMutex>
#include <QQuickImageProvider>

// Serves `image://appicon/<app_id>` from the session's installed icon themes.
//
// An application's own icon is that application's identity, which is the same
// exception the tray already stands on: the suite's icon catalogue is closed to
// *first-party* glyphs, and nothing here invents one. What it does is look up a
// name another program chose, exactly as the tray looks up the name a tray item
// chose, through the theme configuration `configureForeignIconThemes()` already
// installs for that purpose.
//
// Resolution happens on the GUI thread, deliberately. `QIcon::fromTheme` reaches
// global loader state that Qt does not promise is thread-safe, so forcing this
// provider asynchronous would move a decode off the GUI thread at the price of a
// race in Qt's own icon loader. The cost that a static audit already recorded —
// decoding icons on the GUI thread — is answered by the cache instead: a given
// application and size is resolved once for the life of the process, and every
// later tile drawing the same application is a hash lookup. A map redrawn on
// every frame therefore costs nothing after its first.
//
// A name that resolves to nothing returns a null image, and the surface shows
// the application's own id as text rather than an empty square.
class AppIconProvider final : public QQuickImageProvider
{
public:
    AppIconProvider();

    QImage requestImage(const QString &id, QSize *size, const QSize &requested) override;

    // How many times a name was actually searched for in the themes.
    //
    // Exposed for one reason: the defect this class had was invisible from
    // outside it. A cached miss looked exactly like a resolved one — same null
    // image, same absence of an icon — while costing a full walk of every
    // theme directory on every frame that drew it. Only the count of searches
    // distinguishes the two, and a timing measurement does not: on a machine
    // with few themes installed the wasted walk is quick enough to hide.
    int resolutionCount() const;

private:
    // Locked because Qt may still ask for an already-cached image from its
    // render thread while the GUI thread is resolving another.
    mutable QMutex m_lock;
    QHash<QString, QImage> m_cache;
    int m_resolutions = 0;
};
