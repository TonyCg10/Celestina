#pragma once

#include <QObject>

// What happens between PAM saying yes and the session being uncovered.
//
// There is a gap there on purpose. The covers put the backdrop back to the
// geometry the session's own wallpaper already occupies, so the compositor
// reveals the session on a frame that matches it rather than cutting to it.
// That gap is the whole of this class, and its shape is chosen around one
// hazard.
//
// The hazard: if uncovering waited for the retreat to report itself finished,
// then an animation that stalls, a cover that stops rendering or a signal that
// never arrives would keep a person who typed the correct passphrase out of
// their own machine. A decoration would have become the gatekeeper. So the
// retreat is told to play and is never asked about it: `uncover` is emitted by
// a timer that starts the moment `begin` is called and answers to nothing else.
//
// The direction of every failure here is toward staying locked slightly longer,
// which is the direction `ADR 0004` cares about. Nothing in this class can
// uncover a session that was not authenticated — it has no verdict, no PAM and
// no lock session of its own; it only knows how long to wait once somebody who
// does have those has decided.
class LockUncover final : public QObject
{
    Q_OBJECT

public:
    // `ceilingMs` is how long the session stays covered after authentication.
    // It should be at least as long as the retreat it is covering for; a
    // shorter value costs a visible seam and never a lockout.
    explicit LockUncover(int ceilingMs, QObject *parent = nullptr);

    // Starts the sequence. Emits `retreat` immediately and `uncover` once the
    // ceiling has elapsed. Calling it again does nothing at all: a second
    // verdict, a repeated signal or a retried unlock cannot restart the clock,
    // shorten it, or produce a second uncovering.
    void begin();

    bool hasBegun() const { return m_begun; }

signals:
    // Put the session back where it will be once this cover is gone.
    void retreat();
    // Release the lock. Emitted exactly once per process, by the timer.
    void uncover();

private:
    int m_ceilingMs;
    bool m_begun = false;
};
