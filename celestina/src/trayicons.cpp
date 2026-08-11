#include "trayicons.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QIcon>
#include <QImageReader>
#include <QMutexLocker>
#include <QStandardPaths>
#include <QtEndian>

namespace {
// A tray icon is drawn at panel height. Anything an application publishes far
// above that is a waste to convert, and far below it is what to grow from.
constexpr int maxPixmapPixels = 512 * 512;
constexpr qsizetype maxFlatThemeIconNameLength = 256;
constexpr qint64 maxFlatThemeIconFileBytes = 4 * 1024 * 1024;

QString configuredGtkIconThemeName()
{
    for (const QString &config :
         QStandardPaths::standardLocations(QStandardPaths::GenericConfigLocation)) {
        for (const auto *version : {"/gtk-4.0/settings.ini", "/gtk-3.0/settings.ini"}) {
            QFile settings(config + QString::fromLatin1(version));
            if (!settings.open(QIODevice::ReadOnly | QIODevice::Text))
                continue;

            const QString name =
                parseGtkIconThemeName(QString::fromUtf8(settings.readAll()));
            if (!name.isEmpty())
                return name;
        }
    }
    return {};
}
} // namespace

void configureForeignIconThemes()
{
    QStringList paths;
    paths.append(QDir::homePath() + QStringLiteral("/.icons"));
    for (const QString &data :
         QStandardPaths::standardLocations(QStandardPaths::GenericDataLocation)) {
        paths.append(data + QStringLiteral("/icons"));
    }
    paths.append(QIcon::themeSearchPaths());
    paths.removeDuplicates();
    QIcon::setThemeSearchPaths(paths);

    const QString gtkTheme = configuredGtkIconThemeName();
    if (QIcon::themeName().isEmpty() && !gtkTheme.isEmpty())
        QIcon::setThemeName(gtkTheme);

    // A Qt platform theme and the GTK settings can legitimately name different
    // installed themes in one session. StatusNotifierItems may come from either
    // toolkit, so retain Qt's theme as primary and let the declared GTK theme
    // answer only when it could not. `Adwaita`, for example, inherits
    // `AdwaitaLegacy` and `hicolor`, preserving the specification's floor.
    QIcon::setFallbackThemeName(
        trayFallbackThemeName(QIcon::themeName(), gtkTheme)
    );
}

QString parseGtkIconThemeName(const QString &settingsIni)
{
    for (const QString &line : settingsIni.split(u'\n')) {
        const QString trimmed = line.trimmed();
        // A commented-out setting is not a setting.
        if (trimmed.startsWith(u'#') || trimmed.startsWith(u';'))
            continue;

        const auto separator = trimmed.indexOf(u'=');
        if (separator < 0)
            continue;
        if (trimmed.left(separator).trimmed() != QStringLiteral("gtk-icon-theme-name"))
            continue;

        const QString name = trimmed.mid(separator + 1).trimmed();
        // A theme name is a directory name; anything with a separator in it is
        // a path this shell will not follow.
        if (name.isEmpty() || name.contains(u'/') || name.contains(u'\\'))
            continue;
        return name;
    }
    return QString();
}

QString trayFallbackThemeName(
    const QString &primaryTheme,
    const QString &gtkTheme
)
{
    if (!gtkTheme.isEmpty() && gtkTheme != primaryTheme)
        return gtkTheme;
    return QStringLiteral("hicolor");
}

