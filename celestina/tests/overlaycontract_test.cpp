#include <QtTest>

#include <QQmlComponent>
#include <QQmlEngine>
#include <QColor>
#include <QMap>
#include <QQuickItem>
#include <QQuickWindow>
#include <QStringList>
#include <QUrl>
#include <QSignalSpy>
#include <QVariantMap>

#include <memory>

#include "overlaycontroller.h"
#include "panelpopupplacement.h"

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
} // namespace

// What each overlay is handed when it is created, and what a click outside its
// card does.
//
// This is the offscreen half of a defect that only ever showed up live: the
// host injected `providerSource` into every overlay component, and the session
// menu declares `shellSource` instead, so every open logged
// `SessionMenu does not have a property called providerSource`. The surface
// still drew, which is exactly why nothing caught it — the missed property was
// the one the component never wanted.
//
// It proves the contract inside the window, not the surface around it. Whether
// the compositor delivers an outside click to this window at all is what
// `OverlaySurface` now arranges by covering the output, and only a real Wayland
// session can show that; what is provable here is that the window answers such
// a click with `dismissed()` and leaves a click on the card alone.
class OverlayContractTest final : public QObject
{
    Q_OBJECT

private slots:
    void everyOverlayDeclaresTheBridgeTheListNamesForIt();
    void everyInteractiveOverlayUsesOneVeloGlassField();
    void aPropertyTheComponentDoesNotDeclareIsVisibleAsAFailure();
    void aComponentThisShellDoesNotHaveNamesNoBridge();
    void aPanelOpenedOverlayFollowsOnlyItsButton();
    void everyPanelOpenedOverlayUsesTheSamePlacement();
    void sessionCardGrowthDoesNotResizeItsOutputSurface();
    void aClickOutsideTheCardDismissesEveryOverlay();
    void aClickOnTheCardDismissesNothing();

private:
    // The overlays `main()` builds, by component name. Kept here as the set the
    // list must cover, so an overlay added without a bridge name fails this
    // case rather than a session.
    static QStringList overlays()
    {
        return {
            QStringLiteral("LauncherOverlay"),
            QStringLiteral("ClipboardOverlay"),
            QStringLiteral("NotificationCenter"),
            QStringLiteral("ControlCentre"),
            QStringLiteral("SessionMenu"),
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
};

void OverlayContractTest::everyOverlayDeclaresTheBridgeTheListNamesForIt()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &component : overlays()) {
        const QString bridge = overlaySourceProperty(component);
        QVERIFY2(!bridge.isEmpty(), qPrintable(component));

        QQmlComponent overlay(&engine, sourceFor(component));
        QVERIFY2(overlay.isReady(), qPrintable(overlay.errorString()));

        // A null bridge is enough: this is about which properties exist, and
        // every one of these surfaces reads its bridge through a guard that
        // already answers "nothing published yet".
        const QVariantMap properties {
            {QStringLiteral("reducedMotion"), true},
            {bridge, QVariant::fromValue<QObject *>(nullptr)},
            {QStringLiteral("anchoredFromPanel"), true},
            {QStringLiteral("openerRect"), QRect(900, 6, 28, 28)},
        };

        QStringList messages;
        captured = &messages;
        QtMessageHandler previous = qInstallMessageHandler(collect);
        QObject *const root = overlay.createWithInitialProperties(properties);
        qInstallMessageHandler(previous);
        captured = nullptr;

        QVERIFY2(root != nullptr, qPrintable(overlay.errorString()));
        QVERIFY2(
            !complainedAboutAProperty(messages),
            qPrintable(component + QStringLiteral(": ") + messages.join(u'\n'))
        );
        delete root;
    }
}

