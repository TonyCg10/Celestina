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
    void anItemThatNeverDescribedItselfIsStillShown();
    void everyRegistrationThisSessionPublishesParsesAndNamesSomething();
    void thisSessionsFourItemsAllSurviveIntoTheModel();
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

// An item whose `GetAll` fails is the exact shape of "registered with the
// watcher but absent from the tray": the host had nothing to publish for it and
// published nothing, permanently and silently.
void TrayItemsTest::anItemThatNeverDescribedItselfIsStillShown()
{
    const TrayItem ayatana = unreadTrayItem(
        QStringLiteral(":1.32"),
        QStringLiteral("/org/ayatana/NotificationItem/indicator_solaar")
    );
    QCOMPARE(ayatana.title, QStringLiteral("indicator_solaar"));
    // Active by default: hiding it is the failure this exists to prevent.
    QCOMPARE(ayatana.status, QStringLiteral("active"));
    QVERIFY(!ayatana.hasPixmap);
    QVERIFY(ayatana.menuPath.isEmpty());

    // A numbered path names nothing, so the bus name is the only name there is.
    const TrayItem numbered = unreadTrayItem(
        QStringLiteral(":1.83"),
        QStringLiteral("/org/chromium/StatusNotifierItem/1")
    );
    QCOMPARE(numbered.title, QStringLiteral(":1.83"));

    const TrayItem bare = unreadTrayItem(
        QStringLiteral(":1.9"),
        QStringLiteral("/StatusNotifierItem")
    );
    QCOMPARE(bare.title, QStringLiteral("StatusNotifierItem"));
}

// The four registrations this session actually publishes, captured read-only
// from the bus on 2026-08-07 while Noctalia owned the watcher. Two of them —
// Solaar and Slack — were listed by Celestina's own watcher and never appeared
// in its tray, so these are the shapes any correction has to keep working.
void TrayItemsTest::everyRegistrationThisSessionPublishesParsesAndNamesSomething()
{
    const QStringList registry {
        QStringLiteral(":1.22/org/ayatana/NotificationItem/nm_applet"),
        QStringLiteral(":1.26993/org/blueman/sni"),
        QStringLiteral(":1.32/org/ayatana/NotificationItem/indicator_solaar"),
        QStringLiteral(":1.83/org/chromium/StatusNotifierItem/1"),
    };

    for (const QString &entry : registry) {
        QString service;
        QString path;
        QVERIFY2(parseTrayRegistration(entry, &service, &path), qPrintable(entry));
        QVERIFY(service.startsWith(u':'));
        QVERIFY(path.startsWith(u'/'));
        // Whatever happens to its properties afterwards, an entry the watcher
        // lists always reaches the drawer with something to click.
        QVERIFY(!unreadTrayItem(service, path).title.isEmpty());
    }
}

