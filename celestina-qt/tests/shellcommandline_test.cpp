#include <QtTest>

#include "shellcommandline.h"

class ShellCommandLineTest final : public QObject
{
    Q_OBJECT

private slots:
    void readsAVerbWithoutOptions();
    void typesBooleansAndNumbersAndKeepsTheRestAsText();
    void reservesGetStateAsTheReadVerb();
    void refusesAnEmptyLineAndAMalformedVerb();
    void refusesMalformedDuplicatedAndOversizedOptions();
};

void ShellCommandLineTest::readsAVerbWithoutOptions()
{
    const ShellCommandLine line = parseShellCommandLine({QStringLiteral("focus-workspace")});

    QVERIFY(line.error.isEmpty());
    QCOMPARE(line.verb, QStringLiteral("focus-workspace"));
    QVERIFY(line.options.isEmpty());
    QVERIFY(!line.readsState);
}

void ShellCommandLineTest::typesBooleansAndNumbersAndKeepsTheRestAsText()
{
    const ShellCommandLine line = parseShellCommandLine({
        QStringLiteral("focus-workspace"),
        QStringLiteral("output=DP-1"),
        QStringLiteral("index=3"),
        QStringLiteral("confirm=true"),
        // An option's value may itself contain '='.
        QStringLiteral("note=a=b"),
    });

    QVERIFY(line.error.isEmpty());
    QCOMPARE(line.options.value(QStringLiteral("output")), QVariant(QStringLiteral("DP-1")));
    QCOMPARE(line.options.value(QStringLiteral("index")), QVariant(qlonglong(3)));
    QCOMPARE(line.options.value(QStringLiteral("confirm")), QVariant(true));
    QCOMPARE(line.options.value(QStringLiteral("note")), QVariant(QStringLiteral("a=b")));
}

void ShellCommandLineTest::reservesGetStateAsTheReadVerb()
{
    const ShellCommandLine line = parseShellCommandLine({QStringLiteral("get-state")});

    QVERIFY(line.error.isEmpty());
    QVERIFY(line.readsState);
}

void ShellCommandLineTest::refusesAnEmptyLineAndAMalformedVerb()
{
    QVERIFY(!parseShellCommandLine({}).error.isEmpty());
    QVERIFY(!parseShellCommandLine({QStringLiteral("Focus")}).error.isEmpty());
    QVERIFY(!parseShellCommandLine({QStringLiteral("focus workspace")}).error.isEmpty());
    QVERIFY(!parseShellCommandLine({QString(64, u'a')}).error.isEmpty());
}

void ShellCommandLineTest::refusesMalformedDuplicatedAndOversizedOptions()
{
    const QStringList missingValue {QStringLiteral("verb"), QStringLiteral("output")};
    QVERIFY(!parseShellCommandLine(missingValue).error.isEmpty());

    const QStringList duplicated {
        QStringLiteral("verb"),
        QStringLiteral("output=DP-1"),
        QStringLiteral("output=DP-2"),
    };
    QVERIFY(!parseShellCommandLine(duplicated).error.isEmpty());

    const QStringList oversized {
        QStringLiteral("verb"),
        QStringLiteral("output=") + QString(300, u'x'),
    };
    QVERIFY(!parseShellCommandLine(oversized).error.isEmpty());

    QStringList tooMany {QStringLiteral("verb")};
    for (int option = 0; option < 20; ++option)
        tooMany.append(QStringLiteral("key%1=1").arg(option));
    QVERIFY(!parseShellCommandLine(tooMany).error.isEmpty());
}

QTEST_GUILESS_MAIN(ShellCommandLineTest)

#include "shellcommandline_test.moc"
