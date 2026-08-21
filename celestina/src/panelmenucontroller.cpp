#include "panelmenucontroller.h"

#include "quietplacement.h"

#include "diagnosticjournal.h"

#include <QDebug>
#include <QQmlEngine>
#include <QScreen>
#include <QTimer>
#include <QVariantMap>
#include <QWindow>

#include "niriclient.h"
#include "panelpopupplacement.h"
#include "panelmenusurface.h"
#include "shellscale.h"
#include "softclose.h"

namespace {
// Where the card goes inside a surface that now covers the whole output.
//
// The surface covers the output, so both coordinates come from the real panel
// control. This is what lets a soft menu sit immediately beneath a moved,
// resized or stacked panel instead of inheriting a fixed top edge.
void placeCardOnOutput(QWindow *card, const QPoint &bodyOrigin)
{
    card->setProperty("menuX", bodyOrigin.x());
    card->setProperty("menuY", bodyOrigin.y());
}

void placeCard(QWindow *card, QWindow *panel, const QPoint &globalBodyOrigin)
{
    const QScreen *const screen = panel->screen();
    const QPoint outputOrigin = screen ? screen->geometry().topLeft() : QPoint();
    const QPoint origin = globalBodyOrigin - outputOrigin;

    card->setProperty("menuX", origin.x());
    card->setProperty("menuY", origin.y());
}

QVariantMap menuOutputProperties(QWindow *panel)
{
    const QScreen *const screen = panel ? panel->screen() : nullptr;
    return QVariantMap {
        {QStringLiteral("outputName"), screen ? screen->name() : QString()},
        // Every contextual surface is drawn at the size its output asks for,
        // exactly as the panel it comes from is; see shellscale.h. The
        // geometry handed to it below is divided by this, so the QML lays out
        // in the unscaled units its tokens are written in.
        {QStringLiteral("shellScale"), shellScaleForScreen(screen)},
    };
}

/// The factor a surface on this panel's output draws at.
double menuShellScale(QWindow *panel)
{
    return shellScaleForScreen(panel ? panel->screen() : nullptr);
}

QRectF openerOnOutput(QWindow *panel, const QRectF &globalOpener)
{
    const QScreen *const screen = panel ? panel->screen() : nullptr;
    if (!screen || globalOpener.isEmpty())
        return QRectF();

    return panelPopupOpenerOnOutput(
        globalOpener,
        QPointF(screen->geometry().topLeft())
    );
}

struct PanelCarrierGeometry {
    QPoint outputPosition;
    QRectF opener;
    QRectF attachmentAnchor;
    int attachmentStartY = 0;
};

// A panel-attached layer window begins at the panel's physical lower seam.
// Everything the QML scene consumes is translated into that window's local
// coordinate space once here. Floating and side-attached routes never call
// this helper and retain their established output-local geometry.
PanelCarrierGeometry panelCarrierGeometry(
    QWindow *panel,
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor
)
{
    PanelCarrierGeometry geometry;
    geometry.outputPosition = QPoint(0, qMax(0, panel ? panel->height() : 0));

    const QPointF translation(-geometry.outputPosition.x(),
                              -geometry.outputPosition.y());
    const QRectF outputOpener = openerOnOutput(panel, globalOpener);
    if (!outputOpener.isEmpty())
        geometry.opener = outputOpener.translated(translation);
    const QRectF outputAnchor = openerOnOutput(panel, globalAttachmentAnchor);
    if (!outputAnchor.isEmpty())
        geometry.attachmentAnchor = outputAnchor.translated(translation);
    return geometry;
}

/// Output pixels into the units the surface lays out in.
QRectF inShellUnits(const QRectF &rect, double scale)
{
    if (scale <= 0)
        return rect;

    return QRectF(rect.x() / scale, rect.y() / scale,
                  rect.width() / scale, rect.height() / scale);
}

void addPanelOpenerProperties(
    QVariantMap &properties,
    const QRectF &opener,
    const QRectF &attachmentAnchor,
    int attachmentStartY,
    double shellScale
)
{
    if (opener.isEmpty())
        return;

    // The opener, the icon inside it and the bar's lower edge all arrive in
    // output pixels because that is what the panel's own geometry is measured
    // in. The surface lays out in unscaled units, so they are converted once
    // here rather than at every place the QML reads them.
    properties.insert(QStringLiteral("anchoredFromPanel"), true);
    properties.insert(
        QStringLiteral("openerRect"), inShellUnits(opener, shellScale));
    properties.insert(
        QStringLiteral("attachmentAnchorRect"),
        inShellUnits(attachmentAnchor, shellScale));
    properties.insert(
        QStringLiteral("attachmentStartY"),
        shellScale > 0 ? qMax(0, attachmentStartY) / shellScale
                       : qMax(0, attachmentStartY)
    );
}

/// The factor a mapped surface draws at, or 1 when it has not been told.
double surfaceShellScale(const QWindow *surface)
{
    if (!surface)
        return 1.0;
    const double scale = surface->property("shellScale").toDouble();
    return scale > 0.0 ? scale : 1.0;
}

// Ask a popup-backed surface to enter its ordinary dismissal lifecycle. The
// menu may already be inside aboutToHide when an activation signal reaches
// C++; a visible popup starts the lifecycle, and a retiring carrier proves it
// already started. A dormant popup does neither, so callers retain their hard
// teardown fallback for a surface that never opened.
bool requestPopupDismissal(QWindow *window)
{
    if (!window)
        return false;
    QObject *const menu = window->property("menu").value<QObject *>();
    if (!menu)
        return false;
    if (menu->property("visible").toBool())
        return QMetaObject::invokeMethod(menu, "close");
    return window->property("celestinaRetiring").toBool();
}

void capCardHeightBelowAnchor(
    QWindow *card,
    QWindow *panel,
    const QPoint &globalAnchor
)
{
    const QScreen *const screen = panel ? panel->screen() : nullptr;
    if (!card || !screen)
        return;

    // A foreign application can publish the full bounded DBusMenu inventory.
    // Keep that natural height for Menu's internal ListView, but make the card
    // itself a viewport no taller than the output space below the real request.
    // Using the complete output height would force an overflowing card to y=0
    // when the layer-surface clamp runs, detaching it from its invoking tile.
    // Without a cap, the child surface instead adopts the complete model height,
    // so there is nothing for Qt to scroll and lower actions are off-screen.
    const QRect output = screen->geometry();
    const int outputHeight = qMax(1, output.height());
    const int requestedTop = qBound(
        0,
        globalAnchor.y() - output.top(),
        outputHeight - 1
    );
    // The card lays out in the shell's unscaled units; the output and the
    // anchor are real pixels. Capped with the real number, a scaled output
    // was 15 % more generous than the space below the anchor really is, and
    // the child card ran past the screen's bottom edge.
    const double scale = surfaceShellScale(card);
    const int availableInShellUnits =
        qMax(1, qRound((outputHeight - requestedTop) / scale));
    const int outputInShellUnits = qMax(1, qRound(outputHeight / scale));
    const int minimumViewportHeight = qBound(
        1,
        card->property("minimumMenuViewportHeight").toInt(),
        outputInShellUnits
    );
    card->setProperty(
        "maximumContentHeight",
        qBound(
            minimumViewportHeight,
            availableInShellUnits,
            outputInShellUnits
        )
    );
}
} // namespace