QImage loadTrayIconFromFlatThemePath(
    const QString &directory,
    const QString &iconName,
    int preferredSize
)
{
    if (preferredSize <= 0
        || qint64(preferredSize) * preferredSize > maxPixmapPixels
        || iconName.isEmpty()
        || iconName.size() > maxFlatThemeIconNameLength
        || iconName == QStringLiteral(".") || iconName == QStringLiteral("..")
        || iconName.contains(u'/') || iconName.contains(u'\\')) {
        return {};
    }

    const QFileInfo directoryInfo(directory);
    const QString canonicalDirectory = directoryInfo.canonicalFilePath();
    if (canonicalDirectory.isEmpty() || !directoryInfo.isDir())
        return {};

    static const QStringList extensions {
        QStringLiteral(".png"),
        QStringLiteral(".svg"),
        QStringLiteral(".svgz"),
        QStringLiteral(".xpm"),
    };
    QStringList candidates;
    const QString lowerName = iconName.toLower();
    for (const QString &extension : extensions) {
        if (lowerName.endsWith(extension)) {
            candidates.append(iconName);
            break;
        }
    }
    if (candidates.isEmpty()) {
        for (const QString &extension : extensions)
            candidates.append(iconName + extension);
    }

    const QDir iconDirectory(canonicalDirectory);
    for (const QString &candidateName : candidates) {
        const QFileInfo candidateInfo(iconDirectory.filePath(candidateName));
        if (!candidateInfo.isFile() || candidateInfo.size() <= 0
            || candidateInfo.size() > maxFlatThemeIconFileBytes) {
            continue;
        }

        const QString canonicalCandidate = candidateInfo.canonicalFilePath();
        if (canonicalCandidate.isEmpty()
            || QFileInfo(canonicalCandidate).absolutePath() != canonicalDirectory) {
            continue;
        }

        QImageReader reader(canonicalCandidate);
        reader.setAutoTransform(true);
        const QSize sourceSize = reader.size();
        if (!sourceSize.isValid() || sourceSize.isEmpty()
            || qint64(sourceSize.width()) * sourceSize.height() > maxPixmapPixels) {
            continue;
        }
        reader.setScaledSize(
            sourceSize.scaled(
                QSize(preferredSize, preferredSize),
                Qt::KeepAspectRatio
            )
        );
        QImage image = reader.read();
        if (image.isNull()
            || qint64(image.width()) * image.height() > maxPixmapPixels) {
            continue;
        }

        return image;
    }

    return {};
}

QImage bestTrayPixmap(const QList<TrayPixmap> &pixmaps, int preferredSize)
{
    const TrayPixmap *chosen = nullptr;
    for (const TrayPixmap &pixmap : pixmaps) {
        if (pixmap.width <= 0 || pixmap.height <= 0)
            continue;
        if (qint64(pixmap.width) * pixmap.height > maxPixmapPixels)
            continue;
        // An item that miscounts its own pixels is one whose memory this panel
        // will not read past.
        if (pixmap.argb.size() != qsizetype(pixmap.width) * pixmap.height * 4)
            continue;

        if (!chosen) {
            chosen = &pixmap;
            continue;
        }

        // The smallest that still covers what will be drawn; failing that, the
        // largest there is, because growing a small icon looks worse than
        // shrinking a large one.
        const bool chosenCovers = chosen->width >= preferredSize;
        const bool candidateCovers = pixmap.width >= preferredSize;
        if (candidateCovers && (!chosenCovers || pixmap.width < chosen->width))
            chosen = &pixmap;
        else if (!candidateCovers && !chosenCovers && pixmap.width > chosen->width)
            chosen = &pixmap;
    }

    if (!chosen)
        return QImage();

    QImage image(chosen->width, chosen->height, QImage::Format_ARGB32);
    const auto *source = reinterpret_cast<const quint32 *>(chosen->argb.constData());
    for (int y = 0; y < chosen->height; ++y) {
        auto *row = reinterpret_cast<QRgb *>(image.scanLine(y));
        for (int x = 0; x < chosen->width; ++x) {
            // The specification says network byte order; this machine is not.
            row[x] = qFromBigEndian(source[y * chosen->width + x]);
        }
    }
    return image;
}

void TrayIconCache::insert(const QString &key, const QImage &image)
{
    QMutexLocker locked(&m_lock);
    m_images.insert(key, image);
}

void TrayIconCache::remove(const QString &key)
{
    QMutexLocker locked(&m_lock);
    m_images.remove(key);
}

QImage TrayIconCache::take(const QString &key) const
{
    QMutexLocker locked(&m_lock);
    return m_images.value(key);
}
