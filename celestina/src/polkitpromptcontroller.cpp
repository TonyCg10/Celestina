#include "polkitpromptcontroller.h"

#include <QDebug>
#include <QGuiApplication>
#include <QQmlEngine>
#include <QScreen>
#include <QVariantMap>
#include <QWindow>

#include "panelblurcontroller.h"
#include "polkitagent.h"
#include "shellscale.h"
#include "softclose.h"
#include "surfacemanager.h"

namespace {

const char componentName[] = "PolkitPrompt";
// A surface placed over everything, including what other surfaces reserved.
constexpr int ignoreExclusiveZones = -1;

// The prompt's surface: the whole output, on the overlay layer, holding the
// keyboard exclusively.
//
// Exclusive rather than on-demand, which every other focused surface in this
// shell uses. The difference is the point: on-demand asks the compositor for
// the keyboard when the surface is clicked and gives it up when something else
// takes it, and a password field that can quietly lose the keyboard mid-word
// is a password field that types the rest of the password into whatever took
// it.
LayerSurfaceSpec promptSpec(QScreen *screen)
{
    auto anchors = LayerShellQt::Window::Anchors(LayerShellQt::Window::AnchorTop);
    anchors |= LayerShellQt::Window::AnchorBottom;
    anchors |= LayerShellQt::Window::AnchorLeft;
    anchors |= LayerShellQt::Window::AnchorRight;

    LayerSurfaceSpec spec;
    spec.scope = QStringLiteral("celestina-polkit-prompt");
    spec.screen = screen;
    spec.anchors = anchors;
    spec.desiredSize = QSize(0, 0);
    spec.exclusiveZone = ignoreExclusiveZones;
    spec.layer = LayerShellQt::Window::LayerOverlay;
    spec.keyboard = LayerShellQt::Window::KeyboardInteractivityExclusive;
    spec.activateOnShow = true;
    // The compositor dismissing this surface is the person losing their
    // prompt, which the controller turns into a cancellation rather than a
    // window that lingers with nothing behind it.
    spec.closeOnDismissed = true;
    spec.acceptsFocus = true;
    return spec;
}

} // namespace

PromptRefusal promptRefusal(bool componentReady, bool alreadyShowing,
                            LayerShellSupport support, bool hasScreen)
{
    if (!componentReady)
        return PromptRefusal::NoComponent;
    if (alreadyShowing)
        return PromptRefusal::AlreadyShowing;
    if (support != LayerShellSupport::Available)
        return PromptRefusal::NoKeyboardGrab;
    if (!hasScreen)
        return PromptRefusal::NoOutput;
    return PromptRefusal::None;
}

const char *promptRefusalReason(PromptRefusal refusal)
{
    switch (refusal) {
    case PromptRefusal::None:
        return "";
    case PromptRefusal::NoComponent:
        return "its prompt component did not load";
    case PromptRefusal::AlreadyShowing:
        return "another request is already on screen";
    case PromptRefusal::NoKeyboardGrab:
        return "this platform has no layer shell to hold the keyboard with";
    case PromptRefusal::NoOutput:
        return "this session has no output to prompt on";
    }
    return "";
}

PolkitPromptController::PolkitPromptController(QQmlEngine *engine,
                                               PolkitAgent *agent,
                                               QObject *parent)
    : QObject(parent)
    , m_engine(engine)
    , m_agent(agent)
    , m_component(engine)
{
    // A controller with no engine prompts for nothing, and says so rather
    // than asking a null engine to load a component — which is a crash, not a
    // disabled prompt.
    if (m_engine) {
        m_component.loadFromModule("CelestinaDesktop", componentName);
        m_enabled = m_component.isReady();
    }
    if (!m_enabled) {
        qCritical().noquote()
            << "Celestina could not load its authorization prompt:"
            << (m_engine ? m_component.errorString()
                         : QStringLiteral("no QML engine"));
    }

    if (!m_agent)
        return;

    connect(m_agent, &PolkitAgent::authenticationRequested, this,
            &PolkitPromptController::requested);
    connect(m_agent, &PolkitAgent::authenticationFinished, this,
            [this](const QString &cookie, bool) { finished(cookie); });
    connect(m_agent, &PolkitAgent::secretRequested, this,
            [this](const QString &cookie, const QString &prompt) {
                if (m_window && cookie == m_cookie)
                    m_window->setProperty("prompt", prompt);
            });
    connect(m_agent, &PolkitAgent::visibleRequested, this,
            [this](const QString &cookie, const QString &prompt) {
                // An echoing prompt is not a password field. Until there is a
                // surface that says so, it is refused rather than shown with
                // the characters hidden — a person who cannot see what they
                // are typing into a username field will type it wrong and be
                // told their password was rejected.
                if (cookie == m_cookie)
                    m_agent->dismiss(cookie);
            });
    connect(m_agent, &PolkitAgent::problemReported, this,
            [this](const QString &cookie, const QString &text) {
                if (m_window && cookie == m_cookie)
                    m_window->setProperty("problem", text);
            });
    connect(m_agent, &PolkitAgent::informed, this,
            [this](const QString &cookie, const QString &text) {
                if (m_window && cookie == m_cookie)
                    m_window->setProperty("notice", text);
            });
}

