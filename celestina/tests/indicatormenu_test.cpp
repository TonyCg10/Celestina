// language-contract: product-copy
#include <QtTest>

#include <QQmlComponent>
#include <QQmlEngine>
#include <QQmlProperty>
#include <QColor>
#include <QQuickItem>
#include <QQuickWindow>
#include <QSignalSpy>
#include <QStringList>
#include <QUrl>
#include <QVariantMap>

#include <limits>
#include <memory>

#include "panelattachmentlease.h"
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
// The glass publishes only once a field's reveal has begun — armed earlier it
// is a bare milky slab leading the paint, which the author recorded on every
// open. Offscreen, no frame is ever presented, so the cue that starts the
// reveal in a session never arrives; the tests start it by hand, exactly as
// the first presented frame does live.
static void revealAllFields(QQuickWindow *window)
{
    // From the window, not its contentItem: the field is reachable as a
    // QObject descendant of the window while the contentItem's QObject
    // subtree does not contain it, which a first version discovered by
    // finding zero fields and revealing nothing.
    const auto fields = window->findChildren<QQuickItem *>(
        QStringLiteral("celestina-soft-menu-field"));
    for (QQuickItem *const field : fields)
        QMetaObject::invokeMethod(field, "reveal");
}

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

    quint64 sendRequest(
        const QString &provider,
        const QString &verb,
        const QVariantMap &options
    ) override
    {
        sent.append(provider + u'/' + verb);
        sentOptions.append(options);
        return ++m_lastId;
    }

    QStringList sent;
    QList<QVariantMap> sentOptions;

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

// What the panel-owned contextual menus are handed, and what a click or a key
// does to them.
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
    void aPanelAnchorIsTranslatedIntoItsCarrier();
    void eachMenuDeclaresExactlyWhatTheHostHandsIt();
    void aPropertyTheComponentDoesNotDeclareIsVisibleAsAFailure();
    void aClickWhereTheIndicatorIsDismissesTheMenu();
    void aClickOnTheCardDismissesNothing();
    void escapeDismissesTheMenu();
    void popupDismissalStartsBeforeItsRowsLeave();
    void aCardAskedForBeyondTheOutputStaysWhole();
    void aSoftMenuKeepsOneOuterGlassCard();
    void eachPanelIndicatorMenuSpansItsOuterVeilAndTargetsTheIcon();
    void theTrayMenuUsesTheSameVeloCarrier();
    void theWholeMenuIsReachableFromTheKeyboard();
    void aScaledOutputScalesTheRowsWithTheGlassTheySitOn();
    void aScaledCardKeepsItsMembraneOnTheGlyph();
    void everyRowNamesItselfAndItsState();
    void wallpaperPagesStayOnOneBoundedCatalogue();
    void activatingARowClosesTheMenuAndOutlivesItsWindow();
    void aReopenedMenuShowsWhatHappenedWhileItWasClosed();
    void aFailureOutlivesTheRowThatWouldHaveShownIt();
    void aBluetoothFailureCanBeDismissed();
    void aNetworkResultLeavesBluetoothAlone();
    void theControlCentreStopsAskingWhenTheHelperAccepts();
    void aParkThatRacesTheQueuedOpenLeavesNoStrandedPopup();

