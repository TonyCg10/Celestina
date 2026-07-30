#include <QtTest>

#include "niriprotocoldecoder.h"

class NiriProtocolDecoderTest final : public QObject
{
    Q_OBJECT

private slots:
    void assemblesFragmentedLines();
    void emitsSeveralLinesFromOneChunk();
    void discardsThroughNewlineAndRecovers();
};

void NiriProtocolDecoderTest::assemblesFragmentedLines()
{
    NiriProtocolDecoder decoder;
    QCOMPARE(decoder.append(QByteArrayLiteral("{\"kind\":" )).lines.size(), 0);
    const auto result = decoder.append(QByteArrayLiteral("\"unavailable\"}\n"));
    QCOMPARE(result.lines, QList<QByteArray> {QByteArrayLiteral("{\"kind\":\"unavailable\"}")});
    QVERIFY(!result.discardedOversizedLine);
}

void NiriProtocolDecoderTest::emitsSeveralLinesFromOneChunk()
{
    NiriProtocolDecoder decoder;
    const auto result = decoder.append(QByteArrayLiteral("one\n\ntwo\n"));
    QCOMPARE(result.lines, QList<QByteArray>({"one", "two"}));
}

void NiriProtocolDecoderTest::discardsThroughNewlineAndRecovers()
{
    NiriProtocolDecoder decoder;
    QByteArray hostile(1024 * 1024 + 1, 'x');
    auto result = decoder.append(hostile);
    QVERIFY(result.discardedOversizedLine);
    QVERIFY(result.lines.isEmpty());

    result = decoder.append(QByteArrayLiteral("still hostile\nvalid\n"));
    QVERIFY(!result.discardedOversizedLine);
    QCOMPARE(result.lines, QList<QByteArray> {QByteArrayLiteral("valid")});
}

QTEST_GUILESS_MAIN(NiriProtocolDecoderTest)

#include "niriprotocoldecoder_test.moc"
