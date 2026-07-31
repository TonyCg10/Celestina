#include "surfacemanager.h"

#include <QWindow>

LayerShellSupport layerShellSupport(const QString &platformName)
{
    // Qt names its Wayland plugins `wayland`, `wayland-egl` and friends.
    if (platformName.startsWith(QStringLiteral("wayland")))
        return LayerShellSupport::Available;
    if (platformName == QStringLiteral("offscreen"))
        return LayerShellSupport::Headless;

    return LayerShellSupport::Unavailable;
}

bool mapLayerSurface(QWindow *window, const LayerSurfaceSpec &spec)
{
    if (!window)
        return false;

    window->setScreen(spec.screen);
    window->setFlag(Qt::FramelessWindowHint);
    window->setFlag(Qt::WindowDoesNotAcceptFocus, !spec.acceptsFocus);
    if (spec.desiredSize.isValid() && spec.desiredSize.height() > 0)
        window->setHeight(spec.desiredSize.height());

    auto *layerWindow = LayerShellQt::Window::get(window);
    if (!layerWindow)
        return false;

    layerWindow->setScreen(spec.screen);
    layerWindow->setScope(spec.scope);
    layerWindow->setAnchors(spec.anchors);
    layerWindow->setMargins(spec.margins);
    if (spec.desiredSize.isValid())
        layerWindow->setDesiredSize(spec.desiredSize);
    layerWindow->setExclusiveZone(spec.exclusiveZone);
    layerWindow->setLayer(spec.layer);
    layerWindow->setKeyboardInteractivity(spec.keyboard);
    layerWindow->setActivateOnShow(spec.activateOnShow);
    layerWindow->setCloseOnDismissed(spec.closeOnDismissed);

    // Mapping last ensures the output and every layer-shell property are fixed
    // before the compositor creates the surface.
    window->show();
    return true;
}
