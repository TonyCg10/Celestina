#include <QtTest>

#include <QScopeGuard>
#include <QSignalSpy>

#include <QQmlComponent>
#include <QQmlEngine>
#include <QCoreApplication>
#include <QImage>
#include <QQuickItem>
#include <QQuickWindow>
#include <QScreen>
#include <QTemporaryDir>
#include <QUrl>
#include <QVariantList>
#include <QVariantMap>
#include <QWindow>

#include <limits>

#include "overlaysurface.h"
#include "panelattachmentlease.h"
#include "panelblurcontroller.h"
#include "panelmenucontroller.h"
#include "panelpopupplacement.h"
#include "panelmenusurface.h"
#include "surfacemanager.h"
#include "wallpaperidentity.h"
#include "wallpapermanager.h"

namespace {
class FakeTraySource final : public QObject
{
    Q_OBJECT
    Q_PROPERTY(QVariantList items READ items NOTIFY changed)

public:
    QVariantList items() const
    {
        return QVariantList {
            QVariantMap {
                {QStringLiteral("service"), QStringLiteral(":1.83")},
                {QStringLiteral("path"),
                 QStringLiteral("/org/chromium/StatusNotifierItem/1")},
                {QStringLiteral("title"), QStringLiteral("Slack")},
                {QStringLiteral("status"), QStringLiteral("active")},
                {QStringLiteral("iconSource"), QString()},
            },
        };
    }

signals:
    void changed();
};

class FakeTrayProviderSource final : public QObject
{
    Q_OBJECT
    Q_PROPERTY(QVariantMap providers READ providers CONSTANT)
    Q_PROPERTY(qulonglong revision READ revision CONSTANT)
    Q_PROPERTY(QObject *requests READ requests CONSTANT)

public:
    QVariantMap providers() const
    {
        return QVariantMap {
            {QStringLiteral("settings"),
             QVariantMap {{QStringLiteral("trayItems"), QVariantList()}}},
        };
    }

    qulonglong revision() const { return 1; }
    QObject *requests() const { return nullptr; }
};

class FakePanelWindow final : public QQuickWindow
{
    Q_OBJECT

public:
    Q_INVOKABLE void openWallpaperFolderPicker()
    {
        m_wallpaperFolderPickerOpened = true;
        emit wallpaperFolderPickerOpened();
    }

    bool hasOpenedWallpaperFolderPicker() const
    {
        return m_wallpaperFolderPickerOpened;
    }

signals:
    void wallpaperFolderPickerOpened();

private:
    bool m_wallpaperFolderPickerOpened = false;
};

class SemanticAttachmentSource final : public QQuickItem
{
    Q_OBJECT
    Q_PROPERTY(
        bool isPanelAttachmentSource
        READ isPanelAttachmentSource
        CONSTANT
    )
    Q_PROPERTY(QQuickItem *attachmentAnchor READ attachmentAnchor CONSTANT)
    Q_PROPERTY(bool menuOpen READ menuOpen WRITE setMenuOpen NOTIFY menuOpenChanged)

public:
    explicit SemanticAttachmentSource(QQuickItem *parent = nullptr)
        : QQuickItem(parent)
        , m_anchor(new QQuickItem(this))
    {
        m_anchor->setSize(QSizeF(18, 18));
    }

    bool isPanelAttachmentSource() const { return true; }
    QQuickItem *attachmentAnchor() const { return m_anchor; }
    bool menuOpen() const { return m_menuOpen; }

    void setMenuOpen(bool open)
    {
        if (m_menuOpen == open)
            return;
        m_menuOpen = open;
        emit menuOpenChanged();
    }

    Q_INVOKABLE QRectF attachmentAnchorGlobalRectNow() const
    {
        return QRectF(
            m_anchor->mapToGlobal(QPointF(0, 0)),
            QSizeF(m_anchor->width(), m_anchor->height())
        );
    }

    void placeAnchor(const QPointF &position)
    {
        m_anchor->setPosition(position);
    }

    void resizeAnchor(const QSizeF &size)
    {
        m_anchor->setSize(size);
    }

signals:
    void menuOpenChanged();

private:
    QQuickItem *m_anchor;
    bool m_menuOpen = false;
};

QWindow *windowWithProperty(const char *propertyName)
{
    // Prefer the mapped one: earlier cases in this same process can leave a
    // retiring window carrying the same property, and acting on that ghost —
    // resizing it, reading its card — quietly diverges from what the
    // controller under test is really using.
    QWindow *fallback = nullptr;
    for (QWindow *const window : QGuiApplication::topLevelWindows()) {
        if (!window->property(propertyName).isValid())
            continue;
        if (window->isVisible())
            return window;
        if (!fallback)
            fallback = window;
    }

    return fallback;
}

QVariantList trayEntries(int count = 1)
{
    QVariantList entries;
    entries.reserve(count);
    for (int index = 0; index < count; ++index) {
        entries.append(QVariantMap {
            {QStringLiteral("id"), 7 + index},
            {QStringLiteral("label"),
             QStringLiteral("Action %1").arg(index + 1)},
            {QStringLiteral("enabled"), true},
            {QStringLiteral("separator"), false},
            {QStringLiteral("depth"), 0},
            {QStringLiteral("iconName"), QString()},
            {QStringLiteral("toggleType"), QString()},
            {QStringLiteral("toggleState"), 0},
        });
    }
    return entries;
}

void registerPanelMenuTypesFromSource()
{
    static bool registered = false;
    if (registered)
        return;
    registered = true;

    const QUrl root = QUrl::fromLocalFile(QStringLiteral(CELESTINA_QML_DIR "/"));
    for (const QString &name : {
             QStringLiteral("TrayMenu"),
             QStringLiteral("TrayItemsMenu"),
             QStringLiteral("NetworkMenu"),
             QStringLiteral("BluetoothMenu"),
             QStringLiteral("PerformanceMenu"),
             QStringLiteral("CaptureMenu"),
             QStringLiteral("WallpaperMenu"),
             QStringLiteral("WorkspaceMap"),
         }) {
        qmlRegisterType(
            root.resolved(QUrl(name + QStringLiteral(".qml"))),
            "CelestinaDesktop",
            1,
            0,
            name.toUtf8().constData()
        );
    }
}
} // namespace

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
    void aCardMenuUsesABoundedLayerSurface();
    void aParentAndTrayChildCanStayMappedTogether();
    void theMenuRefusesToOpenTwiceAndSurvivesReopening();
    void theMenuReportsAndCleansUpAnExternalDismissal();
    void aClosedMenuLeavesNoWindowBehind();
    void theMenuIsOnUnlessTheEnvironmentTurnsItOff();
    void aMenuKeepsTheInvokingControlsAnchor();
    void aRetiredAttachmentLeaseCannotClearItsSuccessor();
    void aLiveAttachmentLeaseFollowsItsAnchorAndAncestors();
    void anAmbiguousAttachmentSourceLeavesTheSurfaceFloating();
    void aFailedAttachmentLeasePublishesNothing();
    void aTrayChildStaysAdjacentAndInsideTheOutput();
    void aTrayChildOnAScaledOutputIsPlacedInRealPixels();
    void aScaledTrayChildStaysBesideItsParentAndOnScreen();
    void anOverflowingTrayMenuUsesABoundedScrollableViewport();
    void trayInventoryAndForeignMenuHaveIndependentLifecycles();
    void wallpaperMenuHandsTheFolderChooserBackToThePermanentPanel();
    void aFullWidthBarShapeRemainsFinite();
    void aTallGlassCardKeepsItsRoundedRectangle();
    void aPublishedPolygonOverridesItsRoundedBoundingRect();
    void anInvalidPolygonFallsBackToRoundedGlass();
    void emptyPublishedGlassNeverBecomesFullWindowBlur();
    void anArmedBlurSurvivesLayerShellExposureLoss();
    void theMenuContentLoadsAndFitsItsSurface();
    void theMenuSurfaceIsBigEnoughToClickEveryItem();
    void theMapListsEveryWindowAndWalksThemWithTheKeyboard();
    void theMapSurvivesAWorkspaceWithNoMapAtAll();

    void anOverlaySurfaceCoversItsOutputAndTakesFocus();
    void theOverlayRefusesToOpenTwiceAndSurvivesReopening();
    void theOverlayReportsAndCleansUpAnExternalDismissal();
    void aClosedOverlayLeavesNoWindowBehind();
    void thePanelOverlayPrototypeLoadsAndMaps();
    void aCornerSurfaceSitsUnderThePanelAndRefusesFocus();
    void aReadoutSurfaceSitsLowAndCentredSoItNeverCoversAToast();
    void aWallpaperCoversItsOutputAndReservesNothing();
    void wallpaperIdentityRejectsMalformedOrDuplicateRows();
    void wallpaperRevisionChangesTheQmlImageRequest();

