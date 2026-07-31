#pragma once

#include <QObject>
#include <QPoint>
#include <QPointer>
#include <QWindow>

// The panel's menu surface: the second surface kind this shell maps.
//
// R0-E compared it against an `xdg_popup` of the panel on a real session and
// found them indistinguishable for mapping, content and pointer interaction —
// so the choice fell to what only this one can do: ask for keyboard focus of
// its own, which a popup of a surface that refuses the keyboard cannot inherit.
// The popup candidate is gone; what is left is the winner, and it now describes
// itself through the shared `LayerSurfaceSpec` rather than configuring a
// surface by hand.
//
// It owns surface mechanics and lifetime only. It adopts a content window
// someone else built and never looks at what is drawn in it.
class PanelMenuSurface final : public QObject
{
    Q_OBJECT

public:
    explicit PanelMenuSurface(QObject *parent = nullptr);
    ~PanelMenuSurface() override;

    // Adopts `content` — created, not yet shown — and maps it at
    // `globalAnchor` on the panel's screen. Returns false without taking
    // ownership when the surface cannot be mapped.
    bool open(QWindow *content, QWindow *panel, const QPoint &globalAnchor);
    void close();
    bool isOpen() const { return !m_content.isNull(); }
    QWindow *window() const { return m_content.data(); }

signals:
    void dismissed();

private:
    void contentVisibilityChanged(bool visible);

    QPointer<QWindow> m_content;
};
