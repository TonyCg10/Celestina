#include "toastcontroller.h"

#include <QCursor>
#include <QDateTime>
#include <QDebug>
#include <QGuiApplication>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QVariantList>
#include <QPointer>
#include <QMargins>
#include <QRegion>
#include <QScreen>
#include <QVariantMap>
#include <QTimer>
#include <QWindow>

#include "diagnosticjournal.h"
#include "overlaysurface.h"
#include "panelmanager.h"
#include "quietplacement.h"
#include "shellprovidersclient.h"
#include "shellscale.h"

namespace {
const char componentName[] = "ToastStack";
const char bellIconObjectName[] = "celestina-notification-icon";

// The card the QML draws, in shell units. The stack grows downward as toasts
// arrive; this is one card's footprint, which is what the zone question and
// the window's width need.
constexpr QSizeF cardSize(380, 140);
constexpr qreal cardInset = 8;
// Upper bound for the connector gap plus the drop's overshoot; the QML
// computes the exact proportional gap from its theme tokens.
constexpr qreal connectorSlack = 96;
// The server shows at most this many toasts at once (MAX_VISIBLE); the
// window is sized for that whole pile from the very first mapping, exactly
// as the display's windows are sized for their whole card file. A layer
// surface that grew with the animated column reconfigured — fresh buffer,
// blur re-arm and all — on every frame of every growth, which is the
// recorded stutter, and the configure storm it caused is what wedged the
// bottom-centre pile with its content still below the screen's edge.
constexpr int stackDepth = 5;
constexpr qreal stackSpacing = 8;
constexpr qreal columnPadding = 24;
constexpr qreal edgeBreath = 16;
constexpr qreal runwayColumnHeight =
    stackDepth * cardSize.height() + (stackDepth - 1) * stackSpacing
    + columnPadding;

QRectF onOutputInShellUnits(
    const QRectF &globalRect,
    const QPointF &outputOrigin,
    double shellScale
)
{
    const QRectF onOutput = globalRect.translated(-outputOrigin);
    return shellScale > 0
        ? QRectF(onOutput.x() / shellScale, onOutput.y() / shellScale,
                 onOutput.width() / shellScale, onOutput.height() / shellScale)
        : onOutput;
}
} // namespace

// Every content change can start up to one full beat of motion, plus the
// sweep that follows a fold; the pump outlives the longest of them and dies
// on its deadline. Restarted freely: a burst of arrivals just moves the
// deadline out.
void ToastController::pumpAnimations()
{
    m_pumpDeadline = QDateTime::currentMSecsSinceEpoch() + 700;
    if (!m_animationPump.isActive())
        m_animationPump.start();
}

namespace {
void quietKickRender(QWindow *window)
{
    const QPointer<QWindow> tracked(window);
    const auto kick = [tracked]() {
        if (auto *quick = qobject_cast<QQuickWindow *>(tracked.data()))
            quick->requestUpdate();
    };
    kick();
    QTimer::singleShot(50, tracked, kick);
    QTimer::singleShot(250, tracked, kick);
}
} // namespace

