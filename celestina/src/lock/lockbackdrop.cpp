#include "lockbackdrop.h"

#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonValue>
#include <QSocketNotifier>

#include <cerrno>
#include <unistd.h>

namespace {

// The shell's half of these limits lives in `lockcontroller.cpp`. Both sides
// bound the same message, because a bound only the sender honours is not a
// bound — the reader is what a hostile or broken peer actually meets.
constexpr int backdropVersion = 1;
constexpr int maximumBackdropOutputs = 16;
constexpr int maximumBackdropPathChars = 4096;
constexpr qsizetype maximumBackdropLineBytes = 65536;
// One page per readable notification. The message is far smaller; this only
// bounds how much is taken from the pipe before returning to the event loop,
// so a large write cannot hold the GUI thread.
constexpr qsizetype readChunkBytes = 4096;

} // namespace

LockBackdrop::LockBackdrop(QObject *parent)
    : QObject(parent)
{
    // Standard input. A lock started without one — from a launcher, or by
    // hand — simply never hears anything, which is a state this class already
    // has to handle for a shell that sends nothing.
    m_notifier = new QSocketNotifier(STDIN_FILENO, QSocketNotifier::Read, this);
    connect(m_notifier, &QSocketNotifier::activated,
            this, &LockBackdrop::readAvailable);
}

QString LockBackdrop::sourceFor(const QString &output) const
{
    return m_sources.value(output);
}

void LockBackdrop::stopReading()
{
    if (!m_notifier)
        return;
    m_notifier->setEnabled(false);
    m_notifier->deleteLater();
    m_notifier = nullptr;
    m_pending.clear();
}

void LockBackdrop::readAvailable()
{
    char buffer[readChunkBytes];
    const ssize_t taken = ::read(STDIN_FILENO, buffer, sizeof(buffer));

    if (taken < 0) {
        // Nothing to take yet, or a signal interrupted the call. Neither is an
        // end of input, so the notifier stays armed.
        if (errno == EAGAIN || errno == EWOULDBLOCK || errno == EINTR)
            return;
        stopReading();
        return;
    }

    if (taken == 0) {
        // End of input. A final line without its newline is still a line; one
        // that never arrived leaves every cover on its canvas.
        const QByteArray line = m_pending;
        stopReading();
        if (!line.isEmpty())
            adopt(line);
        return;
    }

    m_pending.append(buffer, static_cast<qsizetype>(taken));

    const qsizetype newline = m_pending.indexOf('\n');
    if (newline < 0) {
        // A peer that will not terminate its line does not get to grow this
        // process. Dropped rather than trimmed: a truncated JSON object is not
        // a smaller message.
        if (m_pending.size() > maximumBackdropLineBytes)
            stopReading();
        return;
    }

    const QByteArray line = m_pending.left(newline);
    // One line is the whole protocol. Anything after it is not read, and the
    // notifier is retired rather than left waiting on a channel nobody will
    // write to again.
    stopReading();
    adopt(line);
}

void LockBackdrop::adopt(const QByteArray &line)
{
    QJsonParseError failure;
    const QJsonDocument document = QJsonDocument::fromJson(line, &failure);
    if (failure.error != QJsonParseError::NoError || !document.isObject())
        return;

    const QJsonObject payload = document.object();
    // An unknown version is not guessed at. A future shell that changes this
    // shape meets a lock that keeps its canvas, which is the honest failure.
    if (payload.value(QStringLiteral("version")).toInt() != backdropVersion)
        return;

    const QJsonValue wallpapers = payload.value(QStringLiteral("wallpapers"));
    if (!wallpapers.isObject())
        return;

    QHash<QString, QString> adopted;
    const QJsonObject chosen = wallpapers.toObject();
    for (auto entry = chosen.constBegin(); entry != chosen.constEnd(); ++entry) {
        if (adopted.size() >= maximumBackdropOutputs)
            break;
        if (entry.key().isEmpty() || !entry.value().isString())
            continue;
        const QString path = entry.value().toString();
        if (path.isEmpty() || path.size() > maximumBackdropPathChars)
            continue;
        // Absolute or nothing. This process has its own working directory and
        // a relative path would name a different file here than it did in the
        // shell — a wrong picture is worse than the canvas.
        if (!QFileInfo(path).isAbsolute())
            continue;
        adopted.insert(entry.key(), path);
    }

    if (adopted.isEmpty())
        return;

    m_sources = adopted;
    emit changed();
}
