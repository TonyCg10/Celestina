#pragma once

#include <QList>
#include <QRectF>
#include <QSizeF>
#include <QString>

class QScreen;
class QWindow;

// Where a quiet surface — the on-screen display, the toast stack — goes, and
// when it retreats.
//
// Both default to the top-right, attached to the bar by the same membrane the
// menus use, with the mouth on the panel icon of the thing they report. Both
// yield the zone rather than paint over something interactive that is already
// there: the display retreats to the bottom-right corner, the toasts to the
// bottom centre — two different fallbacks so the two retreats cannot collide
// with each other either. The decisions are free functions over rectangles so
// a regression can pin them without a compositor.

// The panel icon a quiet surface attaches to, and the control that carries it.
// Both rectangles are global pixels, exactly what a `PanelMenuButton` click
// would have published; empty when the panel has no such icon to offer.
struct QuietAnchor
{
    QRectF opener;
    QRectF icon;

    bool valid() const
    {
        return opener.width() > 0 && opener.height() > 0
            && icon.width() > 0 && icon.height() > 0;
    }
};

// Resolve the icon named `iconObjectName` inside a mapped panel window, and
// the nearest enclosing control that declares itself a panel attachment
// source. No click is involved: a reading changes without one, so the host
// asks the panel for the geometry a click would have reported.
QuietAnchor quietAnchorForIcon(QWindow *panel, const QString &iconObjectName);

// The window and card geometry for one attached quiet surface, all in
// output-local shell units. The card centres on the opener exactly as
// `PanelPopupPlacement` centres a menu; the window spans from the leftmost
// thing it must contain — the card or the icon whose mouth it draws — to the
// output's right edge, because a mouth outside its own window is a mouth the
// compositor clips.
struct QuietSurfaceGeometry
{
    // The layer surface: anchored top-right, so only its size and left edge
    // matter. Its top is the panel's lower seam, making that seam y = 0 in
    // every QML item the carrier owns.
    QRectF surface;
    // Where the card would land, used both to place it and to ask whether
    // something else is already there. Its height includes the connector
    // travel, so an intrusion into the membrane's path counts as occupied.
    QRectF card;
    bool valid = false;

    // Translate output-local shell geometry into this carrier's local QML
    // coordinate space. Callers retain output-local `card` for occupancy
    // comparisons and pass only this translated form to the surface.
    QRectF onSurface(const QRectF &outputRect) const
    {
        return valid ? outputRect.translated(-surface.topLeft()) : QRectF();
    }

    // Layer-shell consumes output/Qt units while `surface` uses the unscaled
    // QML units above. Convert the carrier's top exactly once at that boundary.
    int topInsetInOutputUnits(double shellScale) const;
};

QuietSurfaceGeometry attachedQuietGeometry(
    const QSizeF &outputSize,
    qreal barHeight,
    const QRectF &opener,
    const QRectF &icon,
    const QSizeF &cardSize,
    qreal inset,
    qreal connectorSlack
);

// Whether anything in `openCards` already intrudes where this card would land.
bool quietZoneOccupied(const QRectF &prospectiveCard, const QList<QRectF> &openCards);

// The card of one open contextual surface, in output-local shell units: the
// root's own `cardX`/`cardY`/`cardWidth`/`cardHeight`, which every anchored
// card and overlay declares on an output-covering window. Empty when the
// window is not on `screen`, not visible, or declares no card.
QRectF quietOpenCardRect(QWindow *window, QScreen *screen);

// The panel icon that names one on-screen display kind, or empty for a kind
// the panel has no control for. The host owns this vocabulary exactly as the
// display itself does.
QString osdIconObjectName(const QString &kind);

// A level being changed from inside its own open menu needs no display: the
// menu's slider is already showing it. The audio menu owns both the volume
// and the microphone; the brightness menu owns brightness; every other open
// menu changes nothing about a level, so its display still appears.
bool osdSuppressedByOpenMenu(const QString &kind, const QString &openIndicator);
