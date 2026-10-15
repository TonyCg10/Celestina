#include "lockauthenticator.h"

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QProcessEnvironment>

namespace {

// The child's whole vocabulary, mirrored from `src/lockverify/main.cpp`.
constexpr int authenticatedExit = 0;
constexpr int refusedExit = 1;

// Where the verifier lives. The build points `CELESTINA_LOCK_VERIFY` at the
// binary it produced, exactly as it does for the two helpers; a deployed shell
// finds it beside itself. Nothing searches `PATH`: the one process allowed to
// say "authenticated" is not chosen by an environment this session's programs
// can rewrite.
QString verifierPath()
{
    // An explicit setting is authoritative, including when it is wrong: a
    // named verifier that cannot be run refuses, and never falls back to
    // another binary. Which process may say "authenticated" is not a question
    // to answer by guessing.
    const QByteArray configured = qgetenv("CELESTINA_LOCK_VERIFY");
    if (!configured.isEmpty()) {
        const QFileInfo named(QString::fromLocal8Bit(configured));
        return named.isExecutable() ? named.absoluteFilePath() : QString();
    }
    const QFileInfo beside(
        QDir(QCoreApplication::applicationDirPath())
            .filePath(QStringLiteral("celestina-lock-verify")));
    return beside.isExecutable() ? beside.absoluteFilePath() : QString();
}

// Overwritten in place through a volatile pointer, for the reason the child
// does the same: a clear of a buffer nothing reads again is what a compiler
// removes.
void wipe(QString &secret)
{
    // Through a volatile view of the raw storage rather than of `QChar`: the
    // point is that the compiler may not prove these writes dead, and a
    // volatile `QChar` cannot be assigned through its own operator.
    auto *at = reinterpret_cast<volatile char16_t *>(secret.data());
    for (qsizetype index = 0; index < secret.size(); ++index)
        at[index] = u'\0';
    secret.clear();
}

} // namespace

LockAuthenticator::LockAuthenticator(QObject *parent)
    : QObject(parent)
    , m_process(new QProcess(this))
    , m_user(QProcessEnvironment::systemEnvironment().value(
          QStringLiteral("USER")))
    , m_service(QStringLiteral("login"))
{
    // The child says nothing on stdout by contract; its stderr is the author's
    // and is deliberately not forwarded into anything this shell records.
    m_process->setProcessChannelMode(QProcess::ForwardedErrorChannel);
    connect(m_process, &QProcess::finished, this, &LockAuthenticator::finished);
    connect(m_process, &QProcess::stateChanged, this,
            [this](QProcess::ProcessState) { emit busyChanged(); });
    connect(m_process, &QProcess::errorOccurred, this,
            [this](QProcess::ProcessError) {
                // A child that could not run has answered nothing. That is
                // never an unlock.
                if (m_process->state() == QProcess::NotRunning) {
                    emit answered(Verdict::Unavailable);
                }
            });
}

bool LockAuthenticator::isBusy() const
{
    return m_process->state() != QProcess::NotRunning;
}

void LockAuthenticator::authenticate(QString secret)
{
    if (isBusy()) {
        wipe(secret);
        emit answered(Verdict::Unavailable);
        return;
    }

    const QString verifier = verifierPath();
    if (verifier.isEmpty() || m_user.isEmpty()) {
        wipe(secret);
        emit answered(Verdict::Unavailable);
        return;
    }

    m_process->start(verifier,
                     {QStringLiteral("--user"), m_user,
                      QStringLiteral("--service"), m_service});
    if (!m_process->waitForStarted(2000)) {
        wipe(secret);
        emit answered(Verdict::Unavailable);
        return;
    }

    // Down the pipe and out of this process, in that order. The local copy is
    // wiped whether or not the write succeeded.
    const QByteArray line = secret.toUtf8() + '\n';
    m_process->write(line);
    m_process->waitForBytesWritten(2000);
    m_process->closeWriteChannel();

    QByteArray scratch = line;
    volatile char *at = scratch.data();
    for (qsizetype index = 0; index < scratch.size(); ++index)
        at[index] = '\0';
    wipe(secret);
}

void LockAuthenticator::cancel()
{
    if (!isBusy())
        return;
    m_process->disconnect(this);
    m_process->kill();
    m_process->waitForFinished(1000);
    connect(m_process, &QProcess::finished, this, &LockAuthenticator::finished);
}

void LockAuthenticator::finished(int exitCode, QProcess::ExitStatus status)
{
    // A child that crashed decided nothing. It is not a refusal the person
    // should be told to retype past, and it is certainly not an unlock.
    if (status != QProcess::NormalExit) {
        emit answered(Verdict::Unavailable);
        return;
    }

    switch (exitCode) {
    case authenticatedExit:
        emit answered(Verdict::Authenticated);
        return;
    case refusedExit:
        emit answered(Verdict::Refused);
        return;
    default:
        emit answered(Verdict::Unavailable);
        return;
    }
}