private:
    // The kinds `PanelManager` can forward, which is the set the list must
    // cover. A kind added without a component fails this case rather than a
    // session.
    static QStringList kinds()
    {
        return {
            QStringLiteral("network"),
            QStringLiteral("bluetooth"),
            QStringLiteral("performance"),
            QStringLiteral("brightness"),
            QStringLiteral("calendar"),
            QStringLiteral("phone"),
            QStringLiteral("audio"),
            QStringLiteral("capture"),
            QStringLiteral("wallpaper"),
        };
    }

    // Linear contextual actions keep Qt Quick Menu as their lifecycle owner.
    // Wallpaper is deliberately a custom card because its bounded thumbnail
    // grid is spatial content, not a list of interchangeable menu rows.
    static QStringList softMenuKinds()
    {
        return {
            QStringLiteral("network"),
            QStringLiteral("bluetooth"),
            QStringLiteral("performance"),
            QStringLiteral("phone"),
            QStringLiteral("capture"),
        };
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
        std::unique_ptr<QObject> &owner,
        double shellScale = 1.0,
        bool reducedMotion = true
    )
    {
        QQmlComponent menu(&engine, sourceFor(indicatorMenuComponent(kind)));
        if (!menu.isReady()) {
            qWarning().noquote() << menu.errorString();
            return nullptr;
        }

        QVariantMap initialProperties {
            {QStringLiteral("reducedMotion"), reducedMotion},
            {QStringLiteral("outputName"), QStringLiteral("test-output")},
            {QStringLiteral("shellScale"), shellScale},
        };
        if (kind != QStringLiteral("calendar")) {
            initialProperties.insert(
                QStringLiteral("providerSource"),
                QVariant::fromValue<QObject *>(nullptr)
            );
        }
        owner.reset(menu.createWithInitialProperties(initialProperties));
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

void IndicatorMenuTest::aPanelAnchorIsTranslatedIntoItsCarrier()
{
    const QRectF globalAnchor(100, 70, 30, 20);
    const QPointF outputOrigin(20, 10);

    QCOMPARE(
        panelAttachmentRectOnCarrier(
            globalAnchor,
            outputOrigin,
            QPointF(0, 40),
            2.0),
        QRectF(40, 10, 15, 10)
    );
    // Floating and side-attached surfaces retain the established full-output
    // carrier by publishing the default zero origin.
    QCOMPARE(
        panelAttachmentRectOnCarrier(
            globalAnchor,
            outputOrigin,
            QPointF(),
            2.0),
        QRectF(40, 30, 15, 10)
    );
}

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
    QCOMPARE(
        indicatorMenuComponent(QStringLiteral("performance")),
        QStringLiteral("PerformanceMenu")
    );
    QCOMPARE(
        indicatorMenuComponent(QStringLiteral("brightness")),
        QStringLiteral("BrightnessMenu")
    );
    QCOMPARE(
        indicatorMenuComponent(QStringLiteral("capture")),
        QStringLiteral("CaptureMenu")
    );
    QCOMPARE(
        indicatorMenuComponent(QStringLiteral("wallpaper")),
        QStringLiteral("WallpaperMenu")
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
        QVariantMap initialProperties {
            {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(nullptr)},
            {QStringLiteral("reducedMotion"), true},
            {QStringLiteral("outputName"), QStringLiteral("test-output")},
            {QStringLiteral("anchoredFromPanel"), true},
            {QStringLiteral("openerRect"), QRect(900, 6, 28, 28)},
            {QStringLiteral("attachmentAnchorRect"), QRect(905, 11, 18, 18)},
            {QStringLiteral("attachmentStartY"), 40},
        };
        if (kind == QStringLiteral("calendar"))
            initialProperties.remove(QStringLiteral("providerSource"));
        QObject *const root = menu.createWithInitialProperties(initialProperties);
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
        {QStringLiteral("outputName"), QStringLiteral("test-output")},
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
        QVERIFY(!window->findChild<QObject *>(
            QStringLiteral("celestina-menu-exterior-shadow")
        ));

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
        const int cardWidth = window->property("cardWidth").toInt();
        QVERIFY(cardWidth > 0);
        // Just inside the card's own edge, in its padding rather than on a row,
        // so what stops the click is the card itself.
        QTest::mouseClick(
            window,
            Qt::LeftButton,
            {},
            QPoint(cardX + cardWidth - 2, cardY + 2)
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

void IndicatorMenuTest::popupDismissalStartsBeforeItsRowsLeave()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    std::unique_ptr<QObject> owner;
    QQuickWindow *const window = openMenu(
        engine, QStringLiteral("phone"), owner, 1.0, false);
    QVERIFY(window);
    QObject *const popup = window->property("menu").value<QObject *>();
    QVERIFY(popup);
    QQuickItem *const field = window->findChild<QQuickItem *>(
        QStringLiteral("celestina-soft-menu-field"));
    QVERIFY(field);
    revealAllFields(window);
    QTRY_VERIFY(field->property("revealed").toBool());

    QSignalSpy dismissed(window, SIGNAL(dismissed()));
    QVERIFY(dismissed.isValid());
    QTest::keyClick(window, Qt::Key_Escape);

    // The host is notified from aboutToHide, while Qt is still holding the
    // popup for the shared departure. The old contract emitted only after the
    // rows had completed their private exit and would fail this same-turn
    // assertion.
    QCOMPARE(dismissed.count(), 1);

    // Mirror what the host does on that signal. Popup rows and field now read
    // the same properties; a second observation of the close is idempotent.
    QVERIFY(QMetaObject::invokeMethod(field, "retire"));
    QVERIFY(field->property("retiring").toBool());
    QTest::qWait(25);
    const qreal fieldOpacity = field->property("presentationOpacity").toReal();
    const qreal popupOpacity = popup->property("opacity").toReal();
    QVERIFY(qAbs(fieldOpacity - popupOpacity) < 0.001);
    const qreal fieldScale = field->property("retireScale").toReal();
    const qreal popupScale = popup->property("scale").toReal();
    QVERIFY(qAbs(fieldScale - popupScale) < 0.001);
    QVERIFY(QMetaObject::invokeMethod(field, "retire"));
    QVERIFY(field->property("retiring").toBool());
    QTRY_VERIFY(!popup->property("opened").toBool());
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
        QVERIFY(cardWidth > 0);
        QVERIFY(cardHeight > 0);
        QCOMPARE(cardWidth, window->property("contentWidth").toInt());
        QCOMPARE(cardHeight, window->property("contentHeight").toInt());

        // Asked for past the right edge, and past the bottom.
        window->setProperty("menuX", outputWidth + 500);
        window->setProperty("menuY", outputHeight + 500);
        QCOMPARE(
            window->property("cardX").toInt(),
            outputWidth - cardWidth
        );
        QCOMPARE(
            window->property("cardY").toInt(),
            outputHeight - cardHeight
        );

        // At the other edges the complete visible menu starts at zero.
        window->setProperty("menuX", -400);
        window->setProperty("menuY", -400);
        QCOMPARE(window->property("cardX").toInt(), 0);
        QCOMPARE(window->property("cardY").toInt(), 0);
    }
}

void IndicatorMenuTest::aSoftMenuKeepsOneOuterGlassCard()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &kind : softMenuKinds()) {
        std::unique_ptr<QObject> owner;
        QQuickWindow *const window = openMenu(engine, kind, owner);
        QVERIFY2(window, qPrintable(kind));

        const int anchorGap = window->property("anchorGap").toInt();
        const QRect opener(1540, 12, 28, 28);
        QVERIFY(anchorGap > 0);
        // Distance is the semantic floating gap between the opener and body.
        window->setProperty(
            "menuY", opener.y() + opener.height() + anchorGap
        );

        window->setProperty("compositorBlurAvailable", true);

        QObject *const menu = window->property("menu").value<QObject *>();
        QVERIFY(menu);
        QCOMPARE(menu->property("x").toInt(), window->property("cardX").toInt());
        QCOMPARE(
            menu->property("y").toInt(),
            window->property("cardY").toInt()
        );
        // The whole menu contributes one compositor-glass region. Its rows are
        // denser visual divisions, not independent compositor samples.
        const QRectF menuBounds(
            menu->property("x").toReal(),
            menu->property("y").toReal(),
            menu->property("width").toReal(),
            menu->property("height").toReal()
        );
        // The field may already have published its creation geometry before
        // this case moves `menuY`. Wait for the deferred geometry collection,
        // not merely for a list that was already non-empty at the old origin.
        QTRY_COMPARE(window->property("glassRects").toList().size(), 1);
        QTRY_COMPARE(
            window->property("glassRects").toList().constFirst().toRectF(),
            menuBounds
        );
        const QRectF published = window->property("glassRects").toList()
                                     .constFirst().toRectF();
        QCOMPARE(published, menuBounds);
        revealAllFields(window);
        QTRY_COMPARE(window->property("glassRegions").toList().size(), 1);
        const QVariantMap publishedShape = window->property("glassRegions")
                                               .toList().constFirst().toMap();
        QCOMPARE(publishedShape.value(QStringLiteral("rect")).toRectF(), menuBounds);
        QCOMPARE(publishedShape.value(QStringLiteral("radius")).toInt(), 20);
        QVERIFY(
            publishedShape.value(QStringLiteral("polygon")).toList().isEmpty()
        );

        const QList<QObject *> groupedFields = window->findChildren<QObject *>(
            QStringLiteral("celestina-menu-section")
        );
        QCOMPARE(groupedFields.size(), 2);
        const int sectionRole = groupedFields.constFirst()
                                    ->property("materialRole").toInt();
        const qreal sectionStrength = groupedFields.constFirst()
                                          ->property("materialStrength").toReal();
        const QColor sectionTint = groupedFields.constFirst()
                                       ->property("materialTint").value<QColor>();
        for (QObject *const section : groupedFields) {
            QVERIFY(section->metaObject()->indexOfProperty("backdropSource") >= 0);
            QVERIFY(section->metaObject()->indexOfProperty("captureActive") >= 0);
            QVERIFY(section->metaObject()->indexOfProperty("density") >= 0);
            QVERIFY(section->metaObject()->indexOfProperty("cornerRadius") >= 0);
            QCOMPARE(section->property("backdropMode").toInt(), 1);
            QCOMPARE(section->property("externalBackdropReady").toBool(), true);
            QCOMPARE(section->property("captureEnabled").toBool(), false);
            QCOMPARE(section->property("captureActive").toBool(), false);
            QCOMPARE(section->property("elevation").toInt(), 0);
            QCOMPARE(section->property("materialRole").toInt(), sectionRole);
            QCOMPARE(
                section->property("materialStrength").toReal(), sectionStrength
            );
            QCOMPARE(
                section->property("materialTint").value<QColor>(), sectionTint
            );
            QCOMPARE(
                section->findChildren<QObject *>(
                    QStringLiteral("celestina-compositor-glass-region")
                ).size(),
                0
            );
        }
        QVERIFY(window->findChild<QObject *>(
            QStringLiteral("celestina-menu-header")
        ));
        QVERIFY(!window->findChild<QObject *>(
            QStringLiteral("celestina-menu-exterior-shadow")
        ));

        // The compositor owns the one blur sample. The shared GlassSurface
        // renderer adds the denser content material without starting a QML
        // capture or publishing another compositor region. Offscreen still
        // cannot claim how Niri renders the final sample.
        QObject *const bodyTint = window->findChild<QObject *>(
            QStringLiteral("celestina-menu-body-tint")
        );
        QVERIFY(bodyTint);
        QVERIFY(bodyTint->property("visible").toBool());
        QCOMPARE(bodyTint->property("backdropMode").toInt(), 1);
        QCOMPARE(bodyTint->property("externalBackdropReady").toBool(), true);
        QCOMPARE(bodyTint->property("captureEnabled").toBool(), false);
        QCOMPARE(bodyTint->property("captureActive").toBool(), false);
        QCOMPARE(bodyTint->property("elevation").toInt(), 0);
        QVERIFY(bodyTint->property("materialRole").toInt() != sectionRole);
        QVERIFY(
            bodyTint->property("materialStrength").toReal() < sectionStrength
        );
        const QColor bodyColor = bodyTint->property("materialTint").value<QColor>();
        const QColor sectionColor = sectionTint;
        QVERIFY(bodyColor.isValid());
        QVERIFY(sectionColor.isValid());
        QVERIFY(bodyColor.lightnessF() > 0.9);
        QVERIFY(sectionColor.lightnessF() < 0.1);
        const qreal bodyDensity = bodyColor.alphaF()
                                  * bodyTint->property("materialOpacity").toReal()
                                  * bodyTint->property("materialStrength").toReal();
        const qreal sectionDensity = sectionColor.alphaF()
                                     * groupedFields.constFirst()
                                           ->property("materialOpacity").toReal()
                                     * sectionStrength;
        const qreal compositeDensity = bodyDensity + sectionDensity
                                       - bodyDensity * sectionDensity;
        QVERIFY(bodyDensity < 0.04);
        QVERIFY(sectionDensity > 0.55);
        QVERIFY(sectionDensity < 0.70);
        QVERIFY(compositeDensity < 0.72);
        QVERIFY(bodyDensity < sectionDensity);

        QObject *const hiddenMenuBackground = menu->property("background")
                                                   .value<QObject *>();
        QVERIFY(hiddenMenuBackground);
        QCOMPARE(hiddenMenuBackground->property("visible").toBool(), false);
        QCOMPARE(hiddenMenuBackground->property("elevation").toInt(), 0);

        QQuickItem *firstRow = nullptr;
        QMetaObject::invokeMethod(
            menu, "itemAt", Q_RETURN_ARG(QQuickItem *, firstRow), Q_ARG(int, 0)
        );
        QVERIFY(firstRow);
        QCOMPARE(
            firstRow->findChildren<QObject *>(
                QStringLiteral("celestina-menu-section")
            ).size(),
            1
        );
    }
}

