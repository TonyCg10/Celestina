#include "traywatcher.h"

#include <QCoreApplication>
#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusPendingCall>
#include <QDBusPendingCallWatcher>
#include <QDBusServiceWatcher>
#include <QDBusVariant>
#include <QDateTime>
#include <QDebug>
#include <QIcon>
#include <QUrl>

namespace {
constexpr auto watcherService = "org.kde.StatusNotifierWatcher";
constexpr auto watcherPath = "/StatusNotifierWatcher";
constexpr auto watcherInterface = "org.kde.StatusNotifierWatcher";
constexpr auto itemInterface = "org.kde.StatusNotifierItem";
constexpr auto propertiesInterface = "org.freedesktop.DBus.Properties";
constexpr auto menuInterface = "com.canonical.dbusmenu";

QString itemKey(const QString &service, const QString &path)
{
    return service + path;
}

// The panel draws a tray icon at about this size; the choice of published
// pixmap and the size a theme is asked for both follow from it.
constexpr int drawnIconSize = 18;

/// Demarshals `a(iiay)` — the sizes an item published, each with its own
/// pixels. QtDBus hands nested containers back lazily, so this is where the
/// shape is trusted or not at all.
QList<TrayPixmap> readPixmaps(const QVariant &value)
{
    QList<TrayPixmap> pixmaps;
    if (!value.canConvert<QDBusArgument>())
        return pixmaps;

    const QDBusArgument argument = value.value<QDBusArgument>();
    if (argument.currentType() != QDBusArgument::ArrayType)
        return pixmaps;

    argument.beginArray();
    while (!argument.atEnd()) {
        TrayPixmap pixmap;
        argument.beginStructure();
        argument >> pixmap.width >> pixmap.height >> pixmap.argb;
        argument.endStructure();
        pixmaps.append(pixmap);
    }
    argument.endArray();
    return pixmaps;
}

// Every call this class makes is asynchronous. A tray item is another
// application's process — often one behind a portal proxy — and a blocking
// call would let any of them stall the shell.
void callAsync(
    QObject *context,
    const QDBusMessage &message,
    const std::function<void(const QDBusMessage &)> &onReply
)
{
    QDBusPendingCall pending = QDBusConnection::sessionBus().asyncCall(message);
    auto *watcher = new QDBusPendingCallWatcher(pending, context);
    QObject::connect(
        watcher,
        &QDBusPendingCallWatcher::finished,
        context,
        [onReply](QDBusPendingCallWatcher *watcher) {
            const QDBusMessage reply = watcher->reply();
            watcher->deleteLater();
            if (reply.type() == QDBusMessage::ReplyMessage)
                onReply(reply);
        }
    );
}
} // namespace

TrayWatcher::TrayWatcher(QSharedPointer<TrayIconCache> icons, QObject *parent)
    : QObject(parent)
    , m_icons(std::move(icons))
    , m_watcherPresence(new QDBusServiceWatcher(
          QString::fromLatin1(watcherService),
          QDBusConnection::sessionBus(),
          QDBusServiceWatcher::WatchForOwnerChange,
          this
      ))
{
    connect(
        m_watcherPresence,
        &QDBusServiceWatcher::serviceOwnerChanged,
        this,
        &TrayWatcher::watcherOwnerChanged
    );

    // Foreign icons only: without this Qt can resolve none of them.
    configureForeignIconThemes();
    attach();
}