ToastController::ToastController(
    QQmlEngine *engine,
    ShellProvidersClient *providers,
    QObject *parent
)
    : QObject(parent)
    , m_component(engine)
    , m_providers(providers)
    , m_surface(new OverlaySurface(
          OverlaySurface::Placement::Corner,
          QStringLiteral("celestina-toasts"),
          this
      ))
    , m_enabled(true)
{
    m_component.loadFromModule("CelestinaDesktop", QLatin1String(componentName));
    if (!m_component.isReady()) {
        qCritical().noquote()
            << "Celestina could not load its toast stack:"
            << m_component.errorString();
        m_enabled = false;
    }

    // QML owns the real completion edge. One full exit beat plus a breath is
    // only the watchdog for an unpresented or otherwise stalled scene.
    m_closeTimer.setSingleShot(true);
    m_closeTimer.setInterval(260);
    connect(&m_closeTimer, &QTimer::timeout, this, &ToastController::hide);

    // One forced frame per tick while a toast animation can be in flight.
    //
    // The entry ride's very first frame holds the whole column outside the
    // canvas, so the buffer this window commits is fully transparent — and a
    // compositor that culls an invisible surface stops feeding it frame
    // callbacks, which freezes the render-driven animation on that same
    // first frame forever: the compositor's material stands at its final
    // place while the content hangs half off the screen, exactly what the
    // author recorded twice. Forced updates advance the scene without
    // waiting for callbacks — measured: three kicks moved a stalled growth
    // by three frames — so a 60 Hz pump for the bounded span of the motion
    // plays it whole, and the moment pixels reach the canvas the callbacks
    // resume on their own. The display's cards never commit an all-clear
    // buffer, which is why its triple kick has always been enough there.
    m_animationPump.setInterval(16);
    connect(&m_animationPump, &QTimer::timeout, this, [this]() {
        if (QDateTime::currentMSecsSinceEpoch() > m_pumpDeadline) {
            m_animationPump.stop();
            return;
        }
        if (auto *quick = qobject_cast<QQuickWindow *>(m_surface->window()))
            quick->requestUpdate();
        else
            m_animationPump.stop();
    });

    if (m_providers) {
        connect(
            m_providers,
            &ShellProvidersClient::changed,
            this,
            &ToastController::providersChanged
        );
    }
}

bool ToastController::isVisible() const
{
    return m_surface->isOpen();
}

QRectF ToastController::openCardRectOnOutput(QScreen *screen) const
{
    if (!m_surface->isOpen() || !screen || m_openScreen.data() != screen)
        return QRectF();
    return m_openCard;
}

namespace {
void collectToastCards(QQuickItem *item, QVariantList &cards)
{
    if (!item)
        return;
    for (QQuickItem *const child : item->childItems()) {
        if (child->objectName().startsWith(
                QStringLiteral("celestina-toast-card-"))) {
            QVariantMap card;
            card.insert(QStringLiteral("name"), child->objectName());
            // In the window's own physical pixels, transforms included —
            // the coordinates a screenshot of the buffer would measure.
            const QPointF top = child->mapToScene(QPointF(0, 0));
            const QPointF bottom =
                child->mapToScene(QPointF(0, child->height()));
            card.insert(QStringLiteral("sceneY"), top.y());
            card.insert(QStringLiteral("sceneBottom"), bottom.y());
            card.insert(QStringLiteral("height"), child->height());
            card.insert(QStringLiteral("opacity"), child->opacity());
            card.insert(
                QStringLiteral("leaving"),
                child->property("leaving"));
            card.insert(
                QStringLiteral("hatching"),
                child->property("hatching"));
            cards.append(card);
        }
        collectToastCards(child, cards);
    }
}
} // namespace

