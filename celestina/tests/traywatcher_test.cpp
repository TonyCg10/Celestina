// The tray host against a real bus and five real StatusNotifierItems.
//
// Everything else about the tray is tested against fabricated values: a map
// handed to `readTrayItem`, a list handed to `TrayDrawer`. That leaves the part
// the live failure actually ran through completely unproven — registration,
// asynchronous `GetAll`, QtDBus demarshalling of `a{sv}` and `a(iiay)`, the
// generation guard on registry re-reads, `m_read`, `publish()` and `items()`.
// Four registered applications and two rendered ones is a symptom that could
// live anywhere along that path, and no unit test can see any of it.
//
// So this walks it. The bus is private and started by this process, and the
// watcher name it claims is the real one — which is exactly why it must never
// be the author's session bus: claiming `org.kde.StatusNotifierWatcher` there
// would take the tray from the shell that is running.

#include <QtTest>

#include <QByteArray>
#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusMetaType>
#include <QDBusObjectPath>
#include <QDBusPendingCall>
#include <QGuiApplication>
#include <QProcess>
#include <QSharedPointer>
#include <QVariantList>

#include "trayicons.h"
#include "traywatcher.h"

namespace {
constexpr auto watcherService = "org.kde.StatusNotifierWatcher";
constexpr auto watcherPath = "/StatusNotifierWatcher";
constexpr int settleMs = 10000;

/// One entry of `IconPixmap`, whose D-Bus signature is `a(iiay)`. Declaring it
/// is the only way a Qt client can publish the shape Chromium really publishes,
/// and demarshalling it is the step this test exists to exercise.
struct SniPixmap {
    int width = 0;
    int height = 0;
    QByteArray argb;
};

QDBusArgument &operator<<(QDBusArgument &argument, const SniPixmap &pixmap)
{
    argument.beginStructure();
    argument << pixmap.width << pixmap.height << pixmap.argb;
    argument.endStructure();
    return argument;
}

const QDBusArgument &operator>>(const QDBusArgument &argument, SniPixmap &pixmap)
{
    argument.beginStructure();
    argument >> pixmap.width >> pixmap.height >> pixmap.argb;
    argument.endStructure();
    return argument;
}
} // namespace

Q_DECLARE_METATYPE(SniPixmap)
Q_DECLARE_METATYPE(QList<SniPixmap>)

namespace {
struct SniToolTip {
    QString iconName;
    QList<SniPixmap> iconPixmaps;
    QString title;
    QString description;
};

QDBusArgument &operator<<(QDBusArgument &argument, const SniToolTip &toolTip)
{
    argument.beginStructure();
    argument << toolTip.iconName << toolTip.iconPixmaps
             << toolTip.title << toolTip.description;
    argument.endStructure();
    return argument;
}

const QDBusArgument &operator>>(const QDBusArgument &argument,
                                SniToolTip &toolTip)
{
    argument.beginStructure();
    argument >> toolTip.iconName >> toolTip.iconPixmaps
             >> toolTip.title >> toolTip.description;
    argument.endStructure();
    return argument;
}
} // namespace

Q_DECLARE_METATYPE(SniToolTip)

/// An item that names a themed icon and publishes no pixels: Solaar, nm-applet
/// and Blueman on this session. It deliberately has no `IconPixmap` property at
/// all, because `GetAll` omitting a key is the shape the host must survive.
class ThemedItem final : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.kde.StatusNotifierItem")
    Q_PROPERTY(QString Id READ id CONSTANT)
    Q_PROPERTY(QString Title READ title CONSTANT)
    Q_PROPERTY(QString Status READ status CONSTANT)
    Q_PROPERTY(QString Category READ category CONSTANT)
    Q_PROPERTY(QString IconName READ iconName CONSTANT)
    Q_PROPERTY(QString IconThemePath READ iconThemePath CONSTANT)
    Q_PROPERTY(QDBusObjectPath Menu READ menu CONSTANT)

