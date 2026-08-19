#include <QtTest>

#include <QQmlComponent>
#include <QQmlEngine>
#include <QColor>
#include <QFile>
#include <QMap>
#include <QQuickItem>
#include <QQuickWindow>
#include <QStringList>
#include <QUrl>
#include <QSignalSpy>
#include <QVariantMap>
#include <qqml.h>

#include <limits>
#include <memory>

#include "overlaycontroller.h"
#include "overlaysurface.h"
#include "panelpopupplacement.h"
#include "quietplacement.h"
#include "surfacemanager.h"

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

class OverlayContractTest final : public QObject
{
    Q_OBJECT

private slots:
    void initTestCase();
    void everyOverlayDeclaresTheBridgeTheListNamesForIt();
    void everyInteractiveOverlayUsesOneVeloGlassField();
    void everyOverlayDelegatesItsPresentationGateToTheController();
    void readinessWaitsForTheFrameAfterGlassAndDiesOnRetirement();
    void aNewOverlayRetiresItsPredecessorOnlyAfterFirstGlass();
    void closeIsOneIdempotentSoftRetirement();
    void aPropertyTheComponentDoesNotDeclareIsVisibleAsAFailure();
    void aComponentThisShellDoesNotHaveNamesNoBridge();
    void aPanelOpenedOverlayFollowsOnlyItsButton();
    void everyPanelOpenedOverlayUsesTheSamePlacement();
    void anAttachedOverlayNeverReblursThePanelRows();
    void aScaledOutputDrawsLargerWithoutMovingAnythingItStates();
    void sessionCardGrowthDoesNotResizeItsOutputSurface();
    void attachedCarriersBeginBelowThePanelWithoutChangingInteraction();
    void aPanelAttachedNotificationUsesCarrierLocalGeometryAndDismissesOutside();
    void aClickOutsideTheCardDismissesEveryOverlay();
    void aClickOnTheCardDismissesNothing();
    void quietGeometryCentresTheCardAndReachesTheIcon();
    void quietZoneYieldsOnlyToARealIntrusion();
    void aLevelChangedFromItsOwnMenuRaisesNoDisplay();

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
            QStringLiteral("BubbleSelector"),
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

void OverlayContractTest::initTestCase()
{
    // OverlayController intentionally loads the production module by type
    // name. This executable tests the source files directly, so register the
    // one production type used by the controller lifecycle regressions before
    // any engine exists in this process.
    QVERIFY(qmlRegisterType(
        sourceFor(QStringLiteral("LauncherOverlay")),
        "CelestinaDesktop",
        1,
        0,
        "LauncherOverlay"
    ) >= 0);
}

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
            {QStringLiteral("attachmentAnchorRect"), QRect(905, 11, 18, 18)},
            {QStringLiteral("attachmentStartY"), 40},
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
            {QStringLiteral("BubbleSelector"), 2},
            {QStringLiteral("SessionMenu"), 2},
        };
        QCOMPARE(sections.size(), expectedSections.value(component));
    }
}

void OverlayContractTest::everyOverlayDelegatesItsPresentationGateToTheController()
{
    for (const QString &component : overlays()) {
        QFile source(sourceFor(component).toLocalFile());
        QVERIFY2(source.open(QIODevice::ReadOnly), qPrintable(component));
        const QByteArray qml = source.readAll();

        // Configure, exposure and frame ownership belong to the mapper. A
        // component-local copy can drift (and did: the width-only bootstrap
        // test left equal-width outputs permanently transparent).
        QVERIFY2(!qml.contains("revealPending"), qPrintable(component));
        QVERIFY2(!qml.contains("surfaceConfigured"), qPrintable(component));
        QVERIFY2(!qml.contains("onFrameSwapped"), qPrintable(component));
        QVERIFY2(!qml.contains(".reveal("), qPrintable(component));
    }
}

