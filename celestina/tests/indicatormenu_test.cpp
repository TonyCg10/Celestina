#include <QtTest>

#include <QQmlComponent>
#include <QQmlEngine>
#include <QQmlProperty>
#include <QQuickItem>
#include <QQuickWindow>
#include <QSignalSpy>
#include <QStringList>
#include <QUrl>
#include <QVariantMap>

#include <memory>

#include "panelmenucontroller.h"
#include "requestledger.h"

namespace {
// Collects what Qt says while a component is created. Qt does not fail a
// component over an initial property it does not declare — it logs and carries
// on — so the log is the only place that mistake exists until a person opens
// the surface and finds it half-bound.
QStringList *captured = nullptr;

void collect(QtMsgType, const QMessageLogContext &, const QString &message)
{
    if (captured)
        captured->append(message);
}

// A provider bridge with the shape the menus really read: a snapshot, a
// revision, and the durable request ledger. The ledger is the real one — the
// point of these cases is that it outlives the window that used it.
class FakeBridge final : public QObject, public RequestSink
{
    Q_OBJECT
    Q_PROPERTY(QVariantMap providers READ providers NOTIFY changed)
    Q_PROPERTY(qulonglong revision READ revision NOTIFY changed)
    Q_PROPERTY(bool available READ available NOTIFY changed)
    Q_PROPERTY(RequestLedger *requests READ requests CONSTANT)

public:
    explicit FakeBridge(QObject *parent = nullptr)
        : QObject(parent)
        , m_ledger(new RequestLedger(this, this))
    {
    }

    QVariantMap providers() const { return m_providers; }
    qulonglong revision() const { return m_revision; }
    bool available() const { return m_available; }
    RequestLedger *requests() const { return m_ledger; }

    void publish(const QVariantMap &next)
    {
        m_providers = next;
        ++m_revision;
        emit changed();
    }

    quint64 sendRequest(const QString &provider, const QString &verb, const QVariantMap &) override
    {
        sent.append(provider + u'/' + verb);
        return ++m_lastId;
    }

    QStringList sent;

signals:
    void changed();

private:
    RequestLedger *m_ledger;
    QVariantMap m_providers;
    qulonglong m_revision = 0;
    quint64 m_lastId = 0;
    bool m_available = true;
};

// What a compositor configure leaves behind, applied by hand because nothing
// here has a compositor. The surface covers the output; the card is a fraction
// of it, which is what makes "outside the card" a real place to click.
constexpr int outputWidth = 1920;
constexpr int outputHeight = 1080;
// The panel is 40 px tall and its indicators sit inside that band, so a click
// aimed at an indicator lands here — above the card, which the host places on
// the panel's bottom edge.
constexpr int indicatorRow = 20;
} // namespace

// What the two connectivity menus are handed, and what a click or a key does to
// them.
//
// This proves the contract inside the window, not the surface around it.
// Whether the compositor delivers a click at the indicator to this window at
// all is what `PanelMenuSurface` arranges by covering the output, and only a
// real Wayland session can show that; what is provable here is that the window
// answers such a click with `dismissed()` and leaves a click on the card alone.
class IndicatorMenuTest final : public QObject
{
    Q_OBJECT

private slots:
    void everyIndicatorKindNamesTheComponentThatDrawsIt();
    void eachMenuDeclaresExactlyWhatTheHostHandsIt();
    void aPropertyTheComponentDoesNotDeclareIsVisibleAsAFailure();
    void aClickWhereTheIndicatorIsDismissesTheMenu();
    void aClickOnTheCardDismissesNothing();
    void escapeDismissesTheMenu();
    void aCardAskedForBeyondTheOutputStaysWhole();
    void theWholeMenuIsReachableFromTheKeyboard();
    void everyRowNamesItselfAndItsState();
    void activatingARowClosesTheMenuAndOutlivesItsWindow();
    void aReopenedMenuShowsWhatHappenedWhileItWasClosed();
    void aFailureOutlivesTheRowThatWouldHaveShownIt();
    void aNetworkResultLeavesBluetoothAlone();
    void theControlCentreStopsAskingWhenTheHelperAccepts();

private:
    // The kinds `PanelManager` can forward, which is the set the list must
    // cover. A kind added without a component fails this case rather than a
    // session.
    static QStringList kinds()
    {
        return {QStringLiteral("network"), QStringLiteral("bluetooth")};
    }

