#pragma once

#include <QHash>
#include <QList>
#include <QObject>
#include <QPointer>
#include <QRectF>
#include <QTimer>
#include <QWindow>

class QQuickWindow;
class QScreen;

// The dense sections' own compositor blur, one strength above the veil's.
//
// The author's material (2026-08-14) wants two blurs in one card: the
// colourless veil keeps the session's slight sample, and only the dark
// content sections summarize the colours behind them — Mica's recipe. The
// compositor applies one blur strength per surface, so a second strength
// needs a second surface: this aggregator keeps one invisible companion
// layer per output, collects every dark section of every contextual surface
// on it, and asks the compositor to blur exactly those rectangles at the
// strength the `celestina-dense-glass` layer rule names.
//
// The companion lives on the *top* layer while every publisher lives on the
// overlay layer above it. That ordering is load-bearing: a same-layer
// companion would stack by map order, and one mapped after a menu would blur
// the menu's own painted rows instead of the backdrop behind them. The one
// surface deliberately excluded is the panel — it shares the top layer, so
// the companion cannot be guaranteed beneath it.

// One dark section, in output-local coordinates.
struct DenseGlassShape
{
    QRectF rect;
    qreal radius = 0;
};

// Where a layer surface sits on its output, derived from the anchors and
// margins it was mapped with — the same numbers the compositor positions it
// by. A free function so a regression can pin the arithmetic without a
// compositor.
QPointF layerSurfaceOriginOnOutput(
    int anchors,
    const QMargins &margins,
    const QSizeF &windowSize,
    const QSizeF &outputSize
);

// Every visible dark section in a window's scene, in window coordinates.
QList<DenseGlassShape> collectDenseSections(QQuickWindow *window);

class DenseGlassAggregator final : public QObject
{
    Q_OBJECT

public:
    static DenseGlassAggregator &instance();

    // Replaces `source`'s sections, already translated to output-local
    // coordinates. An empty list is a real statement — the sections left —
    // and keeps the source registered for cheap re-publication.
    void publish(QWindow *source, const QList<DenseGlassShape> &shapes);
    void withdraw(QWindow *source);
    // The closing gesture's exit: the source's shapes collapse toward their
    // own centres over the fade's length, republished every tick, and only
    // then leave. A region cannot fade, but it can shrink — and a block that
    // shrinks under fading paint reads as one movement, where a block that
    // vanished in a single step stood naked for the frames the paint had
    // already left (the author's recording, 2026-08-14).
    void retire(QWindow *source);

private:
    explicit DenseGlassAggregator(QObject *parent = nullptr);
    void refresh(QScreen *screen);
    QList<QPointer<QQuickWindow>> companionsFor(QScreen *screen);
    void pulse();

    struct Source
    {
        QPointer<QWindow> window;
        QPointer<QScreen> screen;
        QList<DenseGlassShape> shapes;
    };

    QHash<QWindow *, Source> m_sources;
    // Several companions per output, stacked. Each one blurs what is already
    // below it, so N of them compose N samples over the same rectangles.
    //
    // The strength itself comes from Celestina's own compositor patch, in
    // `packaging/niri/`, which lets a layer rule name the blur's passes and
    // offset. Stacking alone was measured as the way to need no patch and
    // rejected on the numbers: blur radius grows with the square root of the
    // sample count, so matching that strength from the session's slight
    // profile would take about twenty-five surfaces. The depth kept here is
    // what the patched strength wants under it, and on an unpatched niri it
    // is also all there is — the shell still runs, with a plainer material.
    QHash<QScreen *, QList<QPointer<QQuickWindow>>> m_companions;
    // The companion's pulse. Effect state is double-buffered and rides the
    // next commit, and a window whose scene never changes stops committing —
    // the measured lesson of every quiet surface. While anything is armed
    // the companions are kept committing, and for a few beats after the last
    // withdrawal, so the disable itself always lands; without those beats an
    // armed region survived its menu as a ghost of blurred rectangles.
    QTimer m_pulse;
    int m_quietBeats = 0;
};