void OverlayContractTest::everyInteractiveOverlayUsesOneVeloGlassField()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &component : overlays()) {
        QQmlComponent overlay(&engine, sourceFor(component));
        QVERIFY2(overlay.isReady(), qPrintable(overlay.errorString()));

        std::unique_ptr<QObject> root(overlay.createWithInitialProperties({
            {QStringLiteral("reducedMotion"), true},
            {overlaySourceProperty(component), QVariant::fromValue<QObject *>(nullptr)},
        }));
        QVERIFY2(root, qPrintable(component));
        QVERIFY2(
            root->metaObject()->indexOfProperty("glassRegions") >= 0,
            qPrintable(component)
        );
        const QList<QObject *> outerGlass = root->findChildren<QObject *>(
            QStringLiteral("celestina-compositor-glass-region")
        );
        QCOMPARE(outerGlass.size(), 1);
        QObject *const bodyMaterial = root->findChild<QObject *>(
            QStringLiteral("celestina-menu-body-tint")
        );
        QVERIFY2(bodyMaterial, qPrintable(component));
        QCOMPARE(bodyMaterial->property("backdropMode").toInt(), 1);
        QCOMPARE(bodyMaterial->property("externalBackdropReady").toBool(), true);
        QCOMPARE(bodyMaterial->property("captureActive").toBool(), false);
        QCOMPARE(bodyMaterial->property("elevation").toInt(), 0);
        const QList<QObject *> sections = root->findChildren<QObject *>(
            QStringLiteral("celestina-menu-section")
        );
        QVERIFY2(!sections.isEmpty(), qPrintable(component));
        const int sectionRole = sections.constFirst()
                                    ->property("materialRole").toInt();
        const qreal sectionStrength = sections.constFirst()
                                          ->property("materialStrength").toReal();
        const QColor sectionTint = sections.constFirst()
                                       ->property("materialTint").value<QColor>();
        QVERIFY2(
            bodyMaterial->property("materialRole").toInt() != sectionRole,
            qPrintable(component)
        );
        QVERIFY2(
            bodyMaterial->property("materialStrength").toReal()
                < sectionStrength,
            qPrintable(component)
        );
        for (QObject *const section : sections) {
            QVERIFY2(
                section->metaObject()->indexOfProperty("captureActive") >= 0,
                qPrintable(component)
            );
            QCOMPARE(section->property("backdropMode").toInt(), 1);
            QCOMPARE(section->property("externalBackdropReady").toBool(), true);
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
        QCOMPARE(
            root->findChildren<QObject *>(
                QStringLiteral("celestina-menu-header")
            ).size(),
            1
        );

        const QMap<QString, int> expectedSections {
            {QStringLiteral("LauncherOverlay"), 3},
            {QStringLiteral("ClipboardOverlay"), 2},
            {QStringLiteral("NotificationCenter"), 2},
            {QStringLiteral("ControlCentre"), 4},
            {QStringLiteral("SessionMenu"), 2},
        };
        QCOMPARE(sections.size(), expectedSections.value(component));
    }
}

void OverlayContractTest::everyPanelOpenedOverlayUsesTheSamePlacement()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    constexpr int testOutputWidth = 1280;
    // Keep the synthetic output taller than every overlay. Bottom-edge
    // clamping is covered separately; this case isolates the opener gap.
    constexpr int testOutputHeight = 1600;
    const QRect opener(1000, 5, 28, 28);
    for (const QString &component : {
             QStringLiteral("LauncherOverlay"),
             QStringLiteral("ClipboardOverlay"),
             QStringLiteral("NotificationCenter"),
             QStringLiteral("ControlCentre"),
             QStringLiteral("SessionMenu"),
         }) {
        QQmlComponent overlay(&engine, sourceFor(component));
        QVERIFY2(overlay.isReady(), qPrintable(overlay.errorString()));

        std::unique_ptr<QObject> root(overlay.createWithInitialProperties({
            {QStringLiteral("reducedMotion"), true},
            {overlaySourceProperty(component), QVariant::fromValue<QObject *>(nullptr)},
            {QStringLiteral("anchoredFromPanel"), true},
            {QStringLiteral("openerRect"), opener},
        }));
        auto *window = qobject_cast<QQuickWindow *>(root.get());
        QVERIFY2(window, qPrintable(component));
        window->resize(testOutputWidth, testOutputHeight);

        QCOMPARE(window->property("cardY").toInt(), opener.bottom() + 1 + 8);

        const int cardWidth = window->property("cardWidth").toInt();
        const qreal centred = opener.x() + opener.width() / 2.0
                              - cardWidth / 2.0;
        const int expectedX = qRound(qBound(
            qreal(0),
            centred,
            qreal(testOutputWidth - cardWidth)
        ));
        QCOMPARE(window->property("cardX").toInt(), expectedX);

        QQuickItem *const body = window->findChild<QQuickItem *>(
            QStringLiteral("celestina-compositor-glass-region")
        );
        QVERIFY2(body, qPrintable(component));
        const QPointF bodyOrigin = body->mapToItem(window->contentItem(), 0, 0);
        QCOMPARE(qRound(bodyOrigin.x()), window->property("cardX").toInt());
        QCOMPARE(qRound(bodyOrigin.y()), window->property("cardY").toInt());

        // With no opener the same reusable placement falls back to the centre.
        window->setProperty("anchoredFromPanel", false);
        QCOMPARE(
            window->property("cardX").toInt(),
            (testOutputWidth - cardWidth) / 2
        );
    }
}