    static QUrl sourceFor(const QString &component)
    {
        return QUrl::fromLocalFile(
            QStringLiteral(CELESTINA_QML_DIR "/") + component + QStringLiteral(".qml")
        );
    }

    static bool complainedAboutAProperty(const QStringList &messages)
    {
        for (const QString &message : messages) {
            if (message.contains(QStringLiteral("does not have a property called")))
                return true;
        }
        return false;
    }

    // A menu, sized and shown as its surface would be. The provider bridge is
    // null on purpose: every reading goes through a guard that already answers
    // "nothing published yet", and this case is about the window, not the data.
    static QQuickWindow *openMenu(
        QQmlEngine &engine,
        const QString &kind,
        std::unique_ptr<QObject> &owner
    )
    {
        QQmlComponent menu(&engine, sourceFor(indicatorMenuComponent(kind)));
        if (!menu.isReady()) {
            qWarning().noquote() << menu.errorString();
            return nullptr;
        }

        owner.reset(menu.createWithInitialProperties({
            {QStringLiteral("reducedMotion"), true},
            {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(nullptr)},
        }));
        auto *window = qobject_cast<QQuickWindow *>(owner.get());
        if (!window)
            return nullptr;

        window->resize(outputWidth, outputHeight);
        // Where the host puts it: the column of the click, the line of the
        // panel's bottom edge.
        window->setProperty("menuX", 1600);
        window->setProperty("menuY", 40);
        window->show();
        return QTest::qWaitForWindowExposed(window) ? window : nullptr;
    }
};

void IndicatorMenuTest::everyIndicatorKindNamesTheComponentThatDrawsIt()
{
    QCOMPARE(
        indicatorMenuComponent(QStringLiteral("network")),
        QStringLiteral("NetworkMenu")
    );
    QCOMPARE(
        indicatorMenuComponent(QStringLiteral("bluetooth")),
        QStringLiteral("BluetoothMenu")
    );
    // A kind this shell does not have opens nothing rather than something else.
    QVERIFY(indicatorMenuComponent(QStringLiteral("power")).isEmpty());
    QVERIFY(indicatorMenuComponent(QString()).isEmpty());
}

void IndicatorMenuTest::eachMenuDeclaresExactlyWhatTheHostHandsIt()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &kind : kinds()) {
        const QString component = indicatorMenuComponent(kind);
        QVERIFY2(!component.isEmpty(), qPrintable(kind));

        QQmlComponent menu(&engine, sourceFor(component));
        QVERIFY2(menu.isReady(), qPrintable(menu.errorString()));

        QStringList messages;
        captured = &messages;
        QtMessageHandler previous = qInstallMessageHandler(collect);
        // Exactly what `toggleIndicatorMenu` passes, and nothing else.
        QObject *const root = menu.createWithInitialProperties({
            {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(nullptr)},
            {QStringLiteral("reducedMotion"), true},
        });
        qInstallMessageHandler(previous);
        captured = nullptr;

        QVERIFY2(root != nullptr, qPrintable(menu.errorString()));
        QVERIFY2(
            !complainedAboutAProperty(messages),
            qPrintable(component + QStringLiteral(": ") + messages.join(u'\n'))
        );
        delete root;
    }
}

// The case above only means something if it can fail.
void IndicatorMenuTest::aPropertyTheComponentDoesNotDeclareIsVisibleAsAFailure()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    QQmlComponent menu(&engine, sourceFor(QStringLiteral("NetworkMenu")));
    QVERIFY2(menu.isReady(), qPrintable(menu.errorString()));

    QStringList messages;
    captured = &messages;
    QtMessageHandler previous = qInstallMessageHandler(collect);
    QObject *const root = menu.createWithInitialProperties({
        {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(nullptr)},
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("shellSource"), QVariant::fromValue<QObject *>(nullptr)},
    });
    qInstallMessageHandler(previous);
    captured = nullptr;

    QVERIFY(complainedAboutAProperty(messages));
    delete root;
}

