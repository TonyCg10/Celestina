#pragma once

#include <QMargins>
#include <QSize>
#include <QString>

#include <LayerShellQt/Window>

class QScreen;
class QWindow;

// The one recipe for putting a layer-shell surface on screen.
//
// Extracted in R0-F from the two consumers that actually exist — the per-output
// panel and the panel's menu — and holding only what both of them set, with
// different values, on a real compositor. Nothing here is speculative: a field
// exists because two surfaces disagree about it.
//
// It is a description plus one function, deliberately not a manager. The
// panel's lifetime (one per output, hotplug, teardown) and the menu's (one at a
// time, dismissed by the compositor) have nothing in common, so neither lives
// here. Later surfaces — OSD, launcher — describe themselves the same way
// instead of copying `ensurePanel`.
struct LayerSurfaceSpec {
    QString scope;
    QScreen *screen = nullptr;
    LayerShellQt::Window::Anchors anchors;
    QMargins margins;
    // An invalid size leaves both axes to the content; a zero axis in a valid
    // size is the layer-shell way of saying "the compositor decides this one".
    QSize desiredSize;
    int exclusiveZone = 0;
    LayerShellQt::Window::Layer layer = LayerShellQt::Window::LayerTop;
    LayerShellQt::Window::KeyboardInteractivity keyboard =
        LayerShellQt::Window::KeyboardInteractivityNone;
    bool activateOnShow = false;
    // Whether the compositor may close the QWindow when it dismisses the
    // surface. A menu should go away; a panel that did would be tracked but
    // gone.
    bool closeOnDismissed = false;
    bool acceptsFocus = false;
};

// Whether a platform can carry the surfaces above at all.
//
// LayerShellQt attaches to any window and quietly declines to create a layer
// surface off Wayland, which would leave the shell mapping ordinary windows
// while reporting mapped panels. The host asks first and refuses instead.
enum class LayerShellSupport {
    // A Wayland session: layer surfaces are real.
    Available,
    // Qt's headless platform: windows exist, surfaces do not. Nothing observed
    // under it is evidence about a compositor.
    Headless,
    // Anything else — X11, minimal, an unknown plugin.
    Unavailable,
};

LayerShellSupport layerShellSupport(const QString &platformName);

// Configures `window` from `spec` and maps it last, so the compositor never
// sees a half-described surface. Returns false — having shown nothing — when
// the platform has no layer shell to attach to.
bool mapLayerSurface(QWindow *window, const LayerSurfaceSpec &spec);
