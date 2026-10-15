#pragma once

#include <QHash>
#include <QObject>
#include <QPointer>
#include <QQmlComponent>
#include <QString>

#include "surfacemanager.h"

class QGuiApplication;
class QQmlEngine;
class QScreen;
class QWindow;
class ShellProvidersClient;

// How a background surface is described to the compositor. Exposed so the
// description can be checked without a QML engine, the way every other surface
// recipe in this shell is checked: the manager's own QML loading is proved by
// the offscreen smoke instead.
LayerSurfaceSpec wallpaperSurfaceSpec(QScreen *screen);

// One background surface per output, for as long as that output exists.
//
// Its lifecycle is the panel's — created per `QScreen`, added and removed with
// hotplug — but nothing else about it is: a wallpaper takes no exclusive zone,
// no keyboard and no focus, sits on the background layer, and has no state of
// its own beyond which file the provider chose for its screen. That is why it
// is a second small manager rather than a mode of `PanelManager`, whose whole
// contract is the panel's geometry, blur and menu.
//
// Which image belongs to which output is decided in `celestina-shell-core` and
// published by the provider; this class tells the helper which screens exist
// and hands each surface the answer for its own.
class WallpaperManager final : public QObject
{
    Q_OBJECT

public:
    WallpaperManager(
        QGuiApplication *application,
        QQmlEngine *engine,
        ShellProvidersClient *providers,
        bool reducedMotion,
        QObject *parent = nullptr
    );
    ~WallpaperManager() override;

    // False when the component failed to load. The session then simply has no
    // shell-drawn wallpaper; nothing else changes.
    bool isEnabled() const { return m_enabled; }
    int surfaceCount() const { return static_cast<int>(m_surfaces.size()); }

    // Maps a surface on every screen that has none. Returns whether anything
    // is mapped at all.
    bool start();

private:
    void addScreen(QScreen *screen);
    void removeScreen(QScreen *screen);
    // Tells the helper which outputs exist, so it can choose per output rather
    // than guess. Sent on start and on every hotplug.
    void publishOutputs();
    void applyChoices();

    QQmlComponent m_component;
    QPointer<ShellProvidersClient> m_providers;
    QHash<QScreen *, QWindow *> m_surfaces;
    bool m_reducedMotion;
    bool m_enabled;
};
