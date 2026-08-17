#pragma once

#include <QByteArray>
#include <QHash>
#include <QObject>
#include <QString>

class QSocketNotifier;

// What the shell told this lock the session was showing.
//
// The lock owns no provider connection, no bus name and no settings reader. It
// is handed one fact — which image belongs to which output — as a single
// bounded JSON line on stdin, and it is handed that fact at a moment when the
// screen may already be covered.
//
// That ordering is the point. Reading is asynchronous and nothing waits for it:
// a lock that delayed its cover until the shell had described the wallpaper
// would have made an ornament into a precondition for covering the session. If
// the line never arrives, arrives late, does not parse, or names a file that
// will not decode, every cover simply keeps the deliberate canvas — which is
// exactly what the lock showed before this class existed.
//
// Nothing here can unlock anything, and nothing here is trusted with more than
// a file path: a value that is not an absolute path to open is discarded rather
// than repaired.
class LockBackdrop final : public QObject
{
    Q_OBJECT

public:
    explicit LockBackdrop(QObject *parent = nullptr);

    // The absolute path chosen for one output, or an empty string when nothing
    // was published for it. An output nobody mentioned never inherits another
    // output's picture.
    QString sourceFor(const QString &output) const;

signals:
    // A backdrop arrived. Covers built before it — which is all of them, in the
    // ordinary case — repaint from it.
    void changed();

private:
    void readAvailable();
    void adopt(const QByteArray &line);
    void stopReading();

    QSocketNotifier *m_notifier = nullptr;
    QByteArray m_pending;
    QHash<QString, QString> m_sources;
};
