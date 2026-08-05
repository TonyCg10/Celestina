#pragma once

#include <QByteArray>
#include <QString>
#include <QVariantMap>

// What the aggregate provider helper said, and what the panel may believe.
//
// The helper is a separate process whose output is validated before any of it
// reaches QML: bounds on how many providers there are, how many fields each
// carries and how long a text field may be, because a provider reads from the
// rest of the session and the session is not trusted input. Everything here is
// pure — no process, no Qt event loop — so each rule is testable on its own.
struct ProviderMessage {
    enum class Kind {
        // Unreadable or out of contract; the reason says which.
        Invalid,
        // A complete set of live providers, stamped with its generation.
        Providers,
        // The answer to one command.
        Result,
    };

    Kind kind = Kind::Invalid;
    QString error;

    quint64 generation = 0;
    // Provider id → that provider's fields. `QVariantMap` of maps is what QML
    // can read; the helper owns each payload's shape.
    QVariantMap providers;

    QString requestId;
    QString state;
    QString reason;
};

ProviderMessage parseProviderMessage(const QByteArray &line);

// What a decoded message means for what the host is already showing.
//
// Named and tested because getting it wrong is not visible in a diff: a live
// session lost every reading on the bar — audio, Wi-Fi, Bluetooth, CPU, RAM —
// because one provider published a value this host could not decode and the
// host answered by clearing *everything*, exactly as it does when the helper
// dies. A frame that cannot be read says nothing about the helper being alive,
// and nothing about what the other providers last truthfully reported.
enum class FrameEffect {
    // Adopt this set of provider values.
    Replace,
    // Drop the frame and change nothing. Losing a frame is not losing a helper.
    Ignore,
    // Answer one command; provider state is untouched.
    Answer,
};

FrameEffect effectOf(const ProviderMessage &message);

// The last state each provider published, and the generation it belongs to.
//
// A generation is one helper process. When it changes, nothing from the
// previous one survives: a restarted helper republishes what is true, and
// until it does the panel shows nothing rather than something stale.
class ProviderStates
{
public:
    // Returns whether what QML reads changed.
    bool apply(const ProviderMessage &message);
    // The helper died or went unusable. Returns whether anything was dropped.
    bool clear();

    QVariantMap providers() const { return m_providers; }
    quint64 generation() const { return m_generation; }
    bool isEmpty() const { return m_providers.isEmpty(); }

private:
    QVariantMap m_providers;
    quint64 m_generation = 0;
};