void OverlayContractTest::readinessWaitsForTheFrameAfterGlassAndDiesOnRetirement()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    QObject source;
    source.setProperty("providers", QVariantMap());

    const QVariantList paintedGlass {
        QVariantMap {
            {QStringLiteral("rect"), QRectF(20, 20, 120, 80)},
            {QStringLiteral("radius"), 16},
        },
    };

    OverlayController presented(
        &engine, QStringLiteral("LauncherOverlay"), &source);
    QSignalSpy presentedReady(
        &presented, &OverlayController::contextualSurfaceOpened);
    QVERIFY(presented.isEnabled());
    QVERIFY(presentedReady.isValid());
    presented.open();
    auto *const presentedSurface = presented.findChild<OverlaySurface *>();
    QVERIFY(presentedSurface);
    QWindow *const presentedWindow = presentedSurface->window();
    QVERIFY(presentedWindow);
    presentedWindow->setProperty("reducedMotion", true);
    QObject *const presentedField = presentedWindow->findChild<QObject *>(
        QStringLiteral("celestina-soft-menu-field"));
    QVERIFY(presentedField);

    // The QML publication describes a buffer Qt Quick has not swapped yet.
    // This used to emit here and retire the predecessor one frame too early.
    presentedField->setProperty("glassRegions", paintedGlass);
    QCOMPARE(presentedReady.count(), 0);
    QVERIFY(QTest::qWaitForWindowExposed(presentedWindow));
    if (presentedReady.isEmpty())
        QMetaObject::invokeMethod(presentedWindow, "frameSwapped");
    QTRY_COMPARE(presentedReady.count(), 1);

    OverlayController retired(
        &engine, QStringLiteral("LauncherOverlay"), &source);
    QSignalSpy retiredReady(
        &retired, &OverlayController::contextualSurfaceOpened);
    QVERIFY(retired.isEnabled());
    QVERIFY(retiredReady.isValid());
    retired.open();
    auto *const retiredSurface = retired.findChild<OverlaySurface *>();
    QVERIFY(retiredSurface);
    QPointer<QWindow> retiredWindow(retiredSurface->window());
    QVERIFY(retiredWindow);
    retiredWindow->setProperty("reducedMotion", true);
    QObject *const retiredField = retiredWindow->findChild<QObject *>(
        QStringLiteral("celestina-soft-menu-field"));
    QVERIFY(retiredField);
    retiredField->setProperty("glassRegions", paintedGlass);
    QCOMPARE(retiredReady.count(), 0);

    // Retirement invalidates the armed publication. A late swap from this
    // carrier may neither announce readiness nor sweep another surface.
    retired.close();
    QVERIFY(retiredWindow->property("celestinaRetiring").toBool());
    QMetaObject::invokeMethod(retiredWindow, "frameSwapped");
    QCoreApplication::processEvents();
    QCOMPARE(retiredReady.count(), 0);
    QTRY_VERIFY(!retired.isOpen());
}

void OverlayContractTest::aNewOverlayRetiresItsPredecessorOnlyAfterFirstGlass()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    QObject source;
    source.setProperty("providers", QVariantMap());

    OverlayController oldOverlay(
        &engine, QStringLiteral("LauncherOverlay"), &source);
    OverlayController newOverlay(
        &engine, QStringLiteral("LauncherOverlay"), &source);
    QVERIFY(oldOverlay.isEnabled());
    QVERIFY(newOverlay.isEnabled());

    QSignalSpy oldReady(
        &oldOverlay, &OverlayController::contextualSurfaceOpened);
    QSignalSpy newReady(
        &newOverlay, &OverlayController::contextualSurfaceOpened);
    QVERIFY(oldReady.isValid());
    QVERIFY(newReady.isValid());

    oldOverlay.open();
    auto *const oldSurface = oldOverlay.findChild<OverlaySurface *>();
    QVERIFY(oldSurface);
    QPointer<QWindow> oldWindow(oldSurface->window());
    QVERIFY(oldWindow);
    QVERIFY(QTest::qWaitForWindowExposed(oldWindow));
    if (oldReady.isEmpty())
        QMetaObject::invokeMethod(oldWindow, "frameSwapped");
    QTRY_COMPARE(oldReady.count(), 1);
    QVERIFY(!oldWindow->property("glassRegions").toList().isEmpty());

    bool oldWasOpenAtReady = false;
    bool oldStayedOpenForRetirement = false;
    bool oldEnteredRetirement = false;
    connect(
        &newOverlay,
        &OverlayController::contextualSurfaceOpened,
        &newOverlay,
        [&]() {
            oldWasOpenAtReady = oldOverlay.isOpen();
            oldOverlay.close();
            oldStayedOpenForRetirement = oldOverlay.isOpen();
            oldEnteredRetirement = oldWindow
                && oldWindow->property("celestinaRetiring").toBool();
        }
    );

    newOverlay.open();
    auto *const newSurface = newOverlay.findChild<OverlaySurface *>();
    QVERIFY(newSurface);
    QWindow *const newWindow = newSurface->window();
    QVERIFY(newWindow);

    // Mapping alone is not readiness and cannot sweep the predecessor.
    QCOMPARE(newReady.count(), 0);
    QVERIFY(oldOverlay.isOpen());
    QVERIFY(!oldWindow->property("celestinaRetiring").toBool());

    QVERIFY(QTest::qWaitForWindowExposed(newWindow));
    if (newReady.isEmpty())
        QMetaObject::invokeMethod(newWindow, "frameSwapped");
    QTRY_COMPARE(newReady.count(), 1);
    QVERIFY(!newWindow->property("glassRegions").toList().isEmpty());
    QVERIFY(oldWasOpenAtReady);
    QVERIFY(oldStayedOpenForRetirement);
    QVERIFY(oldEnteredRetirement);
    QTRY_VERIFY(!oldOverlay.isOpen());

    // Geometry can republish while scale or placement moves. Readiness is one
    // edge for this mapped window, never one signal per glass publication.
    QObject *const field = newWindow->findChild<QObject *>(
        QStringLiteral("celestina-soft-menu-field"));
    QVERIFY(field);
    const QVariant regions = field->property("glassRegions");
    field->setProperty("glassRegions", QVariantList());
    field->setProperty("glassRegions", regions);
    QCoreApplication::processEvents();
    QCOMPARE(newReady.count(), 1);
}