private:
    QWindow *makePanel()
    {
        auto *panel = new QQuickWindow;
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
    // The opener's real vertical coordinate is meaningful only in output
    // space, so the prototype covers the panel strip instead of beginning
    // below a guessed exclusive edge. It still reserves nothing.
    QCOMPARE(layerWindow->exclusionZone(), -1);
    QCOMPARE(layerWindow->margins(), QMargins());
}

void SurfaceManagerTest::aCardMenuUsesABoundedLayerSurface()
{
    QWindow *const panel = makePanel();
    QWindow *const content = makeContent();
    const QSize contentSize = content->size();
    const QSize outputSize = panel->screen()->geometry().size();
    const QPoint requested(outputSize.width() + 100, outputSize.height() + 100);

    PanelMenuSurface surface;
    QVERIFY(surface.open(
        content,
        panel,
        PanelMenuSurface::Coverage::Card,
        requested
    ));

    auto *const layerWindow = LayerShellQt::Window::get(content);
    QVERIFY(layerWindow);
    auto expected = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    expected |= LayerShellQt::Window::AnchorLeft;
    QCOMPARE(layerWindow->anchors(), expected);
    QCOMPARE(layerWindow->desiredSize(), contentSize);
    QCOMPARE(
        layerWindow->margins(),
        QMargins(
            qMax(0, outputSize.width() - contentSize.width()),
            qMax(0, outputSize.height() - contentSize.height()),
            0,
            0
        )
    );
    QCOMPARE(layerWindow->exclusionZone(), -1);
    QVERIFY(!content->flags().testFlag(Qt::WindowDoesNotAcceptFocus));
}

void SurfaceManagerTest::aParentAndTrayChildCanStayMappedTogether()
{
    QWindow *const panel = makePanel();
    QPointer<QWindow> parentContent = makeContent();
    QPointer<QWindow> childContent = makeContent();

    PanelMenuSurface parent;
    PanelMenuSurface child;
    QVERIFY(parent.open(parentContent, panel));
    QVERIFY(child.open(
        childContent,
        panel,
        PanelMenuSurface::Coverage::Card,
        QPoint(300, 120)
    ));
    QVERIFY(parent.isOpen());
    QVERIFY(child.isOpen());
    QCOMPARE(parent.window(), parentContent.data());
    QCOMPARE(child.window(), childContent.data());

    child.close();
    QVERIFY(parent.isOpen());
    QVERIFY(!child.isOpen());
    QTRY_VERIFY(childContent.isNull());

    parent.close();
    QVERIFY(!parent.isOpen());
    QTRY_VERIFY(parentContent.isNull());
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
    const QRect first = panelPopupOpenerOnOutput(
        QRect(2380, 128, 30, 30), outputOrigin
    );
    QCOMPARE(first, QRect(460, 8, 30, 30));
    QCOMPARE(panelPopupBodyOrigin(first, 320, 8), QPoint(315, 46));
    QCOMPARE(panelPopupBodyOrigin(first, 320, 24, 40), QPoint(315, 64));

    // Both axes follow the invoking control inside the full-output surface; a
    // stacked or resized panel therefore supplies geometry rather than a
    // guessed height.
    const QRect second = panelPopupOpenerOnOutput(
        QRect(2512, 260, 30, 30), outputOrigin
    );
    QCOMPARE(second, QRect(592, 140, 30, 30));
    QCOMPARE(panelPopupBodyOrigin(second, 320, 8), QPoint(447, 178));
}

void SurfaceManagerTest::aRetiredAttachmentLeaseCannotClearItsSuccessor()
{
    QQuickWindow panel;
    panel.setGeometry(0, 0, 800, 40);
    panel.show();
    SemanticAttachmentSource source(panel.contentItem());
    source.setPosition(QPointF(620.5, 5));
    source.setSize(QSizeF(180, 30));
    source.placeAnchor(QPointF(70, 6));
    QWindow firstSurface;
    firstSurface.setScreen(panel.screen());
    QWindow successorSurface;
    successorSurface.setScreen(panel.screen());
    const QRectF anchor = source.attachmentAnchorGlobalRectNow();
    PanelAttachmentLease first;
    PanelAttachmentLease successor;

    QVERIFY(first.acquire(&panel, &firstSurface, anchor));
    QVERIFY(source.menuOpen());
    const QScreen *const screen = panel.screen();
    const QPointF outputOrigin =
        screen ? QPointF(screen->geometry().topLeft()) : QPointF();
    const QRectF anchorOnOutput = anchor.translated(-outputOrigin);
    const QString firstToken =
        firstSurface.property(
            "_celestinaAttachmentAnchorLeaseToken").toString();
    QVERIFY(!firstToken.isEmpty());
    QCOMPARE(
        source.property(
            "_celestinaPanelMenuFeedbackLeaseToken").toString(),
        firstToken
    );
    QCOMPARE(
        firstSurface.property("attachmentAnchorRect").toRectF(),
        anchorOnOutput
    );

    // Independent controllers own independent contextual surfaces. Their
    // tokens therefore live on those surfaces rather than in shared panel
    // state, even when both began at the same icon rectangle.
    QVERIFY(successor.acquire(&panel, &successorSurface, anchor));
    QVERIFY(source.menuOpen());
    const QString successorToken =
        successorSurface.property(
            "_celestinaAttachmentAnchorLeaseToken").toString();
    QVERIFY(!successorToken.isEmpty());
    QVERIFY(successorToken != firstToken);
    QCOMPARE(
        source.property(
            "_celestinaPanelMenuFeedbackLeaseToken").toString(),
        successorToken
    );
    QCOMPARE(
        successorSurface.property("attachmentAnchorRect").toRectF(),
        anchorOnOutput
    );

    first.release();
    QVERIFY(source.menuOpen());
    QCOMPARE(
        source.property(
            "_celestinaPanelMenuFeedbackLeaseToken").toString(),
        successorToken
    );
    QVERIFY(firstSurface.property("attachmentAnchorRect").toRectF().isEmpty());
    QVERIFY(firstSurface.property(
        "_celestinaAttachmentAnchorLeaseToken").toString().isEmpty());
    QCOMPARE(
        successorSurface.property("attachmentAnchorRect").toRectF(),
        anchorOnOutput
    );
    QCOMPARE(
        successorSurface.property(
            "_celestinaAttachmentAnchorLeaseToken").toString(),
        successorToken
    );

    successor.release();
    QVERIFY(!source.menuOpen());
    QVERIFY(source.property(
        "_celestinaPanelMenuFeedbackLeaseToken").toString().isEmpty());
    QVERIFY(successorSurface.property("attachmentAnchorRect").toRectF().isEmpty());
    QVERIFY(successorSurface.property(
        "_celestinaAttachmentAnchorLeaseToken").toString().isEmpty());
}

