#pragma once

#include <QDBusContext>
#include <QHash>
#include <QObject>
#include <QStringList>

class QDBusServiceWatcher;

// The session's StatusNotifierWatcher, when nobody else is being it.
//
// This is the other half of a tray, and the half a session cannot do without:
// an application looks for this name and publishes nothing at all if it is
// missing. Noctalia owns it today, so this stays dormant — but R8 is when
// Noctalia leaves, and a tray with a host and no watcher is no tray.
//
// It is deliberately separate from `TrayWatcher`. Being *a* host and being
// *the* watcher are different jobs with different lifetimes: the host reads
// items and may come and go with the panel, while the watcher is a registry
// other applications depend on for as long as the session lasts. When this
// service owns the name, the panel's own host talks to it exactly like it would
// talk to anyone else's.
class TrayWatcherService final : public QObject, protected QDBusContext
{
    Q_OBJECT
    // QtDBus takes the exported interface from this class info, and the wire
    // names of the members from their C++ names — which is why they are
    // capitalized against this project's usual style.
    Q_CLASSINFO("D-Bus Interface", "org.kde.StatusNotifierWatcher")
    Q_PROPERTY(QStringList RegisteredStatusNotifierItems READ registeredItems)
    Q_PROPERTY(bool IsStatusNotifierHostRegistered READ isHostRegistered)
    Q_PROPERTY(int ProtocolVersion READ protocolVersion)

public:
    // The name is a parameter so a test can claim one of its own rather than
    // displacing the session's tray.
    explicit TrayWatcherService(QString serviceName, QObject *parent = nullptr);

    // Claims the name and exports the registry. Returns false when someone else
    // already owns it — which is not a failure, only the answer to "is anyone
    // being the watcher".
    bool claim();
    bool owns() const { return m_owned; }

    QStringList registeredItems() const { return m_items; }
    bool isHostRegistered() const { return !m_hosts.isEmpty(); }
    static int protocolVersion() { return 0; }

    static QString wellKnownName();
    static QString objectPath();

public slots:
    // The specification allows either a bus name or an object path here, and
    // applications on this session use both: whoever sent the message is the
    // service when only a path arrived.
    void RegisterStatusNotifierItem(const QString &serviceOrPath);
    void RegisterStatusNotifierHost(const QString &service);

signals:
    void StatusNotifierItemRegistered(const QString &item);
    void StatusNotifierItemUnregistered(const QString &item);
    void StatusNotifierHostRegistered();
    void StatusNotifierHostUnregistered();

private slots:
    void ownerVanished(const QString &service);

private:
    void watchOwner(const QString &service);

    QString m_serviceName;
    QStringList m_items;
    QStringList m_hosts;
    // Which registrations belong to which bus name, so everything an
    // application published leaves with it.
    QHash<QString, QStringList> m_itemsByOwner;
    QDBusServiceWatcher *m_owners;
    bool m_owned = false;
};