public:
    ThemedItem(QString id, QString title, QString iconName, QString menuPath)
        : m_id(std::move(id))
        , m_title(std::move(title))
        , m_iconName(std::move(iconName))
        , m_menu(std::move(menuPath))
    {
    }

    QString id() const { return m_id; }
    QString title() const { return m_title; }
    QString status() const { return QStringLiteral("Active"); }
    QString category() const { return QStringLiteral("Hardware"); }
    QString iconName() const { return m_iconName; }
    QString iconThemePath() const { return QString(); }
    QDBusObjectPath menu() const { return QDBusObjectPath(m_menu); }

private:
    QString m_id;
    QString m_title;
    QString m_iconName;
    QString m_menu;
};

/// An item that publishes raw pixels and no icon name, and gives no title at
/// all: Slack, through Chromium's own StatusNotifierItem.
class PixmapItem final : public QObject
{
    Q_OBJECT
    Q_CLASSINFO("D-Bus Interface", "org.kde.StatusNotifierItem")
    Q_PROPERTY(QString Id READ id CONSTANT)
    Q_PROPERTY(QString Title READ title CONSTANT)
    Q_PROPERTY(QString Status READ status CONSTANT)
    Q_PROPERTY(QString Category READ category CONSTANT)
    Q_PROPERTY(QList<SniPixmap> IconPixmap READ iconPixmap CONSTANT)
    Q_PROPERTY(SniToolTip ToolTip READ toolTip CONSTANT)
    Q_PROPERTY(QDBusObjectPath Menu READ menu CONSTANT)

public:
    PixmapItem(QString id, QString toolTipTitle)
        : m_id(std::move(id))
        , m_toolTipTitle(std::move(toolTipTitle))
    {
    }

    QString id() const { return m_id; }
    // Empty, exactly as Chromium publishes it.
    QString title() const { return QString(); }
    QString status() const { return QStringLiteral("Active"); }
    QString category() const { return QStringLiteral("ApplicationStatus"); }

    QList<SniPixmap> iconPixmap() const
    {
        SniPixmap pixmap;
        pixmap.width = 22;
        pixmap.height = 22;
        // Opaque white, in the network byte order the specification names. The
        // exact colour does not matter; that the host reads 22 × 22 × 4 bytes
        // back out of a nested container does.
        pixmap.argb = QByteArray(22 * 22 * 4, '\xff');
        return {pixmap};
    }

    SniToolTip toolTip() const
    {
        return SniToolTip {QString(), {}, m_toolTipTitle, QString()};
    }

    QDBusObjectPath menu() const
    {
        return QDBusObjectPath(QStringLiteral("/org/chromium/DbusMenu/1"));
    }

private:
    QString m_id;
    QString m_toolTipTitle;
};

/// A registration whose object is never exported. `GetAll` fails against it, so
/// it is how the host's retry-then-name-it-anyway policy is exercised without
/// pretending to know why a real application would fail.
class SilentItem final : public QObject
{
    Q_OBJECT
};

class TrayWatcherTest final : public QObject
{
    Q_OBJECT

private slots:
    void initTestCase();
    void cleanupTestCase();

    void everyRegisteredItemReachesTheHostsPublishedList();
    void anItemThatNeverAnswersIsStillPublishedUnderItsRegistration();

private:
    /// One fake application: its own bus connection, so its items arrive from
    /// their own unique name exactly as four separate processes' would.
    QDBusConnection application(const QString &name);
    void registerItem(QDBusConnection &bus, const QString &path);
    bool waitForItems(TrayWatcher *watcher, int expected);

    QSharedPointer<TrayIconCache> m_icons;
    QStringList m_connections;
};