void SurfaceManagerTest::aLiveAttachmentLeaseFollowsItsAnchorAndAncestors()
{
    QQuickWindow panel;
    panel.setGeometry(120, 80, 500, 40);
    panel.show();
    auto *const ancestor = new QQuickItem(panel.contentItem());
    ancestor->setPosition(QPointF(30, 5));
    auto *const source = new SemanticAttachmentSource(ancestor);
    source->setPosition(QPointF(10, 0));
    source->setSize(QSizeF(80, 30));
    source->placeAnchor(QPointF(31, 6));
    ancestor->setVisible(false);

    QQuickWindow surface;
    surface.setScreen(panel.screen());
    const auto globalAnchorRect = [source]() {
        return source->attachmentAnchorGlobalRectNow();
    };
    const auto outputLocalRect = [&panel](const QRectF &globalRect) {
        const QScreen *const screen = panel.screen();
        const QPointF origin =
            screen ? QPointF(screen->geometry().topLeft()) : QPointF();
        return globalRect.translated(-origin);
    };

    PanelAttachmentLease lease;
    const QRectF initial = globalAnchorRect();
    const QRectF toleratedSnapshot = initial.translated(QPointF(0.5, 0.5));
    QVERIFY(lease.acquire(&panel, &surface, toleratedSnapshot));
    QVERIFY(source->menuOpen());
    const QString token = surface.property(
        "_celestinaAttachmentAnchorLeaseToken").toString();
    QVERIFY(!token.isEmpty());
    QVERIFY(surface.property("attachmentAnchorRect").toRectF().isEmpty());
    QVERIFY(lease.isActive());

    // A hidden source keeps its identity without briefly painting the stale
    // click snapshot. Once visible, the surface receives the source's current
    // canonical icon rectangle rather than the tolerated snapshot.
    ancestor->setVisible(true);
    QTRY_COMPARE(
        surface.property("attachmentAnchorRect").toRectF(),
        outputLocalRect(initial)
    );
    QCOMPARE(
        surface.property("attachmentAnchorRect").toRectF(),
        outputLocalRect(initial)
    );

    source->attachmentAnchor()->setX(39);
    source->resizeAnchor(QSizeF(20, 18));
    const QRectF movedInsideSource = globalAnchorRect();
    QTRY_COMPARE(
        surface.property("attachmentAnchorRect").toRectF(),
        outputLocalRect(movedInsideSource)
    );

    ancestor->setPosition(QPointF(62, 7));
    const QRectF movedWithAncestor = globalAnchorRect();
    QTRY_COMPARE(
        surface.property("attachmentAnchorRect").toRectF(),
        outputLocalRect(movedWithAncestor)
    );

    ancestor->setVisible(false);
    QTRY_VERIFY(
        surface.property("attachmentAnchorRect").toRectF().isEmpty()
    );
    QVERIFY(lease.isActive());
    QVERIFY(source->menuOpen());
    QCOMPARE(surface.property(
        "_celestinaAttachmentAnchorLeaseToken").toString(), token);
    ancestor->setVisible(true);
    QTRY_COMPARE(
        surface.property("attachmentAnchorRect").toRectF(),
        outputLocalRect(movedWithAncestor)
    );
    QVERIFY(source->menuOpen());

    auto *const secondAncestor = new QQuickItem(panel.contentItem());
    secondAncestor->setPosition(QPointF(74, 9));
    source->setParent(secondAncestor);
    source->setParentItem(secondAncestor);
    const QRectF movedWithReparent = globalAnchorRect();
    QTRY_COMPARE(
        surface.property("attachmentAnchorRect").toRectF(),
        outputLocalRect(movedWithReparent)
    );

    secondAncestor->setX(91);
    const QRectF reparentedPosition = globalAnchorRect();
    QTRY_COMPARE(
        surface.property("attachmentAnchorRect").toRectF(),
        outputLocalRect(reparentedPosition)
    );

    // Losing the source also destroys its declared child anchor. The lease
    // retires its surface token instead of leaving a frozen connector.
    delete source;
    QTRY_VERIFY(
        surface.property("attachmentAnchorRect").toRectF().isEmpty()
    );
    QVERIFY(surface.property(
        "_celestinaAttachmentAnchorLeaseToken").toString().isEmpty());
    QVERIFY(!lease.isActive());
    lease.release();
}

void SurfaceManagerTest::anAmbiguousAttachmentSourceLeavesTheSurfaceFloating()
{
    QQuickWindow panel;
    panel.setGeometry(0, 0, 800, 40);
    panel.show();
    SemanticAttachmentSource first(panel.contentItem());
    SemanticAttachmentSource duplicate(panel.contentItem());
    for (SemanticAttachmentSource *const source : {&first, &duplicate}) {
        source->setPosition(QPointF(620, 5));
        source->setSize(QSizeF(140, 30));
        source->placeAnchor(QPointF(61, 6));
    }

    QQuickWindow surface;
    surface.setScreen(panel.screen());
    const QRectF snapshot = first.attachmentAnchorGlobalRectNow();
    surface.setProperty("attachmentAnchorRect", snapshot);
    PanelAttachmentLease lease;

    QVERIFY(!lease.acquire(&panel, &surface, snapshot));
    QVERIFY(!first.menuOpen());
    QVERIFY(!duplicate.menuOpen());
    QVERIFY(!lease.isActive());
    QVERIFY(surface.property("attachmentAnchorRect").toRectF().isEmpty());
    QVERIFY(surface.property(
        "_celestinaAttachmentAnchorLeaseToken").toString().isEmpty());
}

void SurfaceManagerTest::aFailedAttachmentLeasePublishesNothing()
{
    QQuickWindow panel;
    panel.setGeometry(0, 0, 800, 40);
    panel.show();
    QWindow surface;
    surface.setScreen(panel.screen());
    const QRectF snapshot(640, 11, 18, 18);
    surface.setProperty("attachmentAnchorRect", snapshot);
    PanelAttachmentLease lease;

    QVERIFY(!lease.acquire(&panel, &surface, snapshot));
    QVERIFY(!lease.isActive());
    QVERIFY(surface.property("attachmentAnchorRect").toRectF().isEmpty());
    QVERIFY(surface.property(
        "_celestinaAttachmentAnchorLeaseToken").toString().isEmpty());
}

void SurfaceManagerTest::aTrayChildStaysAdjacentAndInsideTheOutput()
{
    const QSize output(1920, 1080);
    const QSize child(320, 460);

    QCOMPARE(
        adjacentTrayMenuOrigin(
            QRect(1500, 48, 380, 620),
            QPoint(1540, 220),
            child,
            output,
            8
        ),
        QPoint(1172, 220)
    );
    QCOMPARE(
        adjacentTrayMenuOrigin(
            QRect(40, 48, 380, 620),
            QPoint(80, 900),
            child,
            output,
            8
        ),
        QPoint(428, 620)
    );

    // If neither side can contain it, the side with more room wins and the
    // complete child is still clamped to the output.
    QCOMPARE(
        adjacentTrayMenuOrigin(
            QRect(300, 48, 400, 620),
            QPoint(500, -20),
            QSize(760, 400),
            QSize(1000, 700),
            8
        ),
        QPoint(240, 0)
    );
}

// A parent card names its size in the shell's unscaled units; the child's own
// window, the anchor and the output are real pixels. Mixing the two agrees
// only at factor 1, and on a 1.15 output the child was placed a sixth of the
// parent's width too far left — over its parent rather than beside it, which
// is what the author photographed. This case states the conversion the caller
// owes before it may use `adjacentTrayMenuOrigin` at all.
void SurfaceManagerTest::aTrayChildOnAScaledOutputIsPlacedInRealPixels()
{
    constexpr double factor = 1.15;
    const QSize output(1920, 1080);
    // What the parent surface publishes: unscaled.
    const QRect cardInShellUnits(1200, 40, 300, 500);
    // What the child window really is: pixels.
    const QSize child(320, 460);
    const QPoint anchor(1420, 260);

    const QRect cardInPixels(
        qRound(cardInShellUnits.x() * factor),
        qRound(cardInShellUnits.y() * factor),
        qRound(cardInShellUnits.width() * factor),
        qRound(cardInShellUnits.height() * factor)
    );

    const QPoint placed =
        adjacentTrayMenuOrigin(cardInPixels, anchor, child, output, 8);
    const QPoint placedFromUnconverted =
        adjacentTrayMenuOrigin(cardInShellUnits, anchor, child, output, 8);

    // The two disagree, which is the whole point: the unconverted call is the
    // defect, so a future caller cannot quietly pass shell units again.
    QVERIFY(placed != placedFromUnconverted);

    // Beside the parent in real pixels, never overlapping it.
    const QRect childRect(placed, child);
    QVERIFY2(
        !childRect.intersects(cardInPixels),
        qPrintable(QStringLiteral("child %1,%2 overlaps parent")
                       .arg(placed.x()).arg(placed.y()))
    );
    // And the unconverted placement is exactly the failure that was seen.
    QVERIFY(QRect(placedFromUnconverted, child).intersects(cardInPixels));

    // Still whole and inside the output.
    QVERIFY(childRect.left() >= 0);
    QVERIFY(childRect.right() < output.width());
}

