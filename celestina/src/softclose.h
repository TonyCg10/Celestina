#pragma once

#include <QPointer>
#include <QQuickItem>

#include "denseglass.h"
#include <KWindowEffects>

#include "blurreach.h"
#include <QQuickWindow>
#include <QTimer>
#include <QVariantAnimation>
#include <QWindow>

// One closing beat for every contextual surface.
//
// The host owns destruction, so it owns this beat for popup menus, card menus,
// overlays and prompts alike. Popup-backed menus notify it from aboutToHide:
// their rows remain alive for the same field retirement instead of completing
// a private popup exit first and starting this host fade afterwards.
//
// The durations mirror the theme's `motionFast` plus a breath; the theme
// itself is QML and this seam deliberately has no engine in reach.
// The reuse companion to the beat below (SURF-1): a parked carrier is
// brought back for its next open. The completed retirement comes down, the
// content item the fade took to zero paints again, and every field clears
// its terminal edge for a fresh reveal. The caller owns everything else —
// route properties, readiness trackers and the surface's own resume.
inline void reviveSoftClosedWindow(QWindow *window)
{
    if (!window)
        return;
    window->setProperty("celestinaRetiring", false);
    if (auto *quick = qobject_cast<QQuickWindow *>(window)) {
        if (QQuickItem *const content = quick->contentItem())
            content->setOpacity(1.0);
    }
    const auto fields = window->findChildren<QQuickItem *>(
        QStringLiteral("celestina-soft-menu-field"));
    for (QQuickItem *const field : fields)
        QMetaObject::invokeMethod(field, "reviveForReuse");
}

inline void softCloseWindow(QWindow *window, std::function<void()> finish)
{
    if (!window || window->property("celestinaRetiring").toBool())
        return;
    // The same close can be observed through Popup.aboutToHide, a layer-shell
    // dismissal and a repeated toggle in one event turn. The first request
    // owns the beat and its finish callback; later observations may not start
    // another animation or move the destruction deadline.
    window->setProperty("celestinaRetiring", true);

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
    // `motionFast` in the theme; the QML retire animations below run at that
    // token, and this fade must not outlive them nor cut them short.
    constexpr int fadeMs = 150;
    constexpr int closeDelayMs = 170;
    const bool reducedMotion = window->property("reducedMotion").toBool();

    auto *quick = qobject_cast<QQuickWindow *>(window);
    QQuickItem *const content = quick ? quick->contentItem() : nullptr;

    // The universal departure: every menu field aboard this window shrinks
    // into the screen while it fades — the same `retire()` the popup menus
    // already ran on `aboutToHide`. Invoked here so the card overlays, the
    // panel menus and the prompt all leave the same way, glass and content as
    // one block, instead of only fading.
    if (content) {
        // From the window, not its contentItem: the field is a QObject
        // descendant of the window, and a search rooted at the contentItem
        // finds nothing — measured, not assumed.
        const auto fields = window->findChildren<QQuickItem *>(
            QStringLiteral("celestina-soft-menu-field"));
        for (QQuickItem *const field : fields)
            QMetaObject::invokeMethod(field, "retire");
    }

    // The strong sample collapses toward its own centres for the length of
    // the fade — shrinking under fading paint is the one exit a region that
    // cannot fade can share with the paint above it.
    if (reducedMotion)
        DenseGlassAggregator::instance().withdraw(window);
    else
        DenseGlassAggregator::instance().retire(window);
    if (content) {
        if (reducedMotion) {
            content->setOpacity(0.0);
        } else {
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
        }
        quick->requestUpdate();
    }

    // The compositor's blur region cannot fade, but it can leave early:
    // withdrawn a third of the way into the fade, the milky backdrop
    // disappears under paint still opaque enough to cover the swap, instead
    // of sitting bare on the wallpaper after the paint has gone.
    if (content && !reducedMotion) {
        const QPointer<QWindow> tracked(window);
        QTimer::singleShot(60, window, [tracked]() {
            // Only while this very retirement is still the window's state. A
            // reused carrier can be revived between this timer's scheduling
            // and its firing — the revival clears the retiring mark — and a
            // withdraw landing then strips the blur off a menu the person
            // just reopened.
            if (tracked && tracked->property("celestinaRetiring").toBool()) {
                withdrawBlur(tracked.data());
                tracked->requestUpdate();
            }
        });
    } else if (content) {
        withdrawBlur(window);
    }

    // Parented to the window: a window hard-closed mid-beat takes the timer
    // with it, and the late `finish` never fires on a corpse.
    QTimer::singleShot(
        content && !reducedMotion ? closeDelayMs : 0,
        window,
        std::move(finish));
}