void TrayWatcher::attach()
{
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected())
        return;

    // The specification's host name carries the process id so several hosts
    // can watch the same session — which is exactly the state during this
    // replacement, with Noctalia's bar still up.
    if (!m_hostRegistered) {
        const QString hostName =
            QStringLiteral("org.kde.StatusNotifierHost-%1")
                .arg(QCoreApplication::applicationPid());
        m_hostRegistered = bus.registerService(hostName);
        if (m_hostRegistered) {
            QDBusMessage announce = QDBusMessage::createMethodCall(
                QString::fromLatin1(watcherService),
                QString::fromLatin1(watcherPath),
                QString::fromLatin1(watcherInterface),
                QStringLiteral("RegisterStatusNotifierHost")
            );
            announce.setArguments({hostName});
            bus.asyncCall(announce);
        }
    }

    bus.connect(
        QString::fromLatin1(watcherService),
        QString::fromLatin1(watcherPath),
        QString::fromLatin1(watcherInterface),
        QStringLiteral("StatusNotifierItemRegistered"),
        this,
        SLOT(itemRegistered(QString))
    );
    bus.connect(
        QString::fromLatin1(watcherService),
        QString::fromLatin1(watcherPath),
        QString::fromLatin1(watcherInterface),
        QStringLiteral("StatusNotifierItemUnregistered"),
        this,
        SLOT(itemUnregistered(QString))
    );

    refreshRegistrations();
}

void TrayWatcher::watcherOwnerChanged(
    const QString &service,
    const QString &was,
    const QString &now
)
{
    Q_UNUSED(service)
    Q_UNUSED(was)

    if (now.isEmpty()) {
        // No watcher means no application is publishing to anyone. Whatever was
        // on screen belongs to a session that no longer exists.
        qInfo() << "Celestina lost the tray watcher.";
        setUnavailable();
        return;
    }

    qInfo() << "Celestina found a tray watcher; re-registering as a host.";
    attach();
}

void TrayWatcher::refreshRegistrations()
{
    QDBusMessage request = QDBusMessage::createMethodCall(
        QString::fromLatin1(watcherService),
        QString::fromLatin1(watcherPath),
        QString::fromLatin1(propertiesInterface),
        QStringLiteral("Get")
    );
    request.setArguments(
        {QString::fromLatin1(watcherInterface),
         QStringLiteral("RegisteredStatusNotifierItems")}
    );

    callAsync(this, request, [this](const QDBusMessage &reply) {
        const QStringList entries =
            reply.arguments().value(0).value<QDBusVariant>().variant().toStringList();

        m_registrations.clear();
        m_read.clear();
        for (const QString &entry : entries) {
            QString service;
            QString path;
            if (!parseTrayRegistration(entry, &service, &path)) {
                qWarning() << "Celestina ignored an unusable tray registration.";
                continue;
            }
            m_registrations.append({service, path});
            watchItem(service, path);
            readItem(service, path);
        }

        m_available = true;
        publish();
    });
}

void TrayWatcher::watchItem(const QString &service, const QString &path)
{
    QDBusConnection bus = QDBusConnection::sessionBus();
    // An item announces that something about it changed and expects the host to
    // ask again; none of these signals carry the whole truth.
    for (const auto *signal : {"NewIcon", "NewStatus", "NewTitle", "NewAttentionIcon"}) {
        bus.connect(
            service,
            path,
            QString::fromLatin1(itemInterface),
            QString::fromLatin1(signal),
            this,
            SLOT(itemPropertiesChanged())
        );
    }
}

void TrayWatcher::readItem(const QString &service, const QString &path)
{
    QDBusMessage request = QDBusMessage::createMethodCall(
        service,
        path,
        QString::fromLatin1(propertiesInterface),
        QStringLiteral("GetAll")
    );
    request.setArguments({QString::fromLatin1(itemInterface)});

    callAsync(this, request, [this, service, path](const QDBusMessage &reply) {
        // `GetAll` omits any property whose getter failed rather than failing
        // itself, which is how the one item here with no readable icon name
        // still arrives usable.
        // The reply is `a{sv}`, which QtDBus hands back as a lazily typed
        // argument rather than a map.
        QVariantMap properties;
        const QDBusArgument argument =
            reply.arguments().value(0).value<QDBusArgument>();
        argument >> properties;
        const TrayItem item = readTrayItem(service, path, properties);
        const QString key = itemKey(service, path);
        m_iconSources.insert(key, resolveIcon(item, properties));
        m_read.insert(key, item);
        publish();
    });
}

