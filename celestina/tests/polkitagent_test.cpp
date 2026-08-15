// R8-P-B. The agent polkitd calls, and what it does when polkitd is not the
// one calling.
//
// The registration cases are about continuity: a session with no agent gets no
// prompts, so a polkitd that restarts has to find this one again without the
// shell restarting. The request cases are about the same rule R8-P-A holds
// from the other side — nothing here produces an authorization, and every way
// a request can end without one ends as a cancellation polkitd is told about.
//
// A stand-in authority on the session bus stands for polkitd, because the real
// one lives on the system bus, refuses a second agent for a session, and would
// make this regression depend on whichever agent the author's machine happens
// to be running. What is proven here is the conversation with an authority,
// not the author's polkit configuration, which is `VAL-R8`.

#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusMessage>
#include <QDBusMetaType>
#include <QDBusReply>
#include <QDir>
#include <QFile>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

#include <unistd.h>

#include "polkitagent.h"

namespace {

const QString authorityService = QStringLiteral("org.freedesktop.PolicyKit1");
const QString authorityPath =
    QStringLiteral("/org/freedesktop/PolicyKit1/Authority");

// A stand-in for polkitd: it owns polkit's name on the session bus, records
// every registration, and can be taken away and brought back to stand for a
// restart.
class FakeAuthority : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.freedesktop.PolicyKit1.Authority")

public:
    explicit FakeAuthority(QObject *parent = nullptr) : QObject(parent) {}

    bool claim(QDBusConnection bus)
    {
        return bus.registerObject(authorityPath, this,
                                  QDBusConnection::ExportAllSlots)
            && bus.registerService(authorityService);
    }

    void withdraw(QDBusConnection bus)
    {
        bus.unregisterService(authorityService);
        bus.unregisterObject(authorityPath);
    }

    int registrations = 0;
    QString lastSessionId;
    QString lastObjectPath;

public slots:
    void RegisterAuthenticationAgent(const PolkitIdentity &subject,
                                     const QString &locale,
                                     const QString &objectPath)
    {
        Q_UNUSED(locale)
        ++registrations;
        lastSessionId =
            subject.details.value(QStringLiteral("session-id")).toString();
        lastObjectPath = objectPath;
    }
};

QString writeFakeConverse(const QDir &dir, const QString &body)
{
    const QString path =
        dir.filePath(QStringLiteral("celestina-polkit-converse"));
    QFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Text))
        return QString();
    file.write(body.toUtf8());
    file.close();
    file.setPermissions(QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
    return path;
}

QList<PolkitIdentity> thisUser()
{
    PolkitIdentity identity;
    identity.kind = QStringLiteral("unix-user");
    identity.details.insert(QStringLiteral("uid"), uint(::getuid()));
    return {identity};
}

} // namespace

class PolkitAgentTest : public QObject
{
    Q_OBJECT

private slots:
    void initTestCase();
    void init();
    void cleanup();

    void anAgentRegistersForItsOwnSession();
    void aRestartedAuthorityFindsTheAgentAgain();
    void anAuthorityThatRefusesLeavesNoRegistration();
    void aRequestReachesAPromptAndThenTheHelper();
    void twoRequestsAreAnsweredIndependently();
    void aCancelledRequestEndsWithoutAVerdict();
    void aDismissedPromptEndsAsACancellation();
    void anIdentityThisSessionCannotAskIsRefused();

private:
    QDBusConnection m_bus = QDBusConnection::sessionBus();
    FakeAuthority *m_authority = nullptr;
    QTemporaryDir *m_dir = nullptr;
};

void PolkitAgentTest::initTestCase()
{
    if (!m_bus.isConnected())
        QSKIP("No session bus; the agent's interface cannot be exercised.");
}

void PolkitAgentTest::init()
{
    qDBusRegisterMetaType<PolkitIdentity>();
    qDBusRegisterMetaType<QList<PolkitIdentity>>();
    qDBusRegisterMetaType<QMap<QString, QString>>();

    m_dir = new QTemporaryDir();
    QVERIFY(m_dir->isValid());
    m_authority = new FakeAuthority(this);
    QVERIFY(m_authority->claim(m_bus));
}

void PolkitAgentTest::cleanup()
{
    m_authority->withdraw(m_bus);
    delete m_authority;
    m_authority = nullptr;
    qunsetenv("CELESTINA_POLKIT_CONVERSE");
    delete m_dir;
    m_dir = nullptr;
}

