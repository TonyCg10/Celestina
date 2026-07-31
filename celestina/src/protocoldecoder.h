#pragma once

#include <QByteArray>
#include <QList>

// Bounded line framing for the Rust helper's line-delimited JSON protocol.
// An oversized frame is discarded through its next newline, so the following
// valid message can never be interpreted from the middle of hostile input.
class ProtocolDecoder final
{
public:
    struct Result {
        QList<QByteArray> lines;
        bool discardedOversizedLine = false;
    };

    Result append(const QByteArray &chunk);
    void reset();

private:
    static constexpr qsizetype maximumLineBytes = 1024 * 1024;

    QByteArray m_buffer;
    bool m_discardingOversizedLine = false;
};
