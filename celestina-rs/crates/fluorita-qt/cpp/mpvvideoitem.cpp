#include "fluorita/mpvvideoitem.h"

#include <QtCore/QByteArray>
#include <QtGui/QOpenGLContext>
#include <QtOpenGL/QOpenGLFramebufferObject>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/qqml.h>
#include <QtQuick/QQuickWindow>

#include <clocale>

#include <mpv/client.h>
#include <mpv/render_gl.h>

namespace {

// libmpv resolves GL entry points through this; Qt already knows how, whatever
// the platform underneath is.
void *getProcAddress(void *context, const char *name)
{
    Q_UNUSED(context);
    QOpenGLContext *current = QOpenGLContext::currentContext();
    if (!current) {
        return nullptr;
    }
    return reinterpret_cast<void *>(current->getProcAddress(QByteArray(name)));
}

// Fires on whichever thread the backend feels like; it must do nothing but ask
// the item — on the GUI thread — for a new frame.
void onMpvUpdate(void *context)
{
    QMetaObject::invokeMethod(static_cast<MpvVideoItem *>(context), "requestFrame",
                              Qt::QueuedConnection);
}

// The renderer lives on Qt's render thread. Every mpv render-API call below
// happens there, with the GL context current, which is the API's requirement.
class MpvRenderer final : public QQuickFramebufferObject::Renderer
{
public:
    explicit MpvRenderer(MpvVideoItem *item)
        : m_item(item)
    {
    }

    ~MpvRenderer() override
    {
        // Qt destroys the renderer on the render thread with the context
        // current, which is the only place this is legal.
        releaseContext();
    }

    // Runs with the GUI thread blocked: the one safe moment to read the item,
    // and therefore the one safe moment to claim responsibility for a handle.
    void synchronize(QQuickFramebufferObject *object) override
    {
        auto *item = static_cast<MpvVideoItem *>(object);
        m_window = item->window();
        const qulonglong handle = item->handle();
        if (handle != m_handle) {
            m_handle = handle;
            m_recreate = true;
        }
        // One claim per renderer, held from the moment a session's handle is
        // latched until the context built from it is freed. Claiming here — and
        // not in `createContext`, which runs after the GUI thread is released —
        // is what stops `setHandle` from concluding that nobody will answer
        // while this renderer is already committed to answering.
        if (m_handle != 0 && !m_claimed) {
            m_claimed = true;
            item->claimRenderContext();
        }
    }

    void render() override
    {
        if (m_recreate) {
            releaseContext();
            createContext();
            m_recreate = false;
        }
        if (!m_context) {
            return;
        }

        QOpenGLFramebufferObject *target = framebufferObject();
        if (!target) {
            return;
        }

        mpv_opengl_fbo fbo{static_cast<int>(target->handle()), target->width(), target->height(),
                           0};
        // Measured, not assumed: with FLIP_Y=1 every video played upside down
        // (confirmed with a directional test clip — top/bottom text swapped and
        // mirrored). `QQuickFramebufferObject`'s FBO already matches the
        // orientation mpv expects by default, so mpv must not flip a second
        // time. The comment this replaced ("Qt's FBO origin is at the bottom
        // left; mpv's is not") had it backwards: both use the standard GL
        // bottom-left-origin convention here, and passing 1 was the bug, not
        // the fix.
        int flip = 0;
        mpv_render_param params[]{
            {MPV_RENDER_PARAM_OPENGL_FBO, &fbo},
            {MPV_RENDER_PARAM_FLIP_Y, &flip},
            {MPV_RENDER_PARAM_INVALID, nullptr},
        };

        // The backend touches GL state Qt believes it owns; these two calls are
        // how Qt is told to re-establish its own state afterwards.
        if (m_window) {
            m_window->beginExternalCommands();
        }
        mpv_render_context_render(m_context, params);
        if (m_window) {
            m_window->endExternalCommands();
        }

        // Deliberately *not* calling `mpv_render_context_report_swap()` here.
        // It looks like the missing piece — the backend cannot estimate the
        // display's rate or its jitter without it — but this is the end of a
        // framebuffer render, not the moment the frame reaches the screen, and
        // it was measured: feeding the backend that timestamp took a session
        // that dropped **nothing** to one that dropped 18 frames in 14
        // seconds, because a timing model fed wrong times starts discarding
        // frames it believes are late. Reporting a swap that has not happened
        // is worse than reporting none. Doing it right means hooking
        // `QQuickWindow::frameSwapped`, which is the item's to own, not the
        // renderer's — and until something actually judders, the numbers say
        // there is nothing to fix.
    }

private:
    void createContext()
    {
        if (m_handle == 0) {
            return;
        }
        auto *mpv = reinterpret_cast<mpv_handle *>(m_handle);

        mpv_opengl_init_params gl{getProcAddress, nullptr};
        // Advanced control hands the *timing* to the backend: it says when a
        // new frame is due instead of the host redrawing whatever is current
        // whenever Qt feels like it. On a 165 Hz display showing 60 fps
        // content — every frame living for about 2.75 refreshes — that
        // difference is the whole of judder.
        mpv_render_param params[]{
            {MPV_RENDER_PARAM_API_TYPE, const_cast<char *>(MPV_RENDER_API_TYPE_OPENGL)},
            {MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, &gl},
            {MPV_RENDER_PARAM_INVALID, nullptr},
        };

        if (mpv_render_context_create(&m_context, mpv, params) < 0) {
            // A surface that cannot render is not a crash: the player still has
            // sound, a position and an honest state, and the item stays blank.
            // Saying so is the difference between that and a video stuck on
            // "opening" for ever, because the load only starts on
            // `contextCreated` and that will never arrive now.
            m_context = nullptr;
            QMetaObject::invokeMethod(m_item, "notifyContextFailed", Qt::QueuedConnection);
            return;
        }
        mpv_render_context_set_update_callback(m_context, onMpvUpdate, m_item);
        // The player may load now: there is somewhere for the frames to go.
        QMetaObject::invokeMethod(m_item, "notifyContextCreated", Qt::QueuedConnection);
    }

