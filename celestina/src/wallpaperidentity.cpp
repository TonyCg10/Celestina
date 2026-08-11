#include "wallpaperidentity.h"

#include <QMetaType>
#include <QVariant>
#include <QVariantList>

#include <cmath>
#include <limits>

namespace {
bool strictString(const QVariant &value, QString *result)
{
    if (value.metaType().id() != QMetaType::QString)
        return false;

    *result = value.toString();
    return true;
}

bool positiveWholeNumber(const QVariant &value, quint64 maximum, quint64 *result)
{
    quint64 candidate = 0;
    switch (value.metaType().id()) {
    case QMetaType::Int: {
        const int number = value.toInt();
        if (number <= 0)
            return false;
        candidate = static_cast<quint64>(number);
        break;
    }
    case QMetaType::UInt:
        candidate = value.toUInt();
        break;
    case QMetaType::LongLong: {
        const qlonglong number = value.toLongLong();
        if (number <= 0)
            return false;
        candidate = static_cast<quint64>(number);
        break;
    }
    case QMetaType::ULongLong:
        candidate = value.toULongLong();
        break;
    case QMetaType::Double: {
        const double number = value.toDouble();
        if (!std::isfinite(number) || number <= 0.0
            || std::floor(number) != number
            || number > static_cast<double>(maximum)) {
            return false;
        }
        candidate = static_cast<quint64>(number);
        break;
    }
    default:
        return false;
    }

    if (candidate == 0 || candidate > maximum)
        return false;
    *result = candidate;
    return true;
}
}

std::optional<WallpaperIdentityReading> wallpaperIdentityForOutput(
    const QVariantMap &providers,
    const QString &output,
    const QSize &geometry
)
{
    if (output.isEmpty())
        return std::nullopt;

    const QVariantMap identity =
        providers.value(QStringLiteral("wallpaper-identity")).toMap();
    const QVariantList rows = identity.value(QStringLiteral("outputs")).toList();
    std::optional<WallpaperIdentityReading> match;
    for (const QVariant &value : rows) {
        const QVariantMap row = value.toMap();
        QString rowOutput;
        if (!strictString(row.value(QStringLiteral("output")), &rowOutput)
            || rowOutput != output) {
            continue;
        }
        // Duplicate rows make identity ambiguous. Refuse the output rather
        // than picking whichever happened to arrive first.
        if (match)
            return std::nullopt;

        WallpaperIdentityReading reading;
        if (!strictString(row.value(QStringLiteral("source")), &reading.source)
            || reading.source.isEmpty()
            || !strictString(
                row.value(QStringLiteral("revision")), &reading.revision
            )
            || reading.revision.isEmpty()) {
            return std::nullopt;
        }

        constexpr quint64 maxJsonInteger = 9007199254740991ULL;
        if (!positiveWholeNumber(
                row.value(QStringLiteral("generation")),
                maxJsonInteger,
                &reading.generation
            )) {
            return std::nullopt;
        }

        const bool rowHasWidth = row.contains(QStringLiteral("width"));
        const bool rowHasHeight = row.contains(QStringLiteral("height"));
        const bool hostHasGeometry = geometry.width() > 0 && geometry.height() > 0;
        if (hostHasGeometry || rowHasWidth || rowHasHeight) {
            quint64 width = 0;
            quint64 height = 0;
            constexpr quint64 maxDimension =
                static_cast<quint64>(std::numeric_limits<int>::max());
            if (!rowHasWidth || !rowHasHeight
                || !positiveWholeNumber(
                    row.value(QStringLiteral("width")), maxDimension, &width
                )
                || !positiveWholeNumber(
                    row.value(QStringLiteral("height")), maxDimension, &height
                )) {
                return std::nullopt;
            }
            reading.geometry =
                QSize(static_cast<int>(width), static_cast<int>(height));
            if (hostHasGeometry && reading.geometry != geometry)
                return std::nullopt;
        }

        match = reading;
    }

    return match;
}