void PolkitPromptController::requested(const QString &cookie,
                                       const QString &actionId,
                                       const QString &message,
                                       const QString &iconName,
                                       const QString &identity)
{
    const auto refuse = [this, &cookie](const char *why) {
        qWarning().noquote()
            << "Celestina refused to prompt for authorization:" << why;
        m_agent->dismiss(cookie);
    };

    // The focused workspace's output first; the cursor is not knowable here
    // (see setFocusedOutputSource), and the primary screen is the fallback a
    // session with no compositor answer gets.
    QScreen *screen = nullptr;
    if (m_focusedOutput) {
        const QString name = m_focusedOutput();
        const auto screens = QGuiApplication::screens();
        for (QScreen *const candidate : screens) {
            if (!name.isEmpty() && candidate->name() == name) {
                screen = candidate;
                break;
            }
        }
    }
    if (!screen)
        screen = QGuiApplication::primaryScreen();

    const PromptRefusal refusal = promptRefusal(
        m_enabled, isShowing(),
        layerShellSupport(QGuiApplication::platformName()), screen != nullptr);
    if (refusal != PromptRefusal::None) {
        refuse(promptRefusalReason(refusal));
        return;
    }

    const QVariantMap properties {
        {QStringLiteral("promptSource"), QVariant::fromValue(this)},
        {QStringLiteral("actionId"), actionId},
        {QStringLiteral("message"), message},
        {QStringLiteral("iconName"), iconName},
        {QStringLiteral("identity"), identity},
        {QStringLiteral("reducedMotion"),
         qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION")},
    };
    QObject *const root = m_component.createWithInitialProperties(properties);
    auto *const window = qobject_cast<QWindow *>(root);
    if (!window) {
        delete root;
        refuse("its prompt component did not create a window");
        return;
    }
    window->setProperty("shellScale", shellScaleForScreen(screen));

    m_cookie = cookie;
    m_window = window;
    if (!mapLayerSurface(window, promptSpec(screen))) {
        m_window.clear();
        m_cookie.clear();
        delete window;
        refuse("the compositor would not give it a surface");
        return;
    }

    // The same glass the overlays get: the card publishes its shapes on the
    // window and this follows them, arming the compositor blur and the dense
    // companions. Skipped silently before, which left the prompt the one
    // surface in this shell drawn with no material at all.
    if (window->metaObject()->indexOfProperty("glassRects") >= 0) {
        auto *blur = new PanelBlurController(window, window);
        blur->start();
    }

    connect(window, &QWindow::visibleChanged, this, [this](bool visible) {
        // The compositor took the surface away — a session ending, an output
        // going. Nothing was answered, so nothing was authorized.
        if (!visible && !m_cookie.isEmpty())
            dismiss();
    });
}

void PolkitPromptController::respond(QString secret)
{
    if (m_cookie.isEmpty()) {
        auto *at = reinterpret_cast<volatile char16_t *>(secret.data());
        for (qsizetype index = 0; index < secret.size(); ++index)
            at[index] = u'\0';
        secret.clear();
        return;
    }
    m_agent->respond(m_cookie, std::move(secret));
}

void PolkitPromptController::dismiss()
{
    if (m_cookie.isEmpty())
        return;
    const QString cookie = m_cookie;
    // Cleared first: `dismiss` comes back through `authenticationFinished`,
    // and a second pass would answer a request that is already over.
    m_cookie.clear();
    closeWindow();
    m_agent->dismiss(cookie);
}

void PolkitPromptController::finished(const QString &cookie)
{
    if (cookie != m_cookie)
        return;
    m_cookie.clear();
    closeWindow();
}

void PolkitPromptController::closeWindow()
{
    if (!m_window)
        return;
    QWindow *const window = m_window.data();
    m_window.clear();
    // The unified closing beat every surface in this shell has: a short fade
    // with the dense material withdrawn mid-way, instead of a card that is
    // simply gone on the next frame.
    softCloseWindow(window, [window]() {
        window->hide();
        window->deleteLater();
    });
}