void TrayWatcherTest::initTestCase()
{
    qDBusRegisterMetaType<SniPixmap>();
    qDBusRegisterMetaType<QList<SniPixmap>>();
    qDBusRegisterMetaType<SniToolTip>();
    m_icons = QSharedPointer<TrayIconCache>::create();

    QVERIFY2(
        QDBusConnection::sessionBus().isConnected(),
        "the private bus set up in main() is reachable"
    );
    // The bus is private, so the real watcher name is free — and claiming it is
    // what makes this the host's own registry rather than a stand-in.
    QVERIFY(QDBusConnection::sessionBus().interface());
}

void TrayWatcherTest::cleanupTestCase()
{
    for (const QString &name : m_connections)
        QDBusConnection::disconnectFromBus(name);
    m_connections.clear();
}

QDBusConnection TrayWatcherTest::application(const QString &name)
{
    m_connections.append(name);
    return QDBusConnection::connectToBus(QDBusConnection::SessionBus, name);
}

void TrayWatcherTest::registerItem(QDBusConnection &bus, const QString &path)
{
    // The path alone, which is what an application that has not claimed a
    // well-known name sends. The watcher composes it with the sender's unique
    // name, and that composed string is what the host has to parse.
    QDBusMessage request = QDBusMessage::createMethodCall(
        QString::fromLatin1(watcherService),
        QString::fromLatin1(watcherPath),
        QString::fromLatin1(watcherService),
        QStringLiteral("RegisterStatusNotifierItem")
    );
    request.setArguments({path});
    bus.asyncCall(request);
}

bool TrayWatcherTest::waitForItems(TrayWatcher *watcher, int expected)
{
    QElapsedTimer elapsed;
    elapsed.start();
    while (elapsed.elapsed() < settleMs) {
        if (watcher->items().size() == expected)
            return true;
        QCoreApplication::processEvents(QEventLoop::AllEvents, 50);
        QTest::qWait(20);
    }
    return watcher->items().size() == expected;
}