// One click closes it, and the click that does so is the one aimed at the
// indicator that opened it. The surface covers the output, so that click lands
// here rather than on the panel — which is exactly why the second click the
// shell once needed is not needed.
void IndicatorMenuTest::aClickWhereTheIndicatorIsDismissesTheMenu()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &kind : kinds()) {
        std::unique_ptr<QObject> owner;
        QQuickWindow *const window = openMenu(engine, kind, owner);
        QVERIFY2(window, qPrintable(kind));

        QSignalSpy dismissed(window, SIGNAL(dismissed()));
        QVERIFY(dismissed.isValid());
        // The panel's own row, where the indicator that opened this sits.
        QTest::mouseClick(window, Qt::LeftButton, {}, QPoint(1620, indicatorRow));
        // `dismissed()` follows the menu's closing transition, so it is waited
        // for rather than expected on the same turn as the click.
        QTRY_COMPARE(dismissed.count(), 1);
    }
}

void IndicatorMenuTest::aClickOnTheCardDismissesNothing()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &kind : kinds()) {
        std::unique_ptr<QObject> owner;
        QQuickWindow *const window = openMenu(engine, kind, owner);
        QVERIFY2(window, qPrintable(kind));

        QSignalSpy dismissed(window, SIGNAL(dismissed()));
        QVERIFY(dismissed.isValid());

        const int cardX = window->property("cardX").toInt();
        const int cardY = window->property("cardY").toInt();
        const int inset = window->property("shadowMargin").toInt();
        const int cardWidth = window->property("cardWidth").toInt();
        QVERIFY(cardWidth > 0);
        // Just inside the card's own edge, in its padding rather than on a row,
        // so what stops the click is the card itself.
        QTest::mouseClick(
            window,
            Qt::LeftButton,
            {},
            QPoint(cardX + cardWidth - inset - 2, cardY + inset + 2)
        );
        // Long enough for a closing transition to have finished if one had
        // started, so this is silence rather than a race won by the assertion.
        QTest::qWait(120);
        QCOMPARE(dismissed.count(), 0);
    }
}

void IndicatorMenuTest::escapeDismissesTheMenu()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &kind : kinds()) {
        std::unique_ptr<QObject> owner;
        QQuickWindow *const window = openMenu(engine, kind, owner);
        QVERIFY2(window, qPrintable(kind));

        QSignalSpy dismissed(window, SIGNAL(dismissed()));
        QVERIFY(dismissed.isValid());
        QTest::keyClick(window, Qt::Key_Escape);
        QTRY_COMPARE(dismissed.count(), 1);
    }
}

// An indicator near the right edge of an output asks for a card that would hang
// off it. The card stays whole; the surface is the output either way, so this
// is the same arithmetic on every scale and every output.
void IndicatorMenuTest::aCardAskedForBeyondTheOutputStaysWhole()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &kind : kinds()) {
        std::unique_ptr<QObject> owner;
        QQuickWindow *const window = openMenu(engine, kind, owner);
        QVERIFY2(window, qPrintable(kind));

        const int cardWidth = window->property("cardWidth").toInt();
        const int cardHeight = window->property("cardHeight").toInt();
        const int shadowMargin = window->property("shadowMargin").toInt();
        QVERIFY(cardWidth > 0);
        QVERIFY(cardHeight > 0);
        QVERIFY(shadowMargin > 0);

        // Asked for past the right edge, and past the bottom.
        window->setProperty("menuX", outputWidth + 500);
        window->setProperty("menuY", outputHeight + 500);
        QCOMPARE(
            window->property("cardX").toInt(),
            outputWidth - cardWidth + shadowMargin
        );
        QCOMPARE(
            window->property("cardY").toInt(),
            outputHeight - cardHeight + shadowMargin
        );

        // At the other edges only the transparent shadow may leave the
        // surface; the visible menu itself still starts at zero.
        window->setProperty("menuX", -400);
        window->setProperty("menuY", -400);
        QCOMPARE(window->property("cardX").toInt(), -shadowMargin);
        QCOMPARE(window->property("cardY").toInt(), -shadowMargin);
    }
}