void OverlayContractTest::sessionCardGrowthDoesNotResizeItsOutputSurface()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    QQmlComponent overlay(&engine, sourceFor(QStringLiteral("SessionMenu")));
    QVERIFY2(overlay.isReady(), qPrintable(overlay.errorString()));

    const QRect opener(1000, 5, 28, 28);
    std::unique_ptr<QObject> root(overlay.createWithInitialProperties({
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("shellSource"), QVariant::fromValue<QObject *>(nullptr)},
        {QStringLiteral("anchoredFromPanel"), true},
        {QStringLiteral("openerRect"), opener},
    }));
    auto *window = qobject_cast<QQuickWindow *>(root.get());
    QVERIFY(window);

    // The content-sized geometry is used only to bootstrap the Window before
    // layer-shell configures it as an output-sized input surface.
    const int bootstrapCardHeight = window->property("cardHeight").toInt();
    QVERIFY(bootstrapCardHeight > 0);
    QCOMPARE(window->height(), bootstrapCardHeight);

    constexpr int outputWidth = 1280;
    constexpr int outputHeight = 1600;
    window->resize(outputWidth, outputHeight);
    window->show();
    QVERIFY(QTest::qWaitForWindowExposed(window));
    // Repeater delegates are polished once the card is exposed. Their first
    // real implicit height must grow the card without taking the output-sized
    // input surface back with it.
    QTRY_VERIFY(window->property("cardHeight").toInt() > bootstrapCardHeight);
    const int expectedY = opener.bottom() + 1
                          + window->property("anchorGap").toInt();
    QCOMPARE(window->property("cardY").toInt(), expectedY);

    // Later dynamic refusal copy must still leave the visual card independent
    // from the full-output Window. A live height binding here would collapse
    // that Window, after which placement would clamp the card over the panel.
    window->setProperty("outcomeVerb", QStringLiteral("power-off"));
    window->setProperty("outcomeState", QStringLiteral("failed"));
    window->setProperty(
        "outcomeReason",
        QStringLiteral(
            "the session manager returned a deliberately long diagnostic "
            "that must wrap onto several lines inside the card"
        )
    );
    QCoreApplication::processEvents();
    QCOMPARE(window->height(), outputHeight);
    QCOMPARE(window->property("cardY").toInt(), expectedY);
}

// The case above only means something if it can fail. This is the exact
// injection the host used to perform on every overlay, against the one
// component that never declared it.
void OverlayContractTest::aPropertyTheComponentDoesNotDeclareIsVisibleAsAFailure()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    QQmlComponent menu(&engine, sourceFor(QStringLiteral("SessionMenu")));
    QVERIFY2(menu.isReady(), qPrintable(menu.errorString()));

    QStringList messages;
    captured = &messages;
    QtMessageHandler previous = qInstallMessageHandler(collect);
    QObject *const root = menu.createWithInitialProperties({
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("shellSource"), QVariant::fromValue<QObject *>(nullptr)},
        {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(nullptr)},
    });
    qInstallMessageHandler(previous);
    captured = nullptr;

    QVERIFY(complainedAboutAProperty(messages));
    delete root;
}

