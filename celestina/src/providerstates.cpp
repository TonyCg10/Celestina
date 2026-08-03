#include "providerstates.h"

#include <cmath>

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QJsonValue>
#include <QVariantList>

namespace {
// The only protocol version that exists. A helper announcing a newer one is a
// mismatched install, not a message to guess at.
constexpr int supportedVersion = 1;
// The same bounds the helper enforces on its side. The host repeats them
// because a helper is a separate process and may be an older or broken build.
constexpr qsizetype maxProviders = 32;
constexpr qsizetype maxPayloadKeys = 32;
constexpr qsizetype maxIdChars = 32;
constexpr qsizetype maxTextChars = 512;
// Generations and request ids are `u64` on the wire; the host only compares
// them, so it keeps them exact by never letting one arrive as a JSON number.
constexpr qsizetype maxRequestIdChars = 32;
// A list provider (the launcher's results, the clipboard's history) shows a
// screenful of rows, never a database dump — the same order of magnitude each
// one already bounds itself to on the Rust side.
constexpr qsizetype maxArrayItems = 64;

ProviderMessage invalid(const QString &reason)
{
    ProviderMessage message;
    message.kind = ProviderMessage::Kind::Invalid;
    message.error = reason;
    return message;
}

bool isProviderId(const QString &id)
{
    if (id.isEmpty() || id.size() > maxIdChars)
        return false;
    if (!(id.at(0) >= u'a' && id.at(0) <= u'z'))
        return false;

    for (const QChar character : id) {
        const bool allowed = (character >= u'a' && character <= u'z')
            || (character >= u'0' && character <= u'9') || character == u'-';
        if (!allowed)
            return false;
    }
    return true;
}

bool isWholeNumber(const QJsonValue &value)
{
    if (!value.isDouble())
        return false;

    const double number = value.toDouble();
    return std::isfinite(number) && std::floor(number) == number && number >= 0
        && number <= 9007199254740991.0;
}

// A row inside a list field — one search hit, one history entry — follows the
// exact same "flat object of scalars" rule a payload does, just addressed by
// an array index instead of a provider id. Reusing the rule rather than a
// looser one is what keeps a row from becoming a second, unbounded document.
bool readRow(const QJsonObject &source, QVariantMap *row)
{
    if (source.size() > maxPayloadKeys)
        return false;

    for (auto field = source.constBegin(); field != source.constEnd(); ++field) {
        const QJsonValue value = field.value();
        if (value.isObject() || value.isArray())
            return false;
        if (value.isString() && value.toString().size() > maxTextChars)
            return false;

        row->insert(field.key(), value.toVariant());
    }
    return true;
}

// A payload is a flat object of scalars — or, for a field that describes a
// list (the launcher's results, the clipboard's history), a bounded array of
// rows with that same flat shape. One level of structure is what a list
// overlay's row needs; a row that itself nested a list would carry the
// unbounded depth the flat rule exists to rule out, so `readRow` never
// recurses back into this function.
bool readPayload(const QJsonObject &source, QVariantMap *payload)
{
    if (source.size() > maxPayloadKeys)
        return false;

    for (auto field = source.constBegin(); field != source.constEnd(); ++field) {
        const QJsonValue value = field.value();
        if (value.isObject())
            return false;
        if (value.isArray()) {
            const QJsonArray array = value.toArray();
            if (array.size() > maxArrayItems)
                return false;

            QVariantList rows;
            for (const QJsonValue &item : array) {
                if (!item.isObject())
                    return false;
                QVariantMap row;
                if (!readRow(item.toObject(), &row))
                    return false;
                rows.append(row);
            }
            payload->insert(field.key(), rows);
            continue;
        }
        if (value.isString() && value.toString().size() > maxTextChars)
            return false;

        payload->insert(field.key(), value.toVariant());
    }
    return true;
}

ProviderMessage readProviders(const QJsonObject &root)
{
    const QJsonValue generation = root.value(QStringLiteral("generation"));
    const QJsonValue providers = root.value(QStringLiteral("providers"));
    if (!isWholeNumber(generation))
        return invalid(QStringLiteral("the frame carries no usable generation"));
    if (!providers.isObject())
        return invalid(QStringLiteral("the frame carries no provider set"));

    const QJsonObject published = providers.toObject();
    if (published.size() > maxProviders)
        return invalid(QStringLiteral("the frame carries too many providers"));

    ProviderMessage message;
    message.kind = ProviderMessage::Kind::Providers;
    message.generation = static_cast<quint64>(generation.toDouble());

    for (auto provider = published.constBegin(); provider != published.constEnd();
         ++provider) {
        if (!isProviderId(provider.key()))
            return invalid(QStringLiteral("the frame names an unusable provider"));
        if (!provider.value().isObject())
            return invalid(QStringLiteral("a provider published no field set"));

        QVariantMap payload;
        if (!readPayload(provider.value().toObject(), &payload))
            return invalid(QStringLiteral("a provider published an unusable value"));

        message.providers.insert(provider.key(), payload);
    }

    return message;
}

ProviderMessage readResult(const QJsonObject &root)
{
    const QJsonValue id = root.value(QStringLiteral("id"));
    const QJsonValue state = root.value(QStringLiteral("state"));
    if (!id.isString() || id.toString().isEmpty()
        || id.toString().size() > maxRequestIdChars) {
        return invalid(QStringLiteral("the result carries no usable request id"));
    }

    const QString outcome = state.toString();
    if (outcome != QStringLiteral("accepted") && outcome != QStringLiteral("failed"))
        return invalid(QStringLiteral("the result carries an unknown state"));

    ProviderMessage message;
    message.kind = ProviderMessage::Kind::Result;
    message.requestId = id.toString();
    message.state = outcome;
    message.reason = root.value(QStringLiteral("reason")).toString().left(maxTextChars);
    return message;
}
} // namespace

ProviderMessage parseProviderMessage(const QByteArray &line)
{
    QJsonParseError parseError;
    const QJsonDocument document = QJsonDocument::fromJson(line, &parseError);
    if (parseError.error != QJsonParseError::NoError || !document.isObject())
        return invalid(parseError.errorString());

    const QJsonObject root = document.object();
    const QJsonValue version = root.value(QStringLiteral("version"));
    const QString kind = root.value(QStringLiteral("kind")).toString();

    if (kind == QStringLiteral("providers")) {
        if (version.toInt(-1) != supportedVersion) {
            return invalid(
                QStringLiteral("the helper speaks provider protocol version %1")
                    .arg(version.toInt(-1))
            );
        }
        return readProviders(root);
    }
    if (kind == QStringLiteral("result"))
        return readResult(root);

    return invalid(QStringLiteral("unknown frame kind '%1'").arg(kind.left(maxIdChars)));
}

bool ProviderStates::apply(const ProviderMessage &message)
{
    if (message.kind != ProviderMessage::Kind::Providers)
        return false;

    const bool changed =
        m_generation != message.generation || m_providers != message.providers;
    // A frame is the complete set: whatever it omits, the helper no longer
    // carries, so replacing wholesale is what keeps a withdrawn provider from
    // lingering.
    m_generation = message.generation;
    m_providers = message.providers;
    return changed;
}

bool ProviderStates::clear()
{
    if (m_providers.isEmpty() && m_generation == 0)
        return false;

    m_providers.clear();
    m_generation = 0;
    return true;
}
