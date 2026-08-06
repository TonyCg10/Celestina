#include "traywatcherservice.h"

#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusServiceWatcher>
#include <QDebug>

#include "trayitems.h"

TrayWatcherService::TrayWatcherService(QString serviceName, QObject *parent)
    : QObject(parent)
    , m_serviceName(std::move(serviceName))
    , m_owners(new QDBusServiceWatcher(this))
{
    // Owners are added as they register; constructing with a service name would
    // leave an empty one watched forever.
    m_owners->setConnection(QDBusConnection::sessionBus());
    m_owners->setWatchMode(QDBusServiceWatcher::WatchForUnregistration);

    connect(
        m_owners,
        &QDBusServiceWatcher::serviceUnregistered,
        this,
        &TrayWatcherService::ownerVanished
    );
}

QString TrayWatcherService::wellKnownName()
{
    return QStringLiteral("org.kde.StatusNotifierWatcher");
}

QString TrayWatcherService::objectPath()
{
    return QStringLiteral("/StatusNotifierWatcher");
}

bool TrayWatcherService::claim()
{
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected() || m_owned)
        return m_owned;

    // The object first, so an application that sees the name always finds the
    // registry behind it.
    if (!bus.registerObject(
            objectPath(),
            this,
            QDBusConnection::ExportAllSlots | QDBusConnection::ExportAllSignals
                | QDBusConnection::ExportAllProperties
        )) {
        qWarning().noquote()
            << "Celestina could not export a tray watcher:" << bus.lastError().message();
        return false;
    }

    m_owned = bus.registerService(m_serviceName);
    if (!m_owned) {
        // Someone else is the watcher. That is the normal state while another
        // shell is running, not a failure.
        bus.unregisterObject(objectPath());
    }
    return m_owned;
}

void TrayWatcherService::watchOwner(const QString &service)
{
    if (!m_owners->watchedServices().contains(service))
        m_owners->addWatchedService(service);
}

void TrayWatcherService::RegisterStatusNotifierItem(const QString &serviceOrPath)
{
    // An application may name itself, or only its object path — in which case
    // the sender of this very message is the service.
    const QString sender = calledFromDBus() ? message().service() : QString();
    const QString entry = serviceOrPath.startsWith(u'/')
        ? sender + serviceOrPath
        : serviceOrPath;
    const QString owner = serviceOrPath.startsWith(u'/') ? sender : serviceOrPath;

    if (entry.isEmpty() || owner.isEmpty() || m_items.contains(entry))
        return;

    // One client can call this as many times as it likes with a different path
    // each time, and every accepted registration costs the host that reads them
    // four signal match rules on its own bus connection. Past the bus daemon's
    // per-connection quota nothing on that connection can subscribe to anything
    // again — a failure that outlasts the tray and reaches the whole panel. So
    // an over-quota or overlong registration is refused, and refusing it is the
    // whole answer: the registry keeps serving everyone already in it.
    if (entry.size() > maxTrayPathLength || m_items.size() >= maxTrayItems)
        return;

    m_items.append(entry);
    m_itemsByOwner[owner].append(entry);
    watchOwner(owner);
    emit StatusNotifierItemRegistered(entry);
}

void TrayWatcherService::RegisterStatusNotifierHost(const QString &service)
{
    const QString host = service.isEmpty() && calledFromDBus() ? message().service() : service;
    if (host.isEmpty() || m_hosts.contains(host))
        return;

    // A host is watched for unregistration exactly like an item's owner is, so
    // an unbounded list of them buys the same match-rule exhaustion by another
    // door. A session has one or two hosts; the same bound covers them.
    if (host.size() > maxTrayPathLength || m_hosts.size() >= maxTrayItems)
        return;

    const bool first = m_hosts.isEmpty();
    m_hosts.append(host);
    watchOwner(host);
    // Applications wait for this before they publish anything.
    if (first)
        emit StatusNotifierHostRegistered();
}

void TrayWatcherService::ownerVanished(const QString &service)
{
    // Everything an application published leaves with it: an item whose owner
    // is gone is a control nobody can operate.
    const QStringList orphaned = m_itemsByOwner.take(service);
    for (const QString &entry : orphaned) {
        m_items.removeAll(entry);
        emit StatusNotifierItemUnregistered(entry);
    }

    if (m_hosts.removeAll(service) > 0 && m_hosts.isEmpty())
        emit StatusNotifierHostUnregistered();

    // The name is gone and `take()` already removed everything it had, so
    // nothing is left to watch it for. Keeping the watch is how a session that
    // has seen applications come and go accumulates match rules for dead names
    // until the connection cannot subscribe to anything else.
    m_owners->removeWatchedService(service);
}
