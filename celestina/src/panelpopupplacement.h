#pragma once

#include <QPoint>
#include <QPointF>
#include <QRect>
#include <QRectF>
#include <QRegion>
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

// The input region an output-covering contextual surface takes: everything it
// covers except the strip the panel reserved for itself.
//
// Covering the output is what lets one click outside a menu retire it, which
// is why these surfaces are the size of the screen. It also made them swallow
// every click on the bar, and that is the defect the author reported: with one
// menu up, clicking a different opener only dismissed the first, so a second
// click was needed to open the second — the shell looked like it refused to
// swap menus. Leaving the panel's own strip out of the region sends that click
// to the bar, where the opener publishes its request and the host retires the
// old menu and maps the new one in the same gesture. Everywhere else the
// surface still hears the click and retires; the bar answers for its own strip
// through its background, which asks for the same dismissal a click on the
// desktop would have caused.
inline QRegion panelPopupInputRegion(int windowWidth, int windowHeight, int seam)
{
    const int top = qBound(0, seam, windowHeight);
    return QRegion(0, top, qMax(1, windowWidth), qMax(1, windowHeight - top));
}
