#include <QtTest>

#include <QDBusObjectPath>

#include "trayitems.h"

// The fixtures here are the shapes this session actually publishes, taken from
// its four live tray items — including the two that disagree with the
// specification.
class TrayItemsTest final : public QObject
{
    Q_OBJECT

private slots:
    void readsARegistrationWithAndWithoutAPath();
    void refusesARegistrationItCannotUse();
    void readsAWellBehavedItem();
    void anItemWithNoUsableIconStillHasAName();
    void everyStatusOutsideTheSpecificationIsShownAnyway();
    void refusesAnIconThemeItCannotResolve();
    void aListReplacesWholesaleAndReportsRealChangesOnly();
};

void TrayItemsTest::readsARegistrationWithAndWithoutAPath()
{
    QString service;
    QString path;

    QVERIFY(parseTrayRegistration(
        QStringLiteral(":1.19/org/ayatana/NotificationItem/nm_applet"),
        &service,
        &path
    ));
    QCOMPARE(service, QStringLiteral(":1.19"));
    QCOMPARE(path, QStringLiteral("/org/ayatana/NotificationItem/nm_applet"));

    // A bare bus name means the specification's default path.
    QVERIFY(parseTrayRegistration(QStringLiteral(":1.74"), &service, &path));
    QCOMPARE(service, QStringLiteral(":1.74"));
    QCOMPARE(path, QStringLiteral("/StatusNotifierItem"));
}

void TrayItemsTest::refusesARegistrationItCannotUse()
{
    QString service;
    QString path;

    QVERIFY(!parseTrayRegistration(QString(), &service, &path));
    QVERIFY(!parseTrayRegistration(QStringLiteral("   "), &service, &path));
    // A path with no bus name in front of it answers for nobody.
    QVERIFY(!parseTrayRegistration(QStringLiteral("/StatusNotifierItem"), &service, &path));
    QVERIFY(!parseTrayRegistration(QString(600, u'x'), &service, &path));
}

void TrayItemsTest::readsAWellBehavedItem()
{
    const QVariantMap properties {
        {QStringLiteral("Id"), QStringLiteral("nm-applet")},
        {QStringLiteral("Title"), QStringLiteral("Red")},
        {QStringLiteral("Status"), QStringLiteral("Active")},
        {QStringLiteral("IconName"), QStringLiteral("nm-signal-75")},
        {QStringLiteral("IconThemePath"), QString()},
        {QStringLiteral("Menu"),
         QVariant::fromValue(
             QDBusObjectPath(QStringLiteral("/org/ayatana/NotificationItem/nm_applet/Menu"))
         )},
    };

    const TrayItem item = readTrayItem(
        QStringLiteral(":1.19"),
        QStringLiteral("/org/ayatana/NotificationItem/nm_applet"),
        properties
    );

    QCOMPARE(item.id, QStringLiteral("nm-applet"));
    QCOMPARE(item.title, QStringLiteral("Red"));
    QCOMPARE(item.status, QStringLiteral("active"));
    QCOMPARE(item.iconName, QStringLiteral("nm-signal-75"));
    QCOMPARE(item.menuPath, QStringLiteral("/org/ayatana/NotificationItem/nm_applet/Menu"));
    QVERIFY(!item.hasPixmap);
    // `ItemIsMenu` is absent from this item entirely, and reading it must not
    // have invented anything.
    QVERIFY(item.iconThemePath.isEmpty());
}

void TrayItemsTest::anItemWithNoUsableIconStillHasAName()
{
    // The shape this session's Slack item publishes: no `IconName` key at all
    // — its getter fails, so `GetAll` omits it — an empty title, and raw pixels
    // instead.
    const QVariantMap properties {
        {QStringLiteral("Id"), QStringLiteral("Slack_status_icon_1")},
        {QStringLiteral("Title"), QString()},
        {QStringLiteral("Status"), QStringLiteral("Active")},
        {QStringLiteral("IconPixmap"), QVariantList {QVariant(22)}},
    };

    const TrayItem item = readTrayItem(
        QStringLiteral(":1.74"),
        QStringLiteral("/StatusNotifierItem"),
        properties
    );

    QVERIFY(item.iconName.isEmpty());
    QVERIFY(item.hasPixmap);
    // No title, so the panel calls it what the application calls itself.
    QCOMPARE(item.title, QStringLiteral("Slack_status_icon_1"));
    // No menu either: an absent path must not become "/".
    QVERIFY(item.menuPath.isEmpty());
}

void TrayItemsTest::everyStatusOutsideTheSpecificationIsShownAnyway()
{
    const auto statusOf = [](const QVariant &status) {
        return readTrayItem(
                   QStringLiteral(":1.1"),
                   QStringLiteral("/StatusNotifierItem"),
                   QVariantMap {{QStringLiteral("Status"), status}}
        )
            .status;
    };

    QCOMPARE(statusOf(QStringLiteral("Passive")), QStringLiteral("passive"));
    QCOMPARE(statusOf(QStringLiteral("NeedsAttention")), QStringLiteral("attention"));
    QCOMPARE(statusOf(QStringLiteral("Active")), QStringLiteral("active"));
    // An application getting its own status wrong keeps its control: hiding it
    // would be a worse answer than showing it.
    QCOMPARE(statusOf(QStringLiteral("Whatever")), QStringLiteral("active"));
    QCOMPARE(statusOf(QVariant()), QStringLiteral("active"));
}

void TrayItemsTest::refusesAnIconThemeItCannotResolve()
{
    const auto themeOf = [](const QString &path) {
        return readTrayItem(
                   QStringLiteral(":1.1"),
                   QStringLiteral("/StatusNotifierItem"),
                   QVariantMap {{QStringLiteral("IconThemePath"), path}}
        )
            .iconThemePath;
    };

    QCOMPARE(themeOf(QStringLiteral("/usr/share/solaar/icons")),
             QStringLiteral("/usr/share/solaar/icons"));
    // A relative path from another process names nothing this panel can
    // resolve, and guessing it against a working directory would be a way to
    // read an arbitrary place on disk.
    QVERIFY(themeOf(QStringLiteral("icons")).isEmpty());
    QVERIFY(themeOf(QString()).isEmpty());
}

void TrayItemsTest::aListReplacesWholesaleAndReportsRealChangesOnly()
{
    TrayItems items;
    TrayItem first;
    first.service = QStringLiteral(":1.19");
    first.path = QStringLiteral("/item");
    first.title = QStringLiteral("Red");
    first.status = QStringLiteral("active");

    QVERIFY(items.replace({first}));
    // The same list again is not news.
    QVERIFY(!items.replace({first}));

    TrayItem changed = first;
    changed.status = QStringLiteral("attention");
    QVERIFY(items.replace({changed}));
    QCOMPARE(items.toVariantList().size(), 1);
    QCOMPARE(
        items.toVariantList().first().toMap().value(QStringLiteral("status")).toString(),
        QStringLiteral("attention")
    );

    QVERIFY(items.clear());
    QVERIFY(!items.clear());
    QVERIFY(items.isEmpty());
}

QTEST_GUILESS_MAIN(TrayItemsTest)

#include "trayitems_test.moc"