    // Frees the context, if one was built, and settles the claim either way.
    //
    // A claim that produced no context — creation failed — still has to be
    // settled here, because the player is waiting on exactly one answer per
    // session and cannot tell the two cases apart.
    void releaseContext()
    {
        if (m_context) {
            // Freeing drops the update callback first, so no repaint request
            // can reference a context that no longer exists.
            mpv_render_context_free(m_context);
            m_context = nullptr;
        }
        if (!m_claimed) {
            return;
        }
        m_claimed = false;
        // Tell the player, on the GUI thread, that the backend instance may now
        // be destroyed. Without this the two races: Rust drops the instance
        // while this thread still holds a context built from it.
        //
        // Queued *before* the claim is dropped, so the GUI thread can never
        // observe a settled claim with no answer on its way.
        QMetaObject::invokeMethod(m_item, "notifyContextReleased", Qt::QueuedConnection);
        m_item->settleRenderContext();
    }

    MpvVideoItem *m_item = nullptr;
    QQuickWindow *m_window = nullptr;
    mpv_render_context *m_context = nullptr;
    qulonglong m_handle = 0;
    bool m_recreate = false;
    // Whether this renderer owes the player one release notification.
    bool m_claimed = false;
};

} // namespace

MpvVideoItem::MpvVideoItem(QQuickItem *parent)
    : QQuickFramebufferObject(parent)
{
    // Qt's default: the FBO is composited as-is. Correct orientation is
    // achieved by not asking mpv to flip either (see FLIP_Y below) — mirroring
    // here as well would cancel that out and be flipped again.
    setMirrorVertically(false);
}

QQuickFramebufferObject::Renderer *MpvVideoItem::createRenderer() const
{
    return new MpvRenderer(const_cast<MpvVideoItem *>(this));
}

void MpvVideoItem::setHandle(qulonglong handle)
{
    if (m_handle == handle) {
        return;
    }
    const bool hadHandle = m_handle != 0;
    m_handle = handle;
    Q_EMIT handleChanged();
    update();

    // Nobody may be left to answer: a player closed before its item was ever
    // synchronized has no renderer holding anything, and waiting for a
    // notification that cannot arrive would leave the session open for ever.
    //
    // What decides that is the claim count, never visibility. An item that is
    // not drawing can still own a live render context, and reading "no
    // renderer" from "not visible" is what let the mpv core be destroyed
    // underneath one.
    if (handle == 0 && hadHandle && m_claims.loadAcquire() == 0) {
        Q_EMIT contextReleased();
    }
}

void MpvVideoItem::claimRenderContext()
{
    m_claims.ref();
    QMetaObject::invokeMethod(this, "notifyRendererLiveChanged", Qt::QueuedConnection);
}

void MpvVideoItem::settleRenderContext()
{
    m_claims.deref();
    QMetaObject::invokeMethod(this, "notifyRendererLiveChanged", Qt::QueuedConnection);
}

void MpvVideoItem::notifyContextReleased()
{
    Q_EMIT contextReleased();
}

void MpvVideoItem::notifyContextCreated()
{
    Q_EMIT contextCreated();
}

void MpvVideoItem::notifyContextFailed()
{
    Q_EMIT contextFailed();
}

void MpvVideoItem::notifyRendererLiveChanged()
{
    Q_EMIT rendererLiveChanged();
}

void MpvVideoItem::requestFrame()
{
    update();
}

void register_fluorita_video_item(QQmlApplicationEngine &engine)
{
    Q_UNUSED(engine);
    // libmpv refuses to work under a locale whose decimal separator is not a
    // dot, and Qt adopts the environment's. Without this the backend prints
    // "Non-C locale detected" and never finishes starting — which looked
    // exactly like a file that would not open.
    std::setlocale(LC_NUMERIC, "C");

    // libmpv's render API speaks OpenGL, and Qt picks its graphics API the
    // first time a window needs one. Pinning it here — before any window
    // exists — is what stops a Vulkan or software default from silently
    // leaving the video blank.
    QQuickWindow::setGraphicsApi(QSGRendererInterface::OpenGL);
    // Its own namespace on purpose: `org.celestina.fluorita` is a QML module
    // generated by CXX-Qt, and Qt 6 refuses `qmlRegisterType` into a namespace
    // a module already owns ("has already been used for type registration").
    qmlRegisterType<MpvVideoItem>("org.celestina.fluorita.render", 1, 0, "MpvVideo");
}