void PolkitAgentTest::anAgentRegistersForItsOwnSession()
{
    PolkitAgent agent;
    QCOMPARE(agent.attach(m_bus, QStringLiteral("7")),
             PolkitAgent::Attachment::Registered);
    QCOMPARE(m_authority->registrations, 1);
    // What was registered is this session and this object, not a guess at
    // either: polkitd matches both when it decides who to call.
    QCOMPARE(m_authority->lastSessionId, QStringLiteral("7"));
    QCOMPARE(m_authority->lastObjectPath, PolkitAgent::objectPath());
}

// A polkitd upgrade restarts polkitd. An agent that did not come back would
// leave the session unable to authorize anything until the shell restarted,
// which is the kind of failure nobody connects to the upgrade that caused it.
void PolkitAgentTest::aRestartedAuthorityFindsTheAgentAgain()
{
    PolkitAgent agent;
    QCOMPARE(agent.attach(m_bus, QStringLiteral("7")),
             PolkitAgent::Attachment::Registered);
    QSignalSpy changed(&agent, &PolkitAgent::registeredChanged);

    m_authority->withdraw(m_bus);
    QTRY_VERIFY_WITH_TIMEOUT(changed.count() >= 1, 4000);
    QCOMPARE(changed.at(0).at(0).toBool(), false);

    QVERIFY(m_authority->claim(m_bus));
    QTRY_VERIFY_WITH_TIMEOUT(m_authority->registrations >= 2, 4000);
    QCOMPARE(m_authority->lastSessionId, QStringLiteral("7"));
}

// An authority that will not have this agent is reported, not papered over.
// A shell that claimed to be registered would leave the person waiting for a
// prompt that is never coming.
void PolkitAgentTest::anAuthorityThatRefusesLeavesNoRegistration()
{
    m_authority->withdraw(m_bus);

    PolkitAgent agent;
    QCOMPARE(agent.attach(m_bus, QStringLiteral("7")),
             PolkitAgent::Attachment::Refused);
}

void PolkitAgentTest::aRequestReachesAPromptAndThenTheHelper()
{
    const QString converse = writeFakeConverse(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\nread cookie\n"
                       "printf 'secret Password%%3A\\n'\nread answer\nexit 0\n"));
    qputenv("CELESTINA_POLKIT_CONVERSE", converse.toLocal8Bit());

    PolkitAgent agent;
    QCOMPARE(agent.attach(m_bus, QStringLiteral("7")),
             PolkitAgent::Attachment::Registered);

    QSignalSpy asked(&agent, &PolkitAgent::authenticationRequested);
    QSignalSpy prompt(&agent, &PolkitAgent::secretRequested);
    QSignalSpy done(&agent, &PolkitAgent::authenticationFinished);

    agent.BeginAuthentication(QStringLiteral("org.freedesktop.policykit.exec"),
                              QStringLiteral("Authentication is required"),
                              QStringLiteral("dialog-password"), {},
                              QStringLiteral("cookie-1"), thisUser());

    QCOMPARE(asked.count(), 1);
    // Every string the prompt is given came from polkitd, unaltered.
    QCOMPARE(asked.at(0).at(1).toString(),
             QStringLiteral("org.freedesktop.policykit.exec"));
    QCOMPARE(asked.at(0).at(2).toString(),
             QStringLiteral("Authentication is required"));
    QCOMPARE(agent.pendingCount(), 1);

    QVERIFY(prompt.wait(4000));
    QCOMPARE(prompt.at(0).at(0).toString(), QStringLiteral("cookie-1"));
    agent.respond(QStringLiteral("cookie-1"), QStringLiteral("whatever"));

    QVERIFY(done.wait(4000));
    QCOMPARE(done.at(0).at(1).toBool(), true);
    QCOMPARE(agent.pendingCount(), 0);
}