// The whole child-menu route at a real per-output factor: created beside a
// scaled parent, capped to the screen, and still wearing its glass. Every
// number this route mixes — the parent card, the window, the anchor, the
// viewport cap — crosses the shell-units/output-pixels seam somewhere, and
// each crossing has now been wrong at least once.
void SurfaceManagerTest::aScaledTrayChildStaysBesideItsParentAndOnScreen()
{
    qunsetenv("CELESTINA_PANEL_MENU");
    qputenv("CELESTINA_SHELL_SCALE", "1.15");
    const auto restoreScale = qScopeGuard([] {
        qputenv("CELESTINA_SHELL_SCALE", "1");
    });
    registerPanelMenuTypesFromSource();
    QQmlEngine engine;
    engine.addImportPath(QCoreApplication::applicationDirPath());
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    FakeTraySource tray;
    FakeTrayProviderSource providers;
    PanelMenuController controller(&engine, nullptr, nullptr);
    QWindow *const panel = makePanel();

    controller.toggleTrayItemsMenu(
        panel,
        QRect(700, 6, 28, 28),
        QRect(676, 5, 76, 30),
        &tray,
        &providers
    );
    QPointer<QWindow> inventory = windowWithProperty("traySource");
    QVERIFY(inventory);
    const QScreen *const screen = panel->screen();
    QVERIFY(screen);

    const QString service = QStringLiteral(":1.83");
    const QString path = QStringLiteral("/org/chromium/StatusNotifierItem/1");
    QVERIFY(QMetaObject::invokeMethod(
        inventory,
        "itemMenuRequested",
        Q_ARG(QString, service),
        Q_ARG(QString, path),
        Q_ARG(int, 620),
        Q_ARG(int, screen->geometry().top() + 200),
        Q_ARG(int, 48),
        Q_ARG(int, 48)
    ));
    // Enough actions that the natural height overflows the output even in
    // shell units, so the cap is exercised rather than merely present.
    controller.trayMenuReady(service, path, trayEntries(64));

    QPointer<QWindow> child = windowWithProperty("entries");
    QVERIFY(child);

    // Absolute placement is deliberately not asserted here: the offscreen
    // platform clamps a shown window to its own 800x800 screen, so an
    // output-covering surface cannot be given its real size in this harness.
    // Where the card lands, in real pixels against a real parent, is pinned
    // by the `adjacentTrayMenuOrigin` unit cases — including the scaled one —
    // with hand-fed compositor geometry. What this case owns is the child's
    // attachment and lifecycle on a scaled output.

    QVERIFY(child->property("attachedToMenuSide").toBool());
    const QRectF childAnchor =
        child->property("attachmentAnchorRect").toRectF();
    QVERIFY(childAnchor.width() > 0 && childAnchor.height() > 0);
    QTRY_VERIFY(!child->property("glassRegions").toList().isEmpty());

    // The membrane really has travel to cross and the field draws the
    // sideways shape: "both menus glued together with no droplet" is what a
    // zero gap looks like, and it is what the author saw once already.
    QCOMPARE(child->property("sideAttachmentGap").toInt() > 0, true);
    QQuickItem *const field =
        child->findChild<QQuickItem *>(
            QStringLiteral("celestina-soft-menu-field"));
    QVERIFY(field);
    QVERIFY(field->property("edgeShapeActive").toBool());

    // The push moves the surface as one piece: mid-push, the popup that
    // carries the rows sits exactly on the momentary body, not at its settled
    // place. The author recorded the settled card materialising in one frame
    // while only the body section slid inside it.
    QTRY_VERIFY(child->property("menu").value<QObject *>() != nullptr);
    QObject *const menu = child->property("menu").value<QObject *>();
    QTRY_VERIFY(menu->property("visible").toBool());
    const double progress = field->property("attachmentProgress").toDouble();
    QVERIFY2(progress < 1.0,
             qPrintable(QStringLiteral("progress already %1").arg(progress)));
    const QRectF bodyRect = field->property("attachmentBodyRect").toRectF();
    const int cardX = child->property("cardX").toInt();
    QVERIFY2(
        qAbs(menu->property("x").toDouble() - (cardX + bodyRect.x())) < 1.0,
        qPrintable(QStringLiteral("popup at %1, body at %2 (cardX %3)")
                       .arg(menu->property("x").toDouble())
                       .arg(bodyRect.x()).arg(cardX))
    );
    QVERIFY2(qAbs(bodyRect.x()) > 1.0,
             qPrintable(QStringLiteral(
                 "mid-push body still parked at %1").arg(bodyRect.x())));

}

