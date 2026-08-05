#include <QtTest>

#include <QQmlComponent>
#include <QQmlEngine>
#include <QScreen>
#include <QUrl>
#include <QVariantMap>
#include <QWindow>

#include "overlaysurface.h"
#include "panelmenucontroller.h"
#include "panelmenusurface.h"
#include "surfacemanager.h"
#include "wallpapermanager.h"

// Surface *mechanics* only. A platform without a compositor still creates,
// configures, shows, hides and destroys windows, so the shared recipe's window
// side and the menu surface's lifetime are provable here. What the compositor
// decides — placement, keyboard, dismissal, focus return — is not, and was
// answered on real Niri in R0-E.
class SurfaceManagerTest final : public QObject
{
    Q_OBJECT

private slots:
    void aPlatformWithoutLayerShellIsNamedAsSuch();
    void theRecipeRefusesNothingToMap();
    void aPanelSurfaceKeepsItsHeightAndRefusesFocus();
    void aMenuSurfaceTakesFocusAndItsContentSize();
    void theMenuRefusesToOpenTwiceAndSurvivesReopening();
    void theMenuReportsAndCleansUpAnExternalDismissal();
    void aClosedMenuLeavesNoWindowBehind();
    void theMenuIsOnUnlessTheEnvironmentTurnsItOff();
    void theMenuContentLoadsAndFitsItsSurface();
    void theMenuSurfaceIsBigEnoughToClickEveryItem();

    void anOverlaySurfaceCentersWithoutAnAnchorAndTakesFocus();
    void theOverlayRefusesToOpenTwiceAndSurvivesReopening();
    void theOverlayReportsAndCleansUpAnExternalDismissal();
    void aClosedOverlayLeavesNoWindowBehind();
    void theLauncherAndClipboardOverlaysLoadAndMap();
    void aCornerSurfaceSitsUnderThePanelAndRefusesFocus();
    void aReadoutSurfaceSitsLowAndCentredSoItNeverCoversAToast();
    void aWallpaperCoversItsOutputAndReservesNothing();

private:
    QWindow *makePanel()
    {
        auto *panel = new QWindow;
        panel->setGeometry(0, 0, 800, 40);
        m_owned.append(panel);
        return panel;
    }

    static QVariantMap workspace(int index, const QString &label, bool active)
    {
        return QVariantMap {
            {QStringLiteral("index"), index},
            {QStringLiteral("label"), label},
            {QStringLiteral("output"), QStringLiteral("DP-1")},
            {QStringLiteral("active"), active},
            {QStringLiteral("focused"), active},
            {QStringLiteral("urgent"), false},
            {QStringLiteral("activeWindowTitle"), QString()},
            {QStringLiteral("requestState"), QString()},
        };
    }

    static QWindow *makeContent()
    {
        auto *content = new QWindow;
        content->setGeometry(0, 0, 232, 96);
        return content;
    }

    QList<QWindow *> m_owned;
};

// The shell must know the difference between "no layer shell" and "a layer
// shell that said no": off Wayland, Qt maps an ordinary window and LayerShellQt
// only logs, so the host has to refuse before it claims a panel.
void SurfaceManagerTest::aPlatformWithoutLayerShellIsNamedAsSuch()
{
    QCOMPARE(layerShellSupport(QStringLiteral("wayland")), LayerShellSupport::Available);
    QCOMPARE(layerShellSupport(QStringLiteral("wayland-egl")), LayerShellSupport::Available);
    QCOMPARE(layerShellSupport(QStringLiteral("offscreen")), LayerShellSupport::Headless);
    QCOMPARE(layerShellSupport(QStringLiteral("xcb")), LayerShellSupport::Unavailable);
    QCOMPARE(layerShellSupport(QStringLiteral("minimal")), LayerShellSupport::Unavailable);
    QCOMPARE(layerShellSupport(QString()), LayerShellSupport::Unavailable);
}

