#include "lockcontroller.h"

#include <QCoreApplication>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusPendingCall>
#include <QDBusPendingCallWatcher>
#include <QDBusReply>
#include <QDBusUnixFileDescriptor>
#include <QDebug>
#include <QDir>
#include <QFileInfo>

namespace {

constexpr auto logindService = "org.freedesktop.login1";
constexpr auto logindPath = "/org/freedesktop/login1";
constexpr auto logindInterface = "org.freedesktop.login1.Manager";

// The line `celestina-lock` prints once the compositor has the session. It is
// the only thing this shell trusts as "the screen is covered" — not that the
// process started, which says nothing about what is on screen.
constexpr auto confirmedLine = "locked";

QString lockProgram()
{
    // An explicit setting is authoritative, including when it is wrong. A
    // named lock that cannot be run is a refusal, never a quiet fallback to
    // some other binary: "which program is allowed to cover this session" is
    // not a question to answer by guessing.
    const QByteArray configured = qgetenv("CELESTINA_LOCK");
    if (!configured.isEmpty()) {
        const QFileInfo named(QString::fromLocal8Bit(configured));
        return named.isExecutable() ? named.absoluteFilePath() : QString();
    }
#ifdef CELESTINA_LOCK_BINARY
    const QFileInfo built(QStringLiteral(CELESTINA_LOCK_BINARY));
    if (built.isExecutable())
        return built.absoluteFilePath();
#endif
    const QFileInfo beside(
        QDir(QCoreApplication::applicationDirPath())
            .filePath(QStringLiteral("celestina-lock")));
    return beside.isExecutable() ? beside.absoluteFilePath() : QString();
}

} // namespace

LockController::LockController(QObject *parent)
    : QObject(parent)
    , m_process(new QProcess(this))
{
    m_process->setProcessChannelMode(QProcess::ForwardedErrorChannel);
    connect(m_process, &QProcess::readyReadStandardOutput, this, [this]() {
        while (m_process->canReadLine())
            started(QString::fromUtf8(m_process->readLine()).trimmed());
    });
    connect(m_process, &QProcess::finished, this, &LockController::finished);

    QDBusConnection::systemBus().connect(
        QString::fromLatin1(logindService),
        QString::fromLatin1(logindPath),
        QString::fromLatin1(logindInterface),
        QStringLiteral("PrepareForSleep"),
        this,
        SLOT(prepareForSleep(bool))
    );
    takeSleepInhibitor();
}

LockController::~LockController()
{
    if (m_process->state() != QProcess::NotRunning) {
        // Deliberately not killed: a shell going away must not take the lock
        // with it. The lock outlives this process and keeps the session
        // covered, which is the whole point of it being separate.
        m_process->setParent(nullptr);
    }
}

bool LockController::isStarting() const
{
    return m_process->state() != QProcess::NotRunning && !m_confirmed;
}

void LockController::takeSleepInhibitor()
{
    if (m_sleepInhibitor.isValid())
        return;

    QDBusConnection bus = QDBusConnection::systemBus();
    if (!bus.isConnected())
        return;

    QDBusMessage call = QDBusMessage::createMethodCall(
        QString::fromLatin1(logindService),
        QString::fromLatin1(logindPath),
        QString::fromLatin1(logindInterface),
        QStringLiteral("Inhibit"));
    // "delay", not "block": logind waits for this to be released, with its own
    // timeout, rather than being forbidden to sleep at all. A shell that could
    // veto sleep outright would be a shell that can wedge a laptop shut.
    call.setArguments({QStringLiteral("sleep"),
                       QStringLiteral("Celestina"),
                       QStringLiteral("Locking the session before it sleeps"),
                       QStringLiteral("delay")});

    const QDBusMessage reply = bus.call(call, QDBus::Block, 2000);
    if (reply.type() != QDBusMessage::ReplyMessage || reply.arguments().isEmpty()) {
        // Without the inhibitor the machine can still sleep; it simply may do
        // so before the lock is up. Worth saying once, and not worth refusing
        // to run over.
        qWarning() << "Celestina could not take a sleep inhibitor:"
                   << reply.errorMessage();
        return;
    }
    m_sleepInhibitor =
        reply.arguments().first().value<QDBusUnixFileDescriptor>();
}