void OverlayContractTest::aComponentThisShellDoesNotHaveNamesNoBridge()
{
    QVERIFY(overlaySourceProperty(QStringLiteral("Panel")).isEmpty());
    QVERIFY(overlaySourceProperty(QString()).isEmpty());
}

void OverlayContractTest::aPanelOpenedOverlayFollowsOnlyItsButton()
{
    const QPoint outputOrigin(1920, 120);
    const QRect opener(2260, 128, 30, 30);

    const QRect local = panelPopupOpenerOnOutput(opener, outputOrigin);
    QCOMPARE(local.x(), opener.x() - outputOrigin.x());
    QCOMPARE(local.y(), opener.y() - outputOrigin.y());
    QCOMPARE(local.size(), opener.size());

    // The menu follows the real 30 px button and one floating gap, independently
    // of the panel surface's own extent.
    QCOMPARE(panelPopupBodyOrigin(local, 530, 8).y(), 46);
}


namespace {
// The surface covers the output, so the window is bigger than the card. This is
// what a compositor configure leaves behind, applied by hand because nothing
// here has a compositor.
constexpr int outputWidth = 1280;
constexpr int outputHeight = 800;
} // namespace

void OverlayContractTest::aClickOutsideTheCardDismissesEveryOverlay()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &component : overlays()) {
        QQmlComponent overlay(&engine, sourceFor(component));
        QVERIFY2(overlay.isReady(), qPrintable(overlay.errorString()));

        std::unique_ptr<QObject> root(overlay.createWithInitialProperties({
            {QStringLiteral("reducedMotion"), true},
            {overlaySourceProperty(component), QVariant::fromValue<QObject *>(nullptr)},
        }));
        auto *window = qobject_cast<QQuickWindow *>(root.get());
        QVERIFY2(window, qPrintable(component));

        window->resize(outputWidth, outputHeight);
        window->show();
        QVERIFY(QTest::qWaitForWindowExposed(window));

        QSignalSpy dismissed(window, SIGNAL(dismissed()));
        QVERIFY(dismissed.isValid());
        // The far corner of the output: as far outside the centred card as the
        // surface goes.
        QTest::mouseClick(window, Qt::LeftButton, {}, QPoint(4, 4));
        QCOMPARE(dismissed.count(), 1);
    }
}

void OverlayContractTest::aClickOnTheCardDismissesNothing()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &component : overlays()) {
        QQmlComponent overlay(&engine, sourceFor(component));
        QVERIFY2(overlay.isReady(), qPrintable(overlay.errorString()));

        std::unique_ptr<QObject> root(overlay.createWithInitialProperties({
            {QStringLiteral("reducedMotion"), true},
            {overlaySourceProperty(component), QVariant::fromValue<QObject *>(nullptr)},
        }));
        auto *window = qobject_cast<QQuickWindow *>(root.get());
        QVERIFY2(window, qPrintable(component));

        window->resize(outputWidth, outputHeight);
        window->show();
        QVERIFY(QTest::qWaitForWindowExposed(window));

        QSignalSpy dismissed(window, SIGNAL(dismissed()));
        QVERIFY(dismissed.isValid());
        // Just inside the card's right edge: card, but padding rather than a
        // control, so what stops the click is the card's own catch-all rather
        // than a button that would have swallowed it anyway.
        const int cardWidth = window->property("cardWidth").toInt();
        QVERIFY(cardWidth > 0);
        QTest::mouseClick(
            window,
            Qt::LeftButton,
            {},
            QPoint(outputWidth / 2 + cardWidth / 2 - 3, outputHeight / 2)
        );
        QCOMPARE(dismissed.count(), 0);
    }
}

QTEST_MAIN(OverlayContractTest)
#include "overlaycontract_test.moc"