void OverlayContractTest::closeIsOneIdempotentSoftRetirement()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    QObject source;
    source.setProperty("providers", QVariantMap());
    OverlayController overlay(
        &engine, QStringLiteral("LauncherOverlay"), &source);
    QVERIFY(overlay.isEnabled());

    overlay.open();
    auto *const surface = overlay.findChild<OverlaySurface *>();
    QVERIFY(surface);
    QPointer<QWindow> window(surface->window());
    QVERIFY(window);

    overlay.close();
    QVERIFY(overlay.isOpen());
    QVERIFY(window->property("celestinaRetiring").toBool());
    QCOMPARE(surface->window(), window.data());

    // A repeated close observes the same beat instead of destroying early or
    // scheduling a second completion against the window.
    overlay.close();
    QVERIFY(overlay.isOpen());
    QCOMPARE(surface->window(), window.data());
    QTRY_VERIFY(!overlay.isOpen());
}

void OverlayContractTest::everyPanelOpenedOverlayUsesTheSamePlacement()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    constexpr int testOutputWidth = 1280;
    // Keep the synthetic output taller than every overlay. Bottom-edge
    // clamping is covered separately; this case isolates the opener gap.
    constexpr int testOutputHeight = 1600;
    constexpr int attachmentStartY = 40;
    const QRect opener(1000, 5, 28, 28);
    const QRect attachmentAnchor(1005, 10, 18, 18);
    QMap<int, int> attachedGapsByWidth;
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
            {QStringLiteral("attachmentAnchorRect"), attachmentAnchor},
            {QStringLiteral("attachmentStartY"), attachmentStartY},
        }));
        auto *window = qobject_cast<QQuickWindow *>(root.get());
        QVERIFY2(window, qPrintable(component));
        window->resize(testOutputWidth, testOutputHeight);

        const int attachedGap = window->property("anchorGap").toInt();
        QVERIFY(attachedGap > 8);
        attachedGapsByWidth.insert(
            window->property("cardWidth").toInt(), attachedGap
        );
        QCOMPARE(
            window->property("cardY").toInt(),
            attachmentStartY + attachedGap
        );

        const int cardWidth = window->property("cardWidth").toInt();
        const QMap<int, int> expectedGapByWidth {
            {360, 22},
            {460, 28},
            {530, 32},
            {620, 36},
        };
        QCOMPARE(attachedGap, expectedGapByWidth.value(cardWidth));
        const qreal centred = opener.x() + opener.width() / 2.0
                              - cardWidth / 2.0;
        const int expectedX = qRound(qBound(
            qreal(0),
            centred,
            qreal(testOutputWidth - cardWidth)
        ));
        QCOMPARE(window->property("cardX").toInt(), expectedX);
        QCOMPARE(
            window->property("attachmentAnchorRect").toRect(),
            attachmentAnchor
        );

        QQuickItem *const body = window->findChild<QQuickItem *>(
            QStringLiteral("celestina-compositor-glass-region")
        );
        QVERIFY2(body, qPrintable(component));
        const QPointF bodyOrigin = body->mapToItem(window->contentItem(), 0, 0);
        const int attachmentSeamY = attachmentStartY;
        QCOMPARE(qRound(bodyOrigin.x()), window->property("cardX").toInt());
        QCOMPARE(qRound(bodyOrigin.y()), attachmentSeamY);
        QCOMPARE(
            qRound(body->height()),
            window->property("cardY").toInt()
                - attachmentSeamY
                + window->property("cardHeight").toInt()
        );
        QVERIFY(body->property("usesSilhouette").toBool());
        QVERIFY(!body->property("silhouettePath").toString().isEmpty());
        const QVariantList attachedPolygon = body->property("polygon").toList();
        QVERIFY(attachedPolygon.size() >= 3);

        qreal topY = std::numeric_limits<qreal>::max();
        for (const QVariant &value : attachedPolygon)
            topY = qMin(topY, value.toPointF().y());
        qreal upperLeft = std::numeric_limits<qreal>::max();
        qreal upperRight = std::numeric_limits<qreal>::lowest();
        qreal landingLeft = std::numeric_limits<qreal>::max();
        qreal landingRight = std::numeric_limits<qreal>::lowest();
        for (const QVariant &value : attachedPolygon) {
            const QPointF point = value.toPointF();
            if (qAbs(point.y() - topY) < 0.001) {
                upperLeft = qMin(upperLeft, point.x());
                upperRight = qMax(upperRight, point.x());
            }
            if (qAbs(point.y() - attachedGap) < 0.001) {
                landingLeft = qMin(landingLeft, point.x());
                landingRight = qMax(landingRight, point.x());
            }
        }
        // The seam is one narrow droplet mouth centred on the clicked glyph,
        // not a body-wide edge. The swell lands tangent on the body's flat
        // top span, inside the rounded corners that begin at radiusMd.
        const qreal mouthWidth = upperRight - upperLeft;
        QVERIFY(mouthWidth >= attachmentAnchor.width());
        QVERIFY(mouthWidth < cardWidth * 0.25);
        QCOMPARE(
            qRound(bodyOrigin.x() + (upperLeft + upperRight) / 2),
            attachmentAnchor.x() + attachmentAnchor.width() / 2
        );
        QVERIFY(qRound(bodyOrigin.x() + landingLeft)
                > window->property("cardX").toInt());
        QVERIFY(qRound(bodyOrigin.x() + landingRight)
                < window->property("cardX").toInt() + cardWidth);
        QVERIFY(landingRight - landingLeft > mouthWidth);

        QQuickItem *const field = window->findChild<QQuickItem *>(
            QStringLiteral("celestina-soft-menu-field")
        );
        QVERIFY2(field, qPrintable(component));
        QVERIFY(field->property("edgeAttachmentRequested").toBool());
        QVERIFY(field->property("edgeShapeActive").toBool());
        QCOMPARE(
            field->property("attachmentAnchorRect").toRect(),
            attachmentAnchor
        );
        QVERIFY(field->property("attachmentWaistWidth").toReal() > 0);
        QCOMPARE(
            qRound(window->property("cardX").toReal()
                   + field->property("attachmentWaistCenterAtBody").toReal()),
            attachmentAnchor.x() + attachmentAnchor.width() / 2
        );
        QObject *const bodyMaterial = window->findChild<QObject *>(
            QStringLiteral("celestina-menu-body-tint")
        );
        QVERIFY2(bodyMaterial, qPrintable(component));
        // MaterialRole.ContextualVeil is the third declared role. The membrane
        // is only the outer background; no ContentSurface continuation may be
        // painted between the panel and the menu cards.
        QCOMPARE(bodyMaterial->property("materialRole").toInt(), 2);
        QCOMPARE(bodyMaterial->property("elevation").toInt(), 0);
        QVERIFY(!bodyMaterial->property("materialEdgesVisible").toBool());
        QVERIFY(bodyMaterial->property("usesSilhouette").toBool());
        QVERIFY2(
            !window->findChild<QObject *>(
                QStringLiteral("celestina-attachment-material-bridge")
            ),
            qPrintable(component)
        );

        QQuickItem *const revealedContent = window->findChild<QQuickItem *>(
            QStringLiteral("celestina-soft-menu-content")
        );
        QVERIFY2(revealedContent, qPrintable(component));
        QCOMPARE(revealedContent->scale(), 1.0);

        const QList<QObject *> attachedSections = window->findChildren<QObject *>(
            QStringLiteral("celestina-menu-section")
        );
        QVERIFY2(!attachedSections.isEmpty(), qPrintable(component));
        QList<QRectF> sectionGeometry;
        QList<int> sectionRoles;
        QList<qreal> sectionStrengths;
        QList<QColor> sectionTints;
        for (QObject *const section : attachedSections) {
            auto *const item = qobject_cast<QQuickItem *>(section);
            QVERIFY2(item, qPrintable(component));
            sectionGeometry.append(
                QRectF(item->x(), item->y(), item->width(), item->height())
            );
            sectionRoles.append(section->property("materialRole").toInt());
            sectionStrengths.append(
                section->property("materialStrength").toReal()
            );
            sectionTints.append(
                section->property("materialTint").value<QColor>()
            );
            QVERIFY2(!section->property("usesSilhouette").toBool(),
                     qPrintable(component));
        }

        // With no opener the same reusable placement falls back to the centre.
        window->setProperty("anchoredFromPanel", false);
        QCoreApplication::processEvents();
        const int floatingGap = window->property("anchorGap").toInt();
        QCOMPARE(
            window->property("cardX").toInt(),
            (testOutputWidth - cardWidth) / 2
        );
        const QPointF floatingOrigin = body->mapToItem(
            window->contentItem(), 0, 0
        );
        QCOMPARE(
            qRound(floatingOrigin.y()),
            window->property("cardY").toInt()
        );
        QVERIFY(!body->property("usesSilhouette").toBool());
        QVERIFY(body->property("polygon").toList().isEmpty());
        QCOMPARE(revealedContent->scale(), 1.0);

        const QList<QObject *> floatingSections = window->findChildren<QObject *>(
            QStringLiteral("celestina-menu-section")
        );
        QCOMPARE(floatingSections.size(), attachedSections.size());
        for (qsizetype index = 0; index < floatingSections.size(); ++index) {
            QObject *const section = floatingSections.at(index);
            auto *const item = qobject_cast<QQuickItem *>(section);
            QVERIFY2(item, qPrintable(component));
            QCOMPARE(
                QRectF(item->x(), item->y(), item->width(), item->height()),
                sectionGeometry.at(index)
            );
            QCOMPARE(section->property("materialRole").toInt(),
                     sectionRoles.at(index));
            QCOMPARE(section->property("materialStrength").toReal(),
                     sectionStrengths.at(index));
            QCOMPARE(section->property("materialTint").value<QColor>(),
                     sectionTints.at(index));
            QVERIFY2(!section->property("usesSilhouette").toBool(),
                     qPrintable(component));
        }

        // A compatibility caller that names an opener but not the continuous
        // bar edge retains the historical compact gap and floating shape.
        window->setProperty("anchoredFromPanel", true);
        window->setProperty("attachmentStartY", -1);
        QCOMPARE(
            window->property("anchorGap").toInt(),
            floatingGap
        );
        QVERIFY(!field->property("edgeAttachmentRequested").toBool());
    }

    // Connector length follows the stable card width. It must not follow
    // model-driven height, which can change while a menu is already visible.
    QVERIFY(attachedGapsByWidth.value(360)
            < attachedGapsByWidth.value(460));
    QVERIFY(attachedGapsByWidth.value(460)
            < attachedGapsByWidth.value(530));
    QVERIFY(attachedGapsByWidth.value(530)
            < attachedGapsByWidth.value(620));
}