void TrayWatcher::itemRegistered(const QString &entry)
{
    QString service;
    QString path;
    if (!parseTrayRegistration(entry, &service, &path))
        return;

    if (!m_registrations.contains({service, path}))
        m_registrations.append({service, path});
    watchItem(service, path);
    readItem(service, path);
}

void TrayWatcher::itemUnregistered(const QString &entry)
{
    QString service;
    QString path;
    if (!parseTrayRegistration(entry, &service, &path))
        return;

    m_registrations.removeAll({service, path});
    const QString key = itemKey(service, path);
    m_read.remove(key);
    m_iconSources.remove(key);
    m_iconRevisions.remove(key);
    m_icons->remove(key);
    publish();
}

void TrayWatcher::itemPropertiesChanged()
{
    // The signal says only "ask again", and does not say which property moved.
    const QString service = message().service();
    const QString path = message().path();
    if (service.isEmpty() || path.isEmpty())
        return;

    readItem(service, path);
}

QString TrayWatcher::resolveIcon(const TrayItem &item, const QVariantMap &properties)
{
    const QString key = itemKey(item.service, item.path);
    QImage image;

    if (!item.iconName.isEmpty()) {
        // An application that ships its own theme says where; that directory is
        // searched first and only for this lookup.
        QStringList paths = QIcon::themeSearchPaths();
        if (!item.iconThemePath.isEmpty() && !paths.contains(item.iconThemePath)) {
            QIcon::setThemeSearchPaths(QStringList {item.iconThemePath} + paths);
        }
        const QIcon themed = QIcon::fromTheme(item.iconName);
        QIcon::setThemeSearchPaths(paths);

        if (!themed.isNull())
            image = themed.pixmap(drawnIconSize, drawnIconSize).toImage();
    }

    // A name that resolves to nothing anywhere is normal — one item on this
    // session names an icon no installed theme has — so its own pixels are the
    // next answer, not a failure.
    if (image.isNull())
        image = bestTrayPixmap(readPixmaps(properties.value(QStringLiteral("IconPixmap"))), drawnIconSize);

    if (image.isNull()) {
        m_icons->remove(key);
        m_iconRevisions.remove(key);
        return QString();
    }

    m_icons->insert(key, image);
    const int revision = m_iconRevisions.value(key) + 1;
    m_iconRevisions.insert(key, revision);
    // The key is another process's bus name and object path, so it is encoded
    // rather than pasted into a URL — and concatenated rather than composed
    // with `arg()`, which would read the `%2F` of an encoded slash as its own
    // placeholder and eat it.
    return QStringLiteral("image://tray/")
        + QString::fromLatin1(QUrl::toPercentEncoding(key))
        + u'/' + QString::number(revision);
}

QVariantList TrayWatcher::items() const
{
    QVariantList items = m_items.toVariantList();
    for (QVariant &entry : items) {
        QVariantMap item = entry.toMap();
        const QString key = itemKey(
            item.value(QStringLiteral("service")).toString(),
            item.value(QStringLiteral("path")).toString()
        );
        item.insert(QStringLiteral("iconSource"), m_iconSources.value(key));
        entry = item;
    }
    return items;
}

void TrayWatcher::publish()
{
    QList<TrayItem> items;
    items.reserve(m_registrations.size());
    // Registration order, and only the items that have actually answered: an
    // item that has not is not yet something the panel can show.
    for (const auto &registration : m_registrations) {
        const auto item = m_read.constFind(itemKey(registration.first, registration.second));
        if (item != m_read.constEnd())
            items.append(item.value());
    }

    if (m_items.replace(items))
        emit changed();
}

void TrayWatcher::setUnavailable()
{
    m_registrations.clear();
    m_read.clear();
    for (const QString &key : m_iconSources.keys())
        m_icons->remove(key);
    m_iconSources.clear();
    m_iconRevisions.clear();
    const bool dropped = m_items.clear();
    if (!m_available && !dropped)
        return;

    m_available = false;
    emit changed();
}

