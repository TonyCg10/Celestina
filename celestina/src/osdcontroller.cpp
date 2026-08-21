#include "osdcontroller.h"

#include <QCursor>
#include <QDebug>
#include <QGuiApplication>
#include <QQmlEngine>
#include <QRegion>
#include <QScreen>
#include <QStringList>
#include <QVariantMap>
#include <QPointer>
#include <QQuickItem>
#include <QQuickWindow>
#include <QTimer>
#include <QWindow>

#include <utility>

#include "diagnosticjournal.h"
#include "overlaysurface.h"
#include "panelmanager.h"
#include "panelmenucontroller.h"
#include "quietplacement.h"
#include "shellprovidersclient.h"
#include "shellscale.h"

namespace {
// Long enough to read a number, short enough that it is gone before it is in
// the way. Noctalia's own display used the same order of magnitude. Each card
// in the file carries its own clock of this length.
constexpr qint64 visibleMs = 1800;
// One full exit beat plus a breath. The QML's recede runs for the theme's
// `motionNormal` (200 ms, pinned by the style tests); the controller waits a
// little longer so the next card's entry visibly starts after the departure
// has finished, never on top of it.
constexpr int transitionMs = 260;
const char componentName[] = "SessionOsd";

// The card the QML draws, in shell units. Pinned by `tst_sessionosd.qml`
// against the component itself, so a drift between the two is a failing test
// rather than a mouth beside its card.
constexpr QSizeF cardSize(260, 96);
constexpr qreal cardInset = 8;
// Upper bound for the connector gap plus the drop's overshoot: the QML
// computes the exact proportional gap from its theme tokens, and the window
// only has to be tall enough to contain it.
constexpr qreal connectorSlack = 96;
// How much of a card behind stays visible under the one before it, and how
// many kinds can pile up: the window is sized for the full file from the
// start, because a layer surface that grew per card would reconfigure per
// wheel notch. Pinned against the QML's own `stackPeek`.
constexpr qreal stackPeek = 28;
constexpr int stackDepth = 3;

// A rectangle in output pixels, translated onto the output and into shell
// units — the same conversion `OverlayController` applies to a click.
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

OsdController::OsdController(
    QQmlEngine *engine,
    ShellProvidersClient *providers,
    QObject *parent
)
    : QObject(parent)
    , m_component(engine)
    , m_providers(providers)
    , m_surface(new OverlaySurface(
          OverlaySurface::Placement::Corner,
          QStringLiteral("celestina-osd"),
          this
      ))
    , m_fallback(new OverlaySurface(
          OverlaySurface::Placement::BottomRight,
          QStringLiteral("celestina-osd"),
          this
      ))
    , m_enabled(true)
{
    m_component.loadFromModule("CelestinaDesktop", QLatin1String(componentName));
    if (!m_component.isReady()) {
        qCritical().noquote()
            << "Celestina could not load its on-screen display:"
            << m_component.errorString();
        m_enabled = false;
    }

    m_clock.start();
    m_expiryTimer.setSingleShot(true);
    connect(&m_expiryTimer, &QTimer::timeout, this, &OsdController::expire);
    m_transitionTimer.setSingleShot(true);
    m_transitionTimer.setInterval(transitionMs);
    connect(&m_transitionTimer, &QTimer::timeout,
            this, &OsdController::finishClose);

    // Deliberately no premapping: bringing the two persistent surfaces up
    // during the shell's own start stopped the compositor from drawing the
    // whole overlay layer and the wallpaper with it — measured on a fresh
    // nest, twice. The surfaces are brought up by the first reading instead,
    // and persist from then on; the render kick makes that first mapping
    // fast enough for a card's lifetime.

    if (m_providers) {
        connect(
            m_providers,
            &ShellProvidersClient::changed,
            this,
            &OsdController::providersChanged
        );
    }
}

bool OsdController::isVisible() const
{
    // The windows persist between readings; what "visible" means to a caller
    // is whether any card is on one of them.
    return !m_active.isEmpty()
        && (m_surface->isOpen() || m_fallback->isOpen());
}

QWindow *OsdController::activeWindow() const
{
    return m_activeTop ? m_surface->window() : m_fallback->window();
}

bool OsdController::topIntruded() const
{
    QScreen *const screen = m_openScreen.data();
    return screen && m_zoneProbe
        && quietZoneOccupied(m_openCard, m_zoneProbe(screen));
}

void OsdController::setPanels(PanelManager *panels)
{
    m_panels = panels;
}

QRectF OsdController::openCardRectOnOutput(QScreen *screen) const
{
    // A resting persistent window occupies nothing: only live cards make the
    // toasts yield the corner — and only where those cards actually are.
    if (m_active.isEmpty() || !screen || m_openScreen.data() != screen)
        return QRectF();
    return m_activeTop ? m_openCard : m_fallbackCard;
}

void OsdController::ensureSurfaces(QScreen *screen)
{
    if (m_openScreen.data() != screen) {
        m_surface->close();
        m_fallback->close();
        m_openScreen = screen;
        m_openAttached = false;
        m_attachedCarrierOrigin = QPointF();
    }
    if (!m_surface->isOpen())
        openTop(screen);
    if (!m_fallback->isOpen())
        openFallback(screen);
}

QString OsdController::frontKind() const
{
    return m_active.isEmpty()
        ? QString()
        : m_active.first().toMap().value(QStringLiteral("kind")).toString();
}

void OsdController::providersChanged()
{
    if (!m_providers)
        return;

    if (!m_providers->available()) {
        // Nothing published can be compared with what a dead helper last said,
        // so the next value is a baseline rather than a change to announce.
        m_readings.forget();
        return;
    }

    // Several capabilities can change in one frame; each is its own card.
    const QList<OsdReadings::Reading> readings =
        m_readings.apply(m_providers->providers());
    for (const OsdReadings::Reading &reading : readings)
        show(reading);
}

QWindow *OsdController::createWindow(const QVariantMap &placementProperties)
{
    QVariantMap initialProperties {
        // Both persistent twins are born empty. Even the four compatibility
        // properties must not inherit the active front: SessionOsd used to
        // synthesize a card from them while `readings` was empty, so omitting
        // only the list still left the resting twin painting a ghost.
        {QStringLiteral("kind"), QString()},
        {QStringLiteral("percent"), -1},
        {QStringLiteral("muted"), false},
        {QStringLiteral("label"), QString()},
        {QStringLiteral("readings"), QVariantList()},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    initialProperties.insert(placementProperties);
    QObject *rootObject = m_component.createWithInitialProperties(initialProperties);
    if (!rootObject) {
        qCritical().noquote()
            << "Celestina could not create its on-screen display:"
            << m_component.errorString();
        return nullptr;
    }

    auto *window = qobject_cast<QWindow *>(rootObject);
    if (!window) {
        qCritical() << "Celestina's on-screen display component is not a window.";
        delete rootObject;
        return nullptr;
    }
    return window;
}

// The window shows the whole file and announces the front card through the
// same four properties it has always had.
void OsdController::pushReadings(QWindow *window)
{
    const QVariantMap front = m_active.isEmpty()
        ? QVariantMap()
        : m_active.first().toMap();
    window->setProperty("kind", front.value(QStringLiteral("kind"), QString()));
    window->setProperty("percent", front.value(QStringLiteral("percent"), -1));
    window->setProperty("muted", front.value(QStringLiteral("muted"), false));
    window->setProperty("label", front.value(QStringLiteral("label"), QString()));
    window->setProperty("readings", m_active);

    // One bounded line per push: which card fronts the file and how many ride
    // behind it. The nested session's console is unreachable; this is how a
    // card that never appeared is told apart from a card never pushed.
    DiagnosticJournal::instance().record(
        DiagnosticJournal::Record(
            DiagnosticJournal::Level::Info,
            QStringLiteral("osd.pushed"))
            .text(QStringLiteral("front"),
                  front.value(QStringLiteral("kind")).toString())
            .number(QStringLiteral("cards"), m_active.size())
    );
}

// The membrane rectangles for one reading on one screen, in output-local
// shell units, or invalid rects when that screen's panel offers no icon for
// this kind.
bool OsdController::resolveAttachment(
    QScreen *screen,
    const QString &kind,
    QRectF *opener,
    QRectF *icon,
    qreal *barHeight
) const
{
    if (!m_panels || !screen)
        return false;

    QWindow *const panel = m_panels->panelWindowFor(screen);
    if (!panel || !panel->isVisible())
        return false;

    const QuietAnchor anchor =
        quietAnchorForIcon(panel, osdIconObjectName(kind));
    if (!anchor.valid())
        return false;

    const double shellScale = shellScaleForScreen(screen);
    const QPointF outputOrigin = screen->geometry().topLeft();
    *opener = onOutputInShellUnits(anchor.opener, outputOrigin, shellScale);
    *icon = onOutputInShellUnits(anchor.icon, outputOrigin, shellScale);
    *barHeight = shellScale > 0
        ? qMax(0, panel->height()) / shellScale
        : qMax(0, panel->height());
    return true;
}

// The cards are hoverable — a card behind rises to the front. An attached
// window begins at the panel's physical lower seam, so its complete local
// carrier is safe input and the wheel's panel control is outside the QWindow.
void OsdController::applyInputMask(QWindow *window)
{
    if (!window)
        return;

    // Resting — or carrying nothing while its twin carries the file — the
    // persistent window must swallow no input at all.
    const bool carries = !m_active.isEmpty() && window == activeWindow();
    if (!carries) {
        window->setMask(QRegion(0, 0, 1, 1));
        return;
    }

    if (!m_activeTop || !m_openAttached) {
        window->setMask(QRegion());
        return;
    }

    window->setMask(QRegion(
        0, 0, qMax(1, window->width()), qMax(1, window->height())
    ));
}

void OsdController::show(const OsdReadings::Reading &reading)
{
    if (!m_enabled)
        return;

    // The level is being moved from inside its own open menu, whose slider is
    // already showing it. The card that was already up for an earlier blind
    // change of the same kind is stale the moment the menu takes over, so it
    // leaves the file too; the other kinds' cards keep their clocks.
    if (m_menus && osdSuppressedByOpenMenu(reading.kind, m_menus->openIndicator())) {
        if (m_pending && m_pending->kind == reading.kind)
            m_pending.reset();
        m_active = OsdReadings::without(m_active, {reading.kind});
        m_deadlines.remove(reading.kind);
        if (m_active.isEmpty() && !m_closing) {
            beginClose();
        } else if (QWindow *const shown = activeWindow()) {
            pushReadings(shown);
            if (m_activeTop && m_openAttached)
                updateAttachment(shown, frontKind());
            quietKickRender(shown);
        }
        return;
    }

    // One card, always the latest change: the author's rule (2026-08-13) is
    // that the last modification is the information — and no card may enter
    // while another is still leaving. A reading that arrives mid-departure
    // waits for the beat to finish and is then shown from the start.
    if (m_closing) {
        m_pending = reading;
        return;
    }

    // The same kind updates the open card in place, on the surface it is
    // already on: a wheel burst moves the number and refreshes the clock and
    // nothing re-animates.
    if (!m_active.isEmpty() && frontKind() == reading.kind) {
        m_active = OsdReadings::merged(QVariantList(), reading);
        m_deadlines.clear();
        m_deadlines.insert(reading.kind, m_clock.elapsed() + visibleMs);
        scheduleExpiry();
        if (QWindow *const shown = activeWindow()) {
            pushReadings(shown);
            if (m_activeTop && m_openAttached)
                updateAttachment(shown, reading.kind);
            quietKickRender(shown);
        }
        return;
    }

    // A different kind replaces the card, cleanly: the open card plays its
    // whole recede first, and this reading enters once it has gone.
    if (!m_active.isEmpty()) {
        m_pending = reading;
        beginClose();
        return;
    }

    m_active = OsdReadings::merged(QVariantList(), reading);
    m_deadlines.clear();
    m_deadlines.insert(reading.kind, m_clock.elapsed() + visibleMs);
    scheduleExpiry();

    // The display follows the pointer's output, like every other surface the
    // shell opens without a click position, and falls back to the primary
    // screen when the pointer sits nowhere a screen claims.
    QScreen *screen = QGuiApplication::screenAt(QCursor::pos());
    if (!screen)
        screen = QGuiApplication::primaryScreen();

    ensureSurfaces(screen);

    // Home when the bar can hold the cards, the fallback corner when
    // something interactive already sits there. Both surfaces are alive and
    // rendering, so the choice is a property push, never a remap.
    switchTo(!topIntruded());

    if (QWindow *const shown = activeWindow()) {
        pushReadings(shown);
        if (m_activeTop && m_openAttached)
            updateAttachment(shown, reading.kind);
        applyInputMask(shown);
        quietKickRender(shown);
    }
}

void OsdController::openTop(QScreen *screen)
{
    const double shellScale = shellScaleForScreen(screen);
    const QSizeF outputSize = screen && shellScale > 0
        ? QSizeF(screen->geometry().size()) / shellScale
        : QSizeF();

    // The window must contain every icon the membrane could ever point at,
    // because it maps once and the front card's kind changes underneath it.
    // The card geometry follows the front kind when there is one, or the
    // first control the panel offers when the window is brought up empty.
    QRectF opener;
    QRectF icon;
    qreal barHeight = 0;
    QuietSurfaceGeometry geometry;
    {
        bool resolved = resolveAttachment(
            screen, frontKind(), &opener, &icon, &barHeight);
        qreal leftmost = icon.x();
        for (const char *kind : {"volume", "microphone", "brightness"}) {
            QRectF kindOpener;
            QRectF kindIcon;
            qreal kindBar = 0;
            if (!resolveAttachment(screen, QLatin1String(kind),
                                   &kindOpener, &kindIcon, &kindBar)) {
                continue;
            }
            if (!resolved) {
                opener = kindOpener;
                icon = kindIcon;
                barHeight = kindBar;
                leftmost = kindIcon.x();
                resolved = true;
            }
            leftmost = qMin(leftmost, kindIcon.x());
        }
        if (resolved) {
            // Sized for the whole card file from the start: a layer surface
            // that grew per card would reconfigure per wheel notch.
            const QSizeF fileSize(
                cardSize.width(),
                cardSize.height() + stackPeek * (stackDepth - 1)
            );
            geometry = attachedQuietGeometry(
                outputSize,
                barHeight,
                opener,
                icon,
                fileSize,
                cardInset,
                connectorSlack
            );
            if (geometry.valid && leftmost - cardInset < geometry.surface.x()) {
                const qreal left = qMax<qreal>(0, leftmost - cardInset);
                geometry.surface.setX(left);
                geometry.surface.setWidth(outputSize.width() - left);
            }
        }
    }

    // Where the cards would land: attached under their icons, or floating in
    // the top-right corner when this panel offers no icon at all. Occupancy
    // is not decided here any more — both surfaces persist and `show`
    // chooses which one carries the cards.
    const QRectF prospectiveCard = geometry.valid
        ? geometry.card
        : QRectF(outputSize.width() - cardInset - cardSize.width(),
                 cardInset, cardSize.width(),
                 cardSize.height() + stackPeek * (stackDepth - 1));

    OverlaySurface::Placement placement = OverlaySurface::Placement::Corner;
    m_attachedCarrierOrigin = QPointF();
    QVariantMap placementProperties {
        {QStringLiteral("shellScale"), shellScale},
    };
    if (geometry.valid) {
        placement = OverlaySurface::Placement::AttachedTopRight;
        m_attachedCarrierOrigin = geometry.surface.topLeft();
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
    }

    QWindow *const osd = createWindow(placementProperties);
    if (!osd) {
        m_attachedCarrierOrigin = QPointF();
        return;
    }

    const int topInset = placement == OverlaySurface::Placement::AttachedTopRight
        ? geometry.topInsetInOutputUnits(shellScale) : 0;
    if (!m_surface->open(osd, screen, placement, topInset)) {
        delete osd;
        m_attachedCarrierOrigin = QPointF();
        return;
    }

    // A nested session's console is unreachable from outside it; where a
    // quiet surface went and why is the bounded fact this hunt needs.
    DiagnosticJournal::instance().record(
        DiagnosticJournal::Record(
            DiagnosticJournal::Level::Info,
            QStringLiteral("quiet.placed"))
            .text(QStringLiteral("surface"), QStringLiteral("osd"))
            .number(QStringLiteral("placement"), static_cast<int>(placement))
            .flag(QStringLiteral("anchored"), geometry.valid)
            .number(QStringLiteral("card_x"), qRound(prospectiveCard.x()))
            .number(QStringLiteral("card_y"), qRound(prospectiveCard.y()))
            .number(QStringLiteral("card_width"), qRound(prospectiveCard.width()))
    );

    m_openAttached = placement == OverlaySurface::Placement::AttachedTopRight;
    m_openScreen = screen;
    m_openCard = prospectiveCard;
    applyInputMask(osd);
    connect(osd, &QWindow::heightChanged, this, [this]() {
        applyInputMask(m_surface->window());
    });

    // Measured on the nested session: an exposed layer window does not
    // schedule its own first frame — the scene renders only when something
    // dirties it, its first commit arrived seconds late on the provider's
    // poll, and without that first commit no frame callbacks flow, so no
    // animation can tick either. A surface that lives under two seconds died
    // unpainted. Kicking one update right after mapping starts the chain.
    quietKickRender(osd);


}

// The bottom-right twin: floating, card-file sized, permanently mapped. Its
// window grows with the file — the compositor follows the size now — and
// rests transparent and inert between visits.
void OsdController::openFallback(QScreen *screen)
{
    const double shellScale = shellScaleForScreen(screen);
    const QSizeF outputSize = screen && shellScale > 0
        ? QSizeF(screen->geometry().size()) / shellScale
        : QSizeF();

    const QVariantMap placementProperties {
        {QStringLiteral("shellScale"), shellScale},
        {QStringLiteral("entersFromBottom"), true},
    };
    QWindow *const osd = createWindow(placementProperties);
    if (!osd)
        return;
    if (!m_fallback->open(osd, screen, OverlaySurface::Placement::BottomRight)) {
        delete osd;
        return;
    }

    m_fallbackCard = QRectF(
        outputSize.width() - cardInset - cardSize.width(),
        outputSize.height() - cardInset
            - (cardSize.height() + stackPeek * (stackDepth - 1)),
        cardSize.width(),
        cardSize.height() + stackPeek * (stackDepth - 1)
    );
    applyInputMask(osd);
    quietKickRender(osd);
}

// Which persistent surface carries the file. Both are alive and rendering,
// so moving the cards is a property push — which is what lets a menu opening
// over the display push it aside in real time instead of hiding it.
void OsdController::switchTo(bool top)
{
    if (m_activeTop == top)
        return;

    QWindow *const from = m_activeTop ? m_surface->window()
                                      : m_fallback->window();
    m_activeTop = top;
    QWindow *const to = activeWindow();
    if (to) {
        pushReadings(to);
        if (m_activeTop && m_openAttached)
            updateAttachment(to, frontKind());
        applyInputMask(to);
        quietKickRender(to);
    }
    if (from) {
        from->setProperty("kind", QString());
        from->setProperty("percent", -1);
        from->setProperty("muted", false);
        from->setProperty("label", QString());
        from->setProperty("readings", QVariantList());
        from->setProperty("glassRects", QVariantList());
        from->setProperty("glassRegions", QVariantList());
        from->setMask(QRegion(0, 0, 1, 1));
        quietKickRender(from);
    }
}

void OsdController::retreatIfCovered()
{
    if (m_active.isEmpty() || !m_activeTop || !m_zoneProbe)
        return;
    if (!topIntruded())
        return;

    // The zone now belongs to something interactive; the file moves to the
    // corner rather than being painted over, keeping every card and every
    // clock.
    switchTo(false);
}

void OsdController::updateAttachment(QWindow *window, const QString &kind)
{
    QRectF opener;
    QRectF icon;
    qreal barHeight = 0;
    if (resolveAttachment(m_openScreen.data(), kind,
                          &opener, &icon, &barHeight)) {
        window->setProperty(
            "openerRect", opener.translated(-m_attachedCarrierOrigin));
        window->setProperty(
            "attachmentAnchorRect",
            icon.translated(-m_attachedCarrierOrigin)
        );
        window->setProperty(
            "attachmentStartY",
            barHeight - m_attachedCarrierOrigin.y()
        );
        return;
    }
    // The new reading's control is not on this panel; the cards stay where
    // they are and the membrane lets go rather than pointing at the wrong
    // icon.
    window->setProperty("attachmentStartY", -1);
}

void OsdController::scheduleExpiry()
{
    qint64 earliest = -1;
    for (const qint64 deadline : std::as_const(m_deadlines)) {
        if (earliest < 0 || deadline < earliest)
            earliest = deadline;
    }
    if (earliest < 0) {
        m_expiryTimer.stop();
        return;
    }
    m_expiryTimer.start(qMax<qint64>(0, earliest - m_clock.elapsed()));
}

void OsdController::expire()
{
    const qint64 now = m_clock.elapsed();
    QStringList done;
    for (auto it = m_deadlines.cbegin(); it != m_deadlines.cend(); ++it) {
        if (it.value() <= now)
            done.append(it.key());
    }
    for (const QString &kind : std::as_const(done))
        m_deadlines.remove(kind);
    m_active = OsdReadings::without(m_active, done);

    if (m_active.isEmpty()) {
        // Leaving on its own clock uses the same recede as being replaced;
        // the beat runs to its end before anything else may enter.
        beginClose();
        return;
    }
    if (QWindow *const shown = activeWindow()) {
        pushReadings(shown);
        if (m_activeTop && m_openAttached)
            updateAttachment(shown, frontKind());
        quietKickRender(shown);
    }
    scheduleExpiry();
}

// The departure itself: the card file empties, which the QML answers with its
// recede — fading and shrinking away — while the published glass stays under
// it to the last frame. Only when the beat has finished does `finishClose`
// clear the leftovers and admit whatever arrived in the meantime.
void OsdController::beginClose()
{
    m_expiryTimer.stop();
    m_active.clear();
    m_deadlines.clear();
    m_closing = true;
    for (QWindow *const shown : {m_surface->window(), m_fallback->window()}) {
        if (!shown)
            continue;
        pushReadings(shown);
        applyInputMask(shown);
        quietKickRender(shown);
    }
    m_transitionTimer.start();
}

void OsdController::finishClose()
{
    m_closing = false;
    // Belt to the QML's braces, exactly as before — but only now, so the
    // withdraw never cuts the receding card's blur mid-animation.
    for (QWindow *const shown : {m_surface->window(), m_fallback->window()}) {
        if (!shown)
            continue;
        shown->setProperty("glassRects", QVariantList());
        shown->setProperty("glassRegions", QVariantList());
        quietKickRender(shown);
    }
    m_activeTop = true;
    if (m_pending) {
        const OsdReadings::Reading next = *m_pending;
        m_pending.reset();
        show(next);
        return;
    }
    // The file is really at rest now: if a fullscreen window holds this
    // output, the twins give the scanout back until the next reading.
    yieldRestingToFullscreen();
}

void OsdController::setFullscreenOutputs(const QStringList &outputs)
{
    m_fullscreenOutputs = QSet<QString>(outputs.begin(), outputs.end());
    yieldRestingToFullscreen();
}

void OsdController::yieldRestingToFullscreen()
{
    // Only a resting file yields: cards on screen are being read, and the
    // recede beat owns its own end.
    if (!m_active.isEmpty() || m_closing || m_pending)
        return;
    QScreen *const screen = m_openScreen.data();
    if (!screen || !m_fullscreenOutputs.contains(screen->name()))
        return;
    if (m_surface->isOpen() || m_fallback->isOpen())
        hide();
}

void OsdController::hide()
{
    m_expiryTimer.stop();
    m_transitionTimer.stop();
    m_closing = false;
    m_pending.reset();
    m_surface->close();
    m_fallback->close();
    m_active.clear();
    m_deadlines.clear();
    m_openCard = QRectF();
    m_fallbackCard = QRectF();
    m_openScreen.clear();
    m_openAttached = false;
    m_attachedCarrierOrigin = QPointF();
    m_activeTop = true;
}
