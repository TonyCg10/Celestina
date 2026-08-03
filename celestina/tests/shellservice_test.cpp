#include <QtTest>

#include <QDBusConnection>
#include <QDBusError>
#include <QDBusMessage>
#include <QDBusPendingCallWatcher>
#include <QDBusPendingReply>

#include "shellservice.h"

namespace {
// The production name is never claimed here: a test must not displace, or be
// mistaken for, the shell that owns the session.
constexpr auto testServiceName = "org.celestina.ShellTest";
constexpr auto clientConnectionName = "celestina-shell-service-test";

// Both connections live on this thread, so every call is asynchronous and
// built by hand: a blocking call — including the introspection a
// `QDBusInterface` performs while constructing — would park the very thread
// that has to dispatch it.
template<typename Reply>
bool settle(Reply &reply)
{
    QDBusPendingCallWatcher watcher(reply);
    QSignalSpy finished(&watcher, &QDBusPendingCallWatcher::finished);
    return finished.wait(5000);
}
} // namespace

class ShellServiceTest final : public QObject
{
    Q_OBJECT

private slots:
    void initTestCase();
    void cleanupTestCase();

    void exportsItsVersionedInterface();
    void getStateCarriesItsVersion();
    void refusesAVerbItDoesNotServe();
    void refusesFocusWorkspaceWithoutUsableOptions();
    void refusesToPretendARequestWasSentWithoutAnAdapter();
    void refusesOverlayTogglesWithoutAControllerWired();

private:
    QDBusMessage callTo(const QString &interface, const QString &member) const
    {
        return QDBusMessage::createMethodCall(
            QString::fromLatin1(testServiceName),
            ShellService::objectPath(),
            interface,
            member
        );
    }

    QDBusMessage command(const QString &verb, const QVariantMap &options) const
    {
        QDBusMessage message = callTo(ShellService::interfaceName(), QStringLiteral("Command"));
        message.setArguments({verb, options});
        return message;
    }

    std::unique_ptr<ShellService> m_service;
    QDBusConnection m_client = QDBusConnection(QString());
};

void ShellServiceTest::initTestCase()
{
    QDBusConnection host = QDBusConnection::sessionBus();
    if (!host.isConnected())
        QSKIP("this session has no D-Bus session bus");

    // No compositor adapter: the service must answer truthfully about a
    // provider it does not have instead of inventing state.
    m_service = std::make_unique<ShellService>(nullptr);
    QVERIFY(host.registerObject(
        ShellService::objectPath(),
        m_service.get(),
        QDBusConnection::ExportAllSlots | QDBusConnection::ExportAllSignals
    ));
    QVERIFY(host.registerService(QString::fromLatin1(testServiceName)));

    m_client = QDBusConnection::connectToBus(
        QDBusConnection::SessionBus,
        QString::fromLatin1(clientConnectionName)
    );
    QVERIFY(m_client.isConnected());
}

void ShellServiceTest::exportsItsVersionedInterface()
{
    QDBusPendingReply<QString> reply = m_client.asyncCall(callTo(
        QStringLiteral("org.freedesktop.DBus.Introspectable"),
        QStringLiteral("Introspect")
    ));
    QVERIFY(settle(reply));
    QVERIFY2(reply.isValid(), qPrintable(reply.error().message()));

    const QString xml = reply.value();
    QVERIFY(xml.contains(QStringLiteral("<interface name=\"org.celestina.Shell1\">")));
    for (const QString &member : {
             QStringLiteral("<method name=\"GetState\">"),
             QStringLiteral("<method name=\"Command\">"),
             QStringLiteral("<signal name=\"Changed\">"),
             QStringLiteral("<signal name=\"CommandResult\">"),
         }) {
        QVERIFY2(xml.contains(member), qPrintable(member));
    }
}

void ShellServiceTest::cleanupTestCase()
{
    QDBusConnection::disconnectFromBus(QString::fromLatin1(clientConnectionName));

    if (m_service) {
        QDBusConnection host = QDBusConnection::sessionBus();
        host.unregisterService(QString::fromLatin1(testServiceName));
        host.unregisterObject(ShellService::objectPath());
        m_service.reset();
    }
}

void ShellServiceTest::getStateCarriesItsVersion()
{
    QDBusPendingReply<QVariantMap> reply = m_client.asyncCall(
        callTo(ShellService::interfaceName(), QStringLiteral("GetState"))
    );
    QVERIFY(settle(reply));
    QVERIFY2(reply.isValid(), qPrintable(reply.error().message()));

    const QVariantMap state = reply.value();
    QCOMPARE(state.value(QStringLiteral("version")).toInt(), 1);
    QCOMPARE(state.value(QStringLiteral("niriAvailable")).toBool(), false);
    QVERIFY(state.contains(QStringLiteral("workspaces")));
}

void ShellServiceTest::refusesAVerbItDoesNotServe()
{
    QDBusPendingReply<qulonglong> reply =
        m_client.asyncCall(command(QStringLiteral("launch-rocket"), QVariantMap()));
    QVERIFY(settle(reply));
    QVERIFY(!reply.isValid());
    QCOMPARE(reply.error().type(), QDBusError::UnknownMethod);
}

void ShellServiceTest::refusesFocusWorkspaceWithoutUsableOptions()
{
    const QList<QVariantMap> refused {
        QVariantMap(),
        QVariantMap {{QStringLiteral("output"), QStringLiteral("DP-1")}},
        QVariantMap {{QStringLiteral("index"), 2}},
        QVariantMap {
            {QStringLiteral("output"), QStringLiteral("DP-1")},
            {QStringLiteral("index"), 0},
        },
        QVariantMap {
            {QStringLiteral("output"), QStringLiteral("DP-1")},
            {QStringLiteral("index"), QStringLiteral("second")},
        },
    };

    for (const QVariantMap &options : refused) {
        QDBusPendingReply<qulonglong> reply = m_client.asyncCall(
            command(QStringLiteral("focus-workspace"), options)
        );
        QVERIFY(settle(reply));
        QVERIFY(!reply.isValid());
        QCOMPARE(reply.error().type(), QDBusError::InvalidArgs);
    }
}

void ShellServiceTest::refusesToPretendARequestWasSentWithoutAnAdapter()
{
    QDBusPendingReply<qulonglong> reply = m_client.asyncCall(command(
        QStringLiteral("focus-workspace"),
        QVariantMap {
            {QStringLiteral("output"), QStringLiteral("DP-1")},
            {QStringLiteral("index"), 2},
        }
    ));
    QVERIFY(settle(reply));
    // A request that was never sent must not come back as a pending id.
    QVERIFY(!reply.isValid());
    QCOMPARE(reply.error().type(), QDBusError::Failed);
}

// `initTestCase` never wires a launcher or clipboard controller — the same
// "the shell exists but this surface does not" shape
// `refusesToPretendARequestWasSentWithoutAnAdapter` already covers for the
// compositor adapter.
void ShellServiceTest::refusesOverlayTogglesWithoutAControllerWired()
{
    for (const QString &verb : {
             QStringLiteral("launcher-toggle"),
             QStringLiteral("clipboard-toggle"),
         }) {
        QDBusPendingReply<qulonglong> reply =
            m_client.asyncCall(command(verb, QVariantMap()));
        QVERIFY(settle(reply));
        QVERIFY2(!reply.isValid(), qPrintable(verb));
        QCOMPARE(reply.error().type(), QDBusError::Failed);
    }
}

QTEST_MAIN(ShellServiceTest)

#include "shellservice_test.moc"