QPoint adjacentTrayMenuOrigin(
    const QRect &parentCard,
    const QPoint &requestedAnchor,
    const QSize &childSize,
    const QSize &outputSize,
    int gap
)
{
    const int safeGap = qMax(0, gap);
    const int childWidth = qMax(0, childSize.width());
    const int childHeight = qMax(0, childSize.height());
    const int maximumX = qMax(0, outputSize.width() - childWidth);
    const int maximumY = qMax(0, outputSize.height() - childHeight);

    const int rightOrigin = parentCard.right() + 1 + safeGap;
    const int leftOrigin = parentCard.left() - safeGap - childWidth;
    const int rightRoom = outputSize.width() - rightOrigin;
    const int leftRoom = parentCard.left() - safeGap;

    int requestedX = rightOrigin;
    if (rightRoom < childWidth && leftRoom >= childWidth)
        requestedX = leftOrigin;
    else if (rightRoom < childWidth && leftRoom < childWidth)
        requestedX = leftRoom > rightRoom ? leftOrigin : rightOrigin;

    return QPoint(
        qBound(0, requestedX, maximumX),
        qBound(0, requestedAnchor.y(), maximumY)
    );
}

PanelMenuController::PanelMenuController(
    QQmlEngine *engine,
    NiriClient *niri,
    QObject *parent
)
    : QObject(parent)
    , m_trayComponent(engine)
    , m_trayItemsComponent(engine)
    , m_networkComponent(engine)
    , m_bluetoothComponent(engine)
    , m_performanceComponent(engine)
    , m_brightnessComponent(engine)
    , m_calendarComponent(engine)
    , m_phoneComponent(engine)
    , m_audioComponent(engine)
    , m_captureComponent(engine)
    , m_wallpaperComponent(engine)
    , m_workspaceMapComponent(engine)
    , m_niri(niri)
    , m_surface(new PanelMenuSurface(this))
    , m_trayChildSurface(new PanelMenuSurface(this))
    , m_enabled(enabledByEnvironment())
{
    // A layer-shell dismissal can hide the carrier without first closing its
    // QML Menu. Keep controller identity in sync with both lifecycle paths.
    connect(m_surface, &PanelMenuSurface::dismissed, this, [this]() {
        close();
    });
    connect(m_trayChildSurface, &PanelMenuSurface::dismissed, this, [this]() {
        closeTrayChild(true);
    });

    if (!m_enabled)
        return;

    m_trayComponent.loadFromModule("CelestinaDesktop", "TrayMenu");
    m_trayItemsComponent.loadFromModule("CelestinaDesktop", "TrayItemsMenu");
    if (!m_trayItemsComponent.isReady()) {
        qCritical().noquote()
            << "Celestina could not load the tray items menu:"
            << m_trayItemsComponent.errorString();
    }
    // An indicator menu that will not load leaves its indicator inert rather
    // than opening an empty surface; the panel keeps working either way.
    m_networkComponent.loadFromModule("CelestinaDesktop", "NetworkMenu");
    m_bluetoothComponent.loadFromModule("CelestinaDesktop", "BluetoothMenu");
    m_performanceComponent.loadFromModule("CelestinaDesktop", "PerformanceMenu");
    m_brightnessComponent.loadFromModule("CelestinaDesktop", "BrightnessMenu");
    m_calendarComponent.loadFromModule("CelestinaDesktop", "CalendarMenu");
    m_phoneComponent.loadFromModule("CelestinaDesktop", "PhoneMenu");
    m_audioComponent.loadFromModule("CelestinaDesktop", "AudioMenu");
    m_captureComponent.loadFromModule("CelestinaDesktop", "CaptureMenu");
    m_wallpaperComponent.loadFromModule("CelestinaDesktop", "WallpaperMenu");
    for (const QQmlComponent *indicator : {
             &m_networkComponent,
             &m_bluetoothComponent,
             &m_performanceComponent,
             &m_brightnessComponent,
             &m_calendarComponent,
             &m_phoneComponent,
             &m_audioComponent,
             &m_captureComponent,
             &m_wallpaperComponent,
         }) {
        if (!indicator->isReady()) {
            qCritical().noquote()
                << "Celestina could not load an indicator menu:"
                << indicator->errorString();
        }
    }
    // A map that will not load leaves its capsule inert rather than opening an
    // empty surface, exactly as an indicator menu does.
    m_workspaceMapComponent.loadFromModule("CelestinaDesktop", "WorkspaceMap");
    if (!m_workspaceMapComponent.isReady()) {
        qCritical().noquote()
            << "Celestina could not load the workspace map:"
            << m_workspaceMapComponent.errorString();
    }
}

bool PanelMenuController::enabledByEnvironment()
{
    const QByteArray requested = qgetenv("CELESTINA_PANEL_MENU").trimmed().toLower();
    if (requested.isEmpty() || requested == "1" || requested == "true")
        return true;
    if (requested == "0" || requested == "false")
        return false;

    // An unreadable value is not a request to remove a working menu.
    qWarning().noquote()
        << "Celestina ignored an unreadable CELESTINA_PANEL_MENU:" << requested;
    return true;
}

void PanelMenuController::activateWindow(const QString &windowId)
{
    if (m_niri)
        m_niri->requestWindowFocus(windowId);

    // The map has said everything it can say: the answer to this request is the
    // session moving, and it arrives on screen rather than on any control here.
    close();
}

void PanelMenuController::activate(const QString &output, int index)
{
    if (m_niri)
        m_niri->requestWorkspaceFocus(output, index);

    // The answer arrives on the panel's own pill, where the request is
    // visible; the menu has said everything it can say.
    close();
}