/// The whole path the live failure ran through: five applications register,
/// five objects answer `GetAll` asynchronously, and the host publishes five
/// items with what each one really said.
void TrayWatcherTest::everyRegisteredItemReachesTheHostsPublishedList()
{
    TrayWatcher watcher(m_icons);

    PixmapItem slack(
        QStringLiteral("Slack_status_icon_1"),
        QStringLiteral("No unread messages")
    );
    PixmapItem chatgpt(
        QStringLiteral("chrome_status_icon_1"),
        QStringLiteral("ChatGPT")
    );
    ThemedItem solaar(
        QStringLiteral("indicator-solaar"),
        QStringLiteral("Solaar"),
        QStringLiteral("battery-good"),
        QStringLiteral("/org/ayatana/NotificationItem/indicator_solaar/Menu")
    );
    ThemedItem applet(
        QStringLiteral("nm-applet"),
        QStringLiteral("Red"),
        QStringLiteral("nm-signal-100"),
        QStringLiteral("/org/ayatana/NotificationItem/nm_applet/Menu")
    );
    ThemedItem blueman(
        QStringLiteral("blueman"),
        QStringLiteral("blueman"),
        QStringLiteral("blueman-active"),
        QStringLiteral("/org/blueman/sni/menu")
    );

    const QString slackPath = QStringLiteral("/org/chromium/StatusNotifierItem/1");
    const QString chatgptPath = QStringLiteral("/org/chromium/StatusNotifierItem/2");
    const QString solaarPath =
        QStringLiteral("/org/ayatana/NotificationItem/indicator_solaar");
    const QString appletPath = QStringLiteral("/org/ayatana/NotificationItem/nm_applet");
    const QString bluemanPath = QStringLiteral("/org/blueman/sni");

    QDBusConnection slackBus = application(QStringLiteral("fake-slack"));
    QDBusConnection chatgptBus = application(QStringLiteral("fake-chatgpt"));
    QDBusConnection solaarBus = application(QStringLiteral("fake-solaar"));
    QDBusConnection appletBus = application(QStringLiteral("fake-nm-applet"));
    QDBusConnection bluemanBus = application(QStringLiteral("fake-blueman"));

    QVERIFY(slackBus.registerObject(slackPath, &slack, QDBusConnection::ExportAllProperties));
    QVERIFY(chatgptBus.registerObject(
        chatgptPath, &chatgpt, QDBusConnection::ExportAllProperties));
    QVERIFY(solaarBus.registerObject(solaarPath, &solaar, QDBusConnection::ExportAllProperties));
    QVERIFY(appletBus.registerObject(appletPath, &applet, QDBusConnection::ExportAllProperties));
    QVERIFY(bluemanBus.registerObject(bluemanPath, &blueman, QDBusConnection::ExportAllProperties));

    registerItem(slackBus, slackPath);
    registerItem(chatgptBus, chatgptPath);
    registerItem(solaarBus, solaarPath);
    registerItem(appletBus, appletPath);
    registerItem(bluemanBus, bluemanPath);

    if (!waitForItems(&watcher, 5)) {
        QStringList seen;
        for (const QVariant &entry : watcher.items())
            seen.append(entry.toMap().value(QStringLiteral("id")).toString());
        QFAIL(qPrintable(QStringLiteral("published %1: %2")
                             .arg(watcher.items().size())
                             .arg(seen.join(QStringLiteral(", ")))));
    }
    QVERIFY(watcher.available());

    QVariantMap byTitle;
    const QVariantList published = watcher.items();
    for (const QVariant &entry : published) {
        const QVariantMap item = entry.toMap();
        byTitle.insert(item.value(QStringLiteral("title")).toString(), item);
    }
    QCOMPARE(byTitle.size(), 5);

    // Slack's tooltip is transient state; its app-specific Id supplies the
    // display name after the bridge's technical suffix is removed.
    const QVariantMap slackItem = byTitle.value(QStringLiteral("Slack")).toMap();
    QVERIFY(!slackItem.isEmpty());
    QCOMPARE(slackItem.value(QStringLiteral("id")).toString(), QStringLiteral("Slack_status_icon_1"));
    QCOMPARE(slackItem.value(QStringLiteral("status")).toString(), QStringLiteral("active"));
    QVERIFY(slackItem.value(QStringLiteral("hasPixmap")).toBool());
    QVERIFY(slackItem.value(QStringLiteral("hasMenu")).toBool());
    QVERIFY2(
        !slackItem.value(QStringLiteral("iconSource")).toString().isEmpty(),
        "22x22 published pixels resolved to something the drawer can draw"
    );

    // ChatGPT's Id names only the generic Chromium runtime. Its real tooltip
    // structure is therefore the protocol source for the product identity.
    const QVariantMap chatgptItem =
        byTitle.value(QStringLiteral("ChatGPT")).toMap();
    QVERIFY(!chatgptItem.isEmpty());
    QCOMPARE(chatgptItem.value(QStringLiteral("id")).toString(),
             QStringLiteral("chrome_status_icon_1"));

    // Solaar gave a themed name, a title and a menu, and no pixels at all.
    const QVariantMap solaarItem = byTitle.value(QStringLiteral("Solaar")).toMap();
    QVERIFY(!solaarItem.isEmpty());
    QCOMPARE(solaarItem.value(QStringLiteral("id")).toString(), QStringLiteral("indicator-solaar"));
    QCOMPARE(solaarItem.value(QStringLiteral("iconName")).toString(), QStringLiteral("battery-good"));
    QCOMPARE(solaarItem.value(QStringLiteral("status")).toString(), QStringLiteral("active"));
    QVERIFY(!solaarItem.value(QStringLiteral("hasPixmap")).toBool());
    QVERIFY(solaarItem.value(QStringLiteral("hasMenu")).toBool());

    QVERIFY(!byTitle.value(QStringLiteral("Red")).toMap().isEmpty());
    QVERIFY(!byTitle.value(QStringLiteral("blueman")).toMap().isEmpty());

    // None of them asks for attention, which is the state the folded drawer
    // filters on. That is the drawer's rule and not a loss, and it is why the
    // folded bar now carries a count.
    for (const QVariant &entry : published)
        QCOMPARE(entry.toMap().value(QStringLiteral("status")).toString(), QStringLiteral("active"));

    // A second host on the same registry re-reads it from scratch. This is the
    // path where a superseded reply could clear the current one: `attach()` is
    // re-entered on every watcher owner change, including this shell acquiring
    // the name itself.
    TrayWatcher second(m_icons);
    QVERIFY2(
        waitForItems(&second, 5),
        "a fresh registry read finds all five rather than racing itself"
    );
}

