#include "osdreadings.h"

namespace {
// A level the provider did not report. Kept apart from 0, which is a device
// that really is at the bottom of its range.
constexpr int noLevel = -1;

int levelOf(const QVariantMap &values, const QString &key)
{
    const QVariant value = values.value(key);
    bool numeric = false;
    const int level = value.toInt(&numeric);
    return numeric ? level : noLevel;
}
} // namespace

bool OsdReadings::Reading::operator==(const Reading &other) const
{
    return kind == other.kind && percent == other.percent
        && muted == other.muted && label == other.label;
}

void OsdReadings::forget()
{
    m_audio = Audio {};
    m_brightness.clear();
    m_brightnessKnown = false;
}

std::optional<OsdReadings::Reading> OsdReadings::apply(
    const QVariantMap &providers
)
{
    std::optional<Reading> reading;

    const QVariantMap audio = providers.value(QStringLiteral("audio")).toMap();
    if (!audio.isEmpty()) {
        const Audio published {
            levelOf(audio, QStringLiteral("volume")),
            levelOf(audio, QStringLiteral("micVolume")),
            audio.value(QStringLiteral("muted")).toBool(),
            audio.value(QStringLiteral("micMuted")).toBool(),
            true,
        };

        if (m_audio.known) {
            if (published.volume != m_audio.volume
                || published.muted != m_audio.muted) {
                reading = Reading {
                    QStringLiteral("volume"),
                    published.volume,
                    published.muted,
                    QString(),
                };
            } else if (published.micVolume != m_audio.micVolume
                       || published.micMuted != m_audio.micMuted) {
                reading = Reading {
                    QStringLiteral("microphone"),
                    published.micVolume,
                    published.micMuted,
                    QString(),
                };
            }
        }
        m_audio = published;
    } else {
        // The device went away. Whatever comes back is a baseline, not a
        // change: a widget that left and returned announced nothing.
        m_audio = Audio {};
    }

    const QVariantMap brightness =
        providers.value(QStringLiteral("brightness")).toMap();
    QHash<QString, int> levels;
    std::optional<Reading> monitor;
    for (auto entry = brightness.constBegin(); entry != brightness.constEnd();
         ++entry) {
        bool numeric = false;
        const int level = entry.value().toInt(&numeric);
        if (!numeric) {
            // A monitor that speaks DDC but has not answered is unknown, and
            // unknown is never shown.
            continue;
        }

        levels.insert(entry.key(), level);
        const auto previous = m_brightness.constFind(entry.key());
        if (!m_brightnessKnown || previous == m_brightness.constEnd()
            || previous.value() == level) {
            continue;
        }
        // Several monitors changing at once is not a session this shell is
        // built around; showing the first by name is honest and bounded.
        if (!monitor || entry.key() < monitor->label) {
            monitor = Reading {
                QStringLiteral("brightness"),
                level,
                false,
                entry.key(),
            };
        }
    }

    m_brightness = levels;
    m_brightnessKnown = !levels.isEmpty();
    if (!reading)
        reading = monitor;

    return reading;
}