void PanelMenuController::openWorkspaceMap(
    QWindow *panel,
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor,
    const QVariant &workspaces
)
{
    if (!m_enabled || !panel || !m_niri || !m_workspaceMapComponent.isReady())
        return;

    close();
    // The map never reuses an indicator's parked carrier: a mapped surface
    // cannot change its window, so anything resting here is put away first.
    if (m_surface->isParked()) {
        m_parkedMenuKind.clear();
        m_surface->close();
    }

    QVariantMap initialProperties {
        {QStringLiteral("workspaces"), workspaces},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    // The invoking workspace dot is a real panel control: the map attaches
    // with the same droplet membrane as every other panel-opened surface.
    const PanelCarrierGeometry carrier = panelCarrierGeometry(
        panel, globalOpener, globalAttachmentAnchor);
    // What the panel actually hands the surface, per output. The membrane is
    // suppressed when the anchor arrives empty or off-surface, and that is a
    // per-output arithmetic question — the global rectangle comes from Qt,
    // whose idea of where a layer surface sits is not guaranteed to include
    // the output's own origin. Recorded rather than inferred.
    {
        const QScreen *const s = panel ? panel->screen() : nullptr;
        DiagnosticJournal::instance().record(
            DiagnosticJournal::Record(
                DiagnosticJournal::Level::Info,
                QStringLiteral("attachment.carrier"))
                .text(QStringLiteral("route"), QStringLiteral("workspace-map"))
                .text(QStringLiteral("output"), s ? s->name() : QString())
                .number(QStringLiteral("screen_x"), s ? s->geometry().x() : -1)
                .number(QStringLiteral("screen_y"), s ? s->geometry().y() : -1)
                .number(QStringLiteral("global_anchor_x"),
                        qRound(globalAttachmentAnchor.x()))
                .number(QStringLiteral("global_anchor_y"),
                        qRound(globalAttachmentAnchor.y()))
                .number(QStringLiteral("anchor_x"), qRound(carrier.attachmentAnchor.x()))
                .number(QStringLiteral("anchor_y"), qRound(carrier.attachmentAnchor.y()))
                .number(QStringLiteral("anchor_w"), qRound(carrier.attachmentAnchor.width()))
                .number(QStringLiteral("opener_x"), qRound(carrier.opener.x()))
                .number(QStringLiteral("opener_y"), qRound(carrier.opener.y()))
                .number(QStringLiteral("start_y"), carrier.attachmentStartY)
        );
    }
    addPanelOpenerProperties(
        initialProperties,
        carrier.opener,
        carrier.attachmentAnchor,
        carrier.attachmentStartY,
        menuShellScale(panel));
    initialProperties.insert(menuOutputProperties(panel));
    QObject *rootObject =
        m_workspaceMapComponent.createWithInitialProperties(initialProperties);
    auto *card = qobject_cast<QWindow *>(rootObject);
    if (!card) {
        qCritical().noquote()
            << "Celestina could not create the workspace map:"
            << m_workspaceMapComponent.errorString();
        delete rootObject;
        return;
    }

    // Read back from the object that was actually built, not from what was
    // handed to the constructor. The membrane's own precondition lives in QML
    // (`topAttachmentRequested`), and every term of it is exposed here as an
    // alias — so whichever one arrives false is the answer, and no console
    // logging is involved, which this stack has already proved unreliable.
    {
        const QRectF ro = card->property("openerRect").toRectF();
        const QRectF ra = card->property("attachmentAnchorRect").toRectF();
        DiagnosticJournal::instance().record(
            DiagnosticJournal::Record(
                DiagnosticJournal::Level::Info,
                QStringLiteral("attachment.readback"))
                .text(QStringLiteral("anchored"),
                      card->property("anchoredFromPanel").toBool()
                          ? QStringLiteral("true") : QStringLiteral("false"))
                .number(QStringLiteral("opener_w"), qRound(ro.width()))
                .number(QStringLiteral("opener_h"), qRound(ro.height()))
                .number(QStringLiteral("anchor_w"), qRound(ra.width()))
                .number(QStringLiteral("anchor_h"), qRound(ra.height()))
                .number(QStringLiteral("start_y"),
                        card->property("attachmentStartY").toInt())
                .number(QStringLiteral("card_y"),
                        qRound(card->property("cardY").toDouble()))
                .number(QStringLiteral("card_x"),
                        qRound(card->property("cardX").toDouble()))
        );
    }

    // The same two signals the panel menu declares, so the surface that answers
    // a capsule and the one that answers a right click are interchangeable to
    // this controller.
    connect(card, SIGNAL(activated(QString, int)), this, SLOT(activate(QString, int)));
    connect(card, SIGNAL(windowActivated(QString)), this, SLOT(activateWindow(QString)));
    connect(card, SIGNAL(dismissed()), this, SLOT(menuDismissed()));

    placeCardOnOutput(
        card,
        panelPopupBodyOrigin(
            carrier.opener,
            card->property("contentWidth").toInt(),
            card->property("anchorGap").toInt(),
            carrier.attachmentStartY
        )
    );
    passPanelStripThrough(card, panel);
    if (!m_surface->open(
            card,
            panel,
            PanelMenuSurface::Coverage::Output,
            carrier.outputPosition
        )) {
        delete card;
        return;
    }
    m_attachmentLease.acquire(
        panel,
        card,
        globalAttachmentAnchor,
        QPointF(carrier.outputPosition));
}

QRectF PanelMenuController::openCardRectOnOutput(QScreen *screen) const
{
    // A parked carrier is still a mapped, visible window; it occupies no
    // zone. Only an actually open menu answers.
    const QRectF card = quietOpenCardRect(
        m_surface && m_surface->isOpen() ? m_surface->window() : nullptr,
        screen);
    if (!card.isEmpty())
        return card;
    return quietOpenCardRect(
        m_trayChildSurface ? m_trayChildSurface->window() : nullptr, screen);
}

QString indicatorMenuComponent(const QString &kind)
{
    if (kind == QStringLiteral("network"))
        return QStringLiteral("NetworkMenu");
    if (kind == QStringLiteral("bluetooth"))
        return QStringLiteral("BluetoothMenu");
    if (kind == QStringLiteral("performance"))
        return QStringLiteral("PerformanceMenu");
    if (kind == QStringLiteral("brightness"))
        return QStringLiteral("BrightnessMenu");
    if (kind == QStringLiteral("calendar"))
        return QStringLiteral("CalendarMenu");
    if (kind == QStringLiteral("phone"))
        return QStringLiteral("PhoneMenu");
    if (kind == QStringLiteral("audio"))
        return QStringLiteral("AudioMenu");
    if (kind == QStringLiteral("capture"))
        return QStringLiteral("CaptureMenu");
    if (kind == QStringLiteral("wallpaper"))
        return QStringLiteral("WallpaperMenu");

    return QString();
}

void PanelMenuController::toggleIndicatorMenu(
    QWindow *panel,
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor,
    const QString &kind,
    QObject *providerSource
)
{
    const bool needsProvider = kind != QStringLiteral("capture")
        && kind != QStringLiteral("calendar");
    if (!m_enabled || !panel || (needsProvider && !providerSource)) {
        DiagnosticJournal::instance().record(
            DiagnosticJournal::Record(
                DiagnosticJournal::Level::Warn,
                QStringLiteral("ctx.menu_dropped"))
                .text(QStringLiteral("kind"), kind)
                .flag(QStringLiteral("enabled"), m_enabled)
                .flag(QStringLiteral("panel"), panel != nullptr)
                .flag(QStringLiteral("source"), providerSource != nullptr)
        );
        return;
    }
    if (indicatorMenuComponent(kind).isEmpty()) {
        qWarning() << "Celestina has no indicator menu named" << kind;
        return;
    }

    // The same indicator again is a request to put it away. In a live session
    // the click never gets this far — the open surface covers the output and
    // answers it first — but a host that reopened here would resurrect the
    // defect where the first click did nothing visible and only the second
    // closed the menu.
    const bool sameAgain = (m_openMenuKind == kind);
    // The opener rectangle travels into the record because "the menu opened
    // disconnected at the left edge" has already happened and the journal
    // could not say what geometry the gesture actually delivered. An empty
    // rect here is the whole explanation of a floating menu.
    DiagnosticJournal::instance().record(
        DiagnosticJournal::Record(
            DiagnosticJournal::Level::Info,
            QStringLiteral("ctx.menu"))
            .text(QStringLiteral("kind"), kind)
            .text(QStringLiteral("open_before"), m_openMenuKind)
            .flag(QStringLiteral("same_again"), sameAgain)
            .number(QStringLiteral("opener_x"), qRound64(globalOpener.x()))
            .number(QStringLiteral("opener_y"), qRound64(globalOpener.y()))
            .number(QStringLiteral("opener_width"), qRound64(globalOpener.width()))
            .number(QStringLiteral("opener_height"), qRound64(globalOpener.height()))
    );
    if (sameAgain) {
        if (!requestPopupDismissal(m_surface->window()))
            close();
        return;
    }
    close();

    // A carrier parked holding this same kind on this same output resumes in
    // place instead of remapping — the scene change the park exists to
    // avoid. Anything else parked is put away for real, because a mapped
    // surface can change neither its window nor its screen.
    QWindow *reused = nullptr;
    if (m_surface->isParked()) {
        QWindow *const parked = m_surface->window();
        if (parked && parked->handle()
            && m_parkedMenuKind == kind
            && panel->screen() && panel->screen() == parked->screen()) {
            reused = parked;
        } else {
            m_parkedMenuKind.clear();
            m_surface->close();
        }
    }

    QQmlComponent *component = nullptr;
    if (!reused) {
        if (kind == QStringLiteral("network"))
            component = &m_networkComponent;
        else if (kind == QStringLiteral("bluetooth"))
            component = &m_bluetoothComponent;
        else if (kind == QStringLiteral("performance"))
            component = &m_performanceComponent;
        else if (kind == QStringLiteral("brightness"))
            component = &m_brightnessComponent;
        else if (kind == QStringLiteral("calendar"))
            component = &m_calendarComponent;
        else if (kind == QStringLiteral("phone"))
            component = &m_phoneComponent;
        else if (kind == QStringLiteral("audio"))
            component = &m_audioComponent;
        else if (kind == QStringLiteral("capture"))
            component = &m_captureComponent;
        else if (kind == QStringLiteral("wallpaper"))
            component = &m_wallpaperComponent;

        if (!component || !component->isReady())
            return;
    }

    QVariantMap initialProperties {
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    const PanelCarrierGeometry carrier = panelCarrierGeometry(
        panel, globalOpener, globalAttachmentAnchor);
    {
        const QScreen *const s = panel ? panel->screen() : nullptr;
        DiagnosticJournal::instance().record(
            DiagnosticJournal::Record(
                DiagnosticJournal::Level::Info,
                QStringLiteral("attachment.carrier"))
                .text(QStringLiteral("route"), QStringLiteral("indicator"))
                .text(QStringLiteral("output"), s ? s->name() : QString())
                .number(QStringLiteral("anchor_x"), qRound(carrier.attachmentAnchor.x()))
                .number(QStringLiteral("anchor_y"), qRound(carrier.attachmentAnchor.y()))
                .number(QStringLiteral("anchor_w"), qRound(carrier.attachmentAnchor.width()))
                .number(QStringLiteral("opener_x"), qRound(carrier.opener.x()))
                .number(QStringLiteral("opener_y"), qRound(carrier.opener.y()))
                .number(QStringLiteral("start_y"), carrier.attachmentStartY)
        );
    }
    addPanelOpenerProperties(
        initialProperties,
        carrier.opener,
        carrier.attachmentAnchor,
        carrier.attachmentStartY,
        menuShellScale(panel));
    if (needsProvider) {
        initialProperties.insert(
            QStringLiteral("providerSource"),
            QVariant::fromValue(providerSource)
        );
    }
    initialProperties.insert(menuOutputProperties(panel));

    QWindow *window = reused;
    if (reused) {
        // The window keeps the connections its creation made; only its
        // route, its revived fields and the mask followers below change.
        reviveSoftClosedWindow(reused);
        for (auto it = initialProperties.constBegin();
             it != initialProperties.constEnd(); ++it) {
            reused->setProperty(it.key().toUtf8().constData(), it.value());
        }
        QObject::disconnect(
            reused, &QWindow::widthChanged, reused, nullptr);
        QObject::disconnect(
            reused, &QWindow::heightChanged, reused, nullptr);
    } else {
        QObject *rootObject =
            component->createWithInitialProperties(initialProperties);
        window = qobject_cast<QWindow *>(rootObject);
        if (!window) {
            qCritical().noquote()
                << "Celestina could not create the" << kind
                << "menu:" << component->errorString();
            delete rootObject;
            return;
        }

        connect(window, SIGNAL(dismissed()), this, SLOT(menuDismissed()));
        if (kind == QStringLiteral("capture")) {
            connect(
                window,
                SIGNAL(captureRequested()),
                this,
                SLOT(captureScreenshot())
            );
        } else if (kind == QStringLiteral("wallpaper")) {
            connect(
                window,
                SIGNAL(chooseRequested()),
                this,
                SLOT(chooseWallpaperFolder())
            );
        }
    }

    const int contentWidth = window->property("contentWidth").toInt();
    const int anchorGap = window->property("anchorGap").toInt();
    placeCardOnOutput(
        window,
        panelPopupBodyOrigin(
            carrier.opener,
            contentWidth,
            anchorGap,
            carrier.attachmentStartY)
    );
    passPanelStripThrough(window, panel);
    if (!m_surface->open(
            window,
            panel,
            PanelMenuSurface::Coverage::Output,
            carrier.outputPosition
        )) {
        // A refused resume leaves the surface parked and its window adopted;
        // only a fresh window this controller still owns is deleted here.
        if (!reused)
            delete window;
        return;
    }

    m_parkedMenuKind.clear();
    m_openMenuKind = kind;
    emit contextualSurfaceOpened();
    m_openIndicatorPanel = panel;
    m_attachmentLease.acquire(
        panel,
        window,
        globalAttachmentAnchor,
        QPointF(carrier.outputPosition));
}

void PanelMenuController::captureScreenshot()
{
    // A retiring menu can finish a transition after another contextual menu
    // has replaced it. Only the menu currently carried by the surface may ask
    // Niri for a capture.
    if (sender() != m_surface->window())
        return;

    if (m_niri)
        m_niri->requestScreenshot();
    if (!requestPopupDismissal(m_surface->window()))
        close();
}

void PanelMenuController::chooseWallpaperFolder()
{
    if (sender() != m_surface->window()
        || m_openMenuKind != QStringLiteral("wallpaper")
        || !m_openIndicatorPanel) {
        return;
    }

    const QPointer<QWindow> panel = m_openIndicatorPanel;
    close();
    QTimer::singleShot(0, panel, [panel]() {
        if (panel)
            QMetaObject::invokeMethod(panel, "openWallpaperFolderPicker");
    });
}

void PanelMenuController::toggleTrayItemsMenu(
    QWindow *panel,
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor,
    QObject *traySource,
    QObject *providerSource
)
{
    constexpr auto trayItemsKind = "tray-items";
    if (!m_enabled || !panel || !traySource || !providerSource
        || !m_trayItemsComponent.isReady()) {
        return;
    }

    const bool sameAgain = (m_openMenuKind == QLatin1String(trayItemsKind));
    DiagnosticJournal::instance().record(
        DiagnosticJournal::Record(
            DiagnosticJournal::Level::Info,
            QStringLiteral("ctx.menu"))
            .text(QStringLiteral("kind"), QLatin1String(trayItemsKind))
            .text(QStringLiteral("open_before"), m_openMenuKind)
            .flag(QStringLiteral("same_again"), sameAgain)
    );
    if (sameAgain) {
        if (!requestPopupDismissal(m_surface->window()))
            close();
        return;
    }
    close();
    // The popup-backed inventory never reuses an indicator's parked carrier:
    // a mapped surface cannot change its window, so anything resting here is
    // put away first.
    if (m_surface->isParked()) {
        m_parkedMenuKind.clear();
        m_surface->close();
    }

    QVariantMap initialProperties {
        {QStringLiteral("traySource"), QVariant::fromValue(traySource)},
        {QStringLiteral("providerSource"), QVariant::fromValue(providerSource)},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    const PanelCarrierGeometry carrier = panelCarrierGeometry(
        panel, globalOpener, globalAttachmentAnchor);
    addPanelOpenerProperties(
        initialProperties,
        carrier.opener,
        carrier.attachmentAnchor,
        carrier.attachmentStartY,
        menuShellScale(panel));
    initialProperties.insert(menuOutputProperties(panel));
    QObject *rootObject =
        m_trayItemsComponent.createWithInitialProperties(initialProperties);
    auto *window = qobject_cast<QWindow *>(rootObject);
    if (!window) {
        qCritical().noquote()
            << "Celestina could not create the tray items menu:"
            << m_trayItemsComponent.errorString();
        delete rootObject;
        return;
    }

    connect(
        window,
        SIGNAL(activated(QString, QString, int, int)),
        this,
        SLOT(activateTrayItem(QString, QString, int, int))
    );
    connect(
        window,
        SIGNAL(secondaryActivated(QString, QString, int, int)),
        this,
        SLOT(secondaryActivateTrayItem(QString, QString, int, int))
    );
    connect(
        window,
        SIGNAL(itemMenuRequested(QString, QString, QString, int, int, int, int)),
        this,
        SLOT(requestTrayItemMenu(QString, QString, QString, int, int, int, int))
    );
    connect(window, SIGNAL(dismissed()), this, SLOT(menuDismissed()));

    const int contentWidth = window->property("contentWidth").toInt();
    const int anchorGap = window->property("anchorGap").toInt();
    const QScreen *const screen = panel->screen();
    const QPoint bodyOrigin = panelPopupBodyOrigin(
        carrier.opener,
        contentWidth,
        anchorGap,
        carrier.attachmentStartY
    );
    placeCardOnOutput(window, bodyOrigin);
    if (screen) {
        // The tray inventory follows live model and preference snapshots. Its
        // opener-relative top stays fixed; when the rows no longer fit below it,
        // the real Menu scrolls inside the remaining output height instead of
        // moving the whole card over the panel.
        window->setProperty(
            "maximumContentHeight",
            qMax(1, screen->geometry().height()
                     - carrier.outputPosition.y() - bodyOrigin.y())
        );
    }
    passPanelStripThrough(window, panel);
    if (!m_surface->open(
            window,
            panel,
            PanelMenuSurface::Coverage::Output,
            carrier.outputPosition
        )) {
        delete window;
        return;
    }

    m_openMenuKind = QLatin1String(trayItemsKind);
    emit contextualSurfaceOpened();
    m_openPanel = panel;
    m_attachmentLease.acquire(
        panel,
        window,
        globalAttachmentAnchor,
        QPointF(carrier.outputPosition));
}

void PanelMenuController::activateTrayItem(
    const QString &service,
    const QString &path,
    int globalX,
    int globalY
)
{
    if (sender() != m_surface->window())
        return;

    emit trayItemActivated(service, path, globalX, globalY);
    if (!requestPopupDismissal(m_surface->window()))
        close();
}

void PanelMenuController::secondaryActivateTrayItem(
    const QString &service,
    const QString &path,
    int globalX,
    int globalY
)
{
    if (sender() != m_surface->window())
        return;

    emit trayItemSecondaryActivated(service, path, globalX, globalY);
    if (!requestPopupDismissal(m_surface->window()))
        close();
}

void PanelMenuController::requestTrayItemMenu(
    const QString &service,
    const QString &path,
    const QString &appName,
    int globalX,
    int globalY,
    int globalWidth,
    int globalHeight
)
{
    constexpr auto trayItemsKind = "tray-items";
    QWindow *const parentMenu = m_surface->window();
    // Who asks, and how often: the child was observed being rebuilt roughly
    // every 650 ms with no user gesture, and this is the only door that
    // rebuilds it.
    DiagnosticJournal::instance().record(
        DiagnosticJournal::Record(
            DiagnosticJournal::Level::Info,
            QStringLiteral("tray.child.requested"))
            .text(QStringLiteral("service"), service)
            .number(QStringLiteral("x"), globalX)
            .number(QStringLiteral("y"), globalY)
            .number(QStringLiteral("width"), globalWidth)
            .number(QStringLiteral("height"), globalHeight)
    );
    if (sender() != parentMenu || !m_openPanel
        || m_openMenuKind != QLatin1String(trayItemsKind)) {
        return;
    }

    // The D-Bus reply carries only the item's live identity, not a request
    // sequence. Coalesce an exact repeated click while that identity is still
    // pending; sending it twice would make the first reply indistinguishable
    // from the second and could consume the newer request.
    if (m_pendingKeepsTrayItems && m_pendingPanel == m_openPanel
        && m_pendingParentMenu == parentMenu
        && m_pendingService == service && m_pendingPath == path) {
        return;
    }

    // The inventory remains mapped. A second request retires only the previous
    // child (or its pending D-Bus answer) and preserves the parent identity.
    closeTrayChild(false);
    beginTrayMenuRequest(
        m_openPanel,
        QRect(globalX, globalY, qMax(0, globalWidth), qMax(0, globalHeight)),
        QRect(),
        service,
        path,
        appName,
        parentMenu
    );
}

void PanelMenuController::requestTrayMenu(
    QWindow *panel,
    const QRectF &globalOpener,
    const QRectF &globalAttachmentAnchor,
    const QString &service,
    const QString &path,
    const QString &appName
)
{
    if (!m_enabled || !panel)
        return;

    // As above, a peer supplies no request token. Until this exact direct
    // request answers, another click on the same item is the same request, not
    // a distinguishable successor.
    if (!m_pendingKeepsTrayItems && m_pendingPanel == panel
        && m_pendingService == service && m_pendingPath == path) {
        return;
    }

    // A menu requested directly from the bar is standalone, but it is still
    // a panel-attached contextual menu. Its own pinned icon is the exact
    // membrane waist and participates in the same attachment lease as every
    // first-party opener.
    close();
    beginTrayMenuRequest(
        panel,
        globalOpener,
        globalAttachmentAnchor,
        service,
        path,
        appName,
        nullptr);
}

void PanelMenuController::beginTrayMenuRequest(
    QWindow *panel,
    const QRectF &globalAnchor,
    const QRectF &globalAttachmentAnchor,
    const QString &service,
    const QString &path,
    const QString &appName,
    QWindow *parentMenu
)
{
    m_pendingPanel = panel;
    m_pendingParentMenu = parentMenu;
    m_pendingAnchor = globalAnchor;
    m_pendingAttachmentAnchor = globalAttachmentAnchor;
    m_pendingService = service;
    m_pendingPath = path;
    m_pendingAppName = appName.trimmed();
    m_pendingKeepsTrayItems = parentMenu != nullptr;
    emit trayMenuNeeded(service, path);
}

void PanelMenuController::clearPendingTrayMenu()
{
    m_pendingService.clear();
    m_pendingPath.clear();
    m_pendingAppName.clear();
    m_pendingPanel = nullptr;
    m_pendingParentMenu = nullptr;
    m_pendingAnchor = QRectF();
    m_pendingAttachmentAnchor = QRectF();
    m_pendingKeepsTrayItems = false;
}

void PanelMenuController::trayMenuReady(
    const QString &service,
    const QString &path,
    const QVariantList &entries
)
{
    // An answer to a menu nobody is waiting for — a second right-click, or one
    // that arrived after the panel moved on — is not a menu to open.
    if (service != m_pendingService || path != m_pendingPath || !m_pendingPanel)
        return;

    // The request is spent here. An application may answer the same
    // `AboutToShow` twice, or answer a second time seconds later; without
    // this the guard above lets that through, because it only rejects a
    // *different* item — and a menu the user already dismissed reopens itself
    // at an anchor that has since moved.
    const QPointer<QWindow> panel = m_pendingPanel;
    const QPointer<QWindow> parentMenu = m_pendingParentMenu;
    const bool keepTrayItems = m_pendingKeepsTrayItems;
    const QRectF anchor = m_pendingAnchor;
    const QRectF attachmentAnchor = m_pendingAttachmentAnchor;
    const QString appName = m_pendingAppName;
    clearPendingTrayMenu();

    // An empty layout is still the answer that spends the request. Keeping its
    // target pending would let a duplicate response reopen a menu later.
    if (entries.isEmpty())
        return;

    constexpr auto trayItemsKind = "tray-items";
    if (keepTrayItems
        && (!parentMenu || parentMenu != m_surface->window()
            || m_openMenuKind != QLatin1String(trayItemsKind))) {
        return;
    }

    QVariantMap initialProperties {
        {QStringLiteral("entries"), entries},
        {QStringLiteral("appName"),
         appName.isEmpty() ? service : appName},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    const bool panelAttached = !keepTrayItems
                               && !anchor.isEmpty()
                               && !attachmentAnchor.isEmpty();
    const PanelCarrierGeometry carrier = panelAttached
        ? panelCarrierGeometry(panel, anchor, attachmentAnchor)
        : PanelCarrierGeometry();
    if (panelAttached) {
        addPanelOpenerProperties(
            initialProperties,
            carrier.opener,
            carrier.attachmentAnchor,
            carrier.attachmentStartY,
            menuShellScale(panel));
    }
    initialProperties.insert(menuOutputProperties(panel));
    QObject *rootObject = m_trayComponent.createWithInitialProperties(initialProperties);
    auto *window = qobject_cast<QWindow *>(rootObject);
    if (!window) {
        qCritical().noquote()
            << "Celestina could not create a tray menu:" << m_trayComponent.errorString();
        delete rootObject;
        return;
    }

    connect(window, SIGNAL(chosen(int)), this, SLOT(trayEntryChosen(int)));
    connect(window, SIGNAL(dismissed()), this, SLOT(menuDismissed()));

    // Both the standalone foreign menu and the child beside the tray
    // inventory use the same bounded viewport. Set it before reading the
    // window size because card-sized layer surfaces adopt that exact measure.
    capCardHeightBelowAnchor(window, panel, anchor.topLeft().toPoint());

    PanelMenuSurface::Coverage coverage = PanelMenuSurface::Coverage::Output;
    if (keepTrayItems) {
        const QScreen *const screen = panel->screen();
        const QPoint outputOrigin = screen ? screen->geometry().topLeft() : QPoint();
        const QSize outputSize = screen ? screen->geometry().size() : QSize();
        // In output pixels, like everything else this arithmetic touches.
        // The card properties are stated in the shell's unscaled units — the
        // surface lays out in those and the host divides once on the way in —
        // while `window->size()`, the anchor and the output rectangle are real
        // pixels. Mixed, they agree only at factor 1, which is why a scaled
        // output placed the child menu over its parent instead of beside it.
        const double parentScale = surfaceShellScale(parentMenu);
        const QRect parentCard(
            qRound(parentMenu->property("cardX").toInt() * parentScale),
            qRound(parentMenu->property("cardY").toInt() * parentScale),
            qRound(parentMenu->property("cardWidth").toInt() * parentScale),
            qRound(parentMenu->property("cardHeight").toInt() * parentScale)
        );
        const double childScale = surfaceShellScale(window);
        // The child grows a sideways droplet membrane out of the edge facing
        // its parent, on a surface that covers the output like every other
        // menu's. It was a card-sized window once, and that window was the
        // structural reason its push could never read as one piece: the
        // compositor's glass fills a card-sized surface edge to edge, so the
        // card had no canvas to visibly move across — measured mid-push with
        // the body displaced and the glass box pinned. On the whole output it
        // travels exactly as the top-attached drop does. The author accepted
        // the input trade (2026-08-13): with the child open, a click outside
        // it dismisses the child first, as in any nested menu.
        const bool sideAttachment = anchor.width() > 0 && anchor.height() > 0;
        window->setProperty("attachedToMenuSide", sideAttachment);
        // Inventory rows still enter through the integer D-Bus/QML child
        // contract. Preserve QRect's inclusive centre for that established
        // side placement; only the direct panel route carries fractional icon
        // geometry into its semantic attachment lease.
        const QRect outputAnchor =
            anchor.toAlignedRect().translated(-outputOrigin);
        // Keep the invoking tile's centre inside the membrane's flat lateral
        // span rather than at the very corner of the child body.
        const QPoint requestedOrigin(
            outputAnchor.left(),
            sideAttachment
                ? outputAnchor.center().y() - 72
                : outputAnchor.top()
        );
        // The card, not the window: the window is the output now. The gap the
        // card keeps from its parent is the membrane's travel, the same
        // proportional distance the QML derives for the stretch itself.
        const QSize cardSize(
            qRound(window->property("cardWidth").toInt() * childScale),
            qRound(window->property("cardHeight").toInt() * childScale)
        );
        const int membraneGap = qRound(
            window->property("sideAttachmentGap").toInt() * childScale);
        const QPoint cardOrigin = adjacentTrayMenuOrigin(
            parentCard,
            requestedOrigin,
            cardSize,
            outputSize,
            sideAttachment
                ? membraneGap
                : qRound(window->property("anchorGap").toInt() * childScale)
        );
        const bool seamAtRight =
            cardOrigin.x() + cardSize.width() / 2
            < parentCard.center().x();
        if (sideAttachment) {
            window->setProperty("attachmentSideRight", seamAtRight);
            // Output-local, in the shell's unscaled units: the surface covers
            // the output, so the anchor needs no window translation any more.
            window->setProperty(
                "attachmentAnchorRect",
                inShellUnits(QRectF(outputAnchor), childScale)
            );
        }
        coverage = PanelMenuSurface::Coverage::Output;
        placeCardOnOutput(
            window,
            QPoint(
                qRound(cardOrigin.x()
                       / (childScale > 0 ? childScale : 1.0)),
                qRound(cardOrigin.y()
                       / (childScale > 0 ? childScale : 1.0))
            )
        );
        // The numbers this placement mixed, recorded because they have now
        // been wrong across the units seam twice and the nest's console is
        // unreachable from outside: with these, a detached child is a
        // subtraction instead of a guess.
        DiagnosticJournal::instance().record(
            DiagnosticJournal::Record(
                DiagnosticJournal::Level::Info,
                QStringLiteral("tray.child.placed"))
                .flag(QStringLiteral("side_attachment"), sideAttachment)
                .flag(QStringLiteral("seam_at_right"), seamAtRight)
                .number(QStringLiteral("origin_x"), cardOrigin.x())
                .number(QStringLiteral("origin_y"), cardOrigin.y())
                .number(QStringLiteral("child_width"), cardSize.width())
                .number(QStringLiteral("child_height"), cardSize.height())
                .number(QStringLiteral("gap"), membraneGap)
                .number(QStringLiteral("parent_x"), parentCard.x())
                .number(QStringLiteral("parent_y"), parentCard.y())
                .number(QStringLiteral("parent_width"), parentCard.width())
                .number(QStringLiteral("parent_height"), parentCard.height())
        );
    } else if (panelAttached) {
        placeCardOnOutput(
            window,
            panelPopupBodyOrigin(
                carrier.opener,
                window->property("contentWidth").toInt(),
                window->property("anchorGap").toInt(),
                carrier.attachmentStartY));
        passPanelStripThrough(window, panel);
    } else {
        placeCard(window, panel, anchor.topLeft().toPoint());
    }

    // QML construction is synchronous but may execute arbitrary completion
    // handlers. Revalidate the exact parent immediately before adopting the
    // child so a retired inventory cannot gain a late menu.
    if (keepTrayItems
        && (!parentMenu || parentMenu != m_surface->window()
            || m_openMenuKind != QLatin1String(trayItemsKind))) {
        delete window;
        return;
    }

    if (!m_trayChildSurface->open(
            window,
            panel,
            coverage,
            panelAttached ? carrier.outputPosition : QPoint())) {
        delete window;
        return;
    }

    m_openService = service;
    m_openPath = path;
    m_openParentMenu = keepTrayItems ? parentMenu : nullptr;
    if (panelAttached) {
        emit contextualSurfaceOpened();
        m_attachmentLease.acquire(
            panel,
            window,
            attachmentAnchor,
            QPointF(carrier.outputPosition));
    }
}

void PanelMenuController::trayEntryChosen(int entryId)
{
    // Nothing is triggered once the child has closed: its identity is cleared,
    // so a late choice reaches no application rather than the wrong one.
    if (sender() != m_trayChildSurface->window()
        || (m_openService.isEmpty() && m_openPath.isEmpty())) {
        return;
    }

    emit trayEntryTriggered(m_openService, m_openPath, entryId);
    if (!requestPopupDismissal(m_trayChildSurface->window()))
        closeTrayChild(true);
}

void PanelMenuController::menuDismissed()
{
    // The one bounded fact a two-click report needs: what let go, and
    // whether the controller still recognised it as its own.
    DiagnosticJournal::instance().record(
        DiagnosticJournal::Record(
            DiagnosticJournal::Level::Info,
            QStringLiteral("ctx.menu_dismissed"))
            .text(QStringLiteral("open_kind"), m_openMenuKind)
            .flag(QStringLiteral("is_child"),
                  sender() == m_trayChildSurface->window())
            .flag(QStringLiteral("is_current"),
                  sender() == m_surface->window())
    );
    if (sender() == m_trayChildSurface->window()) {
        QWindow *const window = m_trayChildSurface->window();
        softCloseWindow(window, [this, window]() {
            if (m_trayChildSurface->window() == window)
                closeTrayChild(true);
        });
        return;
    }

    // The closing beat every surface family gets: fade, then the real
    // close. A swap that hard-closes mid-beat destroys the window and the
    // pending finish dies with it.
    if (sender() == m_surface->window()) {
        QWindow *const window = m_surface->window();
        softCloseWindow(window, [this, window]() {
            if (m_surface->window() == window)
                close();
        });
    }
}

void PanelMenuController::restoreTrayParentFocus(
    const QPointer<QWindow> &parentMenu
)
{
    if (!parentMenu || parentMenu != m_surface->window())
        return;

    // The controller owns the surface state read by the callback. Use it as
    // the timer context so destroying the controller cancels the callback;
    // `parentMenu` remains guarded independently by its QPointer.
    QTimer::singleShot(0, this, [this, parentMenu]() {
        if (!parentMenu || parentMenu != m_surface->window()
            || !parentMenu->isVisible()) {
            return;
        }

        parentMenu->requestActivate();
        QObject *const menu = parentMenu->property("menu").value<QObject *>();
        if (menu) {
            QMetaObject::invokeMethod(
                menu,
                "forceActiveFocus",
                Q_ARG(Qt::FocusReason, Qt::PopupFocusReason)
            );
        }
    });
}

void PanelMenuController::closeTrayChild(bool restoreParentFocus)
{
    constexpr auto trayItemsKind = "tray-items";
    DiagnosticJournal::instance().record(
        DiagnosticJournal::Record(
            DiagnosticJournal::Level::Info,
            QStringLiteral("tray.child.closed"))
            .flag(QStringLiteral("restore_focus"), restoreParentFocus)
            .flag(QStringLiteral("had_window"),
                  m_trayChildSurface && m_trayChildSurface->window()));
    const QPointer<QWindow> parentMenu = m_openParentMenu;
    const bool inventoryRemainsOpen =
        m_openMenuKind == QLatin1String(trayItemsKind)
        && m_surface && m_surface->window();
    clearPendingTrayMenu();
    m_openService.clear();
    m_openPath.clear();
    m_openParentMenu = nullptr;
    m_trayChildSurface->close();
    if (!parentMenu && !inventoryRemainsOpen)
        m_attachmentLease.release();

    if (restoreParentFocus)
        restoreTrayParentFocus(parentMenu);
}

void PanelMenuController::close()
{
    // Whatever was asked for is no longer wanted. A pending target that
    // outlives the menu is what lets a late answer open a surface the user
    // never asked for.
    closeTrayChild(false);
    // An indicator menu's carrier rests instead of unmapping (SURF-1): its
    // completed retirement is cleared so the park is accepted, and the next
    // toggle of the same kind resumes this same mapped window. The other
    // carriers this surface hosts — the popup-backed tray inventory, the
    // workspace map — keep the hard close their lifecycles were built on.
    QWindow *const window = m_surface->window();
    const bool parkable = window && m_surface->isOpen()
        && !indicatorMenuComponent(m_openMenuKind).isEmpty();
    const QString departingKind = m_openMenuKind;
    m_openMenuKind.clear();
    m_openPanel = nullptr;
    m_openIndicatorPanel = nullptr;
    if (m_surface->isParked()) {
        // A repeated close finds the carrier already resting; putting it
        // away now would be exactly the unmap the park exists to avoid.
    } else if (parkable) {
        window->setProperty("celestinaRetiring", false);
        if (m_surface->park()) {
            m_parkedMenuKind = departingKind;
        } else {
            m_parkedMenuKind.clear();
            m_surface->close();
        }
    } else {
        m_parkedMenuKind.clear();
        m_surface->close();
    }
    m_attachmentLease.release();
}

void PanelMenuController::passPanelStripThrough(QWindow *content, QWindow *panel)
{
    if (!content || !panel)
        return;

    const QPointer<QWindow> tracked(content);
    const QPointer<QWindow> bar(panel);
    const auto apply = [tracked, bar]() {
        if (!tracked || !bar)
            return;
        // This window already begins below the bar. Its local seam is zero, so
        // the complete carrier remains the outside-click barrier while the
        // panel is physically outside the surface and receives its own input.
        tracked->setMask(panelPopupInputRegion(
            tracked->width(), tracked->height(), 0));
    };
    apply();
    // The compositor sizes this surface, so its real extent arrives after the
    // map; keep the explicit full-carrier region aligned with that configure
    // rather than with the card-sized construction request.
    connect(content, &QWindow::widthChanged, content, apply);
    connect(content, &QWindow::heightChanged, content, apply);
    // A mask set before the platform surface exists can be lost with it. That
    // loss is safe now because the platform's default is the complete inset
    // carrier; reapplying on the event loop and after its first commits keeps
    // the explicit contract deterministic on every backend.
    QTimer::singleShot(0, content, apply);
    QTimer::singleShot(120, content, apply);
}
