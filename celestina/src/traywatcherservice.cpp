#include "traywatcherservice.h"

#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusServiceWatcher>
#include <QDebug>

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

    if (orphaned.isEmpty() && !m_itemsByOwner.contains(service))
        m_owners->removeWatchedService(service);
}
