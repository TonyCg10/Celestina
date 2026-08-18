#include "panelattachmentlease.h"

#include <QMetaMethod>
#include <QMetaProperty>
#include <QQuickItem>
#include <QQuickWindow>
#include <QScopedValueRollback>
#include <QScreen>
#include <QUuid>
#include <QVariant>
#include <QWindow>
#include <QtGlobal>

namespace {
constexpr auto surfaceAnchorRectProperty = "attachmentAnchorRect";
constexpr auto surfaceLeaseTokenProperty =
    "_celestinaAttachmentAnchorLeaseToken";
constexpr auto sourceMenuOpenProperty = "menuOpen";
constexpr auto sourceFeedbackTokenProperty =
    "_celestinaPanelMenuFeedbackLeaseToken";
constexpr qreal rectMatchTolerance = 0.75;

bool isFiniteRect(const QRectF &rect)
{
    return rect.isValid() && !rect.isEmpty()
        && qIsFinite(rect.x()) && qIsFinite(rect.y())
        && qIsFinite(rect.width()) && qIsFinite(rect.height());
}

bool rectanglesMatch(const QRectF &left, const QRectF &right)
{
    return qAbs(left.x() - right.x()) <= rectMatchTolerance
        && qAbs(left.y() - right.y()) <= rectMatchTolerance
        && qAbs(left.width() - right.width()) <= rectMatchTolerance
        && qAbs(left.height() - right.height()) <= rectMatchTolerance;
}

QRectF invokeAttachmentAnchorGlobalRect(QQuickItem *item)
{
    if (!item)
        return QRectF();

    const int methodIndex =
        item->metaObject()->indexOfMethod("attachmentAnchorGlobalRectNow()");
    if (methodIndex < 0)
        return QRectF();

    const QMetaMethod method = item->metaObject()->method(methodIndex);
    if (method.returnMetaType() == QMetaType::fromType<QRectF>()) {
        QRectF rect;
        return method.invoke(
            item,
            Qt::DirectConnection,
            Q_RETURN_ARG(QRectF, rect)
        ) ? rect : QRectF();
    }

    QVariant value;
    return method.invoke(
        item,
        Qt::DirectConnection,
        Q_RETURN_ARG(QVariant, value)
    ) ? value.toRectF() : QRectF();
}

void disconnectAll(QList<QMetaObject::Connection> &connections)
{
    for (const QMetaObject::Connection &connection : connections)
        QObject::disconnect(connection);
    connections.clear();
}

void appendVisualDescendants(
    QQuickItem *item,
    QList<QQuickItem *> &descendants
)
{
    if (!item)
        return;

    for (QQuickItem *const child : item->childItems()) {
        descendants.append(child);
        appendVisualDescendants(child, descendants);
    }
}

bool itemIsVisibleInWindow(QQuickItem *item, QQuickWindow *window)
{
    if (!item || !window || !window->isVisible() || !window->contentItem())
        return false;

    for (QQuickItem *current = item; current;
         current = current->parentItem()) {
        if (!current->isVisible())
            return false;
        if (current == window->contentItem())
            return true;
    }
    return false;
}
} // namespace

QRectF panelAttachmentRectOnCarrier(
    const QRectF &globalRect,
    const QPointF &outputOrigin,
    const QPointF &carrierOriginOnOutput,
    double shellScale
)
{
    const QRectF onCarrier = globalRect.translated(
        -outputOrigin - carrierOriginOnOutput);
    if (shellScale <= 0.0 || shellScale == 1.0)
        return onCarrier;
    return QRectF(onCarrier.x() / shellScale,
                  onCarrier.y() / shellScale,
                  onCarrier.width() / shellScale,
                  onCarrier.height() / shellScale);
}

PanelAttachmentLease::PanelAttachmentLease()
{
    m_refreshTimer.setSingleShot(true);
    m_refreshTimer.setInterval(0);
    QObject::connect(
        &m_refreshTimer,
        &QTimer::timeout,
        [this]() { processScheduledRefresh(); }
    );
}