QVariantMap ToastController::stackState() const
{
    QVariantMap state;
    state.insert(QStringLiteral("open"), m_surface->isOpen());
    state.insert(QStringLiteral("parked"), m_surface->isParked());
    QWindow *const window = m_surface->window();
    if (!window)
        return state;

    state.insert(QStringLiteral("windowWidth"), window->width());
    state.insert(QStringLiteral("windowHeight"), window->height());
    state.insert(QStringLiteral("framesSwapped"), m_framesSwapped);
    for (const char *property :
         {"runwayHeight", "canvasHeight", "neededHeight", "entersFromBottom",
          "anchoredFromPanel", "shellScale"}) {
        state.insert(
            QString::fromLatin1(property), window->property(property));
    }

    QQuickItem *const field = quietFindVisibleItem(
        window, QStringLiteral("celestina-soft-menu-field"));
    if (field) {
        QVariantMap measured;
        measured.insert(QStringLiteral("y"), field->y());
        measured.insert(QStringLiteral("height"), field->height());
        for (const char *property :
             {"targetHeight", "blockEntryProgress", "revealed", "departing"}) {
            measured.insert(
                QString::fromLatin1(property), field->property(property));
        }
        measured.insert(QStringLiteral("opacity"), field->opacity());
        measured.insert(QStringLiteral("scale"), field->scale());
        const QPointF top = field->mapToScene(QPointF(0, 0));
        const QPointF bottom = field->mapToScene(QPointF(0, field->height()));
        measured.insert(QStringLiteral("sceneY"), top.y());
        measured.insert(QStringLiteral("sceneBottom"), bottom.y());
        state.insert(QStringLiteral("field"), measured);

        QVariantList cards;
        collectToastCards(field, cards);
        state.insert(QStringLiteral("cards"), cards);

        if (QQuickItem *const rows = quietFindVisibleItem(
                window, QStringLiteral("celestina-toast-rows"))) {
            QVariantMap column;
            column.insert(QStringLiteral("y"), rows->y());
            column.insert(QStringLiteral("height"), rows->height());
            column.insert(
                QStringLiteral("implicitHeight"), rows->implicitHeight());
            column.insert(
                QStringLiteral("sceneY"),
                rows->mapToScene(QPointF(0, 0)).y());
            state.insert(QStringLiteral("rows"), column);
        }
    }

    // Flattened by hand: a QRectF nested in a list crosses D-Bus as null,
    // and a probe that answers null is a probe nobody can assert against.
    const QVariantList regions = window->property("glassRegions").toList();
    QVariantList rects;
    for (const QVariant &each : regions) {
        const QRectF rect =
            each.toMap().value(QStringLiteral("rect")).toRectF();
        rects.append(QVariantMap {
            {QStringLiteral("x"), rect.x()},
            {QStringLiteral("y"), rect.y()},
            {QStringLiteral("width"), rect.width()},
            {QStringLiteral("height"), rect.height()},
        });
    }
    state.insert(QStringLiteral("glassRects"), rects);
    return state;
}

void ToastController::yieldParkedCarrier(const QStringList &fullscreenOutputs)
{
    if (!m_surface->isParked())
        return;
    QWindow *const window = m_surface->window();
    if (window && window->screen()
        && fullscreenOutputs.contains(window->screen()->name())) {
        m_surface->close();
    }
}

void ToastController::providersChanged()
{
    if (!m_providers)
        return;

    // A helper that went away is showing nothing, whatever was on screen when
    // it left. The server is the only thing that knows what is still live.
    const QVariantMap published =
        m_providers->available()
        ? m_providers->providers().value(QStringLiteral("notifications")).toMap()
        : QVariantMap();
    const QVariantList toasts = published.value(QStringLiteral("toasts")).toList();

    if (toasts.isEmpty()) {
        // The last section leaves with an animation, and it needs its window
        // alive to play it: the empty list is pushed onto the mapped surface
        // and the teardown waits out the beat. A window that is not up has
        // nothing to play — it is simply confirmed down.
        if (QWindow *const shown = m_surface->window()) {
            shown->setProperty("actions", QVariantList());
            shown->setProperty("toasts", QVariantList());
            // Reduced motion may finish synchronously while assigning the
            // list. Do not kick or arm a watchdog for a carrier QML already
            // completed and released.
            if (m_surface->window() != shown)
                return;
            quietKickRender(shown);
            pumpAnimations();
            if (!m_closeTimer.isActive())
                m_closeTimer.start();
        } else {
            hide();
        }
        return;
    }
    m_closeTimer.stop();
    // The centre is the whole list, keyboard included; while it is open the
    // corner stays quiet, exactly as a level's display stays quiet while its
    // own menu is up. What is live is still there when the centre closes:
    // the server's next publication brings it back.
    if (m_centreProbe && m_centreProbe()) {
        hide();
        return;
    }
    // The actions travel beside the notifications, not inside them; the surface
    // joins them by id.
    show(toasts, published.value(QStringLiteral("actions")).toList());
}

