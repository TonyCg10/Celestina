#pragma once

#include <QHash>
#include <QString>
#include <QVariantMap>

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

    // The one reading worth showing for this publication, if any.
    //
    // Several capabilities can change in the same frame; the display shows one
    // thing at a time, so the order is fixed and documented rather than left
    // to whichever key of a map came first: volume, then the microphone, then
    // the monitor that changed.
    std::optional<Reading> apply(const QVariantMap &providers);

    // The helper went away or restarted. Whatever it publishes next is a
    // baseline again, not a change somebody made.
    void forget();

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