PanelAttachmentLease::~PanelAttachmentLease()
{
    release();
}

bool PanelAttachmentLease::isActive() const
{
    return ownsPublishedToken();
}

bool PanelAttachmentLease::acquire(
    QWindow *panel,
    QWindow *surface,
    const QRectF &globalAttachmentAnchor,
    const QPointF &carrierOriginOnOutput
)
{
    release();
    if (!panel || !surface || !isFiniteRect(globalAttachmentAnchor)
        || !qIsFinite(carrierOriginOnOutput.x())
        || !qIsFinite(carrierOriginOnOutput.y())) {
        return false;
    }

    const QString token = QUuid::createUuid().toString(QUuid::WithoutBraces);
    // By construction the caller mapped the surface to the panel's own output,
    // so the panel's screen *is* this lease's output. Recorded now, while it is
    // known, rather than asked of a surface that cannot answer yet.
    m_output = panel->screen();
    m_panel = panel;
    QQuickItem *const source = resolveSource(globalAttachmentAnchor);
    m_panel = nullptr;
    QQuickItem *const anchor = anchorForSource(source);
    const QRectF canonicalGlobalAnchor =
        invokeAttachmentAnchorGlobalRect(source);
    auto *const quickPanel = qobject_cast<QQuickWindow *>(panel);
    const int menuOpenPropertyIndex = source
        ? source->metaObject()->indexOfProperty(sourceMenuOpenProperty) : -1;
    if (!source || !anchor || !isFiniteRect(canonicalGlobalAnchor)
        || !quickPanel || anchor->window() != quickPanel
        || menuOpenPropertyIndex < 0
        || !source->metaObject()->property(menuOpenPropertyIndex).isWritable()) {
        // The surface already received the caller's initial snapshot during
        // QML construction. Without one unique semantic source, remove that
        // attachment request and keep the already-open menu safely floating.
        // Never erase geometry owned by a newer lease on the same surface.
        if (surface->property(surfaceLeaseTokenProperty).toString().isEmpty())
            surface->setProperty(surfaceAnchorRectProperty, QRectF());
        return false;
    }

    // Publish both identities first. The rectangle shapes only the contextual
    // background; the source receives only the ordinary hover-feedback bit.
    // Neither path changes the panel capsule or any content geometry.
    const QString previousSourceToken =
        source->property(sourceFeedbackTokenProperty).toString();
    const bool previousMenuOpen =
        source->property(sourceMenuOpenProperty).toBool();
    const QString previousSurfaceToken =
        surface->property(surfaceLeaseTokenProperty).toString();
    const QRectF previousSurfaceRect =
        surface->property(surfaceAnchorRectProperty).toRectF();
    const auto rollBack = [&]() {
        if (source->property(sourceFeedbackTokenProperty).toString() == token) {
            source->setProperty(sourceMenuOpenProperty, previousMenuOpen);
            source->setProperty(
                sourceFeedbackTokenProperty,
                previousSourceToken
            );
        }
        if (surface->property(surfaceLeaseTokenProperty).toString() == token) {
            surface->setProperty(surfaceAnchorRectProperty, previousSurfaceRect);
            surface->setProperty(
                surfaceLeaseTokenProperty,
                previousSurfaceToken
            );
        }
    };
    source->setProperty(sourceFeedbackTokenProperty, token);
    source->setProperty(sourceMenuOpenProperty, true);
    if (source->property(sourceFeedbackTokenProperty).toString() != token
        || !source->property(sourceMenuOpenProperty).toBool()) {
        rollBack();
        return false;
    }
    surface->setProperty(surfaceLeaseTokenProperty, token);
    if (surface->property(surfaceLeaseTokenProperty).toString() != token) {
        rollBack();
        return false;
    }

    const QScreen *const screen = panel->screen();
    const QPointF outputOrigin =
        screen ? QPointF(screen->geometry().topLeft()) : QPointF();
    const double shellScale = surface->property("shellScale").toDouble();
    const QRectF surfaceLocalAnchor = itemIsVisibleInWindow(anchor, quickPanel)
        ? panelAttachmentRectOnCarrier(
              canonicalGlobalAnchor,
              outputOrigin,
              carrierOriginOnOutput,
              shellScale)
        : QRectF();
    surface->setProperty(surfaceAnchorRectProperty, surfaceLocalAnchor);
    if (surface->property(surfaceAnchorRectProperty).toRectF()
        != surfaceLocalAnchor) {
        rollBack();
        return false;
    }

    m_panel = panel;
    m_surface = surface;
    m_source = source;
    m_anchor = anchor;
    m_carrierOriginOnOutput = carrierOriginOnOutput;
    m_token = token;

    m_lifetimeConnections.append(QObject::connect(
        panel,
        &QObject::destroyed,
        [this]() { release(); }
    ));
    m_lifetimeConnections.append(QObject::connect(
        surface,
        &QObject::destroyed,
        [this]() { release(); }
    ));
    m_lifetimeConnections.append(QObject::connect(
        surface,
        &QWindow::visibleChanged,
        [this](bool visible) {
            if (!visible)
                release();
        }
    ));
    m_lifetimeConnections.append(QObject::connect(
        surface,
        &QWindow::screenChanged,
        [this]() { scheduleRebuild(); }
    ));
    m_lifetimeConnections.append(QObject::connect(
        source,
        &QObject::destroyed,
        [this]() { resolvedSourceLost(); }
    ));
    rebuildGeometryTracking();
    return true;
}