// A menu is a keyboard surface first. `GlassContextMenu` is a real `Menu`, so
// arrows move its highlight and Return activates — this checks the shell has
// not broken that, and that the highlight is somewhere to begin with.
void IndicatorMenuTest::theWholeMenuIsReachableFromTheKeyboard()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &kind : kinds()) {
        std::unique_ptr<QObject> owner;
        QQuickWindow *const window = openMenu(engine, kind, owner);
        QVERIFY2(window, qPrintable(kind));

        QObject *const menu = window->property("menu").value<QObject *>();
        QVERIFY(menu);
        QVERIFY(!menu->property("modal").toBool());
        const int count = menu->property("count").toInt();
        // Every menu has at least a line about its state and a refresh row.
        QVERIFY2(count >= 2, qPrintable(kind + QStringLiteral(": %1").arg(count)));

        // Arrowing down moves the highlight onto a row that can be activated,
        // and never onto one that cannot.
        QTest::keyClick(window, Qt::Key_Down);
        int highlighted = -1;
        QTRY_VERIFY((highlighted = menu->property("currentIndex").toInt()) >= 0);

        QQuickItem *current = nullptr;
        QMetaObject::invokeMethod(
            menu, "itemAt", Q_RETURN_ARG(QQuickItem *, current), Q_ARG(int, highlighted)
        );
        QVERIFY(current);
        QVERIFY2(current->property("enabled").toBool(), qPrintable(kind));

        // And Return really activates it, rather than this case only claiming
        // it does. `triggered` is what a `MenuItem` emits when it is chosen.
        QSignalSpy triggered(current, SIGNAL(triggered()));
        QVERIFY(triggered.isValid());
        QSignalSpy dismissed(window, SIGNAL(dismissed()));
        QVERIFY(dismissed.isValid());
        QTest::keyClick(window, Qt::Key_Return);
        QTRY_COMPARE(triggered.count(), 1);
        // Choosing a row closes the menu, which is the whole reason the request
        // ledger cannot live in this window.
        QTRY_COMPARE(dismissed.count(), 1);
    }
}

// Every row says what it is and what state it is in, because a menu read aloud
// is the only one some people get. A row that is a mutually-exclusive state
// says so as one rather than leaving its mark to colour alone.
void IndicatorMenuTest::everyRowNamesItselfAndItsState()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &kind : kinds()) {
        std::unique_ptr<QObject> owner;
        QQuickWindow *const window = openMenu(engine, kind, owner);
        QVERIFY2(window, qPrintable(kind));

        QObject *const menu = window->property("menu").value<QObject *>();
        QVERIFY(menu);
        const int count = menu->property("count").toInt();
        QVERIFY(count >= 2);

        for (int index = 0; index < count; ++index) {
            QQuickItem *row = nullptr;
            QMetaObject::invokeMethod(
                menu, "itemAt", Q_RETURN_ARG(QQuickItem *, row), Q_ARG(int, index)
            );
            QVERIFY(row);

            // `Accessible` is an attached property, so it is read through the
            // QML property system rather than the meta-object. An empty name is
            // a row nothing can announce.
            const QVariant name = QQmlProperty::read(row, QStringLiteral("Accessible.name"));
            const QString announced = name.isValid() && !name.toString().isEmpty()
                ? name.toString()
                : row->property("text").toString();
            QVERIFY2(
                !announced.trimmed().isEmpty(),
                qPrintable(kind + QStringLiteral(" row %1").arg(index))
            );
        }
    }
}

namespace {
// The menu, built against a real bridge. Returned shown and exposed.
QQuickWindow *openAgainst(
    QQmlEngine &engine,
    const QString &kind,
    FakeBridge *bridge,
    std::unique_ptr<QObject> &owner
)
{
    QQmlComponent menu(
        &engine,
        QUrl::fromLocalFile(
            QStringLiteral(CELESTINA_QML_DIR "/") + indicatorMenuComponent(kind)
            + QStringLiteral(".qml")
        )
    );
    if (!menu.isReady()) {
        qWarning().noquote() << menu.errorString();
        return nullptr;
    }

    owner.reset(menu.createWithInitialProperties({
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(bridge)},
    }));
    auto *window = qobject_cast<QQuickWindow *>(owner.get());
    if (!window)
        return nullptr;

