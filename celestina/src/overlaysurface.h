#pragma once

#include <QObject>
#include <QPointer>
#include <QWindow>

class QScreen;

// A centered, on-demand-keyboard overlay surface: the shell's third surface
// kind, after the panel and its menu.
//
// Unlike `PanelMenuSurface` it has no panel to anchor under — the launcher and
// the clipboard history are opened from a keybind, not a click, so there is no
// window position to sit below. Leaving `LayerSurfaceSpec::anchors` empty is
// what tells the compositor to center the surface on its output instead,
// per wlr-layer-shell; the mapping and dismiss-on-hide mechanics are otherwise
// exactly `PanelMenuSurface`'s, which is why this is a second small class
// rather than a third copy of them — see `surfacemanager.h`'s own note that
// later surfaces describe themselves through the same `LayerSurfaceSpec`
// recipe instead of copying a consumer that solves a different placement
// problem.
class OverlaySurface final : public QObject
{
    Q_OBJECT

public:
    // Where the surface sits, and therefore whether it takes the keyboard.
    //
    // The mechanics below — adopt a window, map it, tear it down when the
    // compositor dismisses it — are the same for both; only the description
    // handed to `LayerSurfaceSpec` differs, which is why this is one field
    // rather than a second class copying the teardown.
    enum class Placement {
        // Centered, focused, answering the keyboard: the launcher and the
        // clipboard history.
        Centered,
        // Anchored under the panel in the top-right corner and never focused:
        // the on-screen display, which is read, not used.
        Notification,
    };

    // Both arguments are explicit: a surface's placement is a decision its
    // owner makes, not a default it can drift into.
    OverlaySurface(Placement placement, QObject *parent = nullptr);
    ~OverlaySurface() override;

    // Adopts `content` — created, not yet shown — and maps it centered on
    // `screen`. Returns false without taking ownership when the surface
    // cannot be mapped.
    bool open(QWindow *content, QScreen *screen);
    void close();
    bool isOpen() const { return !m_content.isNull(); }
    QWindow *window() const { return m_content.data(); }

signals:
    void dismissed();

private:
    void contentVisibilityChanged(bool visible);

    QPointer<QWindow> m_content;
    Placement m_placement;
};
