#include "trayitems.h"

#include <QByteArray>
#include <QCryptographicHash>
#include <QDBusArgument>
#include <QDBusObjectPath>
#include <QDir>
#include <QRegularExpression>

#include <algorithm>

namespace {
constexpr qsizetype maxTextLength = 256;

QString boundedText(const QVariant &value, qsizetype maximum = maxTextLength)
{
    return value.toString().left(maximum);
}

// The three the specification defines. Anything else is an application getting
// it wrong, and hiding its control would be a worse answer than showing it.
QString readStatus(const QVariant &value)
{
    const QString status = value.toString();
    if (status.compare(QStringLiteral("Passive"), Qt::CaseInsensitive) == 0)
        return QStringLiteral("passive");
    if (status.compare(QStringLiteral("NeedsAttention"), Qt::CaseInsensitive) == 0)
        return QStringLiteral("attention");
    return QStringLiteral("active");
}

// `IconPixmap` is `a(iiay)`: a list of sizes, each with its raw pixels. Only
// its emptiness matters here — whether the item has pixels at all — because
// choosing and decoding one is the renderer's business.
bool hasPixmapData(const QVariant &value)
{
    if (value.canConvert<QVariantList>())
        return !value.toList().isEmpty();

    // QtDBus hands nested containers back as a lazily typed argument, which
    // says nothing about emptiness without demarshalling it. An item that
    // published the key at all is taken at its word.
    return value.isValid() && !value.isNull();
}

QString readObjectPath(const QVariant &value)
{
    if (value.canConvert<QDBusObjectPath>()) {
        const QString path = value.value<QDBusObjectPath>().path();
        return path == QStringLiteral("/") ? QString() : path.left(maxTrayPathLength);
    }
    const QString path = value.toString().left(maxTrayPathLength);
    return path.startsWith(u'/') && path != QStringLiteral("/") ? path : QString();
}

QString readToolTipTitle(const QVariant &value)
{
    // Plain lists keep the pure unit boundary cheap; QtDBus supplies the real
    // `(sa(iiay)ss)` value as a lazy argument in production.
    if (value.canConvert<QVariantList>()) {
        const QVariantList fields = value.toList();
        return fields.size() >= 3
            ? boundedText(fields.at(2)).trimmed() : QString();
    }
    if (!value.canConvert<QDBusArgument>())
        return {};

    const QDBusArgument argument = value.value<QDBusArgument>();
    if (argument.currentType() != QDBusArgument::StructureType)
        return {};

    QString iconName;
    QString title;
    QString description;
    argument.beginStructure();
    argument >> iconName;
    if (argument.currentType() != QDBusArgument::ArrayType)
        return {};

    // The icon pixels are irrelevant to identity, but the cursor must cross
    // them to reach the tooltip title. Refuse an implausible peer rather than
    // letting a foreign array turn name resolution into unbounded work.
    constexpr qsizetype maxToolTipPixmaps = 64;
    qsizetype pixmapCount = 0;
    argument.beginArray();
    while (!argument.atEnd()) {
        if (++pixmapCount > maxToolTipPixmaps)
            return {};
        int width = 0;
        int height = 0;
        QByteArray pixels;
        argument.beginStructure();
        argument >> width >> height >> pixels;
        argument.endStructure();
    }
    argument.endArray();
    argument >> title >> description;
    argument.endStructure();
    return boundedText(title).trimmed();
}

QString humanizedTrayId(const QString &id)
{
    QString candidate = boundedText(id).trimmed();
    static const QRegularExpression statusIconSuffix(
        QStringLiteral("[_-]status[_-]icon(?:[_-]\\d+)?$"),
        QRegularExpression::CaseInsensitiveOption
    );
    candidate.remove(statusIconSuffix);
    candidate.replace(QRegularExpression(QStringLiteral("[_-]+")),
                      QStringLiteral(" "));
    candidate = candidate.simplified();
    if (!candidate.isEmpty() && candidate == candidate.toLower())
        candidate[0] = candidate.at(0).toUpper();
    return candidate;
}
} // namespace

QString trayDisplayName(
    const QString &id,
    const QString &declaredTitle,
    const QString &toolTipTitle
)
{
    const QString title = boundedText(declaredTitle).trimmed();
    if (!title.isEmpty())
        return title;

    const QString idName = humanizedTrayId(id);
    const QString runtime = idName.toLower();
    const QString toolTip = boundedText(toolTipTitle).trimmed();
    // Chromium/Electron SNI bridges often publish the runtime as Id. Their
    // tooltip is then the only protocol field carrying the product identity.
    // Do not apply that precedence to an app-specific Id: Slack, for example,
    // uses its tooltip for unread state rather than its name.
    if (!toolTip.isEmpty()
        && (runtime == QStringLiteral("chrome")
            || runtime == QStringLiteral("chromium")
            || runtime == QStringLiteral("electron"))) {
        return toolTip;
    }
    if (!idName.isEmpty())
        return idName;
    return toolTip;
}