QQuickItem *PanelAttachmentLease::resolveSource(
    const QRectF &initialGlobalRect
) const
{
    auto *const quickPanel = qobject_cast<QQuickWindow *>(m_panel.data());
    if (!quickPanel || !quickPanel->contentItem())
        return nullptr;

    QList<QQuickItem *> candidates;
    appendVisualDescendants(quickPanel->contentItem(), candidates);
    QList<QQuickItem *> matches;
    QList<QQuickItem *> visibleMatches;
    for (QQuickItem *const candidate : candidates) {
        if (!candidate->property("isPanelAttachmentSource").toBool())
            continue;

        QQuickItem *const anchor = anchorForSource(candidate);
        const QRectF candidateRect =
            invokeAttachmentAnchorGlobalRect(candidate);
        if (!isFiniteRect(candidateRect)
            || !anchor
            || anchor->window() != quickPanel
            || !rectanglesMatch(candidateRect, initialGlobalRect)) {
            continue;
        }

        matches.append(candidate);
        if (itemIsVisibleInWindow(anchor, quickPanel))
            visibleMatches.append(candidate);
    }

    // Effective visibility disambiguates a stale hidden source from the icon
    // that actually occupies the requested rectangle. Any remaining ambiguity
    // degrades to a floating surface instead of following an arbitrary item.
    if (visibleMatches.size() == 1)
        return visibleMatches.constFirst();
    if (visibleMatches.isEmpty() && matches.size() == 1)
        return matches.constFirst();
    return nullptr;
}

QQuickItem *PanelAttachmentLease::anchorForSource(QQuickItem *source) const
{
    if (!source)
        return nullptr;

    QObject *const object =
        source->property("attachmentAnchor").value<QObject *>();
    return qobject_cast<QQuickItem *>(object);
}

QRectF PanelAttachmentLease::currentAnchorRect() const
{
    const QRectF rect = invokeAttachmentAnchorGlobalRect(m_source.data());
    return isFiniteRect(rect) ? rect : QRectF();
}

QRectF PanelAttachmentLease::anchorRectOnSurface(
    const QRectF &globalRect
) const
{
    const QScreen *const screen = m_surface && m_surface->screen()
        ? m_surface->screen()
        : (m_panel ? m_panel->screen() : nullptr);
    const QPointF outputOrigin =
        screen ? QPointF(screen->geometry().topLeft()) : QPointF();
    // Initial publication and every live refresh use this same carrier-local,
    // unscaled contract. Published in output coordinates, the first refresh on
    // a scaled output moved the membrane's mouth beside its glyph; published
    // without the carrier offset, it would move back over the physically
    // excluded panel strip. The factor is read from the surface because that
    // surface is what lays out in those units; see shellscale.h.
    const double scale = m_surface
        ? m_surface->property("shellScale").toDouble() : 0.0;
    return panelAttachmentRectOnCarrier(
        globalRect,
        outputOrigin,
        m_carrierOriginOnOutput,
        scale);
}