void IndicatorMenuTest::eachPanelIndicatorMenuSpansItsOuterVeilAndTargetsTheIcon()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    constexpr int attachmentStartY = 40;
    const QRect opener(1540, 6, 28, 28);
    const QRect attachmentAnchor(1545, 11, 18, 18);
    for (const QString &kind : kinds()) {
        const QString component = indicatorMenuComponent(kind);
        QQmlComponent menu(&engine, sourceFor(component));
        QVERIFY2(menu.isReady(), qPrintable(menu.errorString()));

        QVariantMap properties {
            {QStringLiteral("reducedMotion"), true},
            {QStringLiteral("outputName"), QStringLiteral("test-output")},
            {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(nullptr)},
            {QStringLiteral("anchoredFromPanel"), true},
            {QStringLiteral("openerRect"), opener},
            {QStringLiteral("attachmentAnchorRect"), attachmentAnchor},
            {QStringLiteral("attachmentStartY"), attachmentStartY},
        };

        std::unique_ptr<QObject> owner(menu.createWithInitialProperties(properties));
        auto *const window = qobject_cast<QQuickWindow *>(owner.get());
        QVERIFY2(window, qPrintable(component));
        window->resize(outputWidth, outputHeight);
        window->show();
        QVERIFY2(QTest::qWaitForWindowExposed(window), qPrintable(component));

        QCOMPARE(window->property("openerRect").toRect(), opener);
        QCOMPARE(
            window->property("attachmentAnchorRect").toRect(),
            attachmentAnchor
        );
        QCOMPARE(
            window->property("cardY").toInt(),
            attachmentStartY + window->property("anchorGap").toInt()
        );

        QObject *const field = window->findChild<QObject *>(
            QStringLiteral("celestina-soft-menu-field")
        );
        QVERIFY2(field, qPrintable(component));
        QVERIFY(field->property("edgeAttachmentRequested").toBool());
        QVERIFY(field->property("edgeShapeActive").toBool());
        QCOMPARE(
            field->property("attachmentAnchorRect").toRect(),
            attachmentAnchor
        );

        revealAllFields(window);

        QTRY_COMPARE(window->property("glassRegions").toList().size(), 1);
        const QVariantMap shape = window->property("glassRegions")
                                      .toList().constFirst().toMap();
        QCOMPARE(
            qRound(shape.value(QStringLiteral("rect")).toRectF().top()),
            attachmentStartY
        );
        const QVariantList polygon =
            shape.value(QStringLiteral("polygon")).toList();
        QVERIFY2(polygon.size() >= 3, qPrintable(component));

        qreal top = std::numeric_limits<qreal>::max();
        for (const QVariant &value : polygon)
            top = qMin(top, value.toPointF().y());
        qreal upperLeft = std::numeric_limits<qreal>::max();
        qreal upperRight = std::numeric_limits<qreal>::lowest();
        for (const QVariant &value : polygon) {
            const QPointF point = value.toPointF();
            if (qAbs(point.y() - top) < 0.001) {
                upperLeft = qMin(upperLeft, point.x());
                upperRight = qMax(upperRight, point.x());
            }
        }
        QCOMPARE(qRound(top), attachmentStartY);
        // Only the narrow droplet mouth touches the seam row; it stays
        // centred on the clicked glyph instead of spanning the body.
        QVERIFY(upperRight - upperLeft
                < window->property("cardWidth").toInt() * 0.25);
        QCOMPARE(qRound((upperLeft + upperRight) / 2),
                 attachmentAnchor.x() + attachmentAnchor.width() / 2);
        QCOMPARE(
            qRound(window->property("cardX").toReal()
                   + field->property("attachmentWaistCenterAtBody").toReal()),
            attachmentAnchor.x() + attachmentAnchor.width() / 2
        );

        QObject *const veil = window->findChild<QObject *>(
            QStringLiteral("celestina-menu-body-tint")
        );
        QVERIFY2(veil, qPrintable(component));
        QCOMPARE(veil->property("materialRole").toInt(), 2);
        QCOMPARE(veil->property("elevation").toInt(), 0);
        QVERIFY(veil->property("usesSilhouette").toBool());
        QVERIFY(!veil->property("materialEdgesVisible").toBool());
        QVERIFY2(
            !window->findChild<QObject *>(
                QStringLiteral("celestina-attachment-material-bridge")
            ),
            qPrintable(component)
        );

        const QList<QObject *> sections = window->findChildren<QObject *>(
            QStringLiteral("celestina-menu-section")
        );
        QVERIFY2(!sections.isEmpty(), qPrintable(component));
        for (QObject *const section : sections) {
            QVERIFY2(!section->property("usesSilhouette").toBool(),
                     qPrintable(component));
            QCOMPARE(section->property("elevation").toInt(), 0);
        }
    }
}

