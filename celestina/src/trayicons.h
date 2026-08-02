#pragma once

#include <QByteArray>
#include <QHash>
#include <QImage>
#include <QList>
#include <QMutex>
#include <QString>

// Turning another application's icon into something this panel can draw.
//
// A tray item offers its icon two ways and honours neither reliably. It may
// name one from an icon theme — which this session proves can name an icon that
// exists in *no* installed theme — or it may publish raw pixels, in a byte
// order that is not this machine's. Both paths end here, and either may end in
// nothing, which the drawer shows as the item's name instead of an empty gap.

/// One size an item published: `a(iiay)` is a list of these.
struct TrayPixmap {
    int width = 0;
    int height = 0;
    // ARGB32, most significant byte first — network order, as the
    // specification requires and as no little-endian machine reads directly.
    QByteArray argb;
};

/// Teaches Qt where the session's icon themes are and which one it uses, so a
/// foreign icon named by another application can be found at all.
///
/// Without this Qt resolves nothing: a shell with no platform theme has an
/// empty theme name and one search path into its own resources. `hicolor` is
/// set as the fallback because that is where the specification requires every
/// application to install, and on this session it is where two of the four
/// tray icons actually live.
void configureForeignIconThemes();

/// The icon theme the session tells its applications to use, read from a GTK
/// `settings.ini`. Empty when the file names none.
///
/// The shell reads it for one reason: foreign icons. Nothing about this suite's
/// own look comes from a desktop theme.
QString parseGtkIconThemeName(const QString &settingsIni);

/// Picks the size closest to what will be drawn and converts it, byte-swapping
/// on a little-endian machine. Returns a null image when nothing usable was
/// published — a size that does not match its own byte count included, because
/// an item that miscounts its pixels is one whose memory this panel will not
/// read past.
QImage bestTrayPixmap(const QList<TrayPixmap> &pixmaps, int preferredSize);

/// The images the panel is currently able to draw, by item key.
///
/// Shared between the tray host that fills it and the image provider that
/// serves it, and locked because Qt may ask for an image from its render
/// thread while the host is answering D-Bus on the GUI thread.
class TrayIconCache
{
public:
    void insert(const QString &key, const QImage &image);
    void remove(const QString &key);
    QImage take(const QString &key) const;

private:
    mutable QMutex m_lock;
    QHash<QString, QImage> m_images;
};