bool PanelAttachmentLease::ownsPublishedToken() const
{
    return m_surface && !m_token.isEmpty()
        && m_surface->property(surfaceLeaseTokenProperty).toString() == m_token;
}

bool PanelAttachmentLease::publishAnchorRect(const QRectF &globalRect)
{
    if (!ownsPublishedToken() || !isFiniteRect(globalRect))
        return false;

    const QRectF previousLocal =
        m_surface->property(surfaceAnchorRectProperty).toRectF();
    const QRectF surfaceLocalRect = anchorRectOnSurface(globalRect);
    if (previousLocal == surfaceLocalRect)
        return true;

    m_surface->setProperty(surfaceAnchorRectProperty, surfaceLocalRect);
    if (!ownsPublishedToken()
        || m_surface->property(surfaceAnchorRectProperty).toRectF()
            != surfaceLocalRect) {
        return false;
    }
    return true;
}

bool PanelAttachmentLease::publishHiddenAnchor()
{
    if (!ownsPublishedToken())
        return false;

    m_surface->setProperty(surfaceAnchorRectProperty, QRectF());
    if (!ownsPublishedToken()
        || !m_surface->property(surfaceAnchorRectProperty).toRectF().isEmpty()) {
        return false;
    }
    return true;
}

bool PanelAttachmentLease::trackedAnchorIsVisible() const
{
    auto *const quickPanel = qobject_cast<QQuickWindow *>(m_panel.data());
    if (!quickPanel || !quickPanel->isVisible() || !m_source || !m_anchor)
        return false;

    return itemIsVisibleInWindow(m_anchor.data(), quickPanel);
}

bool PanelAttachmentLease::windowsShareOutput() const
{
    if (!m_panel || !m_surface)
        return false;
    // Against the output this lease was taken for, never against the surface's
    // live `screen()`. The surface is mapped to the panel's output by
    // construction; what changes underneath is only Qt's *knowledge* of it, and
    // acting on that knowledge before `wl_surface.enter` released every
    // attachment that was not on the primary monitor.
    //
    // A genuine change still releases: an output that goes away leaves
    // `m_output` null, and a panel that really moves to another screen no
    // longer matches it.
    if (!m_output)
        return false;
    return !m_panel->screen() || m_panel->screen() == m_output;
}

void PanelAttachmentLease::scheduleRefresh()
{
    if (ownsPublishedToken())
        m_refreshTimer.start();
}

void PanelAttachmentLease::scheduleRebuild()
{
    if (!ownsPublishedToken())
        return;
    m_rebuildPending = true;
    m_refreshTimer.start();
}

void PanelAttachmentLease::processScheduledRefresh()
{
    if (!ownsPublishedToken())
        return;
    if (!windowsShareOutput()) {
        release();
        return;
    }
    if (m_rebuildPending) {
        m_rebuildPending = false;
        rebuildGeometryTracking();
        return;
    }
    refreshAnchorRect();
}

void PanelAttachmentLease::refreshAnchorRect()
{
    if (m_refreshing || !ownsPublishedToken() || !m_source || !m_anchor)
        return;

    QScopedValueRollback guard(m_refreshing, true);
    if (!trackedAnchorIsVisible()) {
        publishHiddenAnchor();
        return;
    }
    const QRectF rect = currentAnchorRect();
    if (!isFiniteRect(rect)) {
        resolvedSourceLost();
        return;
    }
    publishAnchorRect(rect);
}