void SurfaceManagerTest::theRecipeRefusesNothingToMap()
{
    QVERIFY(!mapLayerSurface(nullptr, LayerSurfaceSpec()));
}

void SurfaceManagerTest::aPanelSurfaceKeepsItsHeightAndRefusesFocus()
{
    QWindow *const panel = makePanel();
    LayerSurfaceSpec spec;
    spec.scope = QStringLiteral("celestina-panel");
    spec.screen = panel->screen();
    // Faithful to the panel: a zero width is only legal because the surface is
    // anchored to both side edges. Dropping those anchors is a protocol error
    // ("width 0 requested without setting left and right anchors"), which is
    // why a surface that names no anchors must never leave its size to the
    // compositor.
    auto anchors = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    anchors |= LayerShellQt::Window::AnchorLeft;
    anchors |= LayerShellQt::Window::AnchorRight;
    spec.anchors = anchors;
    spec.desiredSize = QSize(0, 40);
    spec.exclusiveZone = 40;

    QVERIFY(mapLayerSurface(panel, spec));
    QCOMPARE(panel->screen(), spec.screen);
    QCOMPARE(panel->height(), 40);
    QVERIFY(panel->flags().testFlag(Qt::FramelessWindowHint));
    QVERIFY(panel->flags().testFlag(Qt::WindowDoesNotAcceptFocus));
}

void SurfaceManagerTest::aMenuSurfaceTakesFocusAndItsContentSize()
{
    QWindow *const panel = makePanel();
    QWindow *const content = makeContent();
    const int contentHeight = content->height();

    PanelMenuSurface surface;
    QVERIFY(surface.open(content, panel, QPoint(120, 40)));
    QVERIFY(surface.isOpen());
    QCOMPARE(surface.window(), content);
    QCOMPARE(content->screen(), panel->screen());
    // A layer surface is placed by the compositor from its anchors and
    // margins, so it is never a transient child of the panel.
    QCOMPARE(content->transientParent(), nullptr);
    QVERIFY(!content->flags().testFlag(Qt::WindowDoesNotAcceptFocus));
    // The menu describes no size: its content keeps the one it asked for.
    QCOMPARE(content->height(), contentHeight);
}

void SurfaceManagerTest::theMenuRefusesToOpenTwiceAndSurvivesReopening()
{
    QWindow *const panel = makePanel();

    PanelMenuSurface surface;
    QVERIFY(surface.open(makeContent(), panel, QPoint(10, 40)));
    QWindow *const second = makeContent();
    QVERIFY(!surface.open(second, panel, QPoint(20, 40)));
    // A refused open never adopts the window, so its caller still owns it.
    delete second;
    surface.close();
    QVERIFY(!surface.isOpen());
    QVERIFY(surface.open(makeContent(), panel, QPoint(30, 40)));
}

void SurfaceManagerTest::theMenuReportsAndCleansUpAnExternalDismissal()
{
    QWindow *const panel = makePanel();

    PanelMenuSurface surface;
    QSignalSpy dismissed(&surface, &PanelMenuSurface::dismissed);
    QWindow *const content = makeContent();
    QVERIFY(surface.open(content, panel, QPoint(10, 40)));
    // What a compositor dismissal looks like from this side.
    content->hide();
    QCOMPARE(dismissed.count(), 1);
    QVERIFY(!surface.isOpen());
}

void SurfaceManagerTest::aClosedMenuLeavesNoWindowBehind()
{
    QWindow *const panel = makePanel();
    QPointer<QWindow> tracked;

    {
        PanelMenuSurface surface;
        QWindow *const content = makeContent();
        tracked = content;
        QVERIFY(surface.open(content, panel, QPoint(10, 40)));
    }
    // Destruction closes, and closing deletes the adopted window.
    QTRY_VERIFY(tracked.isNull());

    {
        PanelMenuSurface surface;
        QWindow *const content = makeContent();
        tracked = content;
        QVERIFY(surface.open(content, panel, QPoint(10, 40)));
        surface.close();
    }
    QTRY_VERIFY(tracked.isNull());
}

