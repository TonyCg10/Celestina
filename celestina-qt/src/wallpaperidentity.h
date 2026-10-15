#pragma once

#include <QSize>
#include <QString>
#include <QVariantMap>

#include <optional>

// One host-validated row from the additive wallpaper-identity provider.
// This identity keeps Qt image requests current when a file changes at the
// same path. It carries no colour, exposure or foreground policy.
struct WallpaperIdentityReading {
    QString source;
    QString revision;
    quint64 generation = 0;
    QSize geometry;
};

// Returns the unique, strictly typed row for one output. When the host supplied
// a geometry, the row must carry that exact geometry as well; a stale or
// malformed row is not partially trusted.
std::optional<WallpaperIdentityReading> wallpaperIdentityForOutput(
    const QVariantMap &providers,
    const QString &output,
    const QSize &geometry
);
