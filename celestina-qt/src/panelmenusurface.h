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
    enum class Coverage {
        // The menu owns the output from `outputPosition` to its lower-right
        // edge. A floating or side-attached menu starts at (0, 0); a menu born
        // from the panel starts at the panel's lower seam, so no frame from
        // that carrier can be composited over the bar.
        Output,
        // A child menu is bounded to its card. Its still-mapped output-sized
        // parent remains the outside-click barrier and can receive another
        // tray-row request without first destroying the inventory.
        Card,
    };

    explicit PanelMenuSurface(QObject *parent = nullptr);
    ~PanelMenuSurface() override;

    // Adopts `content` — created, not yet shown — and maps it either across the
    // panel's whole screen or as one bounded card. With output coverage, where
    // the card sits inside it is the content's own decision, made from the
    // anchor its owner handed it. Returns false without taking ownership when
    // the surface cannot be mapped.
    // `outputPosition` is expressed in the panel output's local coordinates.
    // With output coverage it is the carrier's top-left origin; with card
    // coverage it is the bounded card position. Content placement is always
    // local to the resulting carrier.
    bool open(
        QWindow *content,
        QWindow *panel,
        Coverage coverage = Coverage::Output,
        const QPoint &outputPosition = QPoint()
    );
    // Rests the mapped surface without unmapping it: input shrinks to one
    // pixel, keyboard and focus are refused, the window stays on screen. The
    // point is the scene change that never happens — SURF-1's measured
    // flicker is one map or unmap per popup. Refuses a surface that is not
    // open or is already retiring; a retiring window is on its way out and
    // parking it would cancel a departure someone is animating.
    bool park();
    void close();
    bool isOpen() const { return !m_content.isNull() && !m_parked; }
    bool isParked() const { return !m_content.isNull() && m_parked; }
    QWindow *window() const { return m_content.data(); }

signals:
    void dismissed();

private:
    void contentVisibilityChanged(bool visible);

    QPointer<QWindow> m_content;
    // What `open` may resume rather than remap. The scope cannot change on a
    // mapped surface, and output and card carriers wear different anchors, so
    // only the same coverage on the same screen is a reuse; everything else
    // is a fresh map.
    Coverage m_mappedCoverage = Coverage::Output;
    bool m_parked = false;
};