// The four items this session really registers, with the exact properties each
// one answered `GetAll` with on 2026-08-07 — Slack's empty `Title`, Solaar's
// themed icon and no pixels, and the two Ayatana applets that did appear.
//
// The author saw only two of the four in the tray. This is the first of the
// four places an item could have been lost: if the model drops one, nothing
// downstream can show it. It does not.
void TrayItemsTest::thisSessionsFourItemsAllSurviveIntoTheModel()
{
    // Chromium publishes pixels and no icon name, and gives no title at all.
    QVariantMap slack;
    slack.insert(QStringLiteral("Id"), QStringLiteral("Slack_status_icon_1"));
    slack.insert(QStringLiteral("Status"), QStringLiteral("Active"));
    slack.insert(QStringLiteral("Title"), QString());
    slack.insert(QStringLiteral("Category"), QStringLiteral("ApplicationStatus"));
    slack.insert(
        QStringLiteral("IconPixmap"),
        QVariant::fromValue(QVariantList {QVariant::fromValue(QVariantList {22, 22})})
    );
    slack.insert(
        QStringLiteral("Menu"),
        QVariant::fromValue(QDBusObjectPath(QStringLiteral("/org/chromium/DbusMenu/1")))
    );

    // Ayatana publishes a themed name and no pixels.
    QVariantMap solaar;
    solaar.insert(QStringLiteral("Id"), QStringLiteral("indicator-solaar"));
    solaar.insert(QStringLiteral("Title"), QStringLiteral("Solaar"));
    solaar.insert(QStringLiteral("Status"), QStringLiteral("Active"));
    solaar.insert(QStringLiteral("IconName"), QStringLiteral("battery-good"));
    solaar.insert(QStringLiteral("IconThemePath"), QString());
    solaar.insert(
        QStringLiteral("Menu"),
        QVariant::fromValue(
            QDBusObjectPath(QStringLiteral("/org/ayatana/NotificationItem/indicator_solaar/Menu"))
        )
    );

    QVariantMap applet;
    applet.insert(QStringLiteral("Id"), QStringLiteral("nm-applet"));
    applet.insert(QStringLiteral("Title"), QStringLiteral("Red"));
    applet.insert(QStringLiteral("Status"), QStringLiteral("Active"));
    applet.insert(QStringLiteral("IconName"), QStringLiteral("nm-signal-100"));
    applet.insert(
        QStringLiteral("Menu"),
        QVariant::fromValue(
            QDBusObjectPath(QStringLiteral("/org/ayatana/NotificationItem/nm_applet/Menu"))
        )
    );

    QVariantMap blueman;
    blueman.insert(QStringLiteral("Id"), QStringLiteral("blueman"));
    blueman.insert(QStringLiteral("Title"), QStringLiteral("blueman"));
    blueman.insert(QStringLiteral("Status"), QStringLiteral("Active"));
    blueman.insert(QStringLiteral("IconName"), QStringLiteral("blueman-active"));
    blueman.insert(
        QStringLiteral("Menu"),
        QVariant::fromValue(QDBusObjectPath(QStringLiteral("/org/blueman/sni/menu")))
    );

    const QList<TrayItem> read {
        readTrayItem(QStringLiteral(":1.83"), QStringLiteral("/org/chromium/StatusNotifierItem/1"), slack),
        readTrayItem(QStringLiteral(":1.32"), QStringLiteral("/org/ayatana/NotificationItem/indicator_solaar"), solaar),
        readTrayItem(QStringLiteral(":1.22"), QStringLiteral("/org/ayatana/NotificationItem/nm_applet"), applet),
        readTrayItem(QStringLiteral(":1.26993"), QStringLiteral("/org/blueman/sni"), blueman),
    };

    // Every one of them is `active`, which is the ordinary state. None asks for
    // attention, so a folded drawer shows none of them — by design, and that is
    // what the QML regression measures next.
    for (const TrayItem &item : read) {
        QCOMPARE(item.status, QStringLiteral("active"));
        QVERIFY2(!item.title.isEmpty(), qPrintable(item.service));
    }
    // Slack gave no title, so it is named by its own id rather than left blank.
    QCOMPARE(read.at(0).title, QStringLiteral("Slack_status_icon_1"));
    QVERIFY(read.at(0).hasPixmap);
    QVERIFY(read.at(0).iconName.isEmpty());
    // Solaar gave a name and no pixels, which is the other way an item arrives.
    QCOMPARE(read.at(1).title, QStringLiteral("Solaar"));
    QVERIFY(!read.at(1).hasPixmap);
    QCOMPARE(read.at(1).iconName, QStringLiteral("battery-good"));

    TrayItems items;
    QVERIFY(items.replace(read));
    const QVariantList published = items.toVariantList();
    QCOMPARE(published.size(), 4);
    for (const QVariant &entry : published) {
        const QVariantMap row = entry.toMap();
        QVERIFY(!row.value(QStringLiteral("title")).toString().isEmpty());
        QCOMPARE(row.value(QStringLiteral("status")).toString(), QStringLiteral("active"));
        QVERIFY(row.value(QStringLiteral("hasMenu")).toBool());
    }
}

QTEST_GUILESS_MAIN(TrayItemsTest)

#include "trayitems_test.moc"