    window->resize(outputWidth, outputHeight);
    window->setProperty("menuX", 1200);
    window->setProperty("menuY", 40);
    window->show();
    return QTest::qWaitForWindowExposed(window) ? window : nullptr;
}

// The last row of either menu is its refresh, which is always present and
// always enabled — the one row every case can activate.
QQuickItem *lastRow(QObject *menu)
{
    const int count = menu->property("count").toInt();
    QQuickItem *row = nullptr;
    QMetaObject::invokeMethod(
        menu, "itemAt", Q_RETURN_ARG(QQuickItem *, row), Q_ARG(int, count - 1)
    );
    return row;
}
} // namespace

// The defect this unit was reopened for. A menu row is a `MenuItem`: activating
// one closes its `Menu`, the surface is dismissed and the host destroys the
// window. Anything the window owned dies with it — which is why the request
// ledger does not live there.
void IndicatorMenuTest::activatingARowClosesTheMenuAndOutlivesItsWindow()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    FakeBridge bridge;
    bridge.publish({{QStringLiteral("network"),
                     QVariantMap {{QStringLiteral("networksState"), QStringLiteral("fresh")}}}});

    std::unique_ptr<QObject> owner;
    QQuickWindow *window = openAgainst(engine, QStringLiteral("network"), &bridge, owner);
    QVERIFY(window);

    QObject *const menu = window->property("menu").value<QObject *>();
    QVERIFY(menu);
    QQuickItem *const refresh = lastRow(menu);
    QVERIFY(refresh);
    QVERIFY(refresh->property("enabled").toBool());

    QSignalSpy dismissed(window, SIGNAL(dismissed()));
    QVERIFY(dismissed.isValid());

    // A real click on a real row, at the point the row occupies.
    const QPointF centre = refresh->mapToScene(
        QPointF(refresh->width() / 2, refresh->height() / 2)
    );
    QTest::mouseClick(window, Qt::LeftButton, {}, centre.toPoint());

    // The request went, and activating the row really did close the menu.
    QTRY_COMPARE(bridge.sent, QStringList {QStringLiteral("network/refresh")});
    QTRY_VERIFY(!menu->property("visible").toBool());
    QTRY_COMPARE(dismissed.count(), 1);

    // The host answers `dismissed` by destroying the window. Everything the
    // window owned goes with it — and the request must not.
    owner.reset();
    QVERIFY(bridge.requests()->isPending(
        QStringLiteral("network"), QStringLiteral("refresh")
    ));

    // `accepted` keeps a connectivity request waiting; only a later
    // observation ends it. Both arrive with no window in existence at all.
    bridge.requests()->result(1, QStringLiteral("accepted"), QString());
    QVERIFY(bridge.requests()->isPending(
        QStringLiteral("network"), QStringLiteral("refresh")
    ));
    bridge.requests()->result(1, QStringLiteral("confirmed"), QString());
    QVERIFY(!bridge.requests()->isPending(
        QStringLiteral("network"), QStringLiteral("refresh")
    ));
}

// And what happened while it was closed is what it says when it comes back.
void IndicatorMenuTest::aReopenedMenuShowsWhatHappenedWhileItWasClosed()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    FakeBridge bridge;
    bridge.publish({{QStringLiteral("bluetooth"),
                     QVariantMap {{QStringLiteral("adapter"), QStringLiteral("on")},
                                  {QStringLiteral("devicesState"), QStringLiteral("fresh")}}}});

    std::unique_ptr<QObject> owner;
    QQuickWindow *window = openAgainst(engine, QStringLiteral("bluetooth"), &bridge, owner);
    QVERIFY(window);
    QMetaObject::invokeMethod(window, "refresh");
    QVERIFY(bridge.requests()->isPending(
        QStringLiteral("bluetooth"), QStringLiteral("refresh")
    ));

    // The window goes, the answer arrives, and a new window is built.
    owner.reset();
    QTest::ignoreMessage(QtWarningMsg, "Celestina's provider request failed: the tool refused it");
    bridge.requests()->result(1, QStringLiteral("failed"), QStringLiteral("the tool refused it"));

    window = openAgainst(engine, QStringLiteral("bluetooth"), &bridge, owner);
    QVERIFY(window);
    // The failure is still there, said in Spanish, with none of the helper's
    // English in it.
    QVariant note;
    QMetaObject::invokeMethod(
        window, "noteFor", Q_RETURN_ARG(QVariant, note),
        Q_ARG(QVariant, QStringLiteral("refresh"))
    );
    QCOMPARE(note.toString(), QStringLiteral(" — no se pudo"));

    // Acting again replaces the report rather than stacking a second one.
    QMetaObject::invokeMethod(window, "refresh");
    QVERIFY(bridge.requests()->isPending(
        QStringLiteral("bluetooth"), QStringLiteral("refresh")
    ));
    QMetaObject::invokeMethod(
        window, "noteFor", Q_RETURN_ARG(QVariant, note),
        Q_ARG(QVariant, QStringLiteral("refresh"))
    );
    QCOMPARE(note.toString(), QStringLiteral(" — solicitando…"));
}

