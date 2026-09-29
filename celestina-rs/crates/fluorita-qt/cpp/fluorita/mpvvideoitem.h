// The Qt Quick surface libmpv renders into.
//
// This is hand-written C++ because CXX-Qt 0.9 cannot express it: the render API
// has to be driven from a `QQuickFramebufferObject::Renderer`, which means
// overriding the virtual `createRenderer()` and running on Qt's render thread
// with the GL context current — CXX-Qt exposes no virtual overriding and no
// render-thread hook. Everything *about* playback still lives in Rust; this
// file only paints what the backend already decoded.
//
// The item owns no session. It is given the backend handle as an opaque
// address by the Rust player and renders while that handle is non-zero; a zero
// handle means "no session", which is what stops it from rendering into a
// context that is being torn down.
#pragma once

#include <QtCore/QAtomicInt>
#include <QtCore/QMutex>
#include <QtCore/QSharedPointer>
#include <QtQuick/QQuickFramebufferObject>

class QQmlApplicationEngine;
class MpvVideoItem;

// What the item and its renderer share, and the only thing the renderer — or
// libmpv's update callback — ever touches outside `synchronize`.
//
// Qt forbids a `QQuickFramebufferObject::Renderer` from using its item outside
// `synchronize()`, and with reason: the renderer is destroyed on the render
// thread, after its item may already be gone (window teardown, a delegate
// destroyed while live), and libmpv calls back from a thread of its own. Both
// hold this object instead of the item. It outlives whichever of them goes
// last, and the item detaches itself from it before it is destroyed, so a
// notification that finds no item is simply dropped.
class MpvRenderState
{
public:
    // Called by the item's constructor and destructor, on the GUI thread.
    void attach(MpvVideoItem *item);
    void detach();

    // Queues `method` onto the item, on the GUI thread, if the item still
    // exists. Safe from any thread: the item cannot finish detaching while a
    // call is being queued, and Qt drops a queued call to an object that is
    // destroyed before it runs.
    void notify(const char *method);

    // The claim count: renderers that hold, or are committed to building, a
    // render context from a handle.
    void claim();
    void settle();
    int claims() const { return m_claims.loadAcquire(); }

private:
    mutable QMutex m_mutex;
    MpvVideoItem *m_item = nullptr;
    // Written from the render thread, read from the GUI thread: a plain `int`
    // would be a data race, and the whole point of this counter is that the
    // GUI thread may trust what it reads.
    QAtomicInt m_claims;
};

class MpvVideoItem : public QQuickFramebufferObject
{
    Q_OBJECT
    Q_PROPERTY(qulonglong handle READ handle WRITE setHandle NOTIFY handleChanged)
    // True while a renderer holds a render context built from a handle, or has
    // latched one and is about to. The surface binds its own visibility to this
    // so the item stays in the scene graph until the context is really gone:
    // the renderer can only free it while the item is still being synchronized,
    // and an item removed first would strand a context nobody can release.
    Q_PROPERTY(bool rendererLive READ rendererLive NOTIFY rendererLiveChanged)

public:
    explicit MpvVideoItem(QQuickItem *parent = nullptr);
    ~MpvVideoItem() override;

    Renderer *createRenderer() const override;

    qulonglong handle() const { return m_handle; }
    void setHandle(qulonglong handle);

    bool rendererLive() const { return m_state->claims() > 0; }

    // Read by the renderer in `synchronize`, the one moment it may touch the
    // item: from then on it holds the state, never the item. A claim is taken
    // there, while the GUI thread is blocked, which is what orders it against
    // `setHandle`; it is settled after the matching release notification has
    // been queued, so the GUI thread never observes the claim gone with no
    // answer on its way.
    QSharedPointer<MpvRenderState> renderState() const { return m_state; }

public Q_SLOTS:
    // Called from libmpv's update callback, which fires on the backend's own
    // thread: the queued connection is what moves the repaint request onto the
    // GUI thread, where asking for a new frame is legal.
    void requestFrame();

    // Emitted by the renderer, queued onto the GUI thread, once the render
    // context built from a handle is gone.
    void notifyContextReleased();

    // Its counterpart: the context exists and frames can be presented.
    void notifyContextCreated();

    // The render context could not be built. Nothing will ever be presented
    // from this handle, so the player must stop waiting for a first frame.
    void notifyContextFailed();

    // Queued from the render thread whenever `rendererLive` changes, because a
    // property notification has to be emitted on the GUI thread.
    void notifyRendererLiveChanged();

Q_SIGNALS:
    void handleChanged();

    void rendererLiveChanged();

    // The surface exists but cannot render. The player turns this into an
    // honest error rather than an "opening" that never resolves.
    void contextFailed();

    // The player waits for this before *loading* anything: with this output the
    // backend has no video until a render context exists, and a file loaded
    // before then ends immediately with "nothing to play".
    void contextCreated();

    // The player waits for this before letting the backend instance go: the
    // render API requires its context to be freed *first*, and the context
    // lives on the render thread where Rust cannot reach.
    void contextReleased();

private:
    qulonglong m_handle = 0;
    QSharedPointer<MpvRenderState> m_state;
};

// Registers `MpvVideo` in its own QML namespace and pins the scene graph to
// OpenGL. Call once, before any window exists: libmpv's render API speaks GL,
// and Qt picks its graphics API at first use.
void register_fluorita_video_item(QQmlApplicationEngine &engine);
