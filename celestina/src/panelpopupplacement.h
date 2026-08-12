#pragma once

#include <QPoint>
#include <QPointF>
#include <QRect>
#include <QRectF>
#include <QtMath>
#include <QtGlobal>

// Pure geometry shared by panel menus and panel-opened overlays. Placement
// follows only the invoking control's real rectangle, so the menu remains tied
// to its button regardless of the carrier window's geometry.
inline QRect panelPopupOpenerOnOutput(
    const QRect &globalOpener,
    const QPoint &outputOrigin
)
{
    return globalOpener.translated(-outputOrigin);
}

inline QRectF panelPopupOpenerOnOutput(
    const QRectF &globalOpener,
    const QPointF &outputOrigin
)
{
    return globalOpener.translated(-outputOrigin);
}

inline QPoint panelPopupBodyOrigin(
    const QRectF &openerOnOutput,
    int bodyWidth,
    int anchorGap,
    int attachmentStartY = -1
)
{
    const int centredX = qRound(
        openerOnOutput.x() + openerOnOutput.width() / 2.0
        - qreal(bodyWidth) / 2.0
    );
    return QPoint(
        centredX,
        attachmentStartY >= 0
            ? attachmentStartY + qMax(0, anchorGap)
            : qCeil(openerOnOutput.bottom()) + qMax(0, anchorGap)
    );
}

// The visible menu body is centred on its opener and begins after the one
// semantic floating-control gap.
inline QPoint panelPopupBodyOrigin(
    const QRect &openerOnOutput,
    int bodyWidth,
    int anchorGap,
    int attachmentStartY = -1
)
{
    const int centredX = qRound(
        qreal(openerOnOutput.x()) + qreal(openerOnOutput.width()) / 2.0
        - qreal(bodyWidth) / 2.0
    );
    return QPoint(
        centredX,
        attachmentStartY >= 0
            ? attachmentStartY + qMax(0, anchorGap)
            : openerOnOutput.bottom() + 1 + qMax(0, anchorGap)
    );
}
