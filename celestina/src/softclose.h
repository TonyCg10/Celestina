#pragma once

#include <QPointer>

#include <functional>
#include <memory>
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
    // Revive alone, deliberately without the reveal. A fresh window reveals
    // from `Component.onCompleted`, which never runs again on a resumed
    // carrier, so each route owes its own re-reveal — and it owes it *after*
    // its attachment is re-established. Revealed here, the field woke while
    // the released lease still had the anchor empty, believed itself a
    // floating card, and published the full settled rectangle for one beat:
    // the author's black empty card flashing detached from the bar
    // (2026-08-21 13:19, the tall-then-small `blur.armed` pairs). The
    // overlay route re-reveals through its controller's presented-frame
    // gate; the indicator route re-reveals after its lease reacquires.
    for (QQuickItem *const field : fields)
        QMetaObject::invokeMethod(field, "reviveForReuse");
}

// The resumed route's reveal, issued only once the attachment is really
// re-established. One deferred turn was not enough: the lease republishes
// the anchor through its own asynchronous refresh, and a reveal that beat it
// woke the field with the anchor still empty — it believed itself a floating
// card and published the full settled rectangle for a beat, the tall-then-
// small `blur.armed` pairs on every resumed open (2026-08-21 14:22). So the
// reveal now waits for the anchor an anchored route owes, in short beats
// with a bounded patience: a route that is genuinely floating — no panel,
// an ambiguous source, a lease that failed — reveals when the patience runs
// out, exactly as the floating contract always has.
inline void revealResumedWindow(QWindow *window)
{
    if (!window)
        return;
    const QPointer<QWindow> tracked(window);
    auto attempt = std::make_shared<std::function<void(int)>>();
    *attempt = [tracked, attempt](int remaining) {
        if (!tracked || tracked->property("celestinaParked").toBool()
            || tracked->property("celestinaRetiring").toBool()) {
            return;
        }
        const bool anchorPending =
            tracked->property("anchoredFromPanel").toBool()
            && tracked->property("attachmentAnchorRect").toRectF().isEmpty();
        if (anchorPending && remaining > 0) {
            QTimer::singleShot(16, tracked.data(), [attempt, remaining]() {
                (*attempt)(remaining - 1);
            });
            return;
        }
        // Each family presents its own way. The popup-backed menus replay
        // their popup, whose aboutToShow carries the reveal exactly as a
        // fresh open; the card family reveals its fields directly.
        if (tracked->metaObject()->indexOfMethod("reopenForReuse()") >= 0) {
            QMetaObject::invokeMethod(tracked.data(), "reopenForReuse");
            return;
        }
        const auto fields = tracked->findChildren<QQuickItem *>(
            QStringLiteral("celestina-soft-menu-field"));
        for (QQuickItem *const field : fields)
            QMetaObject::invokeMethod(field, "reveal");
    };
    QTimer::singleShot(0, window, [attempt]() { (*attempt)(12); });
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