QWindow *ToastController::createWindow(
    const QVariantList &toasts,
    const QVariantList &actions,
    const QVariantMap &placementProperties
)
{
    QVariantMap initialProperties {
        {QStringLiteral("toasts"), toasts},
        {QStringLiteral("actions"), actions},
        {QStringLiteral("providerSource"), QVariant::fromValue(m_providers.data())},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    initialProperties.insert(placementProperties);
    QObject *rootObject = m_component.createWithInitialProperties(initialProperties);
    if (!rootObject) {
        qCritical().noquote()
            << "Celestina could not create its toast stack:"
            << m_component.errorString();
        return nullptr;
    }

    auto *window = qobject_cast<QWindow *>(rootObject);
    if (!window) {
        qCritical() << "Celestina's toast stack component is not a window.";
        delete rootObject;
        return nullptr;
    }
    return window;
}

// The runway is taller than what it shows, on every route, so input belongs
// to the cards alone: the mask is the union of the glass the QML publishes,
// grown a breath so a dismiss cross at a card's edge never lands on a dead
// pixel. Before the first card has published — or between piles — it is one
// pixel, exactly as the park keeps it.
void ToastController::applyInputMask(QWindow *window)
{
    if (!window)
        return;

    // The park owns a resting carrier's one-pixel mask; a late glass change
    // arriving through the persistent connection must not widen it.
    if (window->property("celestinaParked").toBool())
        return;

    const QVariantList rects = window->property("glassRects").toList();
    QRegion region;
    for (const QVariant &each : rects) {
        const QRect rect = each.toRectF().toAlignedRect();
        if (rect.isEmpty())
            continue;
        region += rect.marginsAdded(QMargins(4, 4, 4, 4));
    }
    if (region.isEmpty())
        region = QRegion(0, 0, 1, 1);
    window->setMask(region);
}

void ToastController::toastGlassChanged()
{
    applyInputMask(m_surface->window());
}

void ToastController::show(const QVariantList &toasts, const QVariantList &actions)
{
    if (!m_enabled)
        return;

    // Already up: the list is replaced in place. Remapping the surface for
    // every arrival would make the corner flicker and would lose whatever the
    // person was reading. Open, not merely mapped: a parked carrier also has
    // a window, and content pushed onto it would paint behind one pixel of
    // input instead of resuming the surface first.
    if (m_surface->isOpen()) {
        QWindow *const shown = m_surface->window();
        shown->setProperty("actions", actions);
        shown->setProperty("toasts", toasts);
        applyInputMask(shown);
        // The same kick the mapping and the emptying already get. A layer
        // window the compositor has stopped feeding frame callbacks holds
        // every animation where it stands — measured on the nested session:
        // a card joining an open stack grew the model but not the field, and
        // the newcomer painted below the glass until any other surface
        // dirtied the scene.
        quietKickRender(shown);
        pumpAnimations();
        return;
    }

    // The corner of the output the pointer is on, like every other surface the
    // shell opens without a click position.
    QScreen *screen = QGuiApplication::screenAt(QCursor::pos());
    if (!screen)
        screen = QGuiApplication::primaryScreen();

    const double shellScale = shellScaleForScreen(screen);
    const QSizeF outputSize = screen && shellScale > 0
        ? QSizeF(screen->geometry().size()) / shellScale
        : QSizeF();

    // The membrane's mouth is the panel's own notification bell — the same
    // icon whose indicator counts these toasts.
    QuietSurfaceGeometry geometry;
    QRectF opener;
    QRectF icon;
    qreal barHeight = 0;
    QWindow *const panel = m_panels && screen
        ? m_panels->panelWindowFor(screen) : nullptr;
    if (panel && panel->isVisible()) {
        const QuietAnchor anchor =
            quietAnchorForIcon(panel, QLatin1String(bellIconObjectName));
        if (anchor.valid()) {
            const QPointF outputOrigin = screen->geometry().topLeft();
            opener = onOutputInShellUnits(anchor.opener, outputOrigin, shellScale);
            icon = onOutputInShellUnits(anchor.icon, outputOrigin, shellScale);
            barHeight = shellScale > 0
                ? panelBarBottomDevice(panel) / shellScale
                : panelBarBottomDevice(panel);
            // Sized for the whole pile from the start, like the display's
            // file: the runway maps once and the column grows inside it.
            geometry = attachedQuietGeometry(
                outputSize,
                barHeight,
                opener,
                icon,
                QSizeF(cardSize.width(), runwayColumnHeight),
                cardInset,
                connectorSlack
            );
        }
    }

    const QRectF prospectiveCard = geometry.valid
        ? geometry.card
        : QRectF(outputSize.width() - cardInset - cardSize.width(),
                 cardInset, cardSize.width(), cardSize.height());
    const bool occupied = m_zoneProbe
        && quietZoneOccupied(prospectiveCard, m_zoneProbe(screen));

    OverlaySurface::Placement placement = OverlaySurface::Placement::Corner;
    QVariantMap placementProperties {
        {QStringLiteral("shellScale"), shellScale},
        // The whole pile's room, decided here once: the QML never asks the
        // compositor to follow its column again.
        {QStringLiteral("runwayHeight"), runwayColumnHeight},
    };
    if (occupied) {
        placement = OverlaySurface::Placement::BottomCentre;
        placementProperties.insert(QStringLiteral("entersFromBottom"), true);
        // The bottom pile keeps its breathing room between the block and the
        // physical edge inside the same fixed canvas.
        placementProperties.insert(
            QStringLiteral("runwayHeight"), runwayColumnHeight + edgeBreath);
    } else if (geometry.valid) {
        placement = OverlaySurface::Placement::AttachedTopRight;
        const QRectF localOpener = geometry.onSurface(opener);
        const QRectF localIcon = geometry.onSurface(icon);
        placementProperties.insert(QStringLiteral("anchoredFromPanel"), true);
        placementProperties.insert(QStringLiteral("openerRect"), localOpener);
        placementProperties.insert(
            QStringLiteral("attachmentAnchorRect"), localIcon);
        placementProperties.insert(QStringLiteral("attachmentStartY"), 0);
        placementProperties.insert(QStringLiteral("surfaceOriginX"), 0);
        placementProperties.insert(
            QStringLiteral("surfaceWidth"), geometry.surface.width());
        placementProperties.insert(
            QStringLiteral("surfaceHeight"), geometry.surface.height());
        // Attached, the geometry already spans the connector and the whole
        // pile; that height is this route's runway.
        placementProperties.insert(
            QStringLiteral("runwayHeight"), geometry.surface.height());
    }

    const int topInset = placement == OverlaySurface::Placement::AttachedTopRight
        ? geometry.topInsetInOutputUnits(shellScale) : 0;

    // A carrier parked on this same output resumes in place instead of
    // remapping — the scene change the park exists to avoid (SURF-1). The
    // resume itself insists on the placement the surface was mapped with;
    // a stack that must land somewhere else pays the one remap.
    QWindow *stack = nullptr;
    bool reused = false;
    if (m_surface->isParked()) {
        QWindow *const parked = m_surface->window();
        if (parked && parked->handle() && parked->screen() == screen) {
            // Defaults first, so a corner reuse does not inherit an attached
            // route's geometry, then this open's own placement and content.
            parked->setProperty("anchoredFromPanel", false);
            parked->setProperty("openerRect", QRectF());
            parked->setProperty("attachmentAnchorRect", QRectF());
            parked->setProperty("attachmentStartY", -1);
            parked->setProperty("surfaceOriginX", 0);
            parked->setProperty("surfaceWidth", cardSize.width());
            parked->setProperty("surfaceHeight", 0);
            parked->setProperty("entersFromBottom", false);
            parked->setProperty("runwayHeight", 0);
            for (auto it = placementProperties.constBegin();
                 it != placementProperties.constEnd(); ++it) {
                parked->setProperty(it.key().toUtf8().constData(), it.value());
            }
            parked->setProperty("actions", actions);
            parked->setProperty("toasts", toasts);
            reused = m_surface->open(parked, screen, placement, topInset);
            if (reused)
                stack = parked;
        }
        if (!reused)
            m_surface->close();
    }

    if (!reused) {
        stack = createWindow(toasts, actions, placementProperties);
        if (!stack)
            return;

        // The signal is declared by ToastStack.qml, so it is connected through
        // the runtime meta-object rather than a generated C++ type. Made once
        // per window, never per open: a reused carrier keeps them.
        connect(
            stack,
            SIGNAL(departureFinished()),
            this,
            SLOT(toastDepartureFinished())
        );

        if (!m_surface->open(stack, screen, placement, topInset)) {
            delete stack;
            return;
        }

        // The input region follows the cards, not the canvas: each glass
        // publication redraws the mask over the fixed runway. Connected once
        // per window, like the departure signal; a reused carrier keeps it.
        connect(
            stack,
            SIGNAL(glassRegionsChanged()),
            this,
            SLOT(toastGlassChanged())
        );

        if (auto *quick = qobject_cast<QQuickWindow *>(stack)) {
            connect(quick, &QQuickWindow::frameSwapped, this, [this]() {
                ++m_framesSwapped;
            });
        }
    }

    // A nested session's console is unreachable from outside it; where a
    // quiet surface went and why is the bounded fact this hunt needs.
    DiagnosticJournal::instance().record(
        DiagnosticJournal::Record(
            DiagnosticJournal::Level::Info,
            QStringLiteral("quiet.placed"))
            .text(QStringLiteral("surface"), QStringLiteral("toasts"))
            .number(QStringLiteral("placement"), static_cast<int>(placement))
            .flag(QStringLiteral("anchored"), geometry.valid)
            .flag(QStringLiteral("occupied"), occupied)
            .number(QStringLiteral("card_x"), qRound(prospectiveCard.x()))
            .number(QStringLiteral("card_y"), qRound(prospectiveCard.y()))
            .number(QStringLiteral("card_width"), qRound(prospectiveCard.width()))
            .flag(QStringLiteral("reused"), reused)
    );

    m_openScreen = screen;
    m_openCard = prospectiveCard;
    applyInputMask(stack);

    // Measured on the nested session: an exposed layer window does not
    // schedule its own first frame — the scene renders only when something
    // dirties it, its first commit arrived seconds late on the provider's
    // poll, and without that first commit no frame callbacks flow, so no
    // animation can tick either. A surface that lives under two seconds died
    // unpainted. Kicking one update right after mapping starts the chain.
    quietKickRender(stack);
    pumpAnimations();

}

void ToastController::toastDepartureFinished()
{
    // A retired stack can finish after a replacement was mapped. Its signal
    // must never close that newer carrier.
    if (sender() == m_surface->window())
        hide();
}

void ToastController::hide()
{
    m_closeTimer.stop();
    // SIMPLE-1 retired the toast park (SURF-1). A resumed carrier on this
    // compositor never regains its exposure: Qt's render loop then skips
    // every update — heartbeat included — and the next pile's paint stays a
    // stale buffer under freshly armed frost, which is the milky slab the
    // author kept recording. Fresh opens have never shown the defect, and
    // the remap they cost is one configure round-trip on a surface whose
    // geometry no longer animates. The park existed to avoid scene churn a
    // simpler shell no longer has.
    m_surface->close();
    m_openCard = QRectF();
    m_openScreen.clear();
}