void SurfaceManagerTest::theMenuIsOnUnlessTheEnvironmentTurnsItOff()
{
    qunsetenv("CELESTINA_PANEL_MENU");
    QVERIFY(PanelMenuController::enabledByEnvironment());

    qputenv("CELESTINA_PANEL_MENU", "0");
    QVERIFY(!PanelMenuController::enabledByEnvironment());

    qputenv("CELESTINA_PANEL_MENU", "False");
    QVERIFY(!PanelMenuController::enabledByEnvironment());

    qputenv("CELESTINA_PANEL_MENU", "1");
    QVERIFY(PanelMenuController::enabledByEnvironment());

    // An unreadable value is not a request to remove a working menu.
    qputenv("CELESTINA_PANEL_MENU", "perhaps");
    QVERIFY(PanelMenuController::enabledByEnvironment());
    qunsetenv("CELESTINA_PANEL_MENU");
}

// The real menu file, loaded from source the way the host loads it: this is
// what proves its imports, the shared GlassContextMenu and the window contract
// actually resolve, and that the content the surface adopts is a window.
void SurfaceManagerTest::theMenuContentLoadsAndFitsItsSurface()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    QQmlComponent component(
        &engine,
        QUrl::fromLocalFile(QStringLiteral(CELESTINA_QML_DIR "/PanelMenu.qml"))
    );
    QVERIFY2(component.isReady(), qPrintable(component.errorString()));

    const QVariantMap properties {
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("workspaces"),
         QVariantList {
             workspace(1, QStringLiteral("web"), true),
             workspace(2, QStringLiteral("2"), false),
         }},
    };
    QObject *root = component.createWithInitialProperties(properties);
    QVERIFY2(root, qPrintable(component.errorString()));

    auto *content = qobject_cast<QWindow *>(root);
    QVERIFY(content);
    QVERIFY(content->metaObject()->indexOfSignal("activated(QString,int)") >= 0);
    QVERIFY(content->metaObject()->indexOfSignal("dismissed()") >= 0);

    PanelMenuSurface surface;
    QVERIFY(surface.open(content, makePanel(), QPoint(10, 40)));
}

// The bug this pins down: the window sized itself to the laid-out menu while
// the menu fitted itself to the window, so both shrank one margin per pass
// until the surface was a sliver — and every click in that sliver landed on
// the first item. A menu must be at least as wide as its card and tall enough
// for every row it offers.
void SurfaceManagerTest::theMenuSurfaceIsBigEnoughToClickEveryItem()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    QQmlComponent component(
        &engine,
        QUrl::fromLocalFile(QStringLiteral(CELESTINA_QML_DIR "/PanelMenu.qml"))
    );
    QVERIFY2(component.isReady(), qPrintable(component.errorString()));

    QVariantList workspaces;
    for (int index = 1; index <= 4; ++index)
        workspaces.append(workspace(index, QString::number(index), index == 1));

    QObject *root = component.createWithInitialProperties({
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("workspaces"), workspaces},
    });
    QVERIFY2(root, qPrintable(component.errorString()));
    auto *content = qobject_cast<QWindow *>(root);
    QVERIFY(content);

    const int inset = content->property("shadowMargin").toInt();
    QVERIFY(inset > 0);
    // Four rows of a real control, plus the shadow room on both sides. The
    // exact metrics belong to the shared style; the floor here is that the
    // surface cannot collapse below one usable row per workspace.
    QVERIFY2(
        content->height() >= 4 * 24 + inset * 2,
        qPrintable(QStringLiteral("menu height %1").arg(content->height()))
    );
    QVERIFY(content->width() > inset * 2);

    // And it stays that size once the menu has opened and laid itself out.
    content->show();
    const QSize mapped = content->size();
    QTest::qWait(200);
    QCOMPARE(content->size(), mapped);
}