/// An item that registers and then never answers is a registration the host
/// cannot describe. It is still a control the person can click, so it is named
/// from its registration rather than dropped — the behaviour `LVR-3-F` added
/// after a `GetAll` failure proved able to lose an item silently.
void TrayWatcherTest::anItemThatNeverAnswersIsStillPublishedUnderItsRegistration()
{
    TrayWatcher watcher(m_icons);

    ThemedItem present(
        QStringLiteral("nm-applet"),
        QStringLiteral("Red"),
        QStringLiteral("nm-signal-100"),
        QStringLiteral("/org/ayatana/NotificationItem/nm_applet/Menu")
    );
    const QString presentPath = QStringLiteral("/org/ayatana/NotificationItem/nm_applet");
    const QString silentPath = QStringLiteral("/org/ayatana/NotificationItem/indicator_silent");

    QDBusConnection presentBus = application(QStringLiteral("fake-present"));
    QDBusConnection silentBus = application(QStringLiteral("fake-silent"));
    QVERIFY(presentBus.registerObject(presentPath, &present, QDBusConnection::ExportAllProperties));

    registerItem(presentBus, presentPath);
    // Registered, never exported: `GetAll` reaches nothing.
    registerItem(silentBus, silentPath);

    QVERIFY2(waitForItems(&watcher, 2), "the silent registration is published too");

    QStringList titles;
    for (const QVariant &entry : watcher.items())
        titles.append(entry.toMap().value(QStringLiteral("title")).toString());

    QVERIFY(titles.contains(QStringLiteral("Red")));
    // Named from its own object path, which is the only name it ever gave.
    QVERIFY2(
        titles.contains(QStringLiteral("indicator_silent")),
        qPrintable(titles.join(QStringLiteral(", ")))
    );
    for (const QString &title : titles)
        QVERIFY(!title.isEmpty());
}

// A private session bus, started before anything can ask for the session one.
//
// The order is the whole point: QtDBus caches its session connection on first
// use, so the address has to be in the environment before `sessionBus()` is
// ever called — which is why this is `main` rather than `initTestCase`.
int main(int argc, char *argv[])
{
    QProcess daemon;
    daemon.start(
        QStringLiteral("dbus-daemon"),
        {QStringLiteral("--session"),
         QStringLiteral("--print-address"),
         QStringLiteral("--nofork"),
         QStringLiteral("--nopidfile")}
    );
    if (!daemon.waitForStarted(5000) || !daemon.waitForReadyRead(5000)) {
        qCritical("traywatcher_test: could not start its required private dbus-daemon");
        return 1;
    }

    const QByteArray address = daemon.readLine().trimmed();
    if (address.isEmpty()) {
        daemon.kill();
        daemon.waitForFinished(2000);
        qCritical("traywatcher_test: the required private bus printed no address");
        return 1;
    }
    qputenv("DBUS_SESSION_BUS_ADDRESS", address);

    QGuiApplication application(argc, argv);
    TrayWatcherTest test;
    const int result = QTest::qExec(&test, argc, argv);

    daemon.kill();
    daemon.waitForFinished(2000);
    return result;
}

#include "traywatcher_test.moc"
