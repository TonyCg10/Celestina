#pragma once

#include <QObject>
#include <QPointer>
#include <QString>
#include <QWindow>

class QScreen;

// An on-demand-keyboard overlay surface: the shell's third surface kind, after
// the panel and its menu. Interactive overlays cover their output so outside
// clicks belong to them; their QML content centres the card for keybind/command
// requests or follows a panel opener when one exists. Mapping and dismiss-on-
// hide mechanics remain separate from `PanelMenuSurface` because these focused
// overlays own a different content and input lifecycle.
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
        // the floating fallback for a reading whose panel has no icon to
        // attach to.
        Corner,
        // Attached to the panel's lower seam and the right edge, ignoring every
        // exclusive zone, and never focused: the quiet surfaces that grow a
        // membrane out of the bar — the on-screen display and the toast stack,
        // each under its own panel icon. The physical carrier begins at that
        // seam, so no buffer owned by it can cover the panel strip.
        AttachedTopRight,
        // The corner diagonally opposite the panel's, never focused: where the
        // on-screen display retreats to when something interactive already
        // occupies the top-right zone.
        BottomRight,
        // Low and centred, never focused: where the toast stack retreats to in
        // that same case — deliberately not the same corner the display
        // retreated to, so the two fallbacks cannot paint over each other.
        BottomCentre,
    };

    // Both arguments are explicit: a surface's placement is a decision its
    // owner makes, not a default it can drift into. `scope` names the layer
    // surface for the compositor's rules; the quiet placements share their
    // mechanics across two controllers, so the name cannot be derived from the
    // placement any more.
    OverlaySurface(Placement placement, const QString &scope, QObject *parent = nullptr);
    ~OverlaySurface() override;

    // Adopts `content` — created, not yet shown — and maps it on `screen`.
    // Returns false without taking ownership when the surface cannot be mapped.
    // `placement` overrides the constructed default for this one mapping: the
    // same controller opens attached when its panel offers an icon and falls
    // back to a corner when the zone is taken. `topInset` is an output-local
    // offset in the Qt units consumed by layer-shell. It remains zero for every
    // floating route; a non-zero value makes the layer window itself begin
    // below the panel, so no buffer from that window can cover the bar.
    bool open(QWindow *content, QScreen *screen);
    bool open(
        QWindow *content,
        QScreen *screen,
        Placement placement,
        int topInset = 0
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
    Placement m_placement;
    QString m_scope;
    // What `open` may resume rather than remap. The scope cannot change on a
    // mapped surface, and every placement wears different anchors, so only
    // the same placement on the same screen is a reuse; everything else is a
    // fresh map.
    Placement m_mappedPlacement = Placement::Centered;
    bool m_parked = false;
};
