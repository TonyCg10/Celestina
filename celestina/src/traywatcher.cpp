#include "traywatcher.h"

#include "diagnosticjournal.h"

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
#include <QTimer>
#include <QUrl>

namespace {
constexpr auto watcherService = "org.kde.StatusNotifierWatcher";
constexpr auto watcherPath = "/StatusNotifierWatcher";
constexpr auto watcherInterface = "org.kde.StatusNotifierWatcher";
constexpr auto itemInterface = "org.kde.StatusNotifierItem";
constexpr auto propertiesInterface = "org.freedesktop.DBus.Properties";
constexpr auto menuInterface = "com.canonical.dbusmenu";
constexpr auto busService = "org.freedesktop.DBus";
constexpr auto busPath = "/org/freedesktop/DBus";
constexpr auto busInterface = "org.freedesktop.DBus";

// An item announces that something about it changed and expects the host to
// ask again; none of these signals carry the whole truth. They are named once
// because subscribing and unsubscribing must be the same set: a match rule the
// host forgot to remove keeps a gone item's signals arriving for the rest of
// the session.
constexpr const char *itemSignals[] = {
    "NewIcon",
    "NewStatus",
    "NewTitle",
    "NewToolTip",
    "NewAttentionIcon",
};

QString itemKey(const QString &service, const QString &path)
{
    return service + path;
}

// What a tray icon is rasterized to once, here, before any surface draws it.
//
// It is deliberately far larger than the ~18 logical pixels the panel draws,
// because this is the only rasterization there is: a themed SVG is rendered at
// exactly this size and an application's own pixmaps are chosen against it,
// and every consumer afterwards can only scale what it is given. At 18 the
// panel handed a 1.5-scaled output an image smaller than the physical area it
// filled, and the inventory grid — which draws the same icon larger still —
// magnified it further. Both read as pixelated while an unscaled output did
// not, which is exactly what the author reported.
//
// 64 covers the panel at every supported scale and the inventory grid with
// room to spare; the cost is 16 KiB per tray item.
constexpr int drawnIconSize = 64;
constexpr int settledRegistryRefreshMs = 1000;

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
//
// `onError` is not optional decoration. A refused or unanswered call used to
// leave this function silently, which meant an item the watcher lists and the
// host never renders looked exactly like an item that was never registered —
// with nothing in the log to tell them apart.
void callAsync(
    QObject *context,
    const QDBusMessage &message,
    const std::function<void(const QDBusMessage &)> &onReply,
    const std::function<void(const QString &)> &onError = {}
)
{
    QDBusPendingCall pending = QDBusConnection::sessionBus().asyncCall(message);
    auto *watcher = new QDBusPendingCallWatcher(pending, context);
    QObject::connect(
        watcher,
        &QDBusPendingCallWatcher::finished,
        context,
        [onReply, onError](QDBusPendingCallWatcher *watcher) {
            const QDBusMessage reply = watcher->reply();
            watcher->deleteLater();
            if (reply.type() == QDBusMessage::ReplyMessage) {
                onReply(reply);
                return;
            }
            if (onError) {
                const QString reason = reply.errorMessage().isEmpty()
                    ? QStringLiteral("no reply")
                    : reply.errorMessage();
                onError(reason);
            }
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
    , m_registryRefresh(new QTimer(this))
{
    m_registry = new TrayWatcherService(TrayWatcherService::wellKnownName(), this);

    connect(
        m_watcherPresence,
        &QDBusServiceWatcher::serviceOwnerChanged,
        this,
        &TrayWatcher::watcherOwnerChanged
    );
    m_registryRefresh->setSingleShot(true);
    m_registryRefresh->setInterval(settledRegistryRefreshMs);
    connect(m_registryRefresh, &QTimer::timeout, this, &TrayWatcher::refreshRegistrations);

    // Foreign icons only: without this Qt can resolve none of them.
    configureForeignIconThemes();

    // Be the watcher if nobody is. A session with a host and no watcher has no
    // tray at all: an application looks for that name and publishes nothing
    // when it is missing.
    if (m_registry->claim())
        qInfo() << "Celestina is the session's tray watcher.";

    attach();
}

TrayWatcher::~TrayWatcher()
{
    forgetItems();
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

    DiagnosticJournal::instance().record(
        CELESTINA_JOURNAL(Debug, "tray.attach")
            .flag(QStringLiteral("host_registered"), m_hostRegistered)
    );

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
    // Registering as a host and reading an existing foreign registry are two
    // asynchronous conversations. Reconcile once more after both have had a
    // chance to settle: otherwise a premature empty/error reply is the only
    // snapshot this process ever sees, because already registered items emit
    // no new registration signal merely for a new host.
    m_registryRefresh->start();
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
        // The watcher left. Whatever was on screen belonged to a session that
        // no longer exists — and if nobody else takes the name, this shell
        // does, because otherwise no application can publish a tray item at
        // all.
        qInfo() << "Celestina lost the tray watcher.";
        setUnavailable();
        if (m_registry->claim()) {
            qInfo() << "Celestina took over as the session's tray watcher.";
            attach();
        }
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

    // Only the newest read may reconcile the registry. `attach()` is re-entered
    // whenever the watcher name changes owner — which includes this shell
    // taking it — so without a generation the older reply arrives second and
    // reconciles against a list that has moved on.
    const quint64 generation = ++m_registryGeneration;
    // What was already known when this read was *sent*. Anything learned after
    // that — an application whose `StatusNotifierItemRegistered` arrived while
    // the watcher was answering — is newer than the snapshot coming back, and
    // the snapshot is not entitled to remove it.
    //
    // This is the loss. The reply used to rebuild the list wholesale, so an
    // item that registered during the round trip was dropped from
    // `m_registrations` and from `m_read` — and no second registration signal
    // was ever coming for it, so it stayed registered with the watcher and
    // absent from the panel for the rest of the session.
    const QList<QPair<QString, QString>> baseline = m_registrations;

    callAsync(
        this,
        request,
        [this, generation, baseline](const QDBusMessage &reply) {
            if (generation != m_registryGeneration) {
                DiagnosticJournal::instance().record(
                    CELESTINA_JOURNAL(Debug, "tray.registry.stale")
                        .unsigned_number(QStringLiteral("request_generation"), generation)
                        .unsigned_number(
                            QStringLiteral("current_request_generation"),
                            m_registryGeneration
                        )
                );
                return;
            }

            const QVariant replyValue = reply.arguments().value(0);
            const QVariant registryValue = replyValue.canConvert<QDBusVariant>()
                ? replyValue.value<QDBusVariant>().variant()
                : replyValue;
            const QStringList entries = registryValue.toStringList();

            QList<QPair<QString, QString>> snapshot;
            for (const QString &entry : entries) {
                // The watcher is another process and its list is as long as it
                // likes; the panel accepts only as many items as it could show.
                if (snapshot.size() >= maxTrayItems)
                    break;

                QString service;
                QString path;
                if (!parseTrayRegistration(entry, &service, &path)) {
                    qWarning() << "Celestina ignored an unusable tray registration.";
                    continue;
                }
                snapshot.append({service, path});
            }

            DiagnosticJournal::instance().record(
                CELESTINA_JOURNAL(Debug, "tray.registry.reply")
                    .unsigned_number(QStringLiteral("request_generation"), generation)
                    .number(QStringLiteral("entry_count"), entries.size())
                    .number(QStringLiteral("accepted_count"), snapshot.size())
                    .number(QStringLiteral("baseline_count"), baseline.size())
            );

            // Gone means: known before this read, and absent from what it
            // brought back. An item this host never had in its baseline is not
            // something this snapshot can speak about.
            for (const auto &known : baseline) {
                if (!snapshot.contains(known))
                    removeRegistration(known.first, known.second);
            }

            for (const auto &found : snapshot) {
                if (m_registrations.contains(found))
                    continue;
                if (m_registrations.size() >= maxTrayItems)
                    break;

                m_registrations.append(found);
                watchItem(found.first, found.second);
                readItem(found.first, found.second);
            }

            m_available = true;
            publish();
        },
        [this, generation](const QString &reason) {
            qWarning().noquote()
                << "Celestina could not read the tray registry:" << reason;
            DiagnosticJournal::instance().record(
                CELESTINA_JOURNAL(Warn, "tray.registry.error")
                    .unsigned_number(QStringLiteral("request_generation"), generation)
                    .text(QStringLiteral("reason"), reason)
            );
        }
    );
}

void TrayWatcher::removeRegistration(const QString &service, const QString &path)
{
    m_registrations.removeAll({service, path});
    unwatchItem(service, path);
    const QString key = itemKey(service, path);
    m_read.remove(key);
    m_iconSources.remove(key);
    m_iconRevisions.remove(key);
    m_readFailures.remove(key);
    m_icons->remove(key);
}

void TrayWatcher::watchItem(const QString &service, const QString &path)
{
    QDBusConnection bus = QDBusConnection::sessionBus();
    for (const auto *signal : itemSignals) {
        bus.connect(
            service,
            path,
            QString::fromLatin1(itemInterface),
            QString::fromLatin1(signal),
            this,
            SLOT(itemPropertiesChanged())
        );
    }

    if (service.startsWith(u':')) {
        m_registeredOwners.insert(service, service);
        return;
    }

    // Who is behind a well-known name is the bus daemon's answer, and it is
    // asked for the same reason everything else here is asked asynchronously:
    // the panel does not wait on anyone. Until it answers, that item's own
    // change signals are simply not attributed to it — a late first icon, not a
    // stalled panel.
    QDBusMessage request = QDBusMessage::createMethodCall(
        QString::fromLatin1(busService),
        QString::fromLatin1(busPath),
        QString::fromLatin1(busInterface),
        QStringLiteral("GetNameOwner")
    );
    request.setArguments({service});

    callAsync(this, request, [this, service, path](const QDBusMessage &reply) {
        const QString owner = reply.arguments().value(0).toString();
        // The item may have unregistered while the bus was answering; learning
        // its owner then would leave an entry nothing ever removes.
        if (owner.isEmpty() || !m_registrations.contains({service, path}))
            return;
        m_registeredOwners.insert(owner, service);
    });
}

void TrayWatcher::unwatchItem(const QString &service, const QString &path)
{
    QDBusConnection bus = QDBusConnection::sessionBus();
    for (const auto *signal : itemSignals) {
        bus.disconnect(
            service,
            path,
            QString::fromLatin1(itemInterface),
            QString::fromLatin1(signal),
            this,
            SLOT(itemPropertiesChanged())
        );
    }

    // The owner is forgotten only once the application has no registration
    // left: one process may publish several items behind the one unique name,
    // and dropping it early would stop the others from updating.
    for (const auto &registration : m_registrations) {
        if (registration.first == service)
            return;
    }
    for (auto owner = m_registeredOwners.begin(); owner != m_registeredOwners.end();) {
        if (owner.value() == service)
            owner = m_registeredOwners.erase(owner);
        else
            ++owner;
    }
}

void TrayWatcher::forgetItems()
{
    const QList<QPair<QString, QString>> registrations = m_registrations;
    m_registrations.clear();
    for (const auto &registration : registrations)
        unwatchItem(registration.first, registration.second);
    m_registeredOwners.clear();
}

QPair<QString, QString> TrayWatcher::registrationFor(
    const QString &sender,
    const QString &path
) const
{
    if (m_registrations.contains({sender, path}))
        return {sender, path};

    const QString registered = m_registeredOwners.value(sender);
    if (!registered.isEmpty() && m_registrations.contains({registered, path}))
        return {registered, path};

    return {};
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

    callAsync(
        this,
        request,
        [this, service, path](const QDBusMessage &reply) {
        // The item may have unregistered while it was answering. Its properties
        // are then the last word of something that no longer exists, and
        // inserting them would put back state that `itemUnregistered` has
        // already removed and will never be asked to remove again.
        if (!m_registrations.contains({service, path}))
            return;

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
        m_readFailures.remove(key);
        m_iconSources.insert(key, resolveIcon(item, properties));
        m_read.insert(key, item);
        DiagnosticJournal::instance().record(
            CELESTINA_JOURNAL(Debug, "tray.item.read")
                .number(QStringLiteral("property_count"), properties.size())
                .number(QStringLiteral("read_count"), m_read.size())
                .number(QStringLiteral("registration_count"), m_registrations.size())
        );
        publish();
        },
        [this, service, path](const QString &reason) {
            itemUnreadable(service, path, reason);
        }
    );
}

void TrayWatcher::itemUnreadable(
    const QString &service,
    const QString &path,
    const QString &reason
)
{
    if (!m_registrations.contains({service, path}))
        return;

    const QString key = itemKey(service, path);
    const int failures = m_readFailures.value(key) + 1;
    m_readFailures.insert(key, failures);

    // Once, because an application that has registered its item before
    // exporting the object behind it is the ordinary race, and it resolves in
    // milliseconds.
    if (failures == 1) {
        readItem(service, path);
        return;
    }
    if (failures > 2)
        return;

    // It is not going to describe itself. It is still a registered control the
    // person can click, so it is shown with the name it registered under rather
    // than silently left out of the tray.
    qWarning().noquote() << "Celestina could not read the tray item" << key
                         << "and is showing it unnamed:" << reason;
    if (!m_read.contains(key))
        m_read.insert(key, unreadTrayItem(service, path));
    publish();
}

void TrayWatcher::itemRegistered(const QString &entry)
{
    QString service;
    QString path;
    if (!parseTrayRegistration(entry, &service, &path))
        return;

    if (!m_registrations.contains({service, path})) {
        // Same bound as the registry's, and for the same reason: each accepted
        // item costs this connection four match rules, and a watcher that keeps
        // announcing new ones must not be able to spend them all.
        if (m_registrations.size() >= maxTrayItems)
            return;
        m_registrations.append({service, path});
    }
    watchItem(service, path);
    readItem(service, path);
}

void TrayWatcher::itemUnregistered(const QString &entry)
{
    QString service;
    QString path;
    if (!parseTrayRegistration(entry, &service, &path))
        return;

    removeRegistration(service, path);
    publish();
}

void TrayWatcher::itemPropertiesChanged()
{
    // The signal says only "ask again", and does not say which property moved.
    //
    // The sender is a unique name, always: the bus rewrites it whatever name
    // the application registered its item under. Asking that name for the
    // properties would read an item that is registered under nothing the panel
    // knows, so the registration it belongs to is resolved instead.
    const QString sender = message().service();
    const QString path = message().path();
    if (sender.isEmpty() || path.isEmpty())
        return;

    const auto registration = registrationFor(sender, path);
    if (registration.first.isEmpty())
        return;

    readItem(registration.first, registration.second);
}

QString TrayWatcher::resolveIcon(const TrayItem &item, const QVariantMap &properties)
{
    const QString key = itemKey(item.service, item.path);
    QImage image;

    if (!item.iconName.isEmpty()) {
        // `IconThemePath` is also used by real peers as a flat directory. That
        // is not a QIcon theme root, so try its exact, bounded basename first.
        // Steam's `steam_tray_mono.png` is one such item in this session.
        if (!item.iconThemePath.isEmpty()) {
            image = loadTrayIconFromFlatThemePath(
                item.iconThemePath,
                item.iconName,
                drawnIconSize
            );
        }

        // An application that ships its own theme says where; that directory is
        // searched first and only for this lookup.
        QStringList paths = QIcon::themeSearchPaths();
        if (!item.iconThemePath.isEmpty() && !paths.contains(item.iconThemePath)) {
            QIcon::setThemeSearchPaths(QStringList {item.iconThemePath} + paths);
        }
        const QIcon themed = image.isNull()
            ? QIcon::fromTheme(item.iconName) : QIcon();
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

    // An icon is not part of what `TrayItem` compares — it is merged into the
    // published maps by `items()` — so an application that changed only its
    // icon would leave the list identical and never reach a binding.
    QHash<QString, QString> icons;
    for (const TrayItem &item : items) {
        const QString key = itemKey(item.service, item.path);
        icons.insert(key, m_iconSources.value(key));
    }

    const bool listChanged = m_items.replace(items);
    const bool iconsChanged = icons != m_publishedIcons;
    m_publishedIcons = icons;
    DiagnosticJournal::instance().record(
        CELESTINA_JOURNAL(Debug, "tray.publish")
            .number(QStringLiteral("registration_count"), m_registrations.size())
            .number(QStringLiteral("read_count"), m_read.size())
            .number(QStringLiteral("published_count"), items.size())
            .flag(QStringLiteral("list_changed"), listChanged)
            .flag(QStringLiteral("icons_changed"), iconsChanged)
    );
    if (listChanged || iconsChanged)
        emit changed();
}

void TrayWatcher::setUnavailable()
{
    // The items belonged to a session that no longer exists, and so do the
    // subscriptions to them.
    forgetItems();
    m_read.clear();
    for (const QString &key : m_iconSources.keys())
        m_icons->remove(key);
    m_iconSources.clear();
    m_iconRevisions.clear();
    m_readFailures.clear();
    m_publishedIcons.clear();
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