/// Reads `(ia{sv}av)` — one menu node and, recursively, its children. QtDBus
/// hands the whole tree back lazily, so this is where its shape is trusted or
/// not at all.
static TrayMenuNode readMenuNode(const QDBusArgument &argument, int depth)
{
    TrayMenuNode node;
    // A tree from another process is not walked as far as it says it goes; the
    // reading stops well before anything the panel would draw.
    if (depth > 6)
        return node;

    argument.beginStructure();
    argument >> node.id;

    QVariantMap properties;
    argument >> properties;
    node.properties = properties;

    argument.beginArray();
    while (!argument.atEnd()) {
        QDBusVariant child;
        argument >> child;
        const QDBusArgument nested = child.variant().value<QDBusArgument>();
        if (nested.currentType() == QDBusArgument::StructureType)
            node.children.append(readMenuNode(nested, depth + 1));
    }
    argument.endArray();
    argument.endStructure();
    return node;
}

void TrayWatcher::readMenuLayout(
    const QString &service,
    const QString &menuPath,
    const QString &itemPath
)
{
    QDBusMessage request = QDBusMessage::createMethodCall(
        service,
        menuPath,
        QString::fromLatin1(menuInterface),
        QStringLiteral("GetLayout")
    );
    // The whole menu at once, with every property: asking for a subset means
    // guessing which ones an application chose to use.
    request.setArguments({0, -1, QStringList()});

    callAsync(this, request, [this, service, itemPath](const QDBusMessage &reply) {
        const QDBusArgument layout =
            reply.arguments().value(1).value<QDBusArgument>();
        if (layout.currentType() != QDBusArgument::StructureType) {
            qWarning() << "Celestina could not read a tray menu's layout.";
            return;
        }

        emit menuReady(service, itemPath, buildTrayMenu(readMenuNode(layout, 0)));
    });
}

void TrayWatcher::requestMenu(const QString &service, const QString &path)
{
    const auto item = m_read.constFind(itemKey(service, path));
    if (item == m_read.constEnd() || item.value().menuPath.isEmpty())
        return;

    const QString menuPath = item.value().menuPath;
    // The application is told first, because it may rebuild the menu before
    // answering — a menu read without asking can be the one from last time.
    QDBusMessage aboutToShow = QDBusMessage::createMethodCall(
        service,
        menuPath,
        QString::fromLatin1(menuInterface),
        QStringLiteral("AboutToShow")
    );
    aboutToShow.setArguments({0});

    callAsync(this, aboutToShow, [this, service, menuPath, path](const QDBusMessage &) {
        readMenuLayout(service, menuPath, path);
    });
}

void TrayWatcher::triggerMenuEntry(
    const QString &service,
    const QString &path,
    int entryId
)
{
    const auto item = m_read.constFind(itemKey(service, path));
    if (item == m_read.constEnd() || item.value().menuPath.isEmpty())
        return;

    QDBusMessage event = QDBusMessage::createMethodCall(
        service,
        item.value().menuPath,
        QString::fromLatin1(menuInterface),
        QStringLiteral("Event")
    );
    event.setArguments(
        {entryId,
         QStringLiteral("clicked"),
         QVariant::fromValue(QDBusVariant(QString())),
         quint32(QDateTime::currentSecsSinceEpoch())}
    );
    QDBusConnection::sessionBus().asyncCall(event);
}

void TrayWatcher::activate(const QString &service, const QString &path, int x, int y)
{
    QDBusMessage request = QDBusMessage::createMethodCall(
        service,
        path,
        QString::fromLatin1(itemInterface),
        QStringLiteral("Activate")
    );
    request.setArguments({x, y});
    QDBusConnection::sessionBus().asyncCall(request);
}

void TrayWatcher::secondaryActivate(
    const QString &service,
    const QString &path,
    int x,
    int y
)
{
    QDBusMessage request = QDBusMessage::createMethodCall(
        service,
        path,
        QString::fromLatin1(itemInterface),
        QStringLiteral("SecondaryActivate")
    );
    request.setArguments({x, y});
    QDBusConnection::sessionBus().asyncCall(request);
}
