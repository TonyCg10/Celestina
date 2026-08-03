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
    explicit OverlaySurface(QObject *parent = nullptr);
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
};