void OverlayContractTest::aScaledOutputDrawsLargerWithoutMovingAnythingItStates()
{
    // A denser output draws the same surface larger. What must not change is
    // anything the design states: the card is the same number of units wide,
    // it still centres on its opener and its membrane still starts at the
    // bar's own edge. Only the last step to real pixels differs, so the whole
    // scene is scaled and every number inside it is left alone.
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    QQmlComponent overlay(&engine, sourceFor(QStringLiteral("ControlCentre")));
    QVERIFY2(overlay.isReady(), qPrintable(overlay.errorString()));

    constexpr double scale = 1.15;
    constexpr int outputWidth = 2560;
    constexpr int outputHeight = 1440;
    // What the host divides by that factor before handing it over: a 46-pixel
    // bar and a 34-pixel control on this output are the shell's own 40 and 30.
    const QRectF opener(900 / scale, 5 / scale, 34 / scale, 34 / scale);
    const QRectF anchor(906 / scale, 11 / scale, 21 / scale, 21 / scale);
    const qreal seam = 46 / scale;

    std::unique_ptr<QObject> root(overlay.createWithInitialProperties({
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(nullptr)},
        {QStringLiteral("shellScale"), scale},
        {QStringLiteral("anchoredFromPanel"), true},
        {QStringLiteral("openerRect"), opener},
        {QStringLiteral("attachmentAnchorRect"), anchor},
        {QStringLiteral("attachmentStartY"), seam},
    }));
    auto *window = qobject_cast<QQuickWindow *>(root.get());
    QVERIFY(window);
    window->resize(outputWidth, outputHeight);

    // The surface the card is placed inside is the output in the shell's own
    // units, not in output pixels.
    QCOMPARE(qRound(window->property("surfaceWidth").toReal()),
             qRound(outputWidth / scale));
    QCOMPARE(qRound(window->property("surfaceHeight").toReal()),
             qRound(outputHeight / scale));

    // The card keeps the width the design gives it, and still centres on the
    // control that opened it — both in those same units. The width is checked
    // against the same component drawn unscaled rather than against a number
    // written here, because the claim is that scaling does not change it.
    std::unique_ptr<QObject> unscaledRoot(overlay.createWithInitialProperties({
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(nullptr)},
    }));
    QVERIFY(unscaledRoot);
    const int cardWidth = window->property("cardWidth").toInt();
    QVERIFY(cardWidth > 0);
    QCOMPARE(cardWidth, unscaledRoot->property("cardWidth").toInt());
    QCOMPARE(window->property("cardX").toInt(),
             qRound(opener.x() + opener.width() / 2 - cardWidth / 2.0));
    QCOMPARE(window->property("cardY").toInt(),
             qRound(seam) + window->property("anchorGap").toInt());

    // And the scene really is scaled, so all of that reaches the output
    // larger. Its own size stays in unscaled units, which is what keeps the
    // numbers above meaningful.
    QQuickItem *const scene = window->findChild<QQuickItem *>(
        QStringLiteral("celestina-shell-scene")
    );
    QVERIFY(scene);
    QCOMPARE(scene->scale(), scale);
    QCOMPARE(qRound(scene->width()), qRound(outputWidth / scale));

    // The blur region is published in real window pixels, so it must come back
    // scaled even though everything that produced it was not.
    revealAllFields(window);
    QTRY_COMPARE(window->property("glassRegions").toList().size(), 1);
    const QRectF published = window->property("glassRegions")
                                 .toList().constFirst().toMap()
                                 .value(QStringLiteral("rect")).toRectF();
    QCOMPARE(qRound(published.top()), qRound(seam * scale));
    QVERIFY(published.width() > cardWidth);
}