void IndicatorMenuTest::theTrayMenuUsesTheSameVeloCarrier()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    QQmlComponent component(&engine, sourceFor(QStringLiteral("TrayMenu")));
    QVERIFY2(component.isReady(), qPrintable(component.errorString()));
    const QVariantMap entry {
        {QStringLiteral("id"), 7},
        {QStringLiteral("label"), QStringLiteral("Open")},
        {QStringLiteral("enabled"), true},
        {QStringLiteral("separator"), false},
        {QStringLiteral("depth"), 0},
        {QStringLiteral("toggleType"), QString()},
        {QStringLiteral("toggleState"), 0},
    };
    std::unique_ptr<QObject> owner(component.createWithInitialProperties({
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("outputName"), QStringLiteral("test-output")},
        {QStringLiteral("appName"), QStringLiteral("Solaar")},
        {QStringLiteral("entries"), QVariantList {entry}},
    }));
    auto *window = qobject_cast<QQuickWindow *>(owner.get());
    QVERIFY2(window, qPrintable(component.errorString()));
    window->resize(outputWidth, outputHeight);
    window->setProperty("menuX", 1600);
    window->setProperty("menuY", 40);
    window->show();
    QVERIFY(QTest::qWaitForWindowExposed(window));

    QCOMPARE(
        window->findChildren<QObject *>(
            QStringLiteral("celestina-compositor-glass-region")
        ).size(),
        1
    );
    QVERIFY(window->findChild<QObject *>(
        QStringLiteral("celestina-menu-header")
    ));
    QCOMPARE(window->property("itemSpacing").toInt(), 8);
    QCOMPARE(window->property("title").toString(),
             QStringLiteral("Menú de Solaar"));
    QCOMPARE(window->property("headerBodyGap").toInt(), 12);
    QCOMPARE(window->property("rowVerticalInset").toInt(), 4);
    QCOMPARE(
        window->findChildren<QObject *>(
            QStringLiteral("celestina-menu-section")
        ).size(),
        2
    );
    revealAllFields(window);
    QTRY_COMPARE(window->property("glassRegions").toList().size(), 1);
    QObject *const bodyMaterial = window->findChild<QObject *>(
        QStringLiteral("celestina-menu-body-tint")
    );
    QVERIFY(bodyMaterial);
    const QList<QObject *> sections = window->findChildren<QObject *>(
        QStringLiteral("celestina-menu-section")
    );
    QVERIFY(!sections.isEmpty());
    const qreal sectionStrength = sections.constFirst()
                                      ->property("materialStrength").toReal();
    QVERIFY(
        bodyMaterial->property("materialStrength").toReal() < sectionStrength
    );
    for (QObject *const section : sections) {
        QCOMPARE(section->property("captureActive").toBool(), false);
        QCOMPARE(section->property("elevation").toInt(), 0);
        QCOMPARE(
            section->property("materialStrength").toReal(), sectionStrength
        );
    }
}

