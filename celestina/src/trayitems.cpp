#include "trayitems.h"

#include <QDBusObjectPath>
#include <QDir>

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
} // namespace

bool TrayItem::operator==(const TrayItem &other) const
{
    return service == other.service && path == other.path && id == other.id
        && title == other.title && status == other.status
        && iconName == other.iconName && iconThemePath == other.iconThemePath
        && hasPixmap == other.hasPixmap && menuPath == other.menuPath;
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
    item.status = readStatus(properties.value(QStringLiteral("Status")));
    item.iconName = boundedText(properties.value(QStringLiteral("IconName")));
    item.hasPixmap = hasPixmapData(properties.value(QStringLiteral("IconPixmap")));
    item.menuPath = readObjectPath(properties.value(QStringLiteral("Menu")));

    // An item with no title is not nameless: its id is what the application
    // calls itself, and that is better than an empty slot in a drawer.
    const QString title = boundedText(properties.value(QStringLiteral("Title")));
    item.title = title.isEmpty() ? item.id : title;

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
