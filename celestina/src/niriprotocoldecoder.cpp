#include "niriprotocoldecoder.h"

NiriProtocolDecoder::Result NiriProtocolDecoder::append(const QByteArray &chunk)
{
    Result result;
    qsizetype offset = 0;

    while (offset < chunk.size()) {
        if (m_discardingOversizedLine) {
            const qsizetype newline = chunk.indexOf('\n', offset);
            if (newline < 0)
                return result;
            m_discardingOversizedLine = false;
            offset = newline + 1;
            continue;
        }

        const qsizetype newline = chunk.indexOf('\n', offset);
        const qsizetype end = newline < 0 ? chunk.size() : newline;
        const qsizetype fragmentSize = end - offset;
        if (m_buffer.size() + fragmentSize > maximumLineBytes) {
            m_buffer.clear();
            result.discardedOversizedLine = true;
            if (newline < 0) {
                m_discardingOversizedLine = true;
                return result;
            }
            offset = newline + 1;
            continue;
        }

        m_buffer.append(chunk.constData() + offset, fragmentSize);
        if (newline < 0)
            return result;

        if (!m_buffer.trimmed().isEmpty())
            result.lines.append(m_buffer);
        m_buffer.clear();
        offset = newline + 1;
    }

    return result;
}

void NiriProtocolDecoder::reset()
{
    m_buffer.clear();
    m_discardingOversizedLine = false;
}