// A menu is a keyboard surface first. `GlassContextMenu` is a real `Menu`, so
// arrows move its highlight and Return activates — this checks the shell has
// not broken that, and that the highlight is somewhere to begin with.
void IndicatorMenuTest::theWholeMenuIsReachableFromTheKeyboard()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &kind : softMenuKinds()) {
        std::unique_ptr<QObject> owner;
        QQuickWindow *const window = openMenu(engine, kind, owner);
        QVERIFY2(window, qPrintable(kind));

        QObject *const menu = window->property("menu").value<QObject *>();
        QVERIFY(menu);
        QVERIFY(!menu->property("modal").toBool());
        const int count = menu->property("count").toInt();
        // Every menu has semantic context and at least one enabled action.
        QVERIFY2(count >= 2, qPrintable(kind + QStringLiteral(": %1").arg(count)));

        // Except the menus whose every action is a piece of provider data.
        // Performance opens the system monitor by clicking a reading, so with
        // nothing being measured there is deliberately nothing to act on.
        // Brightness has exactly that shape — each action is one detected
        // monitor's own stepper — and so does the phone, whose actions are the
        // daemon's own device rows. With nothing published each offers a
        // sentence and no action; each still names itself and its absent
        // reading, covered above and by its own cases. They are named here
        // rather than weakening the rule for every menu.
        if (kind == QStringLiteral("performance")
                || kind == QStringLiteral("phone")) {
            if (kind == QStringLiteral("performance"))
                QVERIFY(!window->property("hasReading").toBool());
            else
                QVERIFY(window->property("devices").toList().isEmpty());
            for (int index = 0; index < count; ++index) {
                QQuickItem *row = nullptr;
                QMetaObject::invokeMethod(
                    menu, "itemAt", Q_RETURN_ARG(QQuickItem *, row),
                    Q_ARG(int, index)
                );
                QVERIFY(row);
                QVERIFY2(!row->property("enabled").toBool(), qPrintable(kind));
            }
            continue;
        }

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

// A larger output draws the rows as large as the glass they sit on.
//
// The rows of these menus live in a Popup, and a Popup is drawn in the window's
// own overlay rather than as part of the item tree the card scales. So the scene
// transform never reaches them: the glass grew with the output and the rows
// stayed at their unscaled size, inside a window sized for the scaled card. The
// author photographed the result on a 1.15 output — a body card about 1.15
// times the width of its own rows, and offset, because the popup was also being
// placed in unscaled coordinates inside a scaled window.
//
// Every other case here pins the factor to 1, which is why this defect survived
// the suite that was meant to cover per-output sizing.
void IndicatorMenuTest::aScaledOutputScalesTheRowsWithTheGlassTheySitOn()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    // A real factor from the author's own monitors, not a round number that
    // could hide an arithmetic slip by being one.
    constexpr double factor = 1.15;
    for (const QString &kind : softMenuKinds()) {
        std::unique_ptr<QObject> owner;
        QQuickWindow *const window = openMenu(engine, kind, owner, factor);
        QVERIFY2(window, qPrintable(kind));

        QObject *const menu = window->property("menu").value<QObject *>();
        QVERIFY(menu);

        // The rows' own layer carries the factor, from the same corner the
        // scene scales from. Not the popup: `GlassContextMenu` animates `scale`
        // in its enter and exit transitions, and a transition writes the
        // property directly, so a binding placed there is destroyed the moment
        // the menu opens. This case is what caught that.
        auto *const rows =
            menu->property("contentItem").value<QQuickItem *>();
        QVERIFY2(rows, qPrintable(kind));
        QCOMPARE(rows->property("scale").toDouble(), factor);
        QCOMPARE(
            rows->property("transformOrigin").toInt(),
            static_cast<int>(QQuickItem::TopLeft)
        );

        // What actually decides whether the menu looks whole: the rows sit on
        // their glass, measured in real output pixels through every transform
        // either side carries. This is deliberately not an assertion about
        // which object holds which coordinate — two wrong theories about that
        // each satisfied their own property checks while the live session
        // showed rows floating away from their card. Mapping both trees to
        // scene coordinates is the one comparison a wrong theory cannot pass.
        auto *const glass = window->findChild<QQuickItem *>(
            QStringLiteral("celestina-soft-menu-field"));
        QVERIFY2(glass, qPrintable(kind));
        const QPointF glassAt = glass->mapToScene(QPointF(0, 0));
        const QPointF rowsAt = rows->mapToScene(QPointF(0, 0));
        // The popup carries its own padding between its box and the rows, so
        // exact equality is not the contract; a scale error displaces rows by
        // cardX * 0.15 — an order of magnitude past this bound.
        const double slack = 24.0 * factor;
        QVERIFY2(
            std::abs(rowsAt.x() - glassAt.x()) <= slack,
            qPrintable(QStringLiteral("%1: rows at %2, glass at %3")
                           .arg(kind).arg(rowsAt.x()).arg(glassAt.x()))
        );
        QVERIFY2(
            std::abs(rowsAt.y() - glassAt.y()) <= slack,
            qPrintable(QStringLiteral("%1: rows at %2, glass at %3")
                           .arg(kind).arg(rowsAt.y()).arg(glassAt.y()))
        );

        // And the drawn widths agree, through the same transforms: a header
        // band narrower than the rows it heads is the same defect measured on
        // the other axis, and the author photographed exactly that on the
        // tray menus.
        const QPointF glassFar =
            glass->mapToScene(QPointF(glass->width(), 0));
        const QPointF rowsFar = rows->mapToScene(QPointF(rows->width(), 0));
        const double glassSpan = glassFar.x() - glassAt.x();
        const double rowsSpan = rowsFar.x() - rowsAt.x();
        QVERIFY2(
            std::abs(glassSpan - rowsSpan) <= slack,
            qPrintable(QStringLiteral("%1: rows span %2, glass span %3")
                           .arg(kind).arg(rowsSpan).arg(glassSpan))
        );

        // The card itself is stated in unscaled units and stays that way: the
        // factor belongs on the way to the output, not in the layout numbers.
        const int cardWidth = window->property("cardWidth").toInt();
        QCOMPARE(cardWidth, window->property("contentWidth").toInt());
    }
}

