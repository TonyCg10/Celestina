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

    // Runs with the GUI thread blocked: the one safe moment to read the item.
    void synchronize(QQuickFramebufferObject *object) override
    {
        auto *item = static_cast<MpvVideoItem *>(object);
        m_window = item->window();
        const qulonglong handle = item->handle();
        if (handle != m_handle) {
            m_handle = handle;
            m_recreate = true;
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
        // Qt's framebuffer has its origin at the bottom left; mpv's does not.
        int flip = 1;
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
    }

private:
    void createContext()
    {
        if (m_handle == 0) {
            return;
        }
        auto *mpv = reinterpret_cast<mpv_handle *>(m_handle);

        mpv_opengl_init_params gl{getProcAddress, nullptr};
        mpv_render_param params[]{
            {MPV_RENDER_PARAM_API_TYPE, const_cast<char *>(MPV_RENDER_API_TYPE_OPENGL)},
            {MPV_RENDER_PARAM_OPENGL_INIT_PARAMS, &gl},
            {MPV_RENDER_PARAM_INVALID, nullptr},
        };

        if (mpv_render_context_create(&m_context, mpv, params) < 0) {
            // A surface that cannot render is not a crash: the player still has
            // sound, a position and an honest state, and the item stays blank.
            m_context = nullptr;
            return;
        }
        mpv_render_context_set_update_callback(m_context, onMpvUpdate, m_item);
        // The player may load now: there is somewhere for the frames to go.
        QMetaObject::invokeMethod(m_item, "notifyContextCreated", Qt::QueuedConnection);
    }

    void releaseContext()
    {
        if (!m_context) {
            return;
        }
        // Freeing drops the update callback first, so no repaint request can
        // reference a context that no longer exists.
        mpv_render_context_free(m_context);
        m_context = nullptr;
        // Tell the player, on the GUI thread, that the backend instance may now
        // be destroyed. Without this the two races: Rust drops the instance
        // while this thread still holds a context built from it.
        QMetaObject::invokeMethod(m_item, "notifyContextReleased", Qt::QueuedConnection);
    }

    MpvVideoItem *m_item = nullptr;
    QQuickWindow *m_window = nullptr;
    mpv_render_context *m_context = nullptr;
    qulonglong m_handle = 0;
    bool m_recreate = false;
};

} // namespace

MpvVideoItem::MpvVideoItem(QQuickItem *parent)
    : QQuickFramebufferObject(parent)
{
    // Frames arrive from the backend, not from Qt's animation clock.
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

    // A renderer only exists while the item is on a window and drawing. If
    // there is none, nobody will ever answer, so the release is reported here
    // instead — otherwise closing an off-screen player would wait forever.
    if (handle == 0 && hadHandle && (!window() || !isVisible())) {
        Q_EMIT contextReleased();
    }
}

void MpvVideoItem::notifyContextReleased()
{
    Q_EMIT contextReleased();
}

void MpvVideoItem::notifyContextCreated()
{
    Q_EMIT contextCreated();
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
