#include <QtTest>
#include <QSignalSpy>

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
    void aMenuKeepsTheInvokingControlsAnchor();
    void theMenuContentLoadsAndFitsItsSurface();
    void theMenuSurfaceIsBigEnoughToClickEveryItem();
    void theMapListsEveryWindowAndWalksThemWithTheKeyboard();
    void theMapSurvivesAWorkspaceWithNoMapAtAll();

    void anOverlaySurfaceCoversItsOutputAndTakesFocus();
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

public:
    static QVariantMap workspaceRow(int index)
    {
        return workspace(index, QString::number(index), false);
    }

private:
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
            // A workspace whose helper published no map at all: the card must
            // still build, because an older helper is a valid producer.
            {QStringLiteral("map"), QVariantMap {}},
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
    QVERIFY(surface.open(content, panel));
    QVERIFY(surface.isOpen());
    QCOMPARE(surface.window(), content);
    QCOMPARE(content->screen(), panel->screen());
    // A layer surface is placed by the compositor from its anchors and
    // margins, so it is never a transient child of the panel.
    QCOMPARE(content->transientParent(), nullptr);
    QVERIFY(!content->flags().testFlag(Qt::WindowDoesNotAcceptFocus));
    // The surface covers the output, which the compositor sizes; offscreen
    // nothing configures it, so the content keeps the size it asked for.
    QCOMPARE(content->height(), contentHeight);

    // And it covers it the same way the focused overlays do, for the same
    // reason: a click outside a menu must reach the menu. Where the card sits
    // inside the surface is the content's own business now.
    auto *layerWindow = LayerShellQt::Window::get(content);
    QVERIFY(layerWindow);
    auto expected = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    expected |= LayerShellQt::Window::AnchorBottom;
    expected |= LayerShellQt::Window::AnchorLeft;
    expected |= LayerShellQt::Window::AnchorRight;
    QCOMPARE(layerWindow->anchors(), expected);
    QCOMPARE(layerWindow->desiredSize(), QSize(0, 0));
    // Zero reserves nothing for the menu itself while still asking the
    // compositor to place it after existing exclusive surfaces.
    QCOMPARE(layerWindow->exclusionZone(), 0);
    QCOMPARE(layerWindow->margins(), QMargins());
}

void SurfaceManagerTest::theMenuRefusesToOpenTwiceAndSurvivesReopening()
{
    QWindow *const panel = makePanel();

    PanelMenuSurface surface;
    QVERIFY(surface.open(makeContent(), panel));
    QWindow *const second = makeContent();
    QVERIFY(!surface.open(second, panel));
    // A refused open never adopts the window, so its caller still owns it.
    delete second;
    surface.close();
    QVERIFY(!surface.isOpen());
    QVERIFY(surface.open(makeContent(), panel));
}

