// R8-P-C. When this shell may ask for a password, and what it does when it
// may not.
//
// The rule under test is that there is no lesser prompt. A password typed into
// a surface that does not hold the keyboard can be read by whatever does, so a
// shell that cannot take an exclusive grab must refuse the request rather than
// collect the password anyway and hope. Refusing makes the action fail exactly
// as it does on a machine with no graphical agent at all — worse for the
// person, safe for their password.
//
// The decision is exercised as a function of its own. A regression that only
// drove the controller under `offscreen` would pass while proving nothing: the
// QML component is not loadable there either, so it would refuse for that
// reason and never reach the question about the keyboard.

#include <QDir>
#include <QFile>
#include <QSignalSpy>
#include <QTemporaryDir>
#include <QTest>

#include <unistd.h>

#include "polkitagent.h"
#include "polkitpromptcontroller.h"
#include "surfacemanager.h"

namespace {

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

class PolkitPromptTest : public QObject
{
    Q_OBJECT

private slots:
    void aPlatformWithoutLayerShellCannotPrompt();
    void aReadyShellOnARealCompositorMayPrompt();
    void aSecondRequestIsRefusedWhileOneIsShowing();
    void everyRefusalHasItsOwnReason();
    void anUnpromptableRequestIsDismissedRatherThanLeftOpen();
};

// The case this unit exists for. Everything else about the shell can be
// perfect and the answer is still no.
void PolkitPromptTest::aPlatformWithoutLayerShellCannotPrompt()
{
    QCOMPARE(promptRefusal(true, false, LayerShellSupport::Headless, true),
             PromptRefusal::NoKeyboardGrab);
    QCOMPARE(promptRefusal(true, false, LayerShellSupport::Unavailable, true),
             PromptRefusal::NoKeyboardGrab);
}

void PolkitPromptTest::aReadyShellOnARealCompositorMayPrompt()
{
    QCOMPARE(promptRefusal(true, false, LayerShellSupport::Available, true),
             PromptRefusal::None);
}

void PolkitPromptTest::aSecondRequestIsRefusedWhileOneIsShowing()
{
    QCOMPARE(promptRefusal(true, true, LayerShellSupport::Available, true),
             PromptRefusal::AlreadyShowing);
}

// Each refusal says which one it was. They are not interchangeable: "no layer
// shell" is a session that will never prompt, and "already showing" is one
// that will prompt again in a moment.
void PolkitPromptTest::everyRefusalHasItsOwnReason()
{
    QCOMPARE(promptRefusal(false, false, LayerShellSupport::Available, true),
             PromptRefusal::NoComponent);
    QCOMPARE(promptRefusal(true, false, LayerShellSupport::Available, false),
             PromptRefusal::NoOutput);

    const QList<PromptRefusal> refusals {
        PromptRefusal::NoComponent, PromptRefusal::AlreadyShowing,
        PromptRefusal::NoKeyboardGrab, PromptRefusal::NoOutput,
    };
    QSet<QString> reasons;
    for (const PromptRefusal refusal : refusals) {
        const QString reason =
            QString::fromUtf8(promptRefusalReason(refusal));
        QVERIFY(!reason.isEmpty());
        reasons.insert(reason);
    }
    QCOMPARE(reasons.size(), refusals.size());
    QCOMPARE(QString::fromUtf8(promptRefusalReason(PromptRefusal::None)),
             QString());
}

// End to end under a platform that cannot grab: the request does not sit there
// waiting for a prompt nobody can see. It is dismissed, which polkitd hears as
// a cancellation, and the action fails at once.
void PolkitPromptTest::anUnpromptableRequestIsDismissedRatherThanLeftOpen()
{
    QTemporaryDir dir;
    QVERIFY(dir.isValid());
    const QString converse = writeFakeConverse(
        QDir(dir.path()),
        QStringLiteral("#!/bin/sh\nread cookie\n"
                       "printf 'secret Password%%3A\\n'\nread answer\nexit 0\n"));
    qputenv("CELESTINA_POLKIT_CONVERSE", converse.toLocal8Bit());

    PolkitAgent agent;
    PolkitPromptController prompt(nullptr, &agent);
    QSignalSpy done(&agent, &PolkitAgent::authenticationFinished);

    agent.BeginAuthentication(QStringLiteral("org.freedesktop.policykit.exec"),
                              QStringLiteral("Authentication is required"),
                              QString(), {}, QStringLiteral("cookie-1"),
                              thisUser());

    QCOMPARE(done.count(), 1);
    QCOMPARE(done.at(0).at(1).toBool(), false);
    QVERIFY(!prompt.isShowing());
    QCOMPARE(agent.pendingCount(), 0);
    qunsetenv("CELESTINA_POLKIT_CONVERSE");
}

QTEST_MAIN(PolkitPromptTest)

#include "polkitprompt_test.moc"