void PanelAttachmentLease::rebuildGeometryTracking()
{
    disconnectGeometryTracking();
    auto *const quickPanel = qobject_cast<QQuickWindow *>(m_panel.data());
    if (!m_source || !m_anchor || !quickPanel
        || m_source->window() != quickPanel
        || m_anchor->window() != quickPanel
        || anchorForSource(m_source.data()) != m_anchor.data()) {
        resolvedSourceLost();
        return;
    }

    const auto refresh = [this]() { scheduleRefresh(); };
    const auto rebuild = [this]() { scheduleRebuild(); };
    const auto lost = [this]() { resolvedSourceLost(); };
    for (QQuickItem *item = m_anchor.data(); item; item = item->parentItem()) {
        m_geometryConnections.append(
            QObject::connect(item, &QQuickItem::xChanged, refresh));
        m_geometryConnections.append(
            QObject::connect(item, &QQuickItem::yChanged, refresh));
        m_geometryConnections.append(
            QObject::connect(item, &QQuickItem::widthChanged, refresh));
        m_geometryConnections.append(
            QObject::connect(item, &QQuickItem::heightChanged, refresh));
        m_geometryConnections.append(
            QObject::connect(item, &QQuickItem::scaleChanged, refresh));
        m_geometryConnections.append(
            QObject::connect(item, &QQuickItem::rotationChanged, refresh));
        m_geometryConnections.append(QObject::connect(
            item,
            &QQuickItem::transformOriginChanged,
            refresh
        ));
        m_geometryConnections.append(
            QObject::connect(item, &QQuickItem::visibleChanged, refresh));
        m_geometryConnections.append(
            QObject::connect(item, &QQuickItem::parentChanged, rebuild));
        m_geometryConnections.append(
            QObject::connect(item, &QQuickItem::windowChanged, rebuild));
        m_geometryConnections.append(
            QObject::connect(item, &QObject::destroyed, lost));
    }

    m_geometryConnections.append(QObject::connect(
        m_panel.data(),
        &QWindow::xChanged,
        refresh
    ));
    m_geometryConnections.append(QObject::connect(
        m_panel.data(),
        &QWindow::yChanged,
        refresh
    ));
    m_geometryConnections.append(QObject::connect(
        m_panel.data(),
        &QWindow::widthChanged,
        refresh
    ));
    m_geometryConnections.append(QObject::connect(
        m_panel.data(),
        &QWindow::heightChanged,
        refresh
    ));
    m_geometryConnections.append(QObject::connect(
        m_panel.data(),
        &QWindow::visibleChanged,
        refresh
    ));
    m_geometryConnections.append(QObject::connect(
        m_panel.data(),
        &QWindow::screenChanged,
        rebuild
    ));
    if (QScreen *const screen = m_panel->screen()) {
        m_geometryConnections.append(QObject::connect(
            screen,
            &QScreen::geometryChanged,
            refresh
        ));
    }

    scheduleRefresh();
}

void PanelAttachmentLease::resolvedSourceLost()
{
    release();
}

void PanelAttachmentLease::disconnectGeometryTracking()
{
    disconnectAll(m_geometryConnections);
}

void PanelAttachmentLease::disconnectLifetimeTracking()
{
    disconnectAll(m_lifetimeConnections);
}

void PanelAttachmentLease::release()
{
    QWindow *const surface = m_surface.data();
    QQuickItem *const source = m_source.data();
    const QString token = m_token;
    m_refreshTimer.stop();
    disconnectGeometryTracking();
    disconnectLifetimeTracking();
    m_panel = nullptr;
    m_surface = nullptr;
    m_output = nullptr;
    m_source = nullptr;
    m_anchor = nullptr;
    m_carrierOriginOnOutput = QPointF();
    m_token.clear();
    m_rebuildPending = false;
    m_refreshing = false;

    if (surface && !token.isEmpty()
        && surface->property(surfaceLeaseTokenProperty).toString() == token) {
        surface->setProperty(surfaceAnchorRectProperty, QRectF());
        surface->setProperty(surfaceLeaseTokenProperty, QString());
    }
    if (source && !token.isEmpty()
        && source->property(sourceFeedbackTokenProperty).toString() == token) {
        source->setProperty(sourceMenuOpenProperty, false);
        source->setProperty(sourceFeedbackTokenProperty, QString());
    }
}
