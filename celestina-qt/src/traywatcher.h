#pragma once

#include <QDBusContext>
#include <QHash>
#include <QObject>
#include <QVariantList>

#include <QSharedPointer>

#include "trayicons.h"
#include "trayitems.h"
#include "traymenu.h"
#include "traywatcherservice.h"

class QDBusServiceWatcher;
class QTimer;

// The panel's StatusNotifierItem host.
//
// It registers as a host with whatever watcher owns the session, reads each
// item's properties and publishes what QML can show. Everything is
// asynchronous: these are other applications' processes, and one of them being
// slow to answer must never be something the panel waits for.
//
// It is a *host*, not the watcher. Noctalia owns `org.kde.StatusNotifierWatcher`
// on this session today; owning it is what R8 needs before Noctalia leaves,
// because with no watcher at all no application publishes a tray item to
// anyone.
//
// This is manual C++ for the same reason `DevicesClient` is: the conversation
// is QtDBus, the icons and menus at the other end of it are Qt's to render, and
// no part of it is domain logic. What an item *means* lives in `trayitems.h`.
class TrayWatcher final : public QObject, protected QDBusContext
{
    Q_OBJECT
    Q_PROPERTY(bool available READ available NOTIFY changed)
    Q_PROPERTY(QVariantList items READ items NOTIFY changed)

public:
    TrayWatcher(QSharedPointer<TrayIconCache> icons, QObject *parent = nullptr);
    // Every signal subscription this host made is undone here. A match rule is
    // state on the bus connection, not on this object, so it does not leave
    // with the object unless it is removed.
    ~TrayWatcher() override;

    bool available() const { return m_available; }
    // Each item as QML reads it, with the source of whatever icon could be
    // resolved for it — empty when neither a theme nor the item's own pixels
    // gave one, which the drawer answers with the item's name.
    QVariantList items() const;

    // Asks an item to do what a left click means to it. A click is a request
    // here too: the application decides what happens, and the panel learns
    // nothing back except through the item's own properties.
    Q_INVOKABLE void activate(const QString &service, const QString &path, int x, int y);
    Q_INVOKABLE void secondaryActivate(
        const QString &service,
        const QString &path,
        int x,
        int y
    );
    // Asks an item for its menu. The answer arrives as `menuReady`, because
    // reading another application's menu is a conversation with it — the item
    // is told the menu is about to show, and may rebuild it before answering.
    Q_INVOKABLE void requestMenu(const QString &service, const QString &path);
    // Tells an item one of its entries was chosen. What that does is the
    // application's business; the panel learns nothing back.
    Q_INVOKABLE void triggerMenuEntry(
        const QString &service,
        const QString &path,
        int entryId
    );

signals:
    void menuReady(const QString &service, const QString &path, const QVariantList &entries);

    void changed();

private slots:
    void watcherOwnerChanged(const QString &service, const QString &was, const QString &now);
    void itemRegistered(const QString &entry);
    void itemUnregistered(const QString &entry);
    void itemPropertiesChanged();

private:
    void attach();
    void refreshRegistrations();
    void readItem(const QString &service, const QString &path);
    // Forgets one registration and everything keyed by it. Used both when the
    // watcher says an item left and when a registry read shows one gone.
    void removeRegistration(const QString &service, const QString &path);
    // An item answered `GetAll` with an error, or not at all. Retried once,
    // then published from its registration rather than dropped.
    void itemUnreadable(const QString &service, const QString &path, const QString &reason);
    void watchItem(const QString &service, const QString &path);
    // Undoes exactly what `watchItem` subscribed to, and forgets the item's
    // owner.
    void unwatchItem(const QString &service, const QString &path);
    void forgetItems();
    // Which registration a signal from `sender` about `path` belongs to, empty
    // when none does. The sender is always a unique name; the registration may
    // be under a well-known one.
    QPair<QString, QString> registrationFor(const QString &sender, const QString &path) const;
    void publish();
    void setUnavailable();
    void readMenuLayout(const QString &service, const QString &menuPath, const QString &itemPath);
    // Resolves the item's icon into the shared cache and returns the source
    // QML should ask for, or an empty string when there is none to draw.
    QString resolveIcon(const TrayItem &item, const QVariantMap &properties);

    // Registration order is the order the drawer shows, so the list is kept
    // rather than sorted: an application that has been there all session should
    // not move because another one restarted.
    QList<QPair<QString, QString>> m_registrations;
    // The unique bus name behind each registered service. An item's own signals
    // arrive from its unique name whatever name it registered under, so without
    // this a well-known registration would never be recognized as the sender's
    // and would silently stop updating.
    QHash<QString, QString> m_registeredOwners;
    QHash<QString, TrayItem> m_read;
    // The icon source per item key, and how many times it has changed: an
    // application that swaps its icon keeps the same key, so the number is what
    // makes QML ask again.
    QHash<QString, QString> m_iconSources;
    QHash<QString, int> m_iconRevisions;
    // The icon sources QML has already been told about. An item can change its
    // icon without changing anything `TrayItem` compares, and `items()` merges
    // the source in at read time, so without this such a change would never
    // reach a binding.
    QHash<QString, QString> m_publishedIcons;
    // How many times each item's properties have failed to arrive. One retry,
    // because a peer that is still exporting its object is the ordinary case
    // and a peer that will never answer must not be asked forever.
    QHash<QString, int> m_readFailures;
    // Which registry read is current. Re-reading is triggered by every watcher
    // owner change — including this shell acquiring the name itself — so two
    // reads can be in flight, and the older one's reply would otherwise clear
    // the newer one's items and re-issue every property read.
    quint64 m_registryGeneration = 0;
    QSharedPointer<TrayIconCache> m_icons;
    TrayItems m_items;
    QDBusServiceWatcher *m_watcherPresence;
    // A restarted host reads the foreign registry once immediately and once
    // after host registration has settled. The second bounded reconciliation
    // recovers a premature empty/error snapshot without polling forever.
    QTimer *m_registryRefresh;
    // The registry itself, started only when nobody else is being it. Noctalia
    // owns the name today, so this stays dormant until it leaves.
    TrayWatcherService *m_registry;
    bool m_available = false;
    bool m_hostRegistered = false;
};