void SurfaceManagerTest::anOverflowingTrayMenuUsesABoundedScrollableViewport()
{
    qunsetenv("CELESTINA_PANEL_MENU");
    registerPanelMenuTypesFromSource();
    QQmlEngine engine;
    engine.addImportPath(QCoreApplication::applicationDirPath());
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    FakeTraySource tray;
    FakeTrayProviderSource providers;
    PanelMenuController controller(&engine, nullptr, nullptr);
    QWindow *const panel = makePanel();

    controller.toggleTrayItemsMenu(
        panel,
        QRect(700, 6, 28, 28),
        QRect(676, 5, 76, 30),
        &tray,
        &providers
    );
    QPointer<QWindow> inventory = windowWithProperty("traySource");
    QVERIFY(inventory);
    const QScreen *const screen = panel->screen();
    QVERIFY(screen);

    const QString service = QStringLiteral(":1.83");
    const QString path = QStringLiteral("/org/chromium/StatusNotifierItem/1");
    constexpr int requestedGlobalX = 620;
    const int requestedTop = screen->geometry().height() / 4;
    const int requestedGlobalY = screen->geometry().top() + requestedTop;
    QVERIFY(QMetaObject::invokeMethod(
        inventory,
        "itemMenuRequested",
        Q_ARG(QString, service),
        Q_ARG(QString, path),
        Q_ARG(int, requestedGlobalX),
        Q_ARG(int, requestedGlobalY),
        Q_ARG(int, 48),
        Q_ARG(int, 48)
    ));
    controller.trayMenuReady(service, path, trayEntries(64));

    QPointer<QWindow> child = windowWithProperty("entries");
    QVERIFY(child);
    const int availableHeight = screen->geometry().height() - requestedTop;
    QCOMPARE(
        child->property("maximumContentHeight").toInt(),
        availableHeight
    );
    QVERIFY(
        child->property("naturalMenuHeight").toInt()
        > child->property("cardHeight").toInt()
    );
    QCOMPARE(
        child->property("cardHeight").toInt(),
        availableHeight
    );
    // The side-attached child rises so the invoking tile's centre stays
    // inside the membrane's flat lateral span instead of at the body corner.
    // Its surface covers the output now; the rise lives in the card.
    // The host writes the rise into `menuY`; where the card finally lands is
    // the placement clamp's business against the compositor-given surface,
    // which this harness cannot provide (see the scaled case above).
    QCOMPARE(child->property("menuY").toInt(),
             requestedTop + QRect(0, 0, 48, 48).center().y() - 72);

    QObject *const menu = child->property("menu").value<QObject *>();
    QVERIFY(menu);
    auto *const viewport = qobject_cast<QQuickItem *>(
        menu->property("contentItem").value<QObject *>()
    );
    QVERIFY(viewport);
    QTRY_VERIFY(
        viewport->property("contentHeight").toReal() > viewport->height()
    );

    // No separate scroll bar remains. The clipped viewport itself scrolls,
    // and the pinned heading beside it never enters the scrolled content, so
    // a scrolled row can no longer be painted over the lighter header field.
    QVERIFY(!child->findChild<QObject *>(
        QStringLiteral("celestina-tray-menu-scrollbar")
    ));
    QVERIFY(viewport->property("clip").toBool());
    QObject *const heading = child->findChild<QObject *>(
        QStringLiteral("celestina-tray-menu-heading")
    );
    QVERIFY(heading);
    auto *const headingItem = qobject_cast<QQuickItem *>(heading);
    QVERIFY(headingItem);
    // The popup now opens one tick after creation, once the host has dressed
    // the window; the heading is reparented beside its viewport at that
    // moment, so this waits for it rather than sampling the first tick.
    QTRY_COMPARE(headingItem->window(), child.data());
    QCOMPARE(headingItem->parentItem(), viewport->parentItem());
    QVERIFY(headingItem->height() > 0);
    // The raised top padding keeps the scrolled rows strictly below that
    // pinned heading.
    QVERIFY(menu->property("topPadding").toReal()
            >= headingItem->height());
    viewport->setProperty("contentY", 40.0);
    QTRY_VERIFY(viewport->property("contentY").toReal() > 0.0);

    // The same bounded viewport remains a real Menu: arrow keys reach the last
    // foreign action and move the viewport rather than losing that action
    // below the output.
    viewport->setProperty("contentY", 0.0);
    menu->setProperty("currentIndex", -1);
    child->requestActivate();
    QVERIFY(QMetaObject::invokeMethod(
        menu,
        "forceActiveFocus",
        Q_ARG(Qt::FocusReason, Qt::PopupFocusReason)
    ));
    QCOMPARE(menu->property("count").toInt(), 64);
    const int lastIndex = 63;
    for (int step = 0;
         step <= menu->property("count").toInt()
         && menu->property("currentIndex").toInt() != lastIndex;
         ++step) {
        QTest::keyClick(child, Qt::Key_Down);
    }
    QTRY_COMPARE(menu->property("currentIndex").toInt(), lastIndex);
    QTRY_VERIFY(viewport->property("contentY").toReal() > 0.0);

    // Scrolling the child changes neither carrier identity nor the mapped
    // inventory behind it. Escape closes only that foreign child and restores
    // the still-mapped parent hierarchy.
    QVERIFY(inventory);
    QVERIFY(inventory->isVisible());
    QCOMPARE(controller.openIndicator(), QStringLiteral("tray-items"));
    QTest::keyClick(child, Qt::Key_Escape);
    QTRY_VERIFY(child.isNull());
    QVERIFY(inventory);
    QVERIFY(inventory->isVisible());
    QCOMPARE(controller.openIndicator(), QStringLiteral("tray-items"));

    // A request at the last output pixel moves up only enough to retain the
    // header, section label and one real action. It never becomes a one-pixel
    // viewport after Menu padding.
    const int edgeGlobalY = screen->geometry().bottom();
    QVERIFY(QMetaObject::invokeMethod(
        inventory,
        "itemMenuRequested",
        Q_ARG(QString, service),
        Q_ARG(QString, path),
        Q_ARG(int, requestedGlobalX),
        Q_ARG(int, edgeGlobalY),
        Q_ARG(int, 48),
        Q_ARG(int, 48)
    ));
    controller.trayMenuReady(service, path, trayEntries(64));

    QPointer<QWindow> edgeChild = windowWithProperty("entries");
    QVERIFY(edgeChild);
    const int minimumViewportHeight =
        edgeChild->property("minimumMenuViewportHeight").toInt();
    QVERIFY(minimumViewportHeight > 1);
    QCOMPARE(
        edgeChild->property("maximumContentHeight").toInt(),
        minimumViewportHeight
    );
    QCOMPARE(edgeChild->property("cardHeight").toInt(), minimumViewportHeight);
    // The rise that keeps the minimum viewport on screen is the host's
    // `menuY`; the surface covers the output and carries no margins now.
    QCOMPARE(
        edgeChild->property("menuY").toInt(),
        screen->geometry().height() - minimumViewportHeight
    );
    controller.close();
    QTRY_VERIFY(edgeChild.isNull());
    QTRY_VERIFY(inventory.isNull());
}

