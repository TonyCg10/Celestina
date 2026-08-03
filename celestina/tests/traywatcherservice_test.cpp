#include <QtTest>

#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>

#include "traywatcherservice.h"

namespace {
// Never the session's own name: a test must not become the tray every
// application on this desktop is publishing to.
constexpr auto testServiceName = "org.celestina.StatusNotifierWatcherTest";
constexpr auto clientConnectionName = "celestina-tray-watcher-test";

template<typename Reply>
bool settle(Reply &reply)
{
    QDBusPendingCallWatcher watcher(reply);
    QSignalSpy finished(&watcher, &QDBusPendingCallWatcher::finished);
    return finished.wait(5000);
}
} // namespace

class TrayWatcherServiceTest final : public QObject
{
    Q_OBJECT

private slots:
    void initTestCase();
    void cleanupTestCase();

    void refusesTheNameWhenSomeoneElseIsTheWatcher();
    void registersAnItemThatNamedItsBusName();
    void registersAnItemThatNamedOnlyItsPath();
    void announcesTheFirstHostOnly();
    void everythingAnOwnerPublishedLeavesWithIt();

private:
    QDBusMessage callTo(const QString &member, const QVariantList &arguments) const
    {
        QDBusMessage message = QDBusMessage::createMethodCall(
            QString::fromLatin1(testServiceName),
            TrayWatcherService::objectPath(),
            TrayWatcherService::wellKnownName(),
            member
        );
        message.setArguments(arguments);
        return message;
    }

    std::unique_ptr<TrayWatcherService> m_service;
    QDBusConnection m_client = QDBusConnection(QString());
};

void TrayWatcherServiceTest::initTestCase()
{
    if (!QDBusConnection::sessionBus().isConnected())
        QSKIP("this session has no D-Bus session bus");

    m_service = std::make_unique<TrayWatcherService>(QString::fromLatin1(testServiceName));
    QVERIFY(m_service->claim());
    QVERIFY(m_service->owns());

    m_client = QDBusConnection::connectToBus(
        QDBusConnection::SessionBus,
        QString::fromLatin1(clientConnectionName)
    );
    QVERIFY(m_client.isConnected());
}

void TrayWatcherServiceTest::cleanupTestCase()
{
    QDBusConnection::disconnectFromBus(QString::fromLatin1(clientConnectionName));
    if (m_service) {
        QDBusConnection bus = QDBusConnection::sessionBus();
        bus.unregisterService(QString::fromLatin1(testServiceName));
        bus.unregisterObject(TrayWatcherService::objectPath());
        m_service.reset();
    }
}

void TrayWatcherServiceTest::refusesTheNameWhenSomeoneElseIsTheWatcher()
{
    // A second watcher on a name already taken is the normal state while
    // another shell runs — the answer is "no", not an error.
    TrayWatcherService second(QString::fromLatin1(testServiceName));
    QVERIFY(!second.claim());
    QVERIFY(!second.owns());
}

void TrayWatcherServiceTest::registersAnItemThatNamedItsBusName()
{
    QSignalSpy registered(m_service.get(), &TrayWatcherService::StatusNotifierItemRegistered);

    QDBusPendingReply<> reply = m_client.asyncCall(
        callTo(QStringLiteral("RegisterStatusNotifierItem"), {QStringLiteral(":9.99")})
    );
    QVERIFY(settle(reply));
    QVERIFY2(reply.isValid(), qPrintable(reply.error().message()));

    QTRY_COMPARE(registered.count(), 1);
    QVERIFY(m_service->registeredItems().contains(QStringLiteral(":9.99")));
    // The same registration twice is one item, not two.
    QDBusPendingReply<> again = m_client.asyncCall(
        callTo(QStringLiteral("RegisterStatusNotifierItem"), {QStringLiteral(":9.99")})
    );
    QVERIFY(settle(again));
    QCOMPARE(m_service->registeredItems().count(QStringLiteral(":9.99")), 1);
}

void TrayWatcherServiceTest::registersAnItemThatNamedOnlyItsPath()
{
    // The shape half this session's items use: the path alone, with the sender
    // being the service it belongs to.
    QDBusPendingReply<> reply = m_client.asyncCall(callTo(
        QStringLiteral("RegisterStatusNotifierItem"),
        {QStringLiteral("/StatusNotifierItem")}
    ));
    QVERIFY(settle(reply));
    QVERIFY2(reply.isValid(), qPrintable(reply.error().message()));

    const QString expected = m_client.baseService() + QStringLiteral("/StatusNotifierItem");
    QTRY_VERIFY(m_service->registeredItems().contains(expected));
}

void TrayWatcherServiceTest::announcesTheFirstHostOnly()
{
    QSignalSpy announced(m_service.get(), &TrayWatcherService::StatusNotifierHostRegistered);
    QVERIFY(!m_service->isHostRegistered());

    for (const auto *host : {"org.kde.StatusNotifierHost-1", "org.kde.StatusNotifierHost-2"}) {
        QDBusPendingReply<> reply = m_client.asyncCall(callTo(
            QStringLiteral("RegisterStatusNotifierHost"),
            {QString::fromLatin1(host)}
        ));
        QVERIFY(settle(reply));
    }

    QTRY_VERIFY(m_service->isHostRegistered());
    // Applications wait for the announcement that a tray exists; a second host
    // arriving is not that news again.
    QCOMPARE(announced.count(), 1);
}

void TrayWatcherServiceTest::everythingAnOwnerPublishedLeavesWithIt()
{
    QSignalSpy unregistered(
        m_service.get(),
        &TrayWatcherService::StatusNotifierItemUnregistered
    );

    // A connection that publishes an item and then goes away, which is what an
    // application quitting looks like from here.
    const QString name = QStringLiteral("celestina-tray-owner-test");
    QString expected;
    {
        // Scoped deliberately: `disconnectFromBus` closes a connection only
        // once the last `QDBusConnection` referring to it is gone, so holding
        // one here would keep the "application" alive and nothing would leave.
        QDBusConnection owner =
            QDBusConnection::connectToBus(QDBusConnection::SessionBus, name);
        QVERIFY(owner.isConnected());
        expected = owner.baseService() + QStringLiteral("/StatusNotifierItem");

        QDBusPendingReply<> reply = owner.asyncCall(callTo(
            QStringLiteral("RegisterStatusNotifierItem"),
            {QStringLiteral("/StatusNotifierItem")}
        ));
        QVERIFY(settle(reply));
        QTRY_VERIFY(m_service->registeredItems().contains(expected));
    }

    QDBusConnection::disconnectFromBus(name);
    QTRY_VERIFY(!m_service->registeredItems().contains(expected));
    QCOMPARE(unregistered.count(), 1);
}

QTEST_MAIN(TrayWatcherServiceTest)

#include "traywatcherservice_test.moc"
