#pragma once

#include <QRegion>
#include <QWindow>

#include <KWindowEffects>

// Whether this window's compositor blur can still be spoken to, and the two
// ways of speaking to it.
//
// The distinction this exists for: a live `QWindow` is not a live Wayland
// surface. Qt destroys the platform window — and with it the `wl_surface` —
// the moment a window hides, while the C++ object survives for as long as
// anything holds it. A `QPointer` stays non-null across exactly that gap.
//
// `KWindowEffects::enableBlurBehind` becomes
// `ext_background_effect_surface_v1.set_blur_region`, and a compositor answers
// that request on a destroyed surface with a *fatal* protocol error: not a
// warning, not a no-op — the whole shell is disconnected, every output at
// once. That is the crash that ended the first two live migrations, and it
// needs nothing more exotic than a menu hiding while a blur timer was queued.
//
// So both directions go through here. Withdrawing is refused as firmly as
// arming, because there is nothing to withdraw from a surface the compositor
// has already forgotten — the effect died with it.
// `handle()` alone is not the test, and getting that wrong cost a third live
// crash. Qt Wayland destroys the `wl_surface` when a window hides but keeps its
// `QPlatformWindow` alive, so `handle()` stays non-null across exactly the gap
// this needs to detect. Visibility is what tracks the surface: hidden means the
// compositor no longer has anything to talk to, and the measured failure was a
// withdraw sent to a menu that had just hidden —
// `ext_background_effect_surface_v1: error 0: wl_surface was destroyed`.
//
// Withdrawing therefore has to happen *before* a window hides, never after.
// After is not merely useless, it is fatal.
inline bool blurReachable(const QWindow *window)
{
    return window != nullptr && window->handle() != nullptr
        && window->isVisible();
}

// Withdrawing is the direction that kills. It only ever means "the compositor
// is still showing this and should stop", so a hidden window has nothing to
// withdraw from and asking is the fatal request measured above. The armed state
// died with the surface; the caller resets its own bookkeeping either way.
//
// The withdrawal itself arms one pixel rather than disabling, and that is the
// second crash this header carries the scar of. KWindowSystem 6.29's disable
// path tears down the armed effect *and its cleanup watchers*, then creates a
// fresh `ext_background_effect_surface_v1` just to send the empty region — an
// orphan no watcher will ever clean. Its cache is keyed by `QWindow *`, so
// once the withdrawn window died, the next window the allocator placed at the
// same address inherited the orphan, and its first honest arm became
// `set_blur_region` on a surface the compositor had destroyed — the fatal
// error that killed the shell when a workspace-map click followed a parked
// menu's teardown (2026-08-21, `wldebug` capture: effect #89, born in a
// withdraw, never destroyed, re-armed one second after its surface died). A
// one-pixel arm keeps the effect owned and watched: the enable path installs
// the destroy watchers, and the window's death then really cleans the entry.
inline void withdrawBlur(QWindow *window)
{
    if (!blurReachable(window))
        return;
    KWindowEffects::enableBlurBehind(window, true, QRegion(0, 0, 1, 1));
}

// Arming is deliberately *not* gated on visibility, and that asymmetry is load
// bearing. KWindowSystem caches the region and applies it when the surface is
// next exposed, which is what lets a surface be armed before it is shown —
// `DenseGlassAggregator` depends on exactly that order, because showing a
// companion before its region exists hands the compositor a mapped surface with
// no region, and its per-namespace rule then saturates the whole output. Gating
// this too would have reintroduced that defect while fixing the crash.
inline void armBlur(QWindow *window, const QRegion &region)
{
    if (window == nullptr)
        return;
    KWindowEffects::enableBlurBehind(window, true, region);
}
