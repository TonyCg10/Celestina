#include "toastcontroller.h"

#include <QCursor>
#include <QDebug>
#include <QGuiApplication>
#include <QQmlEngine>
#include <QQuickWindow>
#include <QPointer>
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

    // One full exit beat plus a breath, matching the display's own closing
    // wait: the QML's recede runs for the theme's `motionNormal` (200 ms).
    m_closeTimer.setSingleShot(true);
    m_closeTimer.setInterval(260);
    connect(&m_closeTimer, &QTimer::timeout, this, &ToastController::hide);

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
            shown->setProperty("toasts", QVariantList());
            shown->setProperty("actions", QVariantList());
            quietKickRender(shown);
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

// An attached stack's window reaches the real top edge because the membrane's
// seam is inside the strip the panel reserved — which means it covers the
// bell and its neighbours. The cards are interactive and keep their input;
// everything above the seam belongs to the panel, so the pointer must pass
// through it.
void ToastController::applyInputMask(QWindow *window)
{
    if (!window)
        return;

    if (!m_openAttached) {
        window->setMask(QRegion());
        return;
    }

    // The panel window's own height is the seam, in the same logical units
    // this window's mask uses: both already carry the per-output factor.
    QWindow *const panel =
        m_panels ? m_panels->panelWindowFor(window->screen()) : nullptr;
    const int seam = panel ? qMax(0, panel->height()) : 0;
    window->setMask(QRegion(
        0, seam,
        qMax(1, window->width()), qMax(1, window->height() - seam)
    ));
}

void ToastController::show(const QVariantList &toasts, const QVariantList &actions)
{
    if (!m_enabled)
        return;

    // Already up: the list is replaced in place. Remapping the surface for
    // every arrival would make the corner flicker and would lose whatever the
    // person was reading.
    if (QWindow *const shown = m_surface->window()) {
        shown->setProperty("toasts", toasts);
        shown->setProperty("actions", actions);
        applyInputMask(shown);
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
                ? qMax(0, panel->height()) / shellScale
                : qMax(0, panel->height());
            geometry = attachedQuietGeometry(
                outputSize,
                barHeight,
                opener,
                icon,
                cardSize,
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
    };
    if (occupied) {
        placement = OverlaySurface::Placement::BottomCentre;
        placementProperties.insert(QStringLiteral("entersFromBottom"), true);
    } else if (geometry.valid) {
        placement = OverlaySurface::Placement::AttachedTopRight;
        placementProperties.insert(QStringLiteral("anchoredFromPanel"), true);
        placementProperties.insert(QStringLiteral("openerRect"), opener);
        placementProperties.insert(QStringLiteral("attachmentAnchorRect"), icon);
        placementProperties.insert(QStringLiteral("attachmentStartY"), barHeight);
        placementProperties.insert(
            QStringLiteral("surfaceOriginX"), geometry.surface.x());
        placementProperties.insert(
            QStringLiteral("surfaceWidth"), geometry.surface.width());
        placementProperties.insert(
            QStringLiteral("surfaceHeight"), geometry.surface.height());
    }

    QWindow *const stack = createWindow(toasts, actions, placementProperties);
    if (!stack)
        return;

    if (!m_surface->open(stack, screen, placement)) {
        delete stack;
        return;
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
    );

    m_openAttached = placement == OverlaySurface::Placement::AttachedTopRight;
    m_openScreen = screen;
    m_openCard = prospectiveCard;
    applyInputMask(stack);
    // The stack grows as toasts arrive; the strip above the seam must stay
    // the panel's whatever the height becomes.
    connect(stack, &QWindow::heightChanged, this, [this]() {
        applyInputMask(m_surface->window());
    });

    // Measured on the nested session: an exposed layer window does not
    // schedule its own first frame — the scene renders only when something
    // dirties it, its first commit arrived seconds late on the provider's
    // poll, and without that first commit no frame callbacks flow, so no
    // animation can tick either. A surface that lives under two seconds died
    // unpainted. Kicking one update right after mapping starts the chain.
    quietKickRender(stack);

}

void ToastController::hide()
{
    m_closeTimer.stop();
    m_surface->close();
    m_openCard = QRectF();
    m_openScreen.clear();
    m_openAttached = false;
}
