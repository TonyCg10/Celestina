#pragma once

#include <QPointer>
#include <QQuickItem>

#include "denseglass.h"
#include <QQuickWindow>
#include <QTimer>
#include <QVariantAnimation>
#include <QWindow>

// One closing beat for every contextual surface.
//
// The popup-backed menus always had one: Qt's exit transition and the glass's
// own retire fade run between "put it away" and the host destroying the
// window. Every card menu and overlay instead died on the same instant its
// `dismissed` was emitted — the discrepancy the author heard as "the tray
// animates closed, the rest just vanish" (2026-08-14). The host owns the
// destruction, so the host owns the beat: fade the window's content, then
// run the real close.
//
// The durations mirror the theme's `motionFast` plus a breath; the theme
// itself is QML and this seam deliberately has no engine in reach.
inline void softCloseWindow(QWindow *window, std::function<void()> finish)
{
    // A compositor blur region cannot fade — it exists or it does not — so a
    // closing beat has three instants that can never truly merge: the strong
    // sample's withdrawal, the paint's fade, and the window's death taking
    // the veil sample with it. What can be done is hiding the two hard cuts
    // where the eye cannot separate them: the fade is short and OutCubic, so
    // by its midpoint the paint is already down to about a tenth — the dense
    // withdrawal lands there, under paint too faint to show the swap — and
    // the close lands with the fade's last frame. Withdrawing at the start
    // was tried and inverted the author's complaint: the background left
    // first, through still-opaque cards.
    constexpr int fadeMs = 80;
    constexpr int closeDelayMs = 90;

    auto *quick = qobject_cast<QQuickWindow *>(window);
    QQuickItem *const content = quick ? quick->contentItem() : nullptr;

    // The strong sample collapses toward its own centres for the length of
    // the fade — shrinking under fading paint is the one exit a region that
    // cannot fade can share with the paint above it.
    DenseGlassAggregator::instance().retire(window);
    if (content) {
        auto *fade = new QVariantAnimation(quick);
        fade->setStartValue(content->opacity());
        fade->setEndValue(0.0);
        fade->setDuration(fadeMs);
        fade->setEasingCurve(QEasingCurve::OutCubic);
        const QPointer<QQuickItem> tracked(content);
        QObject::connect(
            fade, &QVariantAnimation::valueChanged, quick,
            [tracked](const QVariant &value) {
                if (tracked)
                    tracked->setOpacity(value.toReal());
            });
        fade->start(QAbstractAnimation::DeleteWhenStopped);
        quick->requestUpdate();
    }

    // Parented to the window: a window hard-closed mid-beat takes the timer
    // with it, and the late `finish` never fires on a corpse.
    QTimer::singleShot(content ? closeDelayMs : 0, window, std::move(finish));
}
