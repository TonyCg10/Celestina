#include <QtTest>

#include "traymenu.h"

// The fixtures are this session's real menus: blueman's, with mnemonics,
// separators and a nested submenu, and nm-applet's, which uses disabled entries
// as headings.
class TrayMenuTest final : public QObject
{
    Q_OBJECT

private slots:
    void stripsGtkMnemonics();
    void readsARealMenu();
    void keepsDisabledEntriesBecauseTheyAreHeadings();
    void dropsWhatTheApplicationHid();
    void refusesToFollowATreeAsFarAsItClaims();

private:
    static TrayMenuNode entry(int id, const QVariantMap &properties)
    {
        TrayMenuNode node;
        node.id = id;
        node.properties = properties;
        return node;
    }
};

void TrayMenuTest::stripsGtkMnemonics()
{
    QCOMPARE(trayMenuLabel(QStringLiteral("_Desactivar Bluetooth")),
             QStringLiteral("Desactivar Bluetooth"));
    QCOMPARE(trayMenuLabel(QStringLiteral("Enviar _archivos al dispositivo…")),
             QStringLiteral("Enviar archivos al dispositivo…"));
    // A doubled underscore is the literal one.
    QCOMPARE(trayMenuLabel(QStringLiteral("a__b")), QStringLiteral("a_b"));
    QCOMPARE(trayMenuLabel(QString()), QString());
}

void TrayMenuTest::readsARealMenu()
{
    TrayMenuNode root;
    root.properties.insert(QStringLiteral("children-display"), QStringLiteral("submenu"));
    root.children = {
        entry(65536,
              {{QStringLiteral("label"), QStringLiteral("_Desactivar Bluetooth")},
               {QStringLiteral("icon-name"), QStringLiteral("bluetooth-disabled-symbolic")},
               {QStringLiteral("enabled"), true}}),
        entry(1376256, {{QStringLiteral("type"), QStringLiteral("separator")}}),
    };

    const QVariantList menu = buildTrayMenu(root);

    QCOMPARE(menu.size(), 2);
    const QVariantMap first = menu.first().toMap();
    QCOMPARE(first.value(QStringLiteral("id")).toInt(), 65536);
    QCOMPARE(first.value(QStringLiteral("label")).toString(),
             QStringLiteral("Desactivar Bluetooth"));
    QCOMPARE(first.value(QStringLiteral("iconName")).toString(),
             QStringLiteral("bluetooth-disabled-symbolic"));
    QVERIFY(first.value(QStringLiteral("enabled")).toBool());
    QVERIFY(!first.value(QStringLiteral("separator")).toBool());
    // Nothing said anything about a toggle, so it is indeterminate rather than
    // off.
    QCOMPARE(first.value(QStringLiteral("toggleState")).toInt(), -1);

    const QVariantMap second = menu.at(1).toMap();
    QVERIFY(second.value(QStringLiteral("separator")).toBool());
    // A separator is never something to click.
    QVERIFY(!second.value(QStringLiteral("enabled")).toBool());
}

void TrayMenuTest::keepsDisabledEntriesBecauseTheyAreHeadings()
{
    TrayMenuNode root;
    root.children = {
        entry(61873,
              {{QStringLiteral("label"), QStringLiteral("Red cableada")},
               {QStringLiteral("enabled"), false}}),
        entry(61874, {{QStringLiteral("label"), QStringLiteral("Conexión cableada 1")}}),
    };

    const QVariantList menu = buildTrayMenu(root);

    QCOMPARE(menu.size(), 2);
    QVERIFY(!menu.first().toMap().value(QStringLiteral("enabled")).toBool());
    QCOMPARE(menu.first().toMap().value(QStringLiteral("label")).toString(),
             QStringLiteral("Red cableada"));
    // An entry that said nothing about being enabled is enabled.
    QVERIFY(menu.at(1).toMap().value(QStringLiteral("enabled")).toBool());
}

void TrayMenuTest::dropsWhatTheApplicationHid()
{
    TrayMenuNode root;
    root.children = {
        entry(1, {{QStringLiteral("label"), QStringLiteral("Visible")}}),
        entry(2,
              {{QStringLiteral("label"), QStringLiteral("Oculto")},
               {QStringLiteral("visible"), false}}),
    };

    const QVariantList menu = buildTrayMenu(root);

    QCOMPARE(menu.size(), 1);
    QCOMPARE(menu.first().toMap().value(QStringLiteral("label")).toString(),
             QStringLiteral("Visible"));
}

void TrayMenuTest::refusesToFollowATreeAsFarAsItClaims()
{
    // A menu that nests forever, and one with more entries than a menu has.
    TrayMenuNode deep;
    TrayMenuNode *cursor = &deep;
    for (int level = 0; level < 12; ++level) {
        TrayMenuNode child;
        child.id = level;
        child.properties.insert(QStringLiteral("label"), QStringLiteral("nivel"));
        child.properties.insert(QStringLiteral("children-display"), QStringLiteral("submenu"));
        cursor->children = {child};
        cursor = &cursor->children.first();
    }

    TrayMenuNode wide;
    for (int index = 0; index < 200; ++index)
        wide.children.append(entry(index, {{QStringLiteral("label"), QStringLiteral("x")}}));

    QVERIFY(buildTrayMenu(deep).size() <= 4);
    QCOMPARE(buildTrayMenu(wide).size(), 64);
}

QTEST_GUILESS_MAIN(TrayMenuTest)

#include "traymenu_test.moc"