void OverlayContractTest::anAttachedOverlayNeverReblursThePanelRows()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    QQmlComponent overlay(&engine, sourceFor(QStringLiteral("ControlCentre")));
    QVERIFY2(overlay.isReady(), qPrintable(overlay.errorString()));

    constexpr int outputWidth = 1280;
    constexpr int outputHeight = 768;
    constexpr int attachmentStartY = 40;
    const QRect opener(900, 5, 30, 30);
    const QRect attachmentAnchor(906, 11, 18, 18);
    std::unique_ptr<QObject> root(overlay.createWithInitialProperties({
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(nullptr)},
        {QStringLiteral("anchoredFromPanel"), true},
        {QStringLiteral("openerRect"), opener},
        {QStringLiteral("attachmentAnchorRect"), attachmentAnchor},
        {QStringLiteral("attachmentStartY"), attachmentStartY},
    }));
    auto *window = qobject_cast<QQuickWindow *>(root.get());
    QVERIFY(window);
    window->resize(outputWidth, outputHeight);

    const int bodyY = attachmentStartY
                      + window->property("anchorGap").toInt();
    QCOMPARE(window->property("anchorGap").toInt(), 32);
    QCOMPARE(bodyY, 72);
    QCOMPARE(window->property("cardY").toInt(), bodyY);
    QVERIFY(window->property("cardY").toInt() > attachmentStartY);
    QObject *const field = window->findChild<QObject *>(
        QStringLiteral("celestina-soft-menu-field")
    );
    QVERIFY(field);

    revealAllFields(window);

    QTRY_COMPARE(window->property("glassRegions").toList().size(), 1);
    QVariantMap published = window->property("glassRegions")
                                .toList().constFirst().toMap();
    QCOMPARE(qRound(published.value(QStringLiteral("rect")).toRectF().top()),
             attachmentStartY);
    const QVariantList polygon =
        published.value(QStringLiteral("polygon")).toList();
    QVERIFY(polygon.size() >= 3);
    qreal minimumY = std::numeric_limits<qreal>::max();
    for (const QVariant &point : polygon)
        minimumY = qMin(minimumY, point.toPointF().y());
    QCOMPARE(qRound(minimumY), attachmentStartY);
    qreal upperLeft = std::numeric_limits<qreal>::max();
    qreal upperRight = std::numeric_limits<qreal>::lowest();
    for (const QVariant &value : polygon) {
        const QPointF point = value.toPointF();
        if (qAbs(point.y() - minimumY) < 0.001) {
            upperLeft = qMin(upperLeft, point.x());
            upperRight = qMax(upperRight, point.x());
        }
    }
    // The published seam row carries only the narrow droplet mouth centred
    // on the clicked glyph; the panel rows beside it are never re-covered.
    QVERIFY(upperRight - upperLeft
            < window->property("cardWidth").toInt() * 0.25);
    QCOMPARE(qRound((upperLeft + upperRight) / 2),
             attachmentAnchor.x() + attachmentAnchor.width() / 2);
    QCOMPARE(
        qRound(window->property("cardX").toReal()
               + field->property("attachmentWaistCenterAtBody").toReal()),
        attachmentAnchor.x() + attachmentAnchor.width() / 2
    );

    // Moving the complete field leaves its local polygon unchanged. Its
    // published window coordinates and icon-targeted waist must nevertheless
    // follow the opener and anchor together.
    const QRectF firstRect =
        published.value(QStringLiteral("rect")).toRectF();
    window->setProperty("openerRect", opener.translated(-100, 0));
    window->setProperty(
        "attachmentAnchorRect", attachmentAnchor.translated(-100, 0)
    );
    QTRY_COMPARE(window->property("cardX").toInt(),
                 qRound(opener.x() - 100 + opener.width() / 2.0
                        - window->property("cardWidth").toInt() / 2.0));
    QTRY_VERIFY(
        qRound(window->property("glassRegions").toList().constFirst().toMap()
                   .value(QStringLiteral("rect")).toRectF().left())
        == qRound(firstRect.left()) - 100
    );
    QTRY_COMPARE(
        qRound(window->property("cardX").toReal()
               + field->property("attachmentWaistCenterAtBody").toReal()),
        attachmentAnchor.x() - 100 + attachmentAnchor.width() / 2
    );
}