// A card menu on a scaled output still hangs from the glyph that opened it.
//
// The membrane cases above pin the factor to 1 and the scaled case above
// passes no opener, so the combination — a card, a real opener, a real
// factor — was covered by neither, and it is exactly where the author saw the
// drop landing beside its icon and the clock and phone cards opening with no
// connection at all.
void IndicatorMenuTest::aScaledCardKeepsItsMembraneOnTheGlyph()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    constexpr double factor = 1.15;
    // In shell units, exactly as the controller hands them after dividing.
    constexpr int attachmentStartY = 40;
    const QRect opener(900, 6, 28, 28);
    const QRect attachmentAnchor(905, 11, 18, 18);

    for (const QString &kind :
         {QStringLiteral("calendar"), QStringLiteral("audio"),
          QStringLiteral("brightness"), QStringLiteral("wallpaper")}) {
        QQmlComponent menu(&engine, sourceFor(indicatorMenuComponent(kind)));
        QVERIFY2(menu.isReady(), qPrintable(menu.errorString()));

        QVariantMap properties {
            {QStringLiteral("reducedMotion"), true},
            {QStringLiteral("outputName"), QStringLiteral("test-output")},
            {QStringLiteral("shellScale"), factor},
            {QStringLiteral("anchoredFromPanel"), true},
            {QStringLiteral("openerRect"), opener},
            {QStringLiteral("attachmentAnchorRect"), attachmentAnchor},
            {QStringLiteral("attachmentStartY"), attachmentStartY},
        };
        if (kind != QStringLiteral("calendar")) {
            properties.insert(
                QStringLiteral("providerSource"),
                QVariant::fromValue<QObject *>(nullptr));
        }

        std::unique_ptr<QObject> owner(menu.createWithInitialProperties(properties));
        auto *const window = qobject_cast<QQuickWindow *>(owner.get());
        QVERIFY2(window, qPrintable(kind));
        window->resize(outputWidth, outputHeight);
        window->show();
        QVERIFY2(QTest::qWaitForWindowExposed(window), qPrintable(kind));

        QObject *const field = window->findChild<QObject *>(
            QStringLiteral("celestina-soft-menu-field"));
        QVERIFY2(field, qPrintable(kind));
        QVERIFY2(field->property("edgeAttachmentRequested").toBool(),
                 qPrintable(kind));
        QVERIFY2(field->property("edgeShapeActive").toBool(), qPrintable(kind));

        // The compositor region is real pixels; the seam and the mouth are
        // the shell numbers times the factor.
        revealAllFields(window);
        QTRY_COMPARE(window->property("glassRegions").toList().size(), 1);
        const QVariantMap shape = window->property("glassRegions")
                                      .toList().constFirst().toMap();
        const QVariantList polygon =
            shape.value(QStringLiteral("polygon")).toList();
        QVERIFY2(polygon.size() >= 3, qPrintable(kind));

        qreal top = std::numeric_limits<qreal>::max();
        for (const QVariant &value : polygon)
            top = qMin(top, value.toPointF().y());
        QCOMPARE(qRound(top), qRound(attachmentStartY * factor));

        qreal mouthLeft = std::numeric_limits<qreal>::max();
        qreal mouthRight = std::numeric_limits<qreal>::lowest();
        for (const QVariant &value : polygon) {
            const QPointF point = value.toPointF();
            if (qAbs(point.y() - top) < 0.001) {
                mouthLeft = qMin(mouthLeft, point.x());
                mouthRight = qMax(mouthRight, point.x());
            }
        }
        const qreal mouthCentre = (mouthLeft + mouthRight) / 2;
        const qreal glyphCentre =
            (attachmentAnchor.x() + attachmentAnchor.width() / 2.0) * factor;
        QVERIFY2(
            qAbs(mouthCentre - glyphCentre) <= 12.0 * factor,
            qPrintable(QStringLiteral("%1: mouth at %2, glyph at %3")
                           .arg(kind).arg(mouthCentre).arg(glyphCentre))
        );
    }
}