// Unlike the panel's menu, an overlay is opened from a keybind rather than a
// click: there is no anchor point, so the recipe leaves `anchors` empty for
// the compositor to read as "center this on its output" (R2's launcher and
// clipboard history).
void SurfaceManagerTest::anOverlaySurfaceCentersWithoutAnAnchorAndTakesFocus()
{
    QWindow *const content = makeContent();
    const QSize contentSize = content->size();

    OverlaySurface surface(OverlaySurface::Placement::Centered);
    QVERIFY(surface.open(content, nullptr));
    QVERIFY(surface.isOpen());
    QCOMPARE(surface.window(), content);
    QCOMPARE(content->transientParent(), nullptr);
    QVERIFY(!content->flags().testFlag(Qt::WindowDoesNotAcceptFocus));
    // The overlay describes no size beyond its content's own.
    QCOMPARE(content->size(), contentSize);

    auto *layerWindow = LayerShellQt::Window::get(content);
    QVERIFY(layerWindow);
    QCOMPARE(layerWindow->anchors(), LayerShellQt::Window::Anchors());
    QCOMPARE(layerWindow->keyboardInteractivity(),
             LayerShellQt::Window::KeyboardInteractivityOnDemand);
}

// Toasts share the overlay's mechanics and nothing else: pinned to the panel's
// own corner, never activated and never given the keyboard, because they are
// read rather than used.
void SurfaceManagerTest::aCornerSurfaceSitsUnderThePanelAndRefusesFocus()
{
    QWindow *const content = makeContent();
    const QSize contentSize = content->size();

    OverlaySurface surface(OverlaySurface::Placement::Corner);
    QVERIFY(surface.open(content, nullptr));
    QCOMPARE(content->size(), contentSize);
    QVERIFY(content->flags().testFlag(Qt::WindowDoesNotAcceptFocus));

    auto *layerWindow = LayerShellQt::Window::get(content);
    QVERIFY(layerWindow);
    auto expected = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    expected |= LayerShellQt::Window::AnchorRight;
    QCOMPARE(layerWindow->anchors(), expected);
    QCOMPARE(layerWindow->keyboardInteractivity(),
             LayerShellQt::Window::KeyboardInteractivityNone);
    QCOMPARE(layerWindow->exclusionZone(), 0);
}

// The readout deliberately does not share that corner: a volume key pressed
// while a notification is up must not paint over it.
void SurfaceManagerTest::aReadoutSurfaceSitsLowAndCentredSoItNeverCoversAToast()
{
    QWindow *const content = makeContent();

    OverlaySurface surface(OverlaySurface::Placement::Readout);
    QVERIFY(surface.open(content, nullptr));
    QVERIFY(content->flags().testFlag(Qt::WindowDoesNotAcceptFocus));

    auto *layerWindow = LayerShellQt::Window::get(content);
    QVERIFY(layerWindow);
    // Anchored to the bottom only: one anchor with no opposing pair is what
    // centres it horizontally.
    QCOMPARE(
        layerWindow->anchors(),
        LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorBottom)
    );
    QCOMPARE(layerWindow->keyboardInteractivity(),
             LayerShellQt::Window::KeyboardInteractivityNone);
}

// The background everything else sits on: anchored on all four edges so the
// compositor sizes it, reserving nothing, and never taking focus or the
// keyboard. Offscreen this proves the description only — never that a
// compositor honoured it.
void SurfaceManagerTest::aWallpaperCoversItsOutputAndReservesNothing()
{
    QWindow *const content = makeContent();
    QVERIFY(mapLayerSurface(content, wallpaperSurfaceSpec(nullptr)));

    auto *layerWindow = LayerShellQt::Window::get(content);
    QVERIFY(layerWindow);
    QCOMPARE(layerWindow->layer(), LayerShellQt::Window::LayerBackground);
    // Anchored on all four edges: the compositor sizes it to the output.
    auto expected = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    expected |= LayerShellQt::Window::AnchorBottom;
    expected |= LayerShellQt::Window::AnchorLeft;
    expected |= LayerShellQt::Window::AnchorRight;
    QCOMPARE(layerWindow->anchors(), expected);
    // A wallpaper reserves nothing; it is what everything else sits on.
    QCOMPARE(layerWindow->exclusionZone(), -1);
    QCOMPARE(layerWindow->keyboardInteractivity(),
             LayerShellQt::Window::KeyboardInteractivityNone);
    QVERIFY(content->flags().testFlag(Qt::WindowDoesNotAcceptFocus));
    content->hide();
    content->deleteLater();
}