void OverlayContractTest::sessionCardGrowthDoesNotResizeItsOutputSurface()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    QQmlComponent overlay(&engine, sourceFor(QStringLiteral("SessionMenu")));
    QVERIFY2(overlay.isReady(), qPrintable(overlay.errorString()));

    const QRect opener(1000, 5, 28, 28);
    const QRect attachmentAnchor(1005, 10, 18, 18);
    constexpr int attachmentStartY = 40;
    std::unique_ptr<QObject> root(overlay.createWithInitialProperties({
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("shellSource"), QVariant::fromValue<QObject *>(nullptr)},
        {QStringLiteral("anchoredFromPanel"), true},
        {QStringLiteral("openerRect"), opener},
        {QStringLiteral("attachmentAnchorRect"), attachmentAnchor},
        {QStringLiteral("attachmentStartY"), attachmentStartY},
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
    const int expectedY = attachmentStartY
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

    // Compatibility callers without a panel edge retain opener-relative
    // placement.
    QCOMPARE(panelPopupBodyOrigin(local, 530, 8).y(), 46);
    // A real panel route measures the connector from the lower edge of the
    // continuous bar backdrop, independently of the opener's own height.
    QCOMPARE(panelPopupBodyOrigin(local, 530, 24, 40).y(), 64);
}

void OverlayContractTest::attachedCarriersBeginBelowThePanelWithoutChangingInteraction()
{
    constexpr int physicalPanelHeight = 46;
    constexpr double shellScale = 1.15;

    const QuietSurfaceGeometry scaledGeometry = attachedQuietGeometry(
        QSizeF(3840.0 / shellScale, 2160.0 / shellScale),
        physicalPanelHeight / shellScale,
        QRectF(3300.0 / shellScale, 5.0 / shellScale,
               34.0 / shellScale, 34.0 / shellScale),
        QRectF(3306.0 / shellScale, 11.0 / shellScale,
               21.0 / shellScale, 21.0 / shellScale),
        QSizeF(380, 240),
        16,
        32
    );
    QVERIFY(scaledGeometry.valid);
    // `surface.y()` is already in unscaled QML units. The one conversion at
    // the layer-shell boundary must recover the real 46-pixel panel, not apply
    // the per-output factor a second time.
    QCOMPARE(
        scaledGeometry.topInsetInOutputUnits(shellScale),
        physicalPanelHeight
    );

    auto *interactiveWindow = new QQuickWindow;
    interactiveWindow->resize(460, 520);
    OverlaySurface interactive(
        OverlaySurface::Placement::Centered,
        QStringLiteral("celestina-overlay")
    );
    QVERIFY(interactive.open(
        interactiveWindow,
        nullptr,
        OverlaySurface::Placement::Centered,
        physicalPanelHeight
    ));
    auto *interactiveLayer = LayerShellQt::Window::get(interactiveWindow);
    QVERIFY(interactiveLayer);
    QCOMPARE(
        interactiveLayer->margins(),
        QMargins(0, physicalPanelHeight, 0, 0)
    );
    QCOMPARE(
        interactiveLayer->keyboardInteractivity(),
        LayerShellQt::Window::KeyboardInteractivityOnDemand
    );
    QVERIFY(!interactiveWindow->flags().testFlag(Qt::WindowDoesNotAcceptFocus));

    auto *quietWindow = new QQuickWindow;
    quietWindow->resize(380, 240);
    OverlaySurface quiet(
        OverlaySurface::Placement::Corner,
        QStringLiteral("celestina-toasts")
    );
    QVERIFY(quiet.open(
        quietWindow,
        nullptr,
        OverlaySurface::Placement::AttachedTopRight,
        physicalPanelHeight
    ));
    auto *quietLayer = LayerShellQt::Window::get(quietWindow);
    QVERIFY(quietLayer);
    QCOMPARE(
        quietLayer->margins(),
        QMargins(0, physicalPanelHeight, 0, 0)
    );
    QCOMPARE(
        quietLayer->keyboardInteractivity(),
        LayerShellQt::Window::KeyboardInteractivityNone
    );
    QVERIFY(quietWindow->flags().testFlag(Qt::WindowDoesNotAcceptFocus));

    // A keybind/floating overlay still begins at the output origin.
    auto *floatingWindow = new QQuickWindow;
    floatingWindow->resize(460, 520);
    OverlaySurface floating(
        OverlaySurface::Placement::Centered,
        QStringLiteral("celestina-overlay")
    );
    QVERIFY(floating.open(floatingWindow, nullptr));
    auto *floatingLayer = LayerShellQt::Window::get(floatingWindow);
    QVERIFY(floatingLayer);
    QCOMPARE(floatingLayer->margins(), QMargins());
    QCOMPARE(
        floatingLayer->keyboardInteractivity(),
        LayerShellQt::Window::KeyboardInteractivityOnDemand
    );
}

void OverlayContractTest::aPanelAttachedNotificationUsesCarrierLocalGeometryAndDismissesOutside()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    QQmlComponent overlay(
        &engine, sourceFor(QStringLiteral("NotificationCenter")));
    QVERIFY2(overlay.isReady(), qPrintable(overlay.errorString()));

    constexpr double shellScale = 1.15;
    constexpr int physicalPanelHeight = 46;
    const QPointF outputOrigin(1920, 0);
    const QPointF carrierOrigin(0, physicalPanelHeight);
    const QRectF globalOpener(5220, 5, 34, 34);
    const QRectF globalIcon(5226, 11, 21, 21);
    const QRectF localOpener = panelAttachmentRectOnCarrier(
        globalOpener, outputOrigin, carrierOrigin, shellScale);
    const QRectF localIcon = panelAttachmentRectOnCarrier(
        globalIcon, outputOrigin, carrierOrigin, shellScale);

    // The physical carrier starts at output y=46. Its QML sees that seam as
    // zero and receives the panel rectangles translated and unscaled into the
    // same local space, including on the author's 1.15-scale 4K output.
    QVERIFY(localOpener.y() < 0);
    QCOMPARE(qRound(localOpener.x() * shellScale), 3300);
    std::unique_ptr<QObject> root(overlay.createWithInitialProperties({
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("providerSource"),
         QVariant::fromValue<QObject *>(nullptr)},
        {QStringLiteral("shellScale"), shellScale},
        {QStringLiteral("anchoredFromPanel"), true},
        {QStringLiteral("openerRect"), localOpener},
        {QStringLiteral("attachmentAnchorRect"), localIcon},
        {QStringLiteral("attachmentStartY"), 0},
    }));
    auto *window = qobject_cast<QQuickWindow *>(root.get());
    QVERIFY(window);
    window->resize(3840, 2160 - physicalPanelHeight);

    QCOMPARE(window->property("cardY").toInt(),
             window->property("anchorGap").toInt());
    QQuickItem *const body = window->findChild<QQuickItem *>(
        QStringLiteral("celestina-compositor-glass-region")
    );
    QVERIFY(body);
    QCOMPARE(qRound(body->mapToItem(window->contentItem(), 0, 0).y()), 0);

    window->show();
    QVERIFY(QTest::qWaitForWindowExposed(window));
    QSignalSpy dismissed(window, SIGNAL(dismissed()));
    QVERIFY(dismissed.isValid());
    // The carrier still covers every pixel below the bar, so an outside click
    // remains the overlay's to dismiss even though the bar is not in its window.
    QTest::mouseClick(window, Qt::LeftButton, {}, QPoint(4, 4));
    QCOMPARE(dismissed.count(), 1);
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

