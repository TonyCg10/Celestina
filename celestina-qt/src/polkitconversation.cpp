#include "polkitconversation.h"

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>

namespace {

// The child's whole vocabulary, mirrored from `src/polkitconverse/main.cpp`.
constexpr int authenticatedExit = 0;
constexpr int refusedExit = 1;

// Where the conversation child lives. The build points
// `CELESTINA_POLKIT_CONVERSE` at the binary it produced; a deployed shell
// finds it beside itself. Nothing searches `PATH`, and an explicit setting is
// authoritative including when it is wrong — a named child that cannot be run
// denies rather than falling back to another binary. Which process may say
// "authenticated" is not a question to answer by guessing, and the lock's
// verifier lookup learned that the hard way.
QString conversePath()
{
    const QByteArray configured = qgetenv("CELESTINA_POLKIT_CONVERSE");
    if (!configured.isEmpty()) {
        const QFileInfo named(QString::fromLocal8Bit(configured));
        return named.isExecutable() ? named.absoluteFilePath() : QString();
    }
    const QFileInfo beside(
        QDir(QCoreApplication::applicationDirPath())
            .filePath(QStringLiteral("celestina-polkit-converse")));
    return beside.isExecutable() ? beside.absoluteFilePath() : QString();
}

// Overwritten in place through a volatile pointer, for the reason both
// children do the same: a clear of a buffer nothing reads again is what a
// compiler removes.
void wipe(QString &secret)
{
    auto *at = reinterpret_cast<volatile char16_t *>(secret.data());
    for (qsizetype index = 0; index < secret.size(); ++index)
        at[index] = u'\0';
    secret.clear();
}

void wipe(QByteArray &bytes)
{
    volatile char *at = bytes.data();
    for (qsizetype index = 0; index < bytes.size(); ++index)
        at[index] = '\0';
    bytes.clear();
}

} // namespace

PolkitConversation::PolkitConversation(QObject *parent)
    : QObject(parent)
    , m_process(new QProcess(this))
{
    // The child's stderr is the author's and is deliberately not forwarded
    // into anything this shell records.
    m_process->setProcessChannelMode(QProcess::ForwardedErrorChannel);
    connect(m_process, &QProcess::readyReadStandardOutput, this,
            &PolkitConversation::readEvents);
    connect(m_process, &QProcess::finished, this,
            &PolkitConversation::finished);
    connect(m_process, &QProcess::stateChanged, this,
            [this](QProcess::ProcessState) { emit busyChanged(); });
    connect(m_process, &QProcess::errorOccurred, this,
            [this](QProcess::ProcessError) {
                // A child that could not run has answered nothing. That is
                // never an authorization.
                if (m_process->state() == QProcess::NotRunning)
                    emit answered(Verdict::Unavailable);
            });
}

bool PolkitConversation::isBusy() const
{
    return m_process->state() != QProcess::NotRunning;
}

void PolkitConversation::start(const QString &user, QString cookie)
{
    if (isBusy()) {
        wipe(cookie);
        emit answered(Verdict::Unavailable);
        return;
    }

    const QString converse = conversePath();
    if (converse.isEmpty() || user.isEmpty() || cookie.isEmpty()) {
        wipe(cookie);
        emit answered(Verdict::Unavailable);
        return;
    }

    m_pending.clear();
    m_process->start(converse, {QStringLiteral("--user"), user});
    if (!m_process->waitForStarted(2000)) {
        wipe(cookie);
        emit answered(Verdict::Unavailable);
        return;
    }

    QByteArray line = cookie.toUtf8() + '\n';
    m_process->write(line);
    m_process->waitForBytesWritten(2000);
    wipe(line);
    wipe(cookie);
}

void PolkitConversation::respond(QString secret)
{
    if (!isBusy()) {
        wipe(secret);
        return;
    }

    // Percent-encoded so a response containing a newline reaches the child
    // whole rather than as two frames, and decoded exactly on the other side
    // or refused there.
    QByteArray line = secret.toUtf8();
    QByteArray encoded = line.toPercentEncoding();
    wipe(line);
    wipe(secret);

    encoded.append('\n');
    m_process->write(encoded);
    m_process->waitForBytesWritten(2000);
    wipe(encoded);
}

void PolkitConversation::cancel()
{
    if (!isBusy())
        return;
    m_process->disconnect(this);
    m_process->kill();
    m_process->waitForFinished(1000);
    m_pending.clear();
    connect(m_process, &QProcess::readyReadStandardOutput, this,
            &PolkitConversation::readEvents);
    connect(m_process, &QProcess::finished, this,
            &PolkitConversation::finished);
}

void PolkitConversation::readEvents()
{
    m_pending.append(m_process->readAllStandardOutput());
    for (;;) {
        const qsizetype end = m_pending.indexOf('\n');
        if (end < 0)
            break;
        const QByteArray frame = m_pending.left(end);
        m_pending.remove(0, end + 1);

        const qsizetype space = frame.indexOf(' ');
        if (space < 0)
            continue;
        const QByteArray kind = frame.left(space);
        const QString text = QString::fromUtf8(
            QByteArray::fromPercentEncoding(frame.mid(space + 1)));

        if (kind == "secret")
            emit secretRequested(text);
        else if (kind == "visible")
            emit visibleRequested(text);
        else if (kind == "info")
            emit informed(text);
        else if (kind == "problem")
            emit problemReported(text);
        // An unknown frame is ignored rather than guessed at. The verdict
        // never arrives on this stream, so nothing here can be mistaken for
        // one.
    }
}

void PolkitConversation::finished(int exitCode, QProcess::ExitStatus status)
{
    m_pending.clear();

    // A child that crashed decided nothing. It is not a denial the person
    // should be told to retype past, and it is certainly not an authorization.
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