void SurfaceManagerTest::trayInventoryAndForeignMenuHaveIndependentLifecycles()
{
    qunsetenv("CELESTINA_PANEL_MENU");
    registerPanelMenuTypesFromSource();
    QQmlEngine engine;
    engine.addImportPath(QCoreApplication::applicationDirPath());
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    FakeTraySource tray;
    FakeTrayProviderSource providers;
    PanelMenuController controller(&engine, nullptr, nullptr);
    QWindow *const panel = makePanel();
    panel->show();
    auto *const quickPanel = qobject_cast<QQuickWindow *>(panel);
    QVERIFY(quickPanel);
    auto *const inventorySource =
        new SemanticAttachmentSource(quickPanel->contentItem());
    inventorySource->setPosition(QPointF(660, 5));
    inventorySource->setSize(QSizeF(120, 30));
    inventorySource->placeAnchor(QPointF(41, 6));

    const QPointF panelGlobal = panel->position();
    const QRectF inventoryOpener(
        panelGlobal + QPointF(700, 6),
        QSizeF(28, 28)
    );
    const QRectF inventoryAnchor =
        inventorySource->attachmentAnchorGlobalRectNow();
    controller.toggleTrayItemsMenu(
        panel,
        inventoryOpener,
        inventoryAnchor,
        &tray,
        &providers
    );
    QVERIFY(inventorySource->menuOpen());
    QCOMPARE(controller.openIndicator(), QStringLiteral("tray-items"));
    QPointer<QWindow> inventory = windowWithProperty("traySource");
    QVERIFY(inventory);
    QVERIFY(inventory->isVisible());
    const QScreen *const inventoryScreen = panel->screen();
    QVERIFY(inventoryScreen);
    const QRectF localInventoryOpener = panelPopupOpenerOnOutput(
        inventoryOpener,
        QPointF(inventoryScreen->geometry().topLeft())
    );
    const QRectF localInventoryAnchor = panelPopupOpenerOnOutput(
        inventoryAnchor,
        QPointF(inventoryScreen->geometry().topLeft())
    );
    QVERIFY(inventory->property("anchoredFromPanel").toBool());
    QCOMPARE(
        inventory->property("openerRect").toRectF(),
        localInventoryOpener
    );
    QCOMPARE(
        inventory->property("attachmentAnchorRect").toRectF(),
        localInventoryAnchor
    );
    QCOMPARE(inventory->property("attachmentStartY").toInt(), panel->height());
    QCOMPARE(inventory->property("anchorGap").toInt(), 20);
    const QPoint inventoryOrigin = panelPopupBodyOrigin(
        localInventoryOpener,
        inventory->property("contentWidth").toInt(),
        inventory->property("anchorGap").toInt(),
        panel->height()
    );
    QCOMPARE(inventory->property("menuY").toInt(), inventoryOrigin.y());
    QCOMPARE(inventory->property("cardY").toInt(), inventoryOrigin.y());
    QCOMPARE(inventory->property("preserveRequestedTop").toBool(), true);
    QCOMPARE(
        inventory->property("maximumContentHeight").toInt(),
        inventoryScreen->geometry().height() - inventoryOrigin.y()
    );

    QSignalSpy needed(&controller, &PanelMenuController::trayMenuNeeded);
    const QString service = QStringLiteral(":1.83");
    const QString path = QStringLiteral("/org/chromium/StatusNotifierItem/1");
    QVERIFY(QMetaObject::invokeMethod(
        inventory,
        "itemMenuRequested",
        Q_ARG(QString, service),
        Q_ARG(QString, path),
        Q_ARG(int, 620),
        Q_ARG(int, 220),
        Q_ARG(int, 48),
        Q_ARG(int, 48)
    ));
    QCOMPARE(needed.size(), 1);
    QVERIFY(inventory->isVisible());
    QCOMPARE(controller.openIndicator(), QStringLiteral("tray-items"));

    // The peer's reply has no request token. A repeated click on this exact
    // still-pending item is therefore coalesced instead of becoming an
    // indistinguishable second request.
    QVERIFY(QMetaObject::invokeMethod(
        inventory,
        "itemMenuRequested",
        Q_ARG(QString, service),
        Q_ARG(QString, path),
        Q_ARG(int, 620),
        Q_ARG(int, 220),
        Q_ARG(int, 48),
        Q_ARG(int, 48)
    ));
    QCOMPARE(needed.size(), 1);

    controller.trayMenuReady(service, path, trayEntries());
    QPointer<QWindow> child = windowWithProperty("entries");
    QVERIFY(child);
    QVERIFY(child->isVisible());
    QVERIFY(!child->property("anchoredFromPanel").toBool());
    QVERIFY(child->property("openerRect").toRect().isEmpty());
    QVERIFY(inventory->isVisible());
    QVERIFY(inventorySource->menuOpen());
    QCOMPARE(controller.openIndicator(), QStringLiteral("tray-items"));

    auto *childLayer = LayerShellQt::Window::get(child);
    QVERIFY(childLayer);
    // The child rides the same output-covering carrier as every menu; a
    // card-sized surface was the structural reason its push could never read
    // as one piece.
    auto childOutputAnchors = LayerShellQt::Window::Anchors(
        LayerShellQt::Window::AnchorTop
    );
    childOutputAnchors |= LayerShellQt::Window::AnchorLeft;
    childOutputAnchors |= LayerShellQt::Window::AnchorRight;
    childOutputAnchors |= LayerShellQt::Window::AnchorBottom;
    QCOMPARE(childLayer->anchors(), childOutputAnchors);
    QVERIFY(child->width() > 0);
    QVERIFY(child->height() > 0);
    // Card-versus-card separation is pinned by the `adjacentTrayMenuOrigin`
    // unit cases with real compositor geometry; this harness cannot size an
    // output-covering surface (offscreen clamps shown windows to 800x800).

    // Asking another item retires only the child. The inventory remains the
    // exact parent while the replacement D-Bus answer is pending.
    const QString replacementService = QStringLiteral(":1.85");
    const QString replacementPath = QStringLiteral("/replacement");
    QVERIFY(QMetaObject::invokeMethod(
        inventory,
        "itemMenuRequested",
        Q_ARG(QString, replacementService),
        Q_ARG(QString, replacementPath),
        Q_ARG(int, 620),
        Q_ARG(int, 300),
        Q_ARG(int, 48),
        Q_ARG(int, 48)
    ));
    QCOMPARE(needed.size(), 2);
    QTRY_VERIFY(child.isNull());
    QVERIFY(inventory);
    QVERIFY(inventory->isVisible());
    QVERIFY(inventorySource->menuOpen());
    QCOMPARE(controller.openIndicator(), QStringLiteral("tray-items"));

    controller.trayMenuReady(
        replacementService,
        replacementPath,
        trayEntries()
    );
    child = windowWithProperty("entries");
    QVERIFY(child);

    QSignalSpy chosen(&controller, &PanelMenuController::trayEntryTriggered);
    QVERIFY(QMetaObject::invokeMethod(child, "chosen", Q_ARG(int, 7)));
    QCOMPARE(chosen.size(), 1);
    QCOMPARE(chosen.constFirst().at(0).toString(), replacementService);
    QCOMPARE(chosen.constFirst().at(1).toString(), replacementPath);
    QCOMPARE(chosen.constFirst().at(2).toInt(), 7);
    QTRY_VERIFY(child.isNull());
    QVERIFY(inventory);
    QVERIFY(inventory->isVisible());
    QCOMPARE(controller.openIndicator(), QStringLiteral("tray-items"));

    // An empty reply consumes the exact pending request. A duplicate populated
    // reply for that same target must not open a child later.
    const QString emptyService = QStringLiteral(":1.84");
    const QString emptyPath = QStringLiteral("/empty");
    QVERIFY(QMetaObject::invokeMethod(
        inventory,
        "itemMenuRequested",
        Q_ARG(QString, emptyService),
        Q_ARG(QString, emptyPath),
        Q_ARG(int, 620),
        Q_ARG(int, 260),
        Q_ARG(int, 48),
        Q_ARG(int, 48)
    ));
    controller.trayMenuReady(emptyService, emptyPath, QVariantList());
    controller.trayMenuReady(emptyService, emptyPath, trayEntries());
    QCoreApplication::processEvents();
    QVERIFY(!windowWithProperty("entries"));
    QVERIFY(inventory);

    // Closing the parent while a child is mapped closes the complete menu
    // hierarchy, not only the inventory carrier.
    const QString cascadeService = QStringLiteral(":1.87");
    const QString cascadePath = QStringLiteral("/cascade");
    QVERIFY(QMetaObject::invokeMethod(
        inventory,
        "itemMenuRequested",
        Q_ARG(QString, cascadeService),
        Q_ARG(QString, cascadePath),
        Q_ARG(int, 620),
        Q_ARG(int, 300),
        Q_ARG(int, 48),
        Q_ARG(int, 48)
    ));
    controller.trayMenuReady(cascadeService, cascadePath, trayEntries());
    child = windowWithProperty("entries");
    QVERIFY(child);
    // Compositor/QML withdrawal follows the surface's dismissed path rather
    // than the controller's explicit close path. It must retire the complete
    // hierarchy and the panel-side lease just the same.
    inventory->hide();
    QTRY_VERIFY(child.isNull());
    QTRY_VERIFY(inventory.isNull());
    QTRY_VERIFY(!inventorySource->menuOpen());
    QVERIFY(controller.openIndicator().isEmpty());

    controller.toggleTrayItemsMenu(
        panel,
        inventoryOpener,
        inventoryAnchor,
        &tray,
        &providers
    );
    QVERIFY(inventorySource->menuOpen());
    inventory = windowWithProperty("traySource");
    QVERIFY(inventory);

    const QString lateService = QStringLiteral(":1.86");
    const QString latePath = QStringLiteral("/late");
    QVERIFY(QMetaObject::invokeMethod(
        inventory,
        "itemMenuRequested",
        Q_ARG(QString, lateService),
        Q_ARG(QString, latePath),
        Q_ARG(int, 620),
        Q_ARG(int, 340),
        Q_ARG(int, 48),
        Q_ARG(int, 48)
    ));
    controller.close();
    QTRY_VERIFY(inventory.isNull());
    controller.trayMenuReady(lateService, latePath, trayEntries());
    QCoreApplication::processEvents();
    QVERIFY(!windowWithProperty("entries"));

    // A direct bar request has no inventory parent and retains the established
    // full-output outside-click carrier.
    const int beforeDirectRequest = needed.size();
    controller.requestTrayMenu(panel, QPoint(700, 40), service, path);
    controller.requestTrayMenu(panel, QPoint(700, 40), service, path);
    QCOMPARE(needed.size(), beforeDirectRequest + 1);
    controller.trayMenuReady(service, path, trayEntries());
    child = windowWithProperty("entries");
    QVERIFY(child);
    childLayer = LayerShellQt::Window::get(child);
    QVERIFY(childLayer);
    QVERIFY(!child->property("anchoredFromPanel").toBool());
    QVERIFY(child->property("openerRect").toRect().isEmpty());
    auto outputAnchors = LayerShellQt::Window::Anchors(
        LayerShellQt::Window::AnchorTop
    );
    outputAnchors |= LayerShellQt::Window::AnchorBottom;
    outputAnchors |= LayerShellQt::Window::AnchorLeft;
    outputAnchors |= LayerShellQt::Window::AnchorRight;
    QCOMPARE(childLayer->anchors(), outputAnchors);
    QCOMPARE(childLayer->desiredSize(), QSize(0, 0));

    controller.close();
    QTRY_VERIFY(child.isNull());
    QTRY_VERIFY(!inventorySource->menuOpen());
}