// The quiet surfaces' placement arithmetic, pinned without a compositor: the
// card centres on its control clamped inside the output, and the window spans
// from the leftmost thing it must contain — card or icon — to the right edge,
// because a mouth outside its own window is a mouth the compositor clips.
void OverlayContractTest::quietGeometryCentresTheCardAndReachesTheIcon()
{
    const QSizeF output(1920, 1080);
    const qreal bar = 40;
    const QSizeF card(260, 96);

    // A control near the middle of the right cluster: the card centres on it.
    const QRectF opener(1500, 5, 60, 30);
    const QRectF icon(1520, 11, 18, 18);
    QuietSurfaceGeometry centred = attachedQuietGeometry(
        output, bar, opener, icon, card, 8, 96);
    QVERIFY(centred.valid);
    QCOMPARE(centred.card.x(), 1500 + 30 - 130);
    QCOMPARE(centred.card.y(), bar);
    QCOMPARE(centred.surface.y(), bar);
    QCOMPARE(centred.surface.right(), output.width());
    // The surface contains both the whole card and the icon's mouth.
    QVERIFY(centred.surface.left() <= centred.card.left());
    QVERIFY(centred.surface.left() <= icon.left());
    QCOMPARE(centred.onSurface(centred.card).y(), 0.0);
    QCOMPARE(centred.onSurface(icon).y(), icon.y() - bar);

    // A control at the very edge: the card clamps inside the output instead
    // of overflowing it, exactly as a menu's card would.
    const QRectF edgeOpener(1880, 5, 36, 30);
    const QRectF edgeIcon(1889, 11, 18, 18);
    QuietSurfaceGeometry clamped = attachedQuietGeometry(
        output, bar, edgeOpener, edgeIcon, card, 8, 96);
    QVERIFY(clamped.valid);
    QCOMPARE(clamped.card.right(), output.width() - 8);

    // No opener is no geometry, never a guess.
    QVERIFY(!attachedQuietGeometry(
        output, bar, QRectF(), edgeIcon, card, 8, 96).valid);
}