void SurfaceManagerTest::theMenuReportsAndCleansUpAnExternalDismissal()
{
    QWindow *const panel = makePanel();

    PanelMenuSurface surface;
    QSignalSpy dismissed(&surface, &PanelMenuSurface::dismissed);
    QWindow *const content = makeContent();
    QVERIFY(surface.open(content, panel));
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
        QVERIFY(surface.open(content, panel));
    }
    // Destruction closes, and closing deletes the adopted window.
    QTRY_VERIFY(tracked.isNull());

    {
        PanelMenuSurface surface;
        QWindow *const content = makeContent();
        tracked = content;
        QVERIFY(surface.open(content, panel));
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

void SurfaceManagerTest::aMenuKeepsTheInvokingControlsAnchor()
{
    const QPoint outputOrigin(1920, 120);
    const int shadowMargin = 18;

    QCOMPARE(
        panelMenuOrigin(QPoint(2380, 164), outputOrigin, shadowMargin),
        QPoint(442, -18)
    );
    // Horizontal movement follows the invoking control. A nominal global Y
    // cannot move the card because Wayland does not expose the compositor's
    // actual stacked-panel Y; the menu surface's exclusive-zone-aware top does.
    QCOMPARE(
        panelMenuOrigin(QPoint(2512, 197), outputOrigin, shadowMargin),
        QPoint(574, -18)
    );
}


// One published window, in the shape the host decodes onto a workspace.
static QVariantMap mapWindow(const QString &id, const QString &title, const QString &appId)
{
    return QVariantMap {
        {QStringLiteral("id"), id},
        {QStringLiteral("title"), title},
        {QStringLiteral("appId"), appId},
        {QStringLiteral("heightShare"), 1.0},
        {QStringLiteral("focused"), false},
        {QStringLiteral("floating"), false},
        {QStringLiteral("urgent"), false},
    };
}

// A workspace holding two windows in two columns, plus one floating.
static QVariantMap workspaceHolding(int index)
{
    QVariantMap first;
    first.insert(QStringLiteral("widthShare"), 0.5);
    first.insert(
        QStringLiteral("windows"),
        QVariantList {mapWindow(QStringLiteral("11"), QStringLiteral("Left"), QStringLiteral("kitty"))}
    );
    QVariantMap second;
    second.insert(QStringLiteral("widthShare"), 0.5);
    second.insert(
        QStringLiteral("windows"),
        QVariantList {mapWindow(QStringLiteral("12"), QStringLiteral("Right"), QStringLiteral("kitty"))}
    );

    QVariantMap map;
    map.insert(QStringLiteral("columns"), QVariantList {first, second});
    map.insert(
        QStringLiteral("floating"),
        QVariantList {mapWindow(QStringLiteral("13"), QStringLiteral("Floater"), QStringLiteral("kitty"))}
    );
    map.insert(QStringLiteral("hidden"), 0);

    QVariantMap workspace = SurfaceManagerTest::workspaceRow(index);
    workspace.insert(QStringLiteral("map"), map);
    return workspace;
}

// The real card, loaded from source the way the host loads it: this is what
// proves its imports, the shared glass components and the window contract
// actually resolve, and that the content the surface adopts is a window.
//
// It is the workspace map rather than the panel's old workspace menu because
// that menu no longer exists — the right button that opened it now opens this,
// and it offers everything the menu did plus what a list could not say.
void SurfaceManagerTest::theMenuContentLoadsAndFitsItsSurface()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    QQmlComponent component(
        &engine,
        QUrl::fromLocalFile(QStringLiteral(CELESTINA_QML_DIR "/WorkspaceMap.qml"))
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
    // The host connects to these by name, so a signature that drifted would
    // fail silently at runtime: the click would reach QML, emit, and land
    // nowhere. This is what makes that impossible to ship unnoticed.
    QVERIFY2(
        content->metaObject()->indexOfSignal("windowActivated(QString)") >= 0,
        "the map must expose windowActivated(QString) for the host to connect"
    );

    PanelMenuSurface surface;
    QVERIFY(surface.open(content, makePanel()));
}

// The bug this pins down: the window sized itself to the laid-out content while
// the content fitted itself to the window, so both shrank one margin per pass
// until the surface was a sliver — and every click in that sliver landed on the
// first item. `AnchoredCard` answers it by taking its measures from the consumer
// rather than from its children, and this is what holds that answer in place: a
// card must be wide enough to carry its content and tall enough for every board
// it offers.
void SurfaceManagerTest::theMenuSurfaceIsBigEnoughToClickEveryItem()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    QQmlComponent component(
        &engine,
        QUrl::fromLocalFile(QStringLiteral(CELESTINA_QML_DIR "/WorkspaceMap.qml"))
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
    // Four workspaces' worth of boards, plus the shadow room on both sides. The
    // exact metrics belong to the shared style; the floor here is that the
    // surface cannot collapse below one usable board per workspace.
    QVERIFY2(
        content->height() >= 4 * 24 + inset * 2,
        qPrintable(QStringLiteral("card height %1").arg(content->height()))
    );
    QVERIFY(content->width() > inset * 2);

    // And it stays that size once the menu has opened and laid itself out.
    content->show();
    const QSize mapped = content->size();
    QTest::qWait(200);
    QCOMPARE(content->size(), mapped);
}


// What the map is for: every window on a workspace is reachable, and reachable
// by keyboard as well as by pointer. The panel surface refuses the keyboard, but
// this card does not — it is opened deliberately and answers arrows and Return.
void SurfaceManagerTest::theMapListsEveryWindowAndWalksThemWithTheKeyboard()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    QQmlComponent component(
        &engine,
        QUrl::fromLocalFile(QStringLiteral(CELESTINA_QML_DIR "/WorkspaceMap.qml"))
    );
    QVERIFY2(component.isReady(), qPrintable(component.errorString()));

    QObject *root = component.createWithInitialProperties({
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("workspaces"), QVariantList {workspaceHolding(1)}},
    });
    QVERIFY2(root, qPrintable(component.errorString()));

    // One workspace row plus its three windows — the two tiled and the floating
    // one, which is kept apart in the fold but is still somewhere to go.
    const QVariantList targets = root->property("targets").toList();
    QCOMPARE(targets.size(), 4);

    // No ring before a key is pressed: a card opened by pointer must not paint
    // a focus nobody asked for.
    QCOMPARE(root->property("cursor").toInt(), -1);
    QVERIFY(root->property("currentKey").toString().isEmpty());

    QMetaObject::invokeMethod(root, "step", Q_ARG(QVariant, 1));
    QCOMPARE(root->property("currentKey").toString(), QStringLiteral("workspace:1"));
    QMetaObject::invokeMethod(root, "step", Q_ARG(QVariant, 1));
    QCOMPARE(root->property("currentKey").toString(), QStringLiteral("window:11"));

    // And it wraps rather than stopping dead at either end.
    QMetaObject::invokeMethod(root, "step", Q_ARG(QVariant, -1));
    QMetaObject::invokeMethod(root, "step", Q_ARG(QVariant, -1));
    QCOMPARE(root->property("currentKey").toString(), QStringLiteral("window:13"));

    // Return on a window asks for that window, not for the workspace under it.
    QSignalSpy windows(root, SIGNAL(windowActivated(QString)));
    QMetaObject::invokeMethod(root, "activateCursor");
    QCOMPARE(windows.size(), 1);
    QCOMPARE(windows.first().first().toString(), QStringLiteral("13"));

    delete root;
}

