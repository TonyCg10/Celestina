import QtQuick

// Source of truth: celestina-rs/crates/cuprita-core/src/fake.rs. Every row
// and field here mirrors its `scripted()` state (in snapshot order); change
// that file first and this one to match.
// A QML stand-in for AudioController and its models over FakeAudio's
// scripted graph: speakers (default) and HDMI out, one microphone, two
// applications and a card with two profiles.
QtObject {
    id: fake

    // Never busy: the stand-in answers every command at once.
    property bool busy: false
    // The first snapshot has arrived. The stand-in starts with it, like the
    // fakes; a test may set it false and call `snapshot()` to deliver it.
    property bool loaded: true
    function snapshot() { loaded = true }

    property var calls: []
    property var profiles: [
        { cardId: 30, id: "output:analog-stereo", description: "Analog Stereo", active: true },
        { cardId: 30, id: "output:hdmi-stereo", description: "HDMI Stereo", active: false }
    ]

    // EndpointModel stand-ins: rows plus the default's summary.
    property ListModel sinks: ListModel {
        property int defaultId: 40
        property string defaultDescription: "Speakers"
        property int defaultPercent: 60
        property bool defaultMuted: false
    }
    property ListModel sources: ListModel {
        property int defaultId: 50
        property string defaultDescription: "Microphone"
        property int defaultPercent: 60
        property bool defaultMuted: false
    }
    property ListModel streams: ListModel { }

    function record(call) { fake.calls = fake.calls.concat([call]) }

    function summarise(model) {
        for (let i = 0; i < model.count; ++i) {
            const e = model.get(i)
            if (e.isDefault) {
                model.defaultId = e.id
                model.defaultDescription = e.description
                model.defaultPercent = e.percent
                model.defaultMuted = e.muted
            }
        }
    }

    // The endpoint model holding `id`: the sinks or the sources.
    function owner(id) {
        for (let i = 0; i < fake.sources.count; ++i)
            if (fake.sources.get(i).id === id)
                return fake.sources
        return fake.sinks
    }

    function setDefault(id) {
        record("setDefault:" + id)
        const model = owner(id)
        for (let i = 0; i < model.count; ++i)
            model.setProperty(i, "isDefault", model.get(i).id === id)
        summarise(model)
    }
    function setVolume(id, volume) { record("setVolume:" + id + ":" + volume) }
    function setMuted(id, muted) {
        record("setMuted:" + id + ":" + muted)
        const model = owner(id)
        for (let i = 0; i < model.count; ++i)
            if (model.get(i).id === id)
                model.setProperty(i, "muted", muted)
        summarise(model)
    }
    function setProfile(cardId, profile) { record("setProfile:" + cardId + ":" + profile) }

    Component.onCompleted: {
        // Rows with an `id` role cannot be ListElements, so they are appended.
        fake.sinks.append({ id: 40, kind: "sink", name: "alsa_output.analog-stereo",
                            description: "Speakers", volume: 0.6, percent: 60,
                            muted: false, isDefault: true })
        fake.sinks.append({ id: 41, kind: "sink", name: "alsa_output.hdmi-stereo",
                            description: "HDMI", volume: 0.6, percent: 60,
                            muted: false, isDefault: false })
        fake.sources.append({ id: 50, kind: "source", name: "alsa_input.analog-stereo",
                              description: "Microphone", volume: 0.6, percent: 60,
                              muted: false, isDefault: true })
        fake.streams.append({ id: 60, appName: "Music", appIcon: "audio-x-generic",
                              volume: 1.0, percent: 100, muted: false })
        fake.streams.append({ id: 61, appName: "Browser", appIcon: "web-browser",
                              volume: 1.0, percent: 100, muted: false })
    }
}