// The zone question is the card's own landing rectangle, travel included: a
// menu under the card occupies it, a menu elsewhere does not, and an empty
// list is a free zone.
void OverlayContractTest::quietZoneYieldsOnlyToARealIntrusion()
{
    const QRectF landing(1400, 40, 260, 192);

    QVERIFY(!quietZoneOccupied(landing, {}));
    QVERIFY(!quietZoneOccupied(landing, {QRectF()}));
    // The calendar's card, centred on the output: nowhere near the corner.
    QVERIFY(!quietZoneOccupied(landing, {QRectF(700, 60, 360, 420)}));
    // The audio menu, dropped from the same cluster: an intrusion.
    QVERIFY(quietZoneOccupied(landing, {QRectF(1450, 60, 360, 420)}));
    // Something crossing only the connector's travel still counts: the drop
    // would fall through it.
    QVERIFY(quietZoneOccupied(landing, {QRectF(1500, 45, 100, 20)}));
}

// A level being changed from inside its own open menu needs no display; every
// other open menu changes nothing about a level, so its display still shows.
void OverlayContractTest::aLevelChangedFromItsOwnMenuRaisesNoDisplay()
{
    QVERIFY(osdSuppressedByOpenMenu(
        QStringLiteral("volume"), QStringLiteral("audio")));
    QVERIFY(osdSuppressedByOpenMenu(
        QStringLiteral("microphone"), QStringLiteral("audio")));
    QVERIFY(osdSuppressedByOpenMenu(
        QStringLiteral("brightness"), QStringLiteral("brightness")));
    // The other menu's display is not suppressed by a menu about something
    // else, and no open menu at all suppresses nothing.
    QVERIFY(!osdSuppressedByOpenMenu(
        QStringLiteral("brightness"), QStringLiteral("audio")));
    QVERIFY(!osdSuppressedByOpenMenu(
        QStringLiteral("volume"), QStringLiteral("brightness")));
    QVERIFY(!osdSuppressedByOpenMenu(
        QStringLiteral("volume"), QString()));
    QVERIFY(!osdSuppressedByOpenMenu(
        QStringLiteral("volume"), QStringLiteral("calendar")));
}

QTEST_MAIN(OverlayContractTest)
#include "overlaycontract_test.moc"
