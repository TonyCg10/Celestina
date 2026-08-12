#pragma once

#include <QList>
#include <QMetaObject>
#include <QPointer>
#include <QRectF>
#include <QString>
#include <QTimer>

class QQuickItem;
class QWindow;

// Owns the live icon-anchor state for one mapped primary contextual surface.
//
// Several independent controllers can retire asynchronously. Every successful
// acquisition therefore publishes a private token beside the output-local
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

    bool acquire(
        QWindow *panel,
        QWindow *surface,
        const QRectF &globalAttachmentAnchor
    );
    void release();

    bool isActive() const;

private:
    QQuickItem *resolveSource(const QRectF &initialGlobalRect) const;
    QQuickItem *anchorForSource(QQuickItem *source) const;
    QRectF currentAnchorRect() const;
    QRectF anchorRectOnOutput(const QRectF &globalRect) const;
    bool ownsPublishedToken() const;
    bool publishAnchorRect(const QRectF &globalRect);
    bool publishHiddenAnchor();
    bool trackedAnchorIsVisible() const;
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
    QPointer<QQuickItem> m_source;
    QPointer<QQuickItem> m_anchor;
    QString m_token;
    QList<QMetaObject::Connection> m_geometryConnections;
    QList<QMetaObject::Connection> m_lifetimeConnections;
    QTimer m_refreshTimer;
    bool m_rebuildPending = false;
    bool m_refreshing = false;
};
