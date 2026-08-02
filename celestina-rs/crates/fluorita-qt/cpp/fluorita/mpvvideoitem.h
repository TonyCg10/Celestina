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

#include <QtQuick/QQuickFramebufferObject>

class QQmlApplicationEngine;

class MpvVideoItem : public QQuickFramebufferObject
{
    Q_OBJECT
    Q_PROPERTY(qulonglong handle READ handle WRITE setHandle NOTIFY handleChanged)

public:
    explicit MpvVideoItem(QQuickItem *parent = nullptr);

    Renderer *createRenderer() const override;

    qulonglong handle() const { return m_handle; }
    void setHandle(qulonglong handle);

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

Q_SIGNALS:
    void handleChanged();

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
};

// Registers `MpvVideo` in its own QML namespace and pins the scene graph to
// OpenGL. Call once, before any window exists: libmpv's render API speaks GL,
// and Qt picks its graphics API at first use.
void register_fluorita_video_item(QQmlApplicationEngine &engine);