// A helper that predates the map publishes no such field, and the host defaults
// it to an empty one. The card must still build and still be dismissible: an
// older producer is a valid producer, not a crash.
void SurfaceManagerTest::theMapSurvivesAWorkspaceWithNoMapAtAll()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    QQmlComponent component(
        &engine,
        QUrl::fromLocalFile(QStringLiteral(CELESTINA_QML_DIR "/WorkspaceMap.qml"))
    );
    QVERIFY2(component.isReady(), qPrintable(component.errorString()));

    QVariantMap bare = workspaceRow(2);
    bare.remove(QStringLiteral("map"));
    QObject *root = component.createWithInitialProperties({
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("workspaces"), QVariantList {bare}},
    });
    QVERIFY2(root, qPrintable(component.errorString()));

    // The workspace itself is still somewhere to go; there is simply nothing
    // known to be on it.
    QCOMPARE(root->property("targets").toList().size(), 1);

    delete root;
}

// Unlike the panel's menu, an overlay is opened from a keybind rather than a
// click: there is no anchor point, so the recipe leaves `anchors` empty for
// the compositor to read as "center this on its output" (R2's launcher and
// clipboard history).
void SurfaceManagerTest::anOverlaySurfaceCoversItsOutputAndTakesFocus()
{
    QWindow *const content = makeContent();
    const QSize contentSize = content->size();

    OverlaySurface surface(OverlaySurface::Placement::Centered);
    QVERIFY(surface.open(content, nullptr));
    QVERIFY(surface.isOpen());
    QCOMPARE(surface.window(), content);
    QCOMPARE(content->transientParent(), nullptr);
    QVERIFY(!content->flags().testFlag(Qt::WindowDoesNotAcceptFocus));
    // Offscreen nothing configures the surface, so the content keeps the size
    // it asked for; on a compositor the four anchors below make it the output.
    QCOMPARE(content->size(), contentSize);

    auto *layerWindow = LayerShellQt::Window::get(content);
    QVERIFY(layerWindow);
    // All four edges with no size of its own: the surface is the whole output,
    // which is what puts a click outside the card inside this surface. The
    // card is centred by the content, not by the absence of anchors.
    auto expected = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    expected |= LayerShellQt::Window::AnchorBottom;
    expected |= LayerShellQt::Window::AnchorLeft;
    expected |= LayerShellQt::Window::AnchorRight;
    QCOMPARE(layerWindow->anchors(), expected);
    QCOMPARE(layerWindow->desiredSize(), QSize(0, 0));
    // And it reserves nothing while ignoring what the panel reserved, so it can
    // cover the button that opened it.
    QCOMPARE(layerWindow->exclusionZone(), -1);
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