// A failed target does not need its old inventory row in order to remain
// visible. The row may be exactly what disappeared while the request was in
// flight, so tying the report to it would erase the only account of failure.
void IndicatorMenuTest::aFailureOutlivesTheRowThatWouldHaveShownIt()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    FakeBridge bridge;
    const QVariantMap profile {
        {QStringLiteral("id"), QStringLiteral("9f1c-1")},
        {QStringLiteral("name"), QStringLiteral("Tonys 1")},
        {QStringLiteral("active"), false},
    };
    bridge.publish({
        {QStringLiteral("network"),
         QVariantMap {
             {QStringLiteral("networksState"), QStringLiteral("fresh")},
             {QStringLiteral("networks"), QVariantList {profile}},
         }},
    });

    bridge.requests()->send(
        QStringLiteral("network"), QStringLiteral("activate-saved"),
        {{QStringLiteral("id"), QStringLiteral("9f1c-1")}},
        QStringLiteral("activate-saved:9f1c-1"), RequestLedger::ConfirmedPolicy
    );
    QTest::ignoreMessage(QtWarningMsg, "Celestina's provider request failed: it disappeared");
    bridge.requests()->result(1, QStringLiteral("failed"), QStringLiteral("it disappeared"));

    // The provider no longer publishes the profile that would normally carry
    // the report.
    bridge.publish({
        {QStringLiteral("network"),
         QVariantMap {
             {QStringLiteral("networksState"), QStringLiteral("fresh")},
             {QStringLiteral("networks"), QVariantList {}},
         }},
    });

    std::unique_ptr<QObject> owner;
    QQuickWindow *const window = openAgainst(
        engine, QStringLiteral("network"), &bridge, owner
    );
    QVERIFY(window);
    QObject *const menu = window->property("menu").value<QObject *>();
    QVERIFY(menu);

    QQuickItem *failure = nullptr;
    const int count = menu->property("count").toInt();
    for (int index = 0; index < count; ++index) {
        QQuickItem *row = nullptr;
        QMetaObject::invokeMethod(
            menu, "itemAt", Q_RETURN_ARG(QQuickItem *, row), Q_ARG(int, index)
        );
        if (row && row->property("text").toString().contains(QStringLiteral("descartar"))) {
            failure = row;
            break;
        }
    }
    QVERIFY(failure);
    QCOMPARE(bridge.requests()->failures(QStringLiteral("network")).size(), 1);

    // The report is an action: acknowledging it removes precisely that stale
    // target from the durable ledger.
    // `triggered` is the menu item's activation contract. Physical click and
    // keyboard activation of that contract are covered above; this assertion
    // isolates which durable target this new row dismisses.
    QVERIFY(QMetaObject::invokeMethod(failure, "triggered"));
    QTRY_COMPARE(bridge.requests()->failures(QStringLiteral("network")).size(), 0);
}