// Every row says what it is and what state it is in, because a menu read aloud
// is the only one some people get. A row that is a mutually-exclusive state
// says so as one rather than leaving its mark to colour alone.
void IndicatorMenuTest::everyRowNamesItselfAndItsState()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &kind : softMenuKinds()) {
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
        {QStringLiteral("outputName"), QStringLiteral("test-output")},
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

void IndicatorMenuTest::wallpaperPagesStayOnOneBoundedCatalogue()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    FakeBridge bridge;
    const QVariantMap image {
        {QStringLiteral("id"), QStringLiteral("wallpaper-000.png")},
        {QStringLiteral("name"), QStringLiteral("wallpaper-000.png")},
        {QStringLiteral("previewUrl"), QStringLiteral("file:///pictures/wallpaper-000.png")},
        {QStringLiteral("revision"), QStringLiteral("10:1")},
    };
    bridge.publish({
        {QStringLiteral("wallpaper-gallery"),
         QVariantMap {
             {QStringLiteral("state"), QStringLiteral("ready")},
             {QStringLiteral("folder"), QStringLiteral("/pictures")},
             {QStringLiteral("folderUrl"), QStringLiteral("file:///pictures")},
             {QStringLiteral("catalogue"), QStringLiteral("7")},
             {QStringLiteral("page"), 1},
             {QStringLiteral("pageCount"), 2},
             {QStringLiteral("total"), 65},
             {QStringLiteral("hasPrevious"), false},
             {QStringLiteral("hasNext"), true},
             {QStringLiteral("images"), QVariantList {image}},
             {QStringLiteral("truncated"), true},
             {QStringLiteral("skipped"), 0},
         }},
    });

    std::unique_ptr<QObject> owner;
    QQuickWindow *const window = openAgainst(
        engine, QStringLiteral("wallpaper"), &bridge, owner
    );
    QVERIFY(window);
    QCOMPARE(window->property("page").toInt(), 1);
    QCOMPARE(window->property("pageCount").toInt(), 2);
    QCOMPARE(window->property("totalImages").toInt(), 65);
    QVERIFY(window->findChild<QObject *>(
        QStringLiteral("celestina-wallpaper-previous-page")
    ));
    QVERIFY(window->findChild<QObject *>(
        QStringLiteral("celestina-wallpaper-next-page")
    ));

    QVariant summary;
    QVERIFY(QMetaObject::invokeMethod(
        window, "folderSummary", Q_RETURN_ARG(QVariant, summary)
    ));
    QVERIFY(!summary.toString().contains(QStringLiteral("limitada")));
    QVERIFY(QMetaObject::invokeMethod(window, "nextPage"));
    QTRY_COMPARE(
        bridge.sent,
        QStringList {QStringLiteral("wallpaper-gallery/set-page")}
    );
    QCOMPARE(bridge.sentOptions.size(), 1);
    QCOMPARE(
        bridge.sentOptions.constFirst().value(QStringLiteral("catalogue")).toString(),
        QStringLiteral("7")
    );
    QCOMPARE(
        bridge.sentOptions.constFirst().value(QStringLiteral("page")).toInt(),
        2
    );
}

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
        if (row && row->property("note").toString().contains(QStringLiteral("descartar"))) {
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

void IndicatorMenuTest::aBluetoothFailureCanBeDismissed()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    FakeBridge bridge;
    bridge.publish({
        {QStringLiteral("bluetooth"),
         QVariantMap {
             {QStringLiteral("adapter"), QStringLiteral("on")},
             {QStringLiteral("devicesState"), QStringLiteral("fresh")},
             {QStringLiteral("devices"), QVariantList {}},
         }},
    });

    bridge.requests()->send(
        QStringLiteral("bluetooth"), QStringLiteral("connect-known"),
        {{QStringLiteral("id"), QStringLiteral("gone-device")}},
        QStringLiteral("device:gone-device"), RequestLedger::ConfirmedPolicy
    );
    QTest::ignoreMessage(
        QtWarningMsg,
        "Celestina's provider request failed: the device disappeared"
    );
    bridge.requests()->result(
        1, QStringLiteral("failed"), QStringLiteral("the device disappeared")
    );

    std::unique_ptr<QObject> owner;
    QQuickWindow *const window = openAgainst(
        engine, QStringLiteral("bluetooth"), &bridge, owner
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
        if (row && row->property("note").toString().contains(
                       QStringLiteral("descartar")
                   )) {
            failure = row;
            break;
        }
    }
    QVERIFY(failure);
    QVERIFY(failure->property("enabled").toBool());
    QCOMPARE(bridge.requests()->failures(QStringLiteral("bluetooth")).size(), 1);

    QVERIFY(QMetaObject::invokeMethod(failure, "triggered"));
    QTRY_COMPARE(
        bridge.requests()->failures(QStringLiteral("bluetooth")).size(), 0
    );
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
        {QStringLiteral("outputName"), QStringLiteral("test-output")},
        {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(&bridge)},
    }));
    QVERIFY(root);
    auto *const centreWindow = qobject_cast<QQuickWindow *>(root.get());
    QVERIFY(centreWindow);
    centreWindow->resize(outputWidth, outputHeight);
    const QRect opener(900, 12, 30, 30);
    const QRect attachmentAnchor(906, 18, 18, 18);
    centreWindow->setProperty("anchoredFromPanel", true);
    centreWindow->setProperty("openerRect", opener);
    centreWindow->setProperty("attachmentAnchorRect", attachmentAnchor);
    centreWindow->setProperty("attachmentStartY", 40);
    QCOMPARE(
        centreWindow->property("attachmentAnchorRect").toRect(),
        attachmentAnchor
    );
    QCOMPARE(
        centreWindow->property("cardY").toInt(),
        40 + centreWindow->property("anchorGap").toInt()
    );
    QVERIFY(
        centreWindow->property("cardY").toInt()
            + centreWindow->property("cardHeight").toInt()
        <= outputHeight
    );
    QObject *const calendar = root->findChild<QObject *>(
        QStringLiteral("celestina-control-centre-calendar")
    );
    QVERIFY(calendar);
    const QList<QObject *> calendarGlass = calendar->findChildren<QObject *>(
        QStringLiteral("celestina-compositor-glass-region")
    );
    QCOMPARE(calendarGlass.size(), 0);
    const QList<QObject *> calendarSections = calendar->findChildren<QObject *>(
        QStringLiteral("celestina-menu-section")
    );
    QCOMPARE(calendarSections.size(), 1);
    QVERIFY(calendar->property("height").toReal() > 30.0);
    QCOMPARE(
        calendarSections.constFirst()->property("height").toReal(),
        calendar->property("height").toReal()
    );
    QVERIFY(
        calendarSections.constFirst()->property("radius").toReal()
        < calendar->property("height").toReal() / 2.0
    );
    const QList<QObject *> centreGlass = root->findChildren<QObject *>(
        QStringLiteral("celestina-compositor-glass-region")
    );
    QCOMPARE(centreGlass.size(), 1);
    QCOMPARE(centreWindow->property("cardWidth").toInt(), 530);
    QCOMPARE(centreWindow->property("cardHeight").toInt(), 805);
    revealAllFields(centreWindow);
    QTRY_COMPARE(centreWindow->property("glassRegions").toList().size(), 1);
    const QVariantMap centreShape = centreWindow->property("glassRegions")
                                        .toList().constFirst().toMap();
    QCOMPARE(centreShape.value(QStringLiteral("radius")).toInt(), 20);
    const QVariantList centrePolygon =
        centreShape.value(QStringLiteral("polygon")).toList();
    QVERIFY(centrePolygon.size() >= 3);
    qreal minimumY = std::numeric_limits<qreal>::max();
    for (const QVariant &point : centrePolygon)
        minimumY = qMin(minimumY, point.toPointF().y());
    const int attachmentSeamY = 40;
    QCOMPARE(qRound(minimumY), attachmentSeamY);
    QCOMPARE(
        qRound(centreShape.value(QStringLiteral("rect")).toRectF().top()),
        attachmentSeamY
    );
    QVERIFY(root->findChild<QObject *>(
        QStringLiteral("celestina-control-centre-quick-controls")
    ));
    QVERIFY(root->findChild<QObject *>(
        QStringLiteral("celestina-control-centre-connectivity")
    ));
    QCOMPARE(
        root->findChildren<QObject *>(
            QStringLiteral("celestina-menu-section")
        ).size(),
        4
    );

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

