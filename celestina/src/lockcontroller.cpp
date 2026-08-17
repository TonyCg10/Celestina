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
#include <QJsonDocument>
#include <QJsonObject>

namespace {

constexpr auto logindService = "org.freedesktop.login1";
constexpr auto logindPath = "/org/freedesktop/login1";
constexpr auto logindInterface = "org.freedesktop.login1.Manager";

// The line `celestina-lock` prints once the compositor has the session. It is
// the only thing this shell trusts as "the screen is covered" — not that the
// process started, which says nothing about what is on screen.
constexpr auto confirmedLine = "locked";

// What the lock is told about the session's backdrop, and the limits on it.
//
// The channel is the lock's stdin rather than its argv: `/proc/<pid>/cmdline`
// is world-readable and a wallpaper's filename can say something about the
// person. The payload is one bounded, versioned JSON line, which is the same
// shape every other channel in this shell uses.
constexpr int backdropVersion = 1;
// More outputs than any session this shell has met, and small enough that a
// confused provider cannot make the line unbounded.
constexpr int maximumBackdropOutputs = 16;
// `PATH_MAX` on Linux. A longer string is not a path this session can open.
constexpr int maximumBackdropPathChars = 4096;
// The whole line, after which it is dropped rather than truncated: half a JSON
// object is not a smaller message, it is a broken one.
constexpr int maximumBackdropLineBytes = 65536;

// One line describing which image belongs to which output, or nothing at all.
//
// Paths must be absolute. The lock is a different process with a different
// working directory, so a relative path here would name a different file
// there — and a lock screen showing the wrong picture is a defect that looks
// like a decode failure.
QByteArray backdropLine(const QVariantMap &wallpapersByOutput)
{
    QJsonObject wallpapers;
    for (auto entry = wallpapersByOutput.constBegin();
         entry != wallpapersByOutput.constEnd();
         ++entry) {
        if (wallpapers.size() >= maximumBackdropOutputs)
            break;
        const QString path = entry.value().toString();
        if (path.isEmpty() || path.size() > maximumBackdropPathChars)
            continue;
        if (!QFileInfo(path).isAbsolute())
            continue;
        wallpapers.insert(entry.key(), path);
    }

    QJsonObject payload;
    payload.insert(QStringLiteral("version"), backdropVersion);
    payload.insert(QStringLiteral("wallpapers"), wallpapers);

    QByteArray line =
        QJsonDocument(payload).toJson(QJsonDocument::Compact);
    if (line.size() + 1 > maximumBackdropLineBytes)
        return QByteArray();
    line.append('\n');
    return line;
}

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
    // Beside the shell first. A deployed bundle carries its own lock, and a
    // shell that preferred the build tree would run whatever was last compiled
    // there — a different version from the one installed beside it, which is
    // exactly the mismatch that makes a locked screen behave unlike the shell
    // that locked it.
    const QFileInfo beside(
        QDir(QCoreApplication::applicationDirPath())
            .filePath(QStringLiteral("celestina-lock")));
    if (beside.isExecutable())
        return beside.absoluteFilePath();
#ifdef CELESTINA_LOCK_BINARY
    // The build tree, for a shell run straight out of it in development.
    const QFileInfo built(QStringLiteral(CELESTINA_LOCK_BINARY));
    if (built.isExecutable())
        return built.absoluteFilePath();
#endif
    return QString();
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
    sendBackdrop();
    return true;
}

void LockController::setBackdrop(const QVariantMap &wallpapersByOutput)
{
    m_backdrop = wallpapersByOutput;
}

void LockController::sendBackdrop()
{
    // Nothing here is waited on, and that is the whole design of this method.
    //
    // The backdrop is decoration: the lock covers the screen whether or not it
    // ever learns which picture to show. Waiting for these bytes to be read —
    // or for the lock to acknowledge them — would turn an ornament into a
    // precondition for covering the session, which is the one thing this
    // channel must never become. `write` buffers into the pipe and returns; if
    // the lock never reads it, the bytes are simply dropped when it exits.
    const QByteArray line = backdropLine(m_backdrop);
    if (!line.isEmpty())
        m_process->write(line);
    // The lock waits for one line or end of input, whichever comes first, so
    // closing is what tells a lock with no backdrop to stop listening rather
    // than hold a reader open for the rest of the session.
    m_process->closeWriteChannel();
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
