#pragma once

#include <QObject>
#include <QPointer>
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
        // the toast stack, which is where this session's notifications belong
        // and where the panel's own unread indicator points.
        Corner,
        // Low and centred, never focused: a value readout tied to a key press.
        // It is deliberately not in the corner — a volume key pressed while a
        // notification is up must not paint over it.
        Readout,
    };

    // Both arguments are explicit: a surface's placement is a decision its
    // owner makes, not a default it can drift into.
    OverlaySurface(Placement placement, QObject *parent = nullptr);
    ~OverlaySurface() override;

    // Adopts `content` — created, not yet shown — and maps it on `screen`.
    // Returns false without taking ownership when the surface cannot be mapped.
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