void SurfaceManagerTest::wallpaperMenuHandsTheFolderChooserBackToThePermanentPanel()
{
    qunsetenv("CELESTINA_PANEL_MENU");
    registerPanelMenuTypesFromSource();
    QQmlEngine engine;
    engine.addImportPath(QCoreApplication::applicationDirPath());
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    FakeTrayProviderSource providers;
    PanelMenuController controller(&engine, nullptr, nullptr);
    FakePanelWindow panel;
    panel.setGeometry(0, 0, 800, 40);
    panel.show();
    SemanticAttachmentSource source(panel.contentItem());
    source.setPosition(QPointF(690, 5));
    source.setSize(QSizeF(100, 30));
    source.placeAnchor(QPointF(35, 6));
    const QPointF panelGlobal = panel.position();
    const QRectF openerRect(
        panelGlobal + QPointF(720, 6),
        QSizeF(28, 28)
    );
    const QRectF anchorRect = source.attachmentAnchorGlobalRectNow();

    controller.toggleIndicatorMenu(
        &panel,
        openerRect,
        anchorRect,
        QStringLiteral("wallpaper"),
        &providers
    );
    QVERIFY(source.menuOpen());
    QCOMPARE(controller.openIndicator(), QStringLiteral("wallpaper"));

    QWindow *const menu = windowWithProperty("providerSource");
    QVERIFY(menu);
    const QScreen *const screen = panel.screen();
    QVERIFY(screen);
    QVERIFY(menu->property("anchoredFromPanel").toBool());
    QCOMPARE(menu->property("attachmentStartY").toInt(), panel.height());
    QCOMPARE(menu->property("anchorGap").toInt(), 25);
    QCOMPARE(
        menu->property("openerRect").toRectF(),
        panelPopupOpenerOnOutput(
            openerRect,
            QPointF(screen->geometry().topLeft())
        )
    );
    QCOMPARE(
        menu->property("attachmentAnchorRect").toRectF(),
        panelPopupOpenerOnOutput(
            anchorRect,
            QPointF(screen->geometry().topLeft())
        )
    );
    QSignalSpy chooser(&panel, &FakePanelWindow::wallpaperFolderPickerOpened);
    QVERIFY(QMetaObject::invokeMethod(menu, "chooseRequested"));

    QTRY_COMPARE(chooser.count(), 1);
    QVERIFY(panel.hasOpenedWallpaperFolderPicker());
    QVERIFY(!source.menuOpen());
    QVERIFY(controller.openIndicator().isEmpty());
}

void SurfaceManagerTest::aTallGlassCardKeepsItsRoundedRectangle()
{
    const QRect card(40, 60, 530, 732);
    const QRegion glass = roundedGlassRegion(card, 20);

    QCOMPARE(glass.boundingRect(), card);
    QVERIFY(glass.contains(QPoint(card.center().x(), card.top())));
    QVERIFY(glass.contains(QPoint(card.center().x(), card.bottom())));
    QVERIFY(glass.contains(QPoint(card.left(), card.center().y())));
    QVERIFY(glass.contains(QPoint(card.right(), card.center().y())));
    QVERIFY(!glass.contains(card.topLeft()));
    QVERIFY(!glass.contains(card.bottomRight()));
}

void SurfaceManagerTest::aFullWidthBarShapeRemainsFinite()
{
    const QRect bar(0, 0, 1920, 40);
    const QVariantMap shape {
        {QStringLiteral("rect"), bar},
        {QStringLiteral("radius"), 0},
        {QStringLiteral("polygon"), QVariantList()},
    };

    const QRegion glass = glassRegionFromPublishedShapes(QVariantList {shape});
    QCOMPARE(glass, QRegion(bar));
    QCOMPARE(glass.boundingRect(), bar);
    QVERIFY(!glass.contains(QPoint(0, 40)));
}

void SurfaceManagerTest::aPublishedPolygonOverridesItsRoundedBoundingRect()
{
    const QVariantList silhouette {
        QPointF(40, 0),
        QPointF(100, 0),
        QPointF(92, 8),
        QPointF(88, 16),
        QPointF(88, 36),
        QPointF(52, 36),
        QPointF(52, 16),
        QPointF(48, 8),
    };
    const QVariantMap shape {
        {QStringLiteral("rect"), QRectF(0, 0, 140, 40)},
        {QStringLiteral("radius"), 20},
        {QStringLiteral("polygon"), silhouette},
    };

    const QRegion glass = glassRegionFromPublishedShapes(QVariantList {shape});

    QVERIFY(!glass.isEmpty());
    QVERIFY(glass.contains(QPoint(70, 2)));
    QVERIFY(glass.contains(QPoint(70, 30)));
    QVERIFY(!glass.contains(QPoint(5, 20)));
    QVERIFY(!glass.contains(QPoint(120, 20)));
}

void SurfaceManagerTest::anInvalidPolygonFallsBackToRoundedGlass()
{
    const QRectF card(40, 60, 530, 732);
    const QVariantMap malformedPoint {
        {QStringLiteral("x"), 50},
        {QStringLiteral("y"), std::numeric_limits<qreal>::infinity()},
    };
    const QVariantMap shape {
        {QStringLiteral("rect"), card},
        {QStringLiteral("radius"), 20},
        {QStringLiteral("polygon"), QVariantList {
             QPointF(40, 60), malformedPoint, QPointF(570, 792),
         }},
    };

    const QRegion glass = glassRegionFromPublishedShapes(QVariantList {shape});

    QCOMPARE(glass, roundedGlassRegion(card.toRect(), 20));
}

void SurfaceManagerTest::emptyPublishedGlassNeverBecomesFullWindowBlur()
{
    QVERIFY(glassRegionFromPublishedShapes({}, {}).isEmpty());

    const QVariantMap shape {
        {QStringLiteral("polygon"), QVariantList {
             QPointF(10, 10), QPointF(10, 10), QPointF(10, 10),
         }},
    };
    QVERIFY(glassRegionFromPublishedShapes(QVariantList {shape}).isEmpty());
    QVERIFY(!blurProbeCanUseEffect(true, true, false, true, true, false));
}

void SurfaceManagerTest::anArmedBlurSurvivesLayerShellExposureLoss()
{
    // Initial setup still waits for an exposed native surface.
    QVERIFY(blurProbeCanUseEffect(false, true, true, true, true, true));
    QVERIFY(!blurProbeCanUseEffect(false, true, false, true, true, true));

    // After a confirmed arm, a layer-shell exposure report may drop while the
    // same visible surface continues rendering. Keep both its effect and any
    // changed region current rather than switching QML to the opaque fallback.
    QVERIFY(blurProbeCanUseEffect(true, true, false, true, true, true));

    // Real lifecycle and capability losses retain the existing fallback.
    QVERIFY(!blurProbeCanUseEffect(true, false, false, true, true, true));
    QVERIFY(!blurProbeCanUseEffect(true, true, false, false, true, true));
    QVERIFY(!blurProbeCanUseEffect(true, true, false, true, false, true));
    QVERIFY(!blurProbeCanUseEffect(true, true, false, true, true, false));
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
        {QStringLiteral("outputName"), QStringLiteral("test-output")},
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
    QVERIFY(!content->property("anchoredFromPanel").toBool());
    QVERIFY(content->property("openerRect").toRect().isEmpty());
    QVERIFY(content->metaObject()->indexOfSignal("activated(QString,int)") >= 0);
    QVERIFY(content->metaObject()->indexOfSignal("dismissed()") >= 0);
    // The host connects to these by name, so a signature that drifted would
    // fail silently at runtime: the click would reach QML, emit, and land
    // nowhere. This is what makes that impossible to ship unnoticed.
    QVERIFY2(
        content->metaObject()->indexOfSignal("windowActivated(QString)") >= 0,
        "the map must expose windowActivated(QString) for the host to connect"
    );
    QVERIFY(content->metaObject()->indexOfProperty("glassRegions") >= 0);
    const QList<QObject *> outerGlass = content->findChildren<QObject *>(
        QStringLiteral("celestina-compositor-glass-region")
    );
    QCOMPARE(outerGlass.size(), 1);
    QObject *const bodyMaterial = content->findChild<QObject *>(
        QStringLiteral("celestina-menu-body-tint")
    );
    QVERIFY(bodyMaterial);
    QCOMPARE(bodyMaterial->property("backdropMode").toInt(), 1);
    QCOMPARE(bodyMaterial->property("externalBackdropReady").toBool(), true);
    QCOMPARE(bodyMaterial->property("captureActive").toBool(), false);
    QCOMPARE(bodyMaterial->property("elevation").toInt(), 0);
    auto *const quickWindow = qobject_cast<QQuickWindow *>(content);
    auto *const body = qobject_cast<QQuickItem *>(outerGlass.constFirst());
    QVERIFY(quickWindow);
    QVERIFY(body);
    const QPointF bodyOrigin = body->mapToItem(quickWindow->contentItem(), 0, 0);
    QCOMPARE(qRound(bodyOrigin.x()), content->property("cardX").toInt());
    QCOMPARE(qRound(bodyOrigin.y()), content->property("cardY").toInt());
    const QList<QObject *> sections = content->findChildren<QObject *>(
        QStringLiteral("celestina-menu-section")
    );
    QTRY_COMPARE(sections.size(), 2);
    for (QObject *const section : sections) {
        QVERIFY(section->metaObject()->indexOfProperty("captureActive") >= 0);
        QCOMPARE(section->property("backdropMode").toInt(), 1);
        QCOMPARE(section->property("externalBackdropReady").toBool(), true);
        QCOMPARE(section->property("captureActive").toBool(), false);
        QCOMPARE(
            section->findChildren<QObject *>(
                QStringLiteral("celestina-compositor-glass-region")
            ).size(),
            0
        );
    }
    QCOMPARE(
        content->findChildren<QObject *>(
            QStringLiteral("celestina-menu-header")
        ).size(),
        1
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
        {QStringLiteral("outputName"), QStringLiteral("test-output")},
        {QStringLiteral("workspaces"), workspaces},
    });
    QVERIFY2(root, qPrintable(component.errorString()));
    auto *content = qobject_cast<QWindow *>(root);
    QVERIFY(content);

    const int cardWidth = content->property("cardWidth").toInt();
    const int cardHeight = content->property("cardHeight").toInt();
    QCOMPARE(cardWidth, content->property("contentWidth").toInt());
    QCOMPARE(cardHeight, content->property("contentHeight").toInt());
    // The floor here is that the surface cannot collapse below one usable
    // board per workspace.
    QVERIFY2(
        content->height() >= 4 * 24,
        qPrintable(QStringLiteral("card height %1").arg(content->height()))
    );
    QVERIFY(content->width() > 0);

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
        {QStringLiteral("outputName"), QStringLiteral("test-output")},
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
        {QStringLiteral("outputName"), QStringLiteral("test-output")},
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