// The persistent ghost of the fast toggle (2026-08-22): both the fresh open
// and the resume replay defer `menu.open()` by one tick, and a park landing
// inside that tick used to be ignored — the popup opened inside the resting
// scene, the next resume's replay found it already open and got no
// `aboutToShow`, and the carrier came back mapped and input-live with nothing
// painted. The queued open now re-checks the park flag at fire time.
void IndicatorMenuTest::aParkThatRacesTheQueuedOpenLeavesNoStrandedPopup()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    std::unique_ptr<QObject> owner;
    QQuickWindow *window = openMenu(engine, QStringLiteral("capture"), owner);
    QVERIFY(window);

    QObject *const menu = window->property("menu").value<QObject *>();
    QVERIFY(menu);
    QTRY_VERIFY(menu->property("visible").toBool());

    // The host parks: the popup closes silently, without a dismissal.
    QVERIFY(QMetaObject::invokeMethod(window, "prepareForPark"));
    QTRY_VERIFY(!menu->property("visible").toBool());

    // A resume queues the replay — and a park races it before the tick.
    QVERIFY(QMetaObject::invokeMethod(window, "reopenForReuse"));
    QVERIFY(QMetaObject::invokeMethod(window, "prepareForPark"));
    QTest::qWait(50);
    QVERIFY2(
        !menu->property("visible").toBool(),
        "a queued open must not land inside the resting scene"
    );

    // And the next honest resume still opens: the skipped replay left no
    // already-visible popup to swallow this one's aboutToShow.
    QVERIFY(QMetaObject::invokeMethod(window, "reopenForReuse"));
    QTRY_VERIFY(menu->property("visible").toBool());
}

QTEST_MAIN(IndicatorMenuTest)
#include "indicatormenu_test.moc"