void LockController::releaseSleepInhibitor()
{
    // Closing the descriptor is the release: logind watches the pipe.
    m_sleepInhibitor = QDBusUnixFileDescriptor();
}

void LockController::prepareForSleep(bool starting)
{
    if (!starting) {
        // Awake again. Take a fresh inhibitor for the next time, and leave the
        // lock exactly as it is — waking up is not a reason to uncover a
        // session.
        m_sleepPending = false;
        takeSleepInhibitor();
        return;
    }

    if (m_confirmed) {
        // Already covered. Nothing to delay for.
        releaseSleepInhibitor();
        return;
    }

    m_sleepPending = true;
    if (!lock()) {
        // The lock will not start. logind's own delay timeout will expire and
        // the machine will sleep unlocked — this shell cannot prevent that
        // without being able to wedge it awake, and says so rather than
        // pretending it handled the sleep.
        qCritical() << "Celestina could not lock before sleep; the session "
                       "will sleep uncovered";
        releaseSleepInhibitor();
    }
}

bool LockController::lock()
{
    if (m_process->state() != QProcess::NotRunning)
        return true;

    const QString program = lockProgram();
    if (program.isEmpty())
        return false;

    m_confirmed = false;
    m_process->start(program, {});
    if (!m_process->waitForStarted(2000)) {
        m_process->kill();
        return false;
    }
    return true;
}

void LockController::started(const QString &line)
{
    if (line != QLatin1String(confirmedLine) || m_confirmed)
        return;

    m_confirmed = true;
    emit lockedChanged();
    if (m_sleepPending) {
        // The screen is covered; logind may proceed.
        m_sleepPending = false;
        releaseSleepInhibitor();
    }
}

void LockController::finished()
{
    // The lock left. Either it was unlocked by an authenticated verdict or it
    // died — this shell cannot tell the two apart from here, and does not need
    // to: in both cases there is no lock any more, and the compositor is the
    // one holding the session if it died.
    if (!m_confirmed)
        return;
    m_confirmed = false;
    emit lockedChanged();
    takeSleepInhibitor();
}

void LockController::lockAndSuspend(std::function<void(const QString &)> answer)
{
    if (m_confirmed) {
        suspendNow(std::move(answer));
        return;
    }

    if (!lock()) {
        answer(QStringLiteral(
            "the session lock could not be started, so this session is not "
            "being suspended unlocked"));
        return;
    }

    // Suspend rides the confirmation, and nothing else. There is no timer here
    // that gives up and suspends anyway: an unconfirmed lock means the screen
    // may be uncovered, and sleeping then is the failure this whole unit
    // exists to prevent. If the confirmation never comes, the session simply
    // does not suspend, which is visible and recoverable.
    auto *tracker = new QObject(this);
    connect(this, &LockController::lockedChanged, tracker,
            [this, tracker, answer]() mutable {
                if (!m_confirmed)
                    return;
                tracker->deleteLater();
                suspendNow(std::move(answer));
            });
    connect(m_process, &QProcess::finished, tracker,
            [this, tracker, answer]() mutable {
                if (m_confirmed)
                    return;
                tracker->deleteLater();
                answer(QStringLiteral(
                    "the session lock ended before it covered the screen, so "
                    "this session is not being suspended unlocked"));
            });
}

void LockController::suspendNow(std::function<void(const QString &)> answer)
{
    QDBusConnection bus = QDBusConnection::systemBus();
    if (!bus.isConnected()) {
        answer(QStringLiteral("the shell cannot reach the session manager"));
        return;
    }

    QDBusMessage call = QDBusMessage::createMethodCall(
        QString::fromLatin1(logindService),
        QString::fromLatin1(logindPath),
        QString::fromLatin1(logindInterface),
        QStringLiteral("Suspend"));
    // Never interactive: a shell cannot answer a polkit prompt, and a session
    // that may not suspend must fail visibly rather than hang on a dialogue
    // nobody will see.
    call.setArguments({false});

    auto *watcher = new QDBusPendingCallWatcher(bus.asyncCall(call), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this,
            [answer](QDBusPendingCallWatcher *call) {
                const QDBusPendingReply<> reply = *call;
                answer(reply.isError() ? reply.error().message() : QString());
                call->deleteLater();
            });
}