void SurfaceManagerTest::theOverlayRefusesToOpenTwiceAndSurvivesReopening()
{
    OverlaySurface surface(OverlaySurface::Placement::Centered);
    QVERIFY(surface.open(makeContent(), nullptr));
    QWindow *const second = makeContent();
    QVERIFY(!surface.open(second, nullptr));
    // A refused open never adopts the window, so its caller still owns it.
    delete second;
    surface.close();
    QVERIFY(!surface.isOpen());
    QVERIFY(surface.open(makeContent(), nullptr));
}

void SurfaceManagerTest::theOverlayReportsAndCleansUpAnExternalDismissal()
{
    OverlaySurface surface(OverlaySurface::Placement::Centered);
    QSignalSpy dismissed(&surface, &OverlaySurface::dismissed);
    QWindow *const content = makeContent();
    QVERIFY(surface.open(content, nullptr));
    // What a compositor dismissal looks like from this side.
    content->hide();
    QCOMPARE(dismissed.count(), 1);
    QVERIFY(!surface.isOpen());
}

void SurfaceManagerTest::aClosedOverlayLeavesNoWindowBehind()
{
    QPointer<QWindow> tracked;

    {
        OverlaySurface surface(OverlaySurface::Placement::Centered);
        QWindow *const content = makeContent();
        tracked = content;
        QVERIFY(surface.open(content, nullptr));
    }
    QTRY_VERIFY(tracked.isNull());

    {
        OverlaySurface surface(OverlaySurface::Placement::Centered);
        QWindow *const content = makeContent();
        tracked = content;
        QVERIFY(surface.open(content, nullptr));
        surface.close();
    }
    QTRY_VERIFY(tracked.isNull());
}

// The real overlay files, loaded from source the way the host loads them —
// `OverlayController` itself loads through the compiled `CelestinaDesktop`
// module rather than a file path, which only the `celestina` binary carries;
// this proves the QML content and window contract on their own, the same
// boundary `theMenuContentLoadsAndFitsItsSurface` already draws for the menu.
// `providerSource` is left null, exercising the same "provider unavailable"
// path a real session hits while its helper is still starting.
void SurfaceManagerTest::theLauncherAndClipboardOverlaysLoadAndMap()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &fileName : {
             QStringLiteral("LauncherOverlay.qml"),
             QStringLiteral("ClipboardOverlay.qml"),
         }) {
        QQmlComponent component(
            &engine,
            QUrl::fromLocalFile(QStringLiteral(CELESTINA_QML_DIR "/") + fileName)
        );
        QVERIFY2(component.isReady(), qPrintable(component.errorString()));

        QObject *root = component.createWithInitialProperties({
            {QStringLiteral("reducedMotion"), true},
            {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(nullptr)},
        });
        QVERIFY2(root, qPrintable(component.errorString()));

        auto *content = qobject_cast<QWindow *>(root);
        QVERIFY2(content, qPrintable(fileName));
        QVERIFY2(content->metaObject()->indexOfSignal("dismissed()") >= 0, qPrintable(fileName));

        OverlaySurface surface(OverlaySurface::Placement::Centered);
        QVERIFY2(surface.open(content, nullptr), qPrintable(fileName));
    }
}

QTEST_MAIN(SurfaceManagerTest)

#include "surfacemanager_test.moc"