void SurfaceManagerTest::wallpaperIdentityRejectsMalformedOrDuplicateRows()
{
    const auto providersWithRow = [](const QVariantMap &row) {
        return QVariantMap {
            {QStringLiteral("wallpaper-identity"),
             QVariantMap {
                 {QStringLiteral("outputs"), QVariantList {row}},
             }},
        };
    };
    const QVariantMap valid {
        {QStringLiteral("output"), QStringLiteral("DP-1")},
        {QStringLiteral("source"), QStringLiteral("/wallpapers/bright.png")},
        {QStringLiteral("revision"), QStringLiteral("320:900")},
        {QStringLiteral("generation"), 7.0},
        {QStringLiteral("width"), 1920.0},
        {QStringLiteral("height"), 1080.0},
    };

    QVERIFY(wallpaperIdentityForOutput(
        providersWithRow(valid), QStringLiteral("DP-1"), QSize(1920, 1080)
    ));

    for (const auto &[field, replacement] : {
             std::pair {QStringLiteral("revision"), QVariant(QString())},
             std::pair {QStringLiteral("generation"), QVariant(QStringLiteral("7"))},
             std::pair {QStringLiteral("generation"), QVariant(0.0)},
             std::pair {QStringLiteral("width"), QVariant(1919.0)},
             std::pair {QStringLiteral("source"), QVariant(QString())},
         }) {
        QVariantMap malformed = valid;
        malformed.insert(field, replacement);
        QVERIFY2(
            !wallpaperIdentityForOutput(
                providersWithRow(malformed),
                QStringLiteral("DP-1"),
                QSize(1920, 1080)
            ),
            qPrintable(field)
        );
    }

    const QVariantMap duplicateProviders {
        {QStringLiteral("wallpaper-identity"),
         QVariantMap {
             {QStringLiteral("outputs"), QVariantList {valid, valid}},
         }},
    };
    QVERIFY(!wallpaperIdentityForOutput(
        duplicateProviders, QStringLiteral("DP-1"), QSize(1920, 1080)
    ));
}

void SurfaceManagerTest::wallpaperRevisionChangesTheQmlImageRequest()
{
    QTemporaryDir directory;
    QVERIFY(directory.isValid());
    const QString imagePath = directory.filePath(QStringLiteral("same image #1.png"));
    QImage image(8, 8, QImage::Format_RGBA8888);
    image.fill(Qt::black);
    QVERIFY(image.save(imagePath));

    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));
    QQmlComponent component(
        &engine,
        QUrl::fromLocalFile(
            QStringLiteral(CELESTINA_QML_DIR "/Wallpaper.qml")
        )
    );
    QVERIFY2(component.isReady(), qPrintable(component.errorString()));

    std::unique_ptr<QObject> root(component.createWithInitialProperties({
        {QStringLiteral("source"), imagePath},
        {QStringLiteral("sourceUrl"), QUrl::fromLocalFile(imagePath)},
        {QStringLiteral("sourceRevision"), QStringLiteral("640:1")},
        {QStringLiteral("sourceGeneration"), 4.0},
        {QStringLiteral("sourceWidth"), 1920},
        {QStringLiteral("sourceHeight"), 1080},
        {QStringLiteral("outputName"), QStringLiteral("DP-1")},
        {QStringLiteral("reducedMotion"), true},
    }));
    QVERIFY2(root, qPrintable(component.errorString()));
    QTRY_VERIFY_WITH_TIMEOUT(root->property("showingImage").toBool(), 2000);
    QCOMPARE(root->property("readyRevision").toString(), QStringLiteral("640:1"));

    const QUrl firstRequest = root->property("imageSource").toUrl();
    QCOMPARE(firstRequest.toLocalFile(), imagePath);
    QVERIFY(firstRequest.toEncoded().contains("same%20image%20%231.png"));
    QCOMPARE(
        firstRequest.fragment(),
        QStringLiteral(
            "celestina-revision=640%3A1&celestina-generation=4&"
            "celestina-geometry=1920x1080"
        )
    );
    image.fill(Qt::white);
    QVERIFY(image.save(imagePath));
    root->setProperty("sourceRevision", QStringLiteral("640:2"));
    const QUrl secondRequest = root->property("imageSource").toUrl();
    QVERIFY(firstRequest != secondRequest);
    QCOMPARE(secondRequest.toLocalFile(), imagePath);
    QVERIFY(secondRequest.fragment().contains(
        QStringLiteral("celestina-revision=640%3A2")
    ));
    QTRY_COMPARE_WITH_TIMEOUT(
        root->property("readyRevision").toString(),
        QStringLiteral("640:2"),
        2000
    );
    QVERIFY(root->property("showingImage").toBool());
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
void SurfaceManagerTest::thePanelOverlayPrototypeLoadsAndMaps()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &fileName : {
             QStringLiteral("LauncherOverlay.qml"),
             QStringLiteral("ClipboardOverlay.qml"),
             QStringLiteral("ControlCentre.qml"),
         }) {
        QQmlComponent component(
            &engine,
            QUrl::fromLocalFile(QStringLiteral(CELESTINA_QML_DIR "/") + fileName)
        );
        QVERIFY2(component.isReady(), qPrintable(component.errorString()));

        QVariantMap properties {
            {QStringLiteral("reducedMotion"), true},
            {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(nullptr)},
        };
        const bool panelPrototype = fileName == QStringLiteral("ControlCentre.qml");
        if (panelPrototype) {
            properties.insert(QStringLiteral("anchoredFromPanel"), true);
            properties.insert(QStringLiteral("openerRect"), QRect(712, 6, 28, 28));
            properties.insert(QStringLiteral("attachmentStartY"), 40);
        }
        QObject *root = component.createWithInitialProperties(properties);
        QVERIFY2(root, qPrintable(component.errorString()));

        if (panelPrototype) {
            QVERIFY(root->property("anchoredFromPanel").toBool());
            QCOMPARE(root->property("openerRect").toRect(), QRect(712, 6, 28, 28));
            QCOMPARE(root->property("attachmentStartY").toInt(), 40);
        }

        auto *content = qobject_cast<QWindow *>(root);
        QVERIFY2(content, qPrintable(fileName));
        QVERIFY2(content->metaObject()->indexOfSignal("dismissed()") >= 0, qPrintable(fileName));

        OverlaySurface surface(OverlaySurface::Placement::Centered);
        QVERIFY2(surface.open(content, nullptr), qPrintable(fileName));
    }
}

QTEST_MAIN(SurfaceManagerTest)

#include "surfacemanager_test.moc"
