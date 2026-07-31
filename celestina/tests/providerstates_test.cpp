#include <QtTest>

#include "providerstates.h"

// The host's side of the provider contract: what it accepts from a separate
// process, and what it refuses. The helper enforces the same bounds, but a
// helper is a separate binary that may be older or broken, so the host does not
// take its word for any of them.
class ProviderStatesTest final : public QObject
{
    Q_OBJECT

private slots:
    void readsAProviderFrameAndItsGeneration();
    void refusesAFrameFromAnotherProtocolVersion();
    void refusesUnusableProviderNamesValuesAndSizes();
    void readsACommandResult();
    void aFrameReplacesTheWholeSetSoAWithdrawnProviderCannotLinger();
    void aNewGenerationIsAChangeEvenWithIdenticalValues();
    void clearingDropsEverythingTheHelperHadPublished();
};

void ProviderStatesTest::readsAProviderFrameAndItsGeneration()
{
    const ProviderMessage message = parseProviderMessage(
        R"({"kind":"providers","version":1,"generation":7,)"
        R"("providers":{"sysmon":{"cpu":12,"ram":48.5},"power-profile":{"active":"balanced"}}})"
    );

    QCOMPARE(message.kind, ProviderMessage::Kind::Providers);
    QCOMPARE(message.generation, 7u);
    QCOMPARE(message.providers.size(), 2);
    const QVariantMap sysmon = message.providers.value(QStringLiteral("sysmon")).toMap();
    QCOMPARE(sysmon.value(QStringLiteral("cpu")).toInt(), 12);
    QCOMPARE(sysmon.value(QStringLiteral("ram")).toDouble(), 48.5);
}

void ProviderStatesTest::refusesAFrameFromAnotherProtocolVersion()
{
    // A helper ahead of its host is a mismatched install, not a frame to guess
    // the meaning of.
    const ProviderMessage newer = parseProviderMessage(
        R"({"kind":"providers","version":2,"generation":1,"providers":{}})"
    );
    QCOMPARE(newer.kind, ProviderMessage::Kind::Invalid);

    const ProviderMessage missing = parseProviderMessage(
        R"({"kind":"providers","generation":1,"providers":{}})"
    );
    QCOMPARE(missing.kind, ProviderMessage::Kind::Invalid);
}

void ProviderStatesTest::refusesUnusableProviderNamesValuesAndSizes()
{
    const QList<QByteArray> refused {
        // Not a provider name.
        R"({"kind":"providers","version":1,"generation":1,"providers":{"SysMon":{}}})",
        // A provider whose value is not a field set.
        R"({"kind":"providers","version":1,"generation":1,"providers":{"sysmon":12}})",
        // Nested structure: the panel reads values, never documents.
        R"({"kind":"providers","version":1,"generation":1,"providers":{"sysmon":{"a":{"b":1}}}})",
        R"({"kind":"providers","version":1,"generation":1,"providers":{"sysmon":{"a":[1,2]}}})",
        // No usable generation.
        R"({"kind":"providers","version":1,"generation":-1,"providers":{}})",
        R"({"kind":"providers","version":1,"providers":{}})",
        // Not a frame this host knows.
        R"({"kind":"weather","version":1})",
        QByteArrayLiteral("not json at all"),
    };

    for (const QByteArray &line : refused) {
        const ProviderMessage message = parseProviderMessage(line);
        QVERIFY2(
            message.kind == ProviderMessage::Kind::Invalid,
            line.constData()
        );
        QVERIFY(!message.error.isEmpty());
    }

    // A text field past the cap is refused rather than truncated: the host does
    // not silently reshape what a provider claimed.
    QByteArray oversized = R"({"kind":"providers","version":1,"generation":1,)"
                           R"("providers":{"media":{"title":")";
    oversized.append(QByteArray(600, 'x'));
    oversized.append(R"("}}})");
    QCOMPARE(parseProviderMessage(oversized).kind, ProviderMessage::Kind::Invalid);
}

void ProviderStatesTest::readsACommandResult()
{
    const ProviderMessage accepted =
        parseProviderMessage(R"({"kind":"result","id":"7","state":"accepted"})");
    QCOMPARE(accepted.kind, ProviderMessage::Kind::Result);
    QCOMPARE(accepted.requestId, QStringLiteral("7"));
    QCOMPARE(accepted.state, QStringLiteral("accepted"));

    const ProviderMessage failed = parseProviderMessage(
        R"({"kind":"result","id":"8","state":"failed","reason":"no such provider"})"
    );
    QCOMPARE(failed.state, QStringLiteral("failed"));
    QCOMPARE(failed.reason, QStringLiteral("no such provider"));

    QCOMPARE(
        parseProviderMessage(R"({"kind":"result","id":"9","state":"maybe"})").kind,
        ProviderMessage::Kind::Invalid
    );
    QCOMPARE(
        parseProviderMessage(R"({"kind":"result","state":"accepted"})").kind,
        ProviderMessage::Kind::Invalid
    );
}

void ProviderStatesTest::aFrameReplacesTheWholeSetSoAWithdrawnProviderCannotLinger()
{
    ProviderStates states;
    QVERIFY(states.apply(parseProviderMessage(
        R"({"kind":"providers","version":1,"generation":1,)"
        R"("providers":{"sysmon":{"cpu":12},"media":{"title":"one"}}})"
    )));
    QCOMPARE(states.providers().size(), 2);

    // The next frame no longer carries `media`: the helper stopped carrying it,
    // so the panel must stop showing it.
    QVERIFY(states.apply(parseProviderMessage(
        R"({"kind":"providers","version":1,"generation":1,"providers":{"sysmon":{"cpu":12}}})"
    )));
    QCOMPARE(states.providers().size(), 1);
    QVERIFY(!states.providers().contains(QStringLiteral("media")));

    // An identical frame is not a change.
    QVERIFY(!states.apply(parseProviderMessage(
        R"({"kind":"providers","version":1,"generation":1,"providers":{"sysmon":{"cpu":12}}})"
    )));
}

void ProviderStatesTest::aNewGenerationIsAChangeEvenWithIdenticalValues()
{
    ProviderStates states;
    const QByteArray first =
        R"({"kind":"providers","version":1,"generation":1,"providers":{"sysmon":{"cpu":12}}})";
    QVERIFY(states.apply(parseProviderMessage(first)));

    const QByteArray restarted =
        R"({"kind":"providers","version":1,"generation":2,"providers":{"sysmon":{"cpu":12}}})";
    QVERIFY(states.apply(parseProviderMessage(restarted)));
    QCOMPARE(states.generation(), 2u);
}

void ProviderStatesTest::clearingDropsEverythingTheHelperHadPublished()
{
    ProviderStates states;
    states.apply(parseProviderMessage(
        R"({"kind":"providers","version":1,"generation":1,"providers":{"sysmon":{"cpu":12}}})"
    ));

    QVERIFY(states.clear());
    QVERIFY(states.isEmpty());
    QCOMPARE(states.generation(), 0u);
    // Nothing left to drop is not a change.
    QVERIFY(!states.clear());

    // A result frame never touches published state.
    QVERIFY(!states.apply(
        parseProviderMessage(R"({"kind":"result","id":"7","state":"accepted"})")
    ));
}

QTEST_GUILESS_MAIN(ProviderStatesTest)

#include "providerstates_test.moc"