// polkitd asks about one action at a time per prompt, but nothing stops two
// actions being started at once. Each carries its own cookie, and an answer
// given to one must not end the other.
void PolkitAgentTest::twoRequestsAreAnsweredIndependently()
{
    const QString converse = writeFakeConverse(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\nread cookie\n"
                       "printf 'secret Password%%3A\\n'\nread answer\n"
                       "test \"$answer\" = right && exit 0\nexit 1\n"));
    qputenv("CELESTINA_POLKIT_CONVERSE", converse.toLocal8Bit());

    PolkitAgent agent;
    QCOMPARE(agent.attach(m_bus, QStringLiteral("7")),
             PolkitAgent::Attachment::Registered);
    QSignalSpy prompt(&agent, &PolkitAgent::secretRequested);
    QSignalSpy done(&agent, &PolkitAgent::authenticationFinished);

    agent.BeginAuthentication(QStringLiteral("action.one"),
                              QStringLiteral("one"), QString(), {},
                              QStringLiteral("cookie-1"), thisUser());
    agent.BeginAuthentication(QStringLiteral("action.two"),
                              QStringLiteral("two"), QString(), {},
                              QStringLiteral("cookie-2"), thisUser());
    QCOMPARE(agent.pendingCount(), 2);

    QTRY_VERIFY_WITH_TIMEOUT(prompt.count() >= 2, 4000);
    agent.respond(QStringLiteral("cookie-2"), QStringLiteral("right"));

    QTRY_VERIFY_WITH_TIMEOUT(done.count() >= 1, 4000);
    QCOMPARE(done.at(0).at(0).toString(), QStringLiteral("cookie-2"));
    QCOMPARE(done.at(0).at(1).toBool(), true);
    // The other one is still waiting for its own answer.
    QCOMPARE(agent.pendingCount(), 1);

    agent.respond(QStringLiteral("cookie-1"), QStringLiteral("wrong"));
    QTRY_VERIFY_WITH_TIMEOUT(done.count() >= 2, 4000);
    QCOMPARE(done.at(1).at(0).toString(), QStringLiteral("cookie-1"));
    QCOMPARE(done.at(1).at(1).toBool(), false);
}

// polkitd cancels when the action it was asked about goes away. The prompt
// ends, and it ends as "nobody authorized this".
void PolkitAgentTest::aCancelledRequestEndsWithoutAVerdict()
{
    const QString converse = writeFakeConverse(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\nread cookie\n"
                       "printf 'secret Password%%3A\\n'\nread answer\nexit 0\n"));
    qputenv("CELESTINA_POLKIT_CONVERSE", converse.toLocal8Bit());

    PolkitAgent agent;
    QCOMPARE(agent.attach(m_bus, QStringLiteral("7")),
             PolkitAgent::Attachment::Registered);
    QSignalSpy done(&agent, &PolkitAgent::authenticationFinished);

    agent.BeginAuthentication(QStringLiteral("action.one"),
                              QStringLiteral("one"), QString(), {},
                              QStringLiteral("cookie-1"), thisUser());
    QCOMPARE(agent.pendingCount(), 1);

    agent.CancelAuthentication(QStringLiteral("cookie-1"));
    QCOMPARE(done.count(), 1);
    QCOMPARE(done.at(0).at(1).toBool(), false);
    QCOMPARE(agent.pendingCount(), 0);

    // An answer arriving for a request that is over is dropped, not held for
    // whatever comes next.
    agent.respond(QStringLiteral("cookie-1"), QStringLiteral("late"));
    QCOMPARE(done.count(), 1);
}

void PolkitAgentTest::aDismissedPromptEndsAsACancellation()
{
    const QString converse = writeFakeConverse(
        QDir(m_dir->path()),
        QStringLiteral("#!/bin/sh\nread cookie\n"
                       "printf 'secret Password%%3A\\n'\nread answer\nexit 0\n"));
    qputenv("CELESTINA_POLKIT_CONVERSE", converse.toLocal8Bit());

    PolkitAgent agent;
    QCOMPARE(agent.attach(m_bus, QStringLiteral("7")),
             PolkitAgent::Attachment::Registered);
    QSignalSpy done(&agent, &PolkitAgent::authenticationFinished);

    agent.BeginAuthentication(QStringLiteral("action.one"),
                              QStringLiteral("one"), QString(), {},
                              QStringLiteral("cookie-1"), thisUser());
    agent.dismiss(QStringLiteral("cookie-1"));

    QCOMPARE(done.count(), 1);
    QCOMPARE(done.at(0).at(1).toBool(), false);
    QCOMPARE(agent.pendingCount(), 0);
}

// polkitd may offer only identities this session has no way to ask — another
// person's account, or a group. Prompting anyway would be asking the person in
// front of the screen for a password that cannot help them.
void PolkitAgentTest::anIdentityThisSessionCannotAskIsRefused()
{
    PolkitAgent agent;
    QCOMPARE(agent.attach(m_bus, QStringLiteral("7")),
             PolkitAgent::Attachment::Registered);
    QSignalSpy asked(&agent, &PolkitAgent::authenticationRequested);

    PolkitIdentity group;
    group.kind = QStringLiteral("unix-group");
    group.details.insert(QStringLiteral("gid"), uint(0));

    agent.BeginAuthentication(QStringLiteral("action.one"),
                              QStringLiteral("one"), QString(), {},
                              QStringLiteral("cookie-1"), {group});

    QCOMPARE(asked.count(), 0);
    QCOMPARE(agent.pendingCount(), 0);
}

QTEST_MAIN(PolkitAgentTest)

#include "polkitagent_test.moc"