bool TrayItem::operator==(const TrayItem &other) const
{
    return service == other.service && path == other.path && id == other.id
        && preferenceKey == other.preferenceKey
        && title == other.title && status == other.status
        && iconName == other.iconName && iconThemePath == other.iconThemePath
        && hasPixmap == other.hasPixmap && menuPath == other.menuPath;
}

QString trayPreferenceKey(const QString &id)
{
    if (id.trimmed().isEmpty())
        return {};

    return QString::fromLatin1(
        QCryptographicHash::hash(id.toUtf8(), QCryptographicHash::Sha256).toHex()
    );
}

bool parseTrayRegistration(const QString &entry, QString *service, QString *path)
{
    if (!service || !path)
        return false;

    const QString trimmed = entry.trimmed();
    if (trimmed.isEmpty() || trimmed.size() > maxTrayPathLength)
        return false;

    const qsizetype separator = trimmed.indexOf(u'/');
    // A bare bus name means the specification's default object path.
    if (separator < 0) {
        *service = trimmed;
        *path = QStringLiteral("/StatusNotifierItem");
        return true;
    }
    if (separator == 0)
        return false;

    *service = trimmed.left(separator);
    *path = trimmed.mid(separator);
    return true;
}

TrayItem unreadTrayItem(const QString &service, const QString &path)
{
    TrayItem item;
    item.service = service;
    item.path = path;
    // Active, because the specification's default is what an item that said
    // nothing has said: hiding it would be the very thing this exists to stop.
    item.status = QStringLiteral("active");

    // The last segment of the object path is what most applications name their
    // item after — `indicator_solaar`, `nm_applet`. Some number theirs instead,
    // and a bare index names nothing, so those fall back to the bus name.
    const qsizetype lastSlash = path.lastIndexOf(u'/');
    const QString segment = lastSlash < 0 ? QString() : path.mid(lastSlash + 1);
    const bool named = std::any_of(segment.cbegin(), segment.cend(), [](QChar character) {
        return character.isLetter();
    });
    item.id = boundedText(named ? segment : service);
    item.title = item.id;
    return item;
}

TrayItem readTrayItem(
    const QString &service,
    const QString &path,
    const QVariantMap &properties
)
{
    TrayItem item;
    item.service = service;
    item.path = path;
    item.id = boundedText(properties.value(QStringLiteral("Id")));
    item.preferenceKey = trayPreferenceKey(item.id);
    item.status = readStatus(properties.value(QStringLiteral("Status")));
    item.iconName = boundedText(properties.value(QStringLiteral("IconName")));
    item.hasPixmap = hasPixmapData(properties.value(QStringLiteral("IconPixmap")));
    item.menuPath = readObjectPath(properties.value(QStringLiteral("Menu")));

    item.title = trayDisplayName(
        item.id,
        boundedText(properties.value(QStringLiteral("Title"))),
        readToolTipTitle(properties.value(QStringLiteral("ToolTip")))
    );

    // An icon theme the application ships itself. A relative path names nothing
    // this panel can resolve, so it is dropped rather than guessed against some
    // working directory.
    const QString themePath =
        boundedText(properties.value(QStringLiteral("IconThemePath")), maxTrayPathLength);
    if (!themePath.isEmpty() && QDir::isAbsolutePath(themePath))
        item.iconThemePath = themePath;

    return item;
}

bool TrayItems::replace(const QList<TrayItem> &items)
{
    QList<TrayItem> bounded = items;
    if (bounded.size() > maxTrayItems)
        bounded.resize(maxTrayItems);

    if (bounded == m_items)
        return false;

    m_items = bounded;
    return true;
}

bool TrayItems::clear()
{
    if (m_items.isEmpty())
        return false;

    m_items.clear();
    return true;
}

QVariantList TrayItems::toVariantList() const
{
    QVariantList items;
    items.reserve(m_items.size());
    for (const TrayItem &item : m_items) {
        items.append(QVariantMap {
            {QStringLiteral("service"), item.service},
            {QStringLiteral("path"), item.path},
            {QStringLiteral("id"), item.id},
            {QStringLiteral("preferenceKey"), item.preferenceKey},
            {QStringLiteral("title"), item.title},
            {QStringLiteral("status"), item.status},
            {QStringLiteral("iconName"), item.iconName},
            {QStringLiteral("iconThemePath"), item.iconThemePath},
            {QStringLiteral("hasPixmap"), item.hasPixmap},
            {QStringLiteral("hasMenu"), !item.menuPath.isEmpty()},
        });
    }
    return items;
}
