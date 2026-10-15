#include "quietplacement.h"

#include <QQuickItem>
#include <QQuickWindow>
#include <QScreen>
#include <QVariant>
#include <QWindow>

namespace {
// The same rectangle a `PanelMenuButton` publishes on a click: the item's own
// bounds mapped through every scene transform, including the per-output shell
// factor the panel's scene carries.
QRectF globalItemRect(QQuickItem *item)
{
    if (!item)
        return QRectF();

    const QPointF topLeft = item->mapToGlobal(QPointF(0, 0));
    const QPointF bottomRight =
        item->mapToGlobal(QPointF(item->width(), item->height()));
    return QRectF(
        qMin(topLeft.x(), bottomRight.x()),
        qMin(topLeft.y(), bottomRight.y()),
        qAbs(bottomRight.x() - topLeft.x()),
        qAbs(bottomRight.y() - topLeft.y())
    );
}

QQuickItem *findVisibleByObjectName(QQuickItem *root, const QString &name)
{
    if (!root)
        return nullptr;

    for (QQuickItem *const child : root->childItems()) {
        if (child->objectName() == name && child->isVisible())
            return child;
        if (QQuickItem *const found = findVisibleByObjectName(child, name))
            return found;
    }
    return nullptr;
}
} // namespace

int QuietSurfaceGeometry::topInsetInOutputUnits(double shellScale) const
{
    if (!valid || shellScale <= 0.0)
        return 0;
    return qMax(0, qRound(surface.y() * shellScale));
}

QQuickItem *quietFindVisibleItem(QWindow *window, const QString &objectName)
{
    auto *quickWindow = qobject_cast<QQuickWindow *>(window);
    if (!quickWindow || !quickWindow->isVisible() || objectName.isEmpty())
        return nullptr;
    return findVisibleByObjectName(quickWindow->contentItem(), objectName);
}

qreal panelBarBottomDevice(QWindow *panel)
{
    if (!panel)
        return 0;
    const QVariant stated = panel->property("barHeight");
    bool numeric = false;
    const qreal bar = stated.toReal(&numeric);
    if (!stated.isValid() || !numeric || bar <= 0)
        return qMax(0, panel->height());
    const qreal scale = panel->property("shellScale").toReal();
    return qRound(bar * (scale > 0 ? scale : 1.0));
}

QuietAnchor quietAnchorForIcon(QWindow *panel, const QString &iconObjectName)
{
    auto *quickPanel = qobject_cast<QQuickWindow *>(panel);
    if (!quickPanel || !quickPanel->isVisible() || iconObjectName.isEmpty())
        return QuietAnchor();

    QQuickItem *const icon =
        findVisibleByObjectName(quickPanel->contentItem(), iconObjectName);
    if (!icon)
        return QuietAnchor();

    // The membrane contract wants two rectangles: the exact icon for the
    // mouth's waist, and the control that carries it for the body's centring.
    // A click gets both from `PanelMenuButton`; here the control is the
    // nearest ancestor that declares itself an attachment source.
    QQuickItem *opener = icon->parentItem();
    while (opener
           && !opener->property("isPanelAttachmentSource").toBool()) {
        opener = opener->parentItem();
    }
    if (!opener || !opener->isVisible())
        return QuietAnchor();

    QuietAnchor anchor;
    anchor.icon = globalItemRect(icon);
    anchor.opener = globalItemRect(opener);
    return anchor.valid() ? anchor : QuietAnchor();
}

QuietSurfaceGeometry attachedQuietGeometry(
    const QSizeF &outputSize,
    qreal barHeight,
    const QRectF &opener,
    const QRectF &icon,
    const QSizeF &cardSize,
    qreal inset,
    qreal connectorSlack
)
{
    QuietSurfaceGeometry geometry;
    if (outputSize.width() <= 0 || outputSize.height() <= 0
        || barHeight < 0 || barHeight >= outputSize.height()
        || cardSize.width() <= 0 || cardSize.height() <= 0
        || opener.width() <= 0 || icon.width() <= 0) {
        return geometry;
    }

    // The card centres on the control, clamped inside the output exactly as
    // `PanelPopupPlacement.clampAxis` clamps a menu.
    const qreal desiredX =
        opener.x() + opener.width() / 2 - cardSize.width() / 2;
    const qreal cardX = cardSize.width() + inset * 2 > outputSize.width()
        ? (outputSize.width() - cardSize.width()) / 2
        : qMax(inset, qMin(desiredX,
                           outputSize.width() - cardSize.width() - inset));

    // The card's own top is the seam plus the proportional connector gap the
    // QML computes from its theme tokens. This rectangle is for the window's
    // extent and the occupancy question, so it spans the whole path from the
    // seam: an intrusion into the falling drop's travel counts as occupied.
    geometry.card = QRectF(
        cardX,
        barHeight,
        cardSize.width(),
        connectorSlack + cardSize.height()
    );

    const qreal left =
        qMax<qreal>(0, qMin(cardX, icon.x()) - inset);
    geometry.surface = QRectF(
        left,
        barHeight,
        outputSize.width() - left,
        qMin(outputSize.height() - barHeight,
             connectorSlack + cardSize.height() + inset)
    );
    geometry.valid = true;
    return geometry;
}

bool quietZoneOccupied(
    const QRectF &prospectiveCard,
    const QList<QRectF> &openCards
)
{
    if (prospectiveCard.isEmpty())
        return false;

    for (const QRectF &card : openCards) {
        if (!card.isEmpty() && card.intersects(prospectiveCard))
            return true;
    }
    return false;
}

QRectF quietOpenCardRect(QWindow *window, QScreen *screen)
{
    if (!window || !screen || !window->isVisible() || window->screen() != screen)
        return QRectF();

    bool haveX = false;
    bool haveY = false;
    bool haveWidth = false;
    bool haveHeight = false;
    const qreal x = window->property("cardX").toReal(&haveX);
    const qreal y = window->property("cardY").toReal(&haveY);
    const qreal width = window->property("cardWidth").toReal(&haveWidth);
    const qreal height = window->property("cardHeight").toReal(&haveHeight);
    if (!haveX || !haveY || !haveWidth || !haveHeight
        || width <= 0 || height <= 0) {
        return QRectF();
    }
    return QRectF(x, y, width, height);
}

QString osdIconObjectName(const QString &kind)
{
    if (kind == QLatin1String("volume"))
        return QStringLiteral("celestina-volume-icon");
    if (kind == QLatin1String("microphone"))
        return QStringLiteral("celestina-mic-icon");
    if (kind == QLatin1String("brightness"))
        return QStringLiteral("celestina-brightness-button-icon");
    return QString();
}

bool osdSuppressedByOpenMenu(const QString &kind, const QString &openIndicator)
{
    if (openIndicator == QLatin1String("audio")) {
        return kind == QLatin1String("volume")
            || kind == QLatin1String("microphone");
    }
    if (openIndicator == QLatin1String("brightness"))
        return kind == QLatin1String("brightness");
    return false;
}
