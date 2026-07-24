#include "siderita/icontheme.h"

#include <QtCore/QDir>
#include <QtCore/QStandardPaths>
#include <QtCore/QStringList>
#include <QtGui/QIcon>

void siderita_apply_icon_theme(const QString &theme)
{
    // This Wayland session has no desktop platform theme to populate the icon
    // theme search paths, so QIcon defaults to just ":/icons" and finds nothing
    // installed on disk — every named icon then falls back. Rebuild the standard
    // freedesktop search paths from the XDG data dirs so a theme in
    // /usr/share/icons, ~/.local/share/icons or ~/.icons actually resolves.
    QStringList searchPaths;
    const QStringList dataDirs =
        QStandardPaths::standardLocations(QStandardPaths::GenericDataLocation);
    for (const QString &dir : dataDirs) {
        searchPaths << dir + QStringLiteral("/icons");
    }
    searchPaths << QDir::homePath() + QStringLiteral("/.icons");
    searchPaths << QStringLiteral(":/icons");
    // Preserve anything Qt already had, without duplicating.
    for (const QString &existing : QIcon::themeSearchPaths()) {
        if (!searchPaths.contains(existing)) {
            searchPaths << existing;
        }
    }
    QIcon::setThemeSearchPaths(searchPaths);

    // Setting the theme name directly overrides whatever the platform theme
    // would have picked — the point here, given there is no DE to supply one.
    if (!theme.isEmpty()) {
        QIcon::setThemeName(theme);
    }

    // For any name the chosen theme lacks, fall back to Adwaita (always present
    // on this system) before Siderita's own handful of bundled SVGs.
    QIcon::setFallbackThemeName(QStringLiteral("Adwaita"));
}
