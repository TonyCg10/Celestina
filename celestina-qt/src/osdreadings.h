#pragma once

#include <QHash>
#include <QString>
#include <QVariantList>
#include <QVariantMap>

#include <QList>

#include <optional>

// What the on-screen display has been given a reason to show.
//
// The OSD is driven by readings, never by requests. A session verb is answered
// on the bus; what appears over the wallpaper is what a provider afterwards
// said the device is at — so a key that did nothing raises nothing, and a
// change made by the panel's wheel, another application or a monitor's own
// buttons raises the same display as a key binding does.
//
// The first value each capability publishes is a baseline and shows nothing: a
// shell that popped an OSD for every provider at startup would be announcing
// that it had started, not that anything changed.
//
// Pure policy over the map the helper published — no window, no timer, no Qt
// event loop — so each rule is testable on its own.
//
// The display keeps a small stack of live readings — the card file: a volume
// change while a brightness card is still up adds a second card rather than
// overwriting the first, because the overwritten number was information
// someone was reading. The two functions below own that list's shape; the
// controller owns only the per-kind clocks.
class OsdReadings
{
public:
    struct Reading {
        // Which capability this is about: `volume`, `microphone` or
        // `brightness`. The presentation layer decides what each looks like.
        QString kind;
        // Whole percent, or -1 when the provider reported no level. A display
        // with no level shows a state, never a bar at zero.
        int percent = -1;
        bool muted = false;
        // Which device the reading belongs to, when there is more than one of
        // them — the connector name for a monitor. Empty otherwise.
        QString label;

        bool operator==(const Reading &other) const;
    };

    // Every reading worth showing for this publication, in a fixed and
    // documented order — volume, then the microphone, then the monitor that
    // changed — rather than whichever key of a map came first. It is a list
    // because the display is a card file now: a volume and a brightness that
    // changed in the same frame are two cards, and returning only the first
    // silently swallowed the second, which is why one command that moved both
    // raised one display.
    QList<Reading> apply(const QVariantMap &providers);

    // The helper went away or restarted. Whatever it publishes next is a
    // baseline again, not a change somebody made.
    void forget();

    // `readings` with this reading at the front: a kind already on the list is
    // updated and moved forward — its card is the news again — never
    // duplicated. Each entry is the map the QML consumes.
    static QVariantList merged(const QVariantList &readings, const Reading &reading);

    // `readings` without the named kinds: what expiring and suppression both
    // do to the list.
    static QVariantList without(const QVariantList &readings, const QStringList &kinds);

private:
    struct Audio {
        int volume = -1;
        int micVolume = -1;
        bool muted = false;
        bool micMuted = false;
        bool known = false;
    };

    Audio m_audio;
    // Connector name → last level published for it. A monitor that has not
    // answered yet is absent rather than stored as a number.
    QHash<QString, int> m_brightness;
    bool m_brightnessKnown = false;
};
