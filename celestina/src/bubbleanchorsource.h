#pragma once

#include <QRectF>
#include <QString>

// What a surface-less minimize needs in order to travel somewhere, and nothing else.
//
// Narrow on purpose, in the same spirit as `RequestSink`. The shell service does not want a
// panel manager: it wants one rectangle and one preference. Keeping it to that lets the
// service's own contract — availability, exact ids, degradation when there is no anchor — be
// driven without a QML engine, a screen, or a mapped panel.
class BubbleAnchorSource
{
public:
    virtual ~BubbleAnchorSource() = default;

    // Where that output's bubbles currently sit, in the compositor's output-local logical
    // coordinates, or an empty rectangle when it has no mapped panel to ask.
    virtual QRectF bubbleAnchorFor(const QString &outputName) const = 0;

    // The session's motion preference. Reduced motion wins over any anchor.
    virtual bool reducedMotion() const = 0;
};