// One ledger, two providers, and neither answers for the other.
void IndicatorMenuTest::aNetworkResultLeavesBluetoothAlone()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    FakeBridge bridge;
    bridge.publish({
        {QStringLiteral("network"),
         QVariantMap {{QStringLiteral("networksState"), QStringLiteral("fresh")}}},
        {QStringLiteral("bluetooth"),
         QVariantMap {{QStringLiteral("adapter"), QStringLiteral("on")},
                      {QStringLiteral("devicesState"), QStringLiteral("fresh")}}},
    });

    std::unique_ptr<QObject> networkOwner;
    QQuickWindow *const networkMenu =
        openAgainst(engine, QStringLiteral("network"), &bridge, networkOwner);
    QVERIFY(networkMenu);
    QMetaObject::invokeMethod(networkMenu, "refresh");
    networkOwner.reset();

    std::unique_ptr<QObject> bluetoothOwner;
    QQuickWindow *const bluetoothMenu =
        openAgainst(engine, QStringLiteral("bluetooth"), &bridge, bluetoothOwner);
    QVERIFY(bluetoothMenu);
    QMetaObject::invokeMethod(bluetoothMenu, "refresh");

    // Both are waiting under the same target name and different providers.
    QVERIFY(bridge.requests()->isPending(QStringLiteral("network"), QStringLiteral("refresh")));
    QVERIFY(bridge.requests()->isPending(QStringLiteral("bluetooth"), QStringLiteral("refresh")));

    bridge.requests()->result(1, QStringLiteral("confirmed"), QString());
    QVERIFY(!bridge.requests()->isPending(QStringLiteral("network"), QStringLiteral("refresh")));
    QVERIFY(bridge.requests()->isPending(QStringLiteral("bluetooth"), QStringLiteral("refresh")));

    // And a lost generation takes what is left, whichever provider it belongs to.
    bridge.requests()->generationLost();
    QVERIFY(!bridge.requests()->isPending(QStringLiteral("bluetooth"), QStringLiteral("refresh")));
}

// The regression the second review found. The control centre's verbs are
// answered by `accepted` and nothing ever sends them a `confirmed`, so the
// connectivity contract applied to them would have left every control saying
// "preguntando…" for the rest of the session.
void IndicatorMenuTest::theControlCentreStopsAskingWhenTheHelperAccepts()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    FakeBridge bridge;
    bridge.publish({{QStringLiteral("audio"),
                     QVariantMap {{QStringLiteral("volume"), 40},
                                  {QStringLiteral("muted"), false}}}});

    QQmlComponent centre(
        &engine,
        QUrl::fromLocalFile(QStringLiteral(CELESTINA_QML_DIR "/ControlCentre.qml"))
    );
    QVERIFY2(centre.isReady(), qPrintable(centre.errorString()));

    std::unique_ptr<QObject> root(centre.createWithInitialProperties({
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(&bridge)},
    }));
    QVERIFY(root);

    QMetaObject::invokeMethod(
        root.get(), "send",
        Q_ARG(QVariant, QStringLiteral("audio")),
        Q_ARG(QVariant, QStringLiteral("mute-toggle")),
        Q_ARG(QVariant, QVariantMap())
    );
    QCOMPARE(bridge.sent, QStringList {QStringLiteral("audio/mute-toggle")});

    QVariant pending;
    const auto asking = [&] {
        QMetaObject::invokeMethod(
            root.get(), "isPending", Q_RETURN_ARG(QVariant, pending),
            Q_ARG(QVariant, QStringLiteral("audio")),
            Q_ARG(QVariant, QStringLiteral("mute-toggle"))
        );
        return pending.toBool();
    };
    QVERIFY(asking());

    // `accepted` is the whole answer here, and the control stops asking.
    bridge.requests()->result(1, QStringLiteral("accepted"), QString());
    QVERIFY(!asking());

    // A connectivity request made through the same ledger is not finished by
    // its acceptance, which is what makes the two contracts distinguishable
    // rather than a matter of taste.
    bridge.requests()->send(
        QStringLiteral("network"), QStringLiteral("refresh"), {},
        QStringLiteral("refresh"), RequestLedger::ConfirmedPolicy
    );
    bridge.requests()->result(2, QStringLiteral("accepted"), QString());
    QVERIFY(bridge.requests()->isPending(
        QStringLiteral("network"), QStringLiteral("refresh")
    ));
}

QTEST_MAIN(IndicatorMenuTest)
#include "indicatormenu_test.moc"
