#pragma once

#include <QList>
#include <QMetaObject>
#include <QPointF>
#include <QPointer>
#include <QRectF>
#include <QString>
#include <QTimer>

class QQuickItem;
class QScreen;
class QWindow;

// Translate one live panel anchor into the coordinate space of its carrier.
// `outputOrigin` and `carrierOriginOnOutput` use compositor output units; the
// returned rectangle uses the surface's unscaled QML units.
QRectF panelAttachmentRectOnCarrier(
    const QRectF &globalRect,
    const QPointF &outputOrigin,
    const QPointF &carrierOriginOnOutput,
    double shellScale
);

// Owns the live icon-anchor state for one mapped primary contextual surface.
//
// Several independent controllers can retire asynchronously. Every successful
// acquisition therefore publishes a private token beside the carrier-local
// anchor rectangle on the contextual surface. The initial global rectangle
// uniquely resolves a marked PanelMenuButton and its declared attachmentAnchor;
// that Item is a stable identity for the lifetime of the source (the current QML
// bindings all name fixed IDs), and the lease follows it and its visual
// ancestors. Dynamic rebinding is deliberately outside this contract. The
// source receives only a tokened `menuOpen` feedback bit, so the exact icon
// keeps its ordinary hover layer while its menu is mapped and an older retiring
// lease cannot clear a successor. No panel capsule state or geometry changes. A
// missing or ambiguous source leaves the menu floating, and a retiring
// controller clears state only while its source and surface tokens are current.
class PanelAttachmentLease final
{
public:
    PanelAttachmentLease();
    ~PanelAttachmentLease();

    PanelAttachmentLease(const PanelAttachmentLease &) = delete;
    PanelAttachmentLease &operator=(const PanelAttachmentLease &) = delete;
    PanelAttachmentLease(PanelAttachmentLease &&) = delete;
    PanelAttachmentLease &operator=(PanelAttachmentLease &&) = delete;

    // `carrierOriginOnOutput` is zero for full-output overlays and floating or
    // side-attached menus. A physically inset panel carrier supplies its real
    // top-left so the initial anchor and every live refresh share local units.
    bool acquire(
        QWindow *panel,
        QWindow *surface,
        const QRectF &globalAttachmentAnchor,
        const QPointF &carrierOriginOnOutput = QPointF()
    );
    void release();

    bool isActive() const;

private:
    QQuickItem *resolveSource(const QRectF &initialGlobalRect) const;
    QQuickItem *anchorForSource(QQuickItem *source) const;
    QRectF currentAnchorRect() const;
    QRectF anchorRectOnSurface(const QRectF &globalRect) const;
    bool ownsPublishedToken() const;
    bool publishAnchorRect(const QRectF &globalRect);
    bool publishHiddenAnchor();
    bool trackedAnchorIsVisible() const;
    // Attempts to adopt a recreated source occupying the lease's own anchor
    // rectangle. Returns false when no unique successor exists.
    bool rebindSource();
    bool windowsShareOutput() const;
    void scheduleRefresh();
    void scheduleRebuild();
    void processScheduledRefresh();
    void refreshAnchorRect();
    void rebuildGeometryTracking();
    void resolvedSourceLost();
    void disconnectGeometryTracking();
    void disconnectLifetimeTracking();

    QPointer<QWindow> m_panel;
    QPointer<QWindow> m_surface;
    // The output this lease was taken for, recorded once at acquisition.
    //
    // It is not derived from the surface on every refresh, and that is the
    // whole point. On Wayland a client is not told which output its surface
    // occupies until `wl_surface.enter` arrives, so Qt answers `screen()` with
    // the *primary* screen until then. The refresh runs on a zero-delay timer —
    // before that event — so comparing the panel against the surface's live
    // screen reported a mismatch on every output except the primary one, and
    // released the attachment permanently. The membrane therefore existed only
    // on the primary monitor, and a single-output nest could never show it.
    QPointer<QScreen> m_output;
    // The source's own canonical anchor rectangle in global coordinates,
    // remembered so a destroyed source can be matched to its successor.
    QRectF m_canonicalGlobalRect;
    bool m_rebindPending = false;
    // How many deferred passes a lost source is given to reappear before the
    // lease really lets go. Each pass is one zero-delay turn behind whatever
    // handler is rebuilding the strip; eight of them cover the slowest
    // rebuild measured while staying imperceptible.
    int m_rebindAttempts = 0;
    QPointer<QQuickItem> m_source;
    QPointer<QQuickItem> m_anchor;
    QPointF m_carrierOriginOnOutput;
    QString m_token;
    QList<QMetaObject::Connection> m_geometryConnections;
    QList<QMetaObject::Connection> m_lifetimeConnections;
    QTimer m_refreshTimer;
    bool m_rebuildPending = false;
    bool m_refreshing = false;
};
