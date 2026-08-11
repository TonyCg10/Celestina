#pragma once

#include <QPoint>
#include <QRect>
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

// The visible menu body is centred on its opener and begins after the one
// semantic floating-control gap.
inline QPoint panelPopupBodyOrigin(
    const QRect &openerOnOutput,
    int bodyWidth,
    int anchorGap
)
{
    return QPoint(
        openerOnOutput.center().x() - bodyWidth / 2,
        openerOnOutput.bottom() + 1 + qMax(0, anchorGap)
    );
}
