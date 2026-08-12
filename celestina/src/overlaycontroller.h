#pragma once

#include <QObject>
#include <QPointer>
#include <QQmlComponent>
#include <QRectF>
#include <QString>
#include <QVariantMap>

#include "panelattachmentlease.h"

class OverlaySurface;
class QQmlEngine;
class QWindow;

// The bridge property each overlay component declares, or an empty string for a
// component this shell does not have.
//
// One list rather than five call sites, because "which property does this
// component declare" is one fact and a second copy of it is what produced
// `SessionMenu does not have a property called providerSource`. `main()` builds
// every controller through it and the regression compares it against the QML
// files themselves.
QString overlaySourceProperty(const QString &qmlComponentName);

// Opens and closes one overlay — the launcher, the clipboard history, the
// notification centre, the control centre, the session menu.
//
// They are identical in mechanics: one on-demand-keyboard surface, centred
// for a `celestina msg`/keybind request or anchored for a panel request, and
// torn down on its own dismissal. They
// differ in which QML component they load and which bridge that component
// reads, so this class owns exactly the shared part. Domain logic — searching,
// launching, selecting a history entry, arming a session verb — lives entirely
// in the QML component, which talks to its bridge the same way every bar widget
// already does (see `Panel.qml`): nothing here parses a provider payload or
// knows a launcher or a clipboard exists.
//
// What it does not do is hand every component the same property set. Qt refuses
// an initial property a component does not declare and says so at runtime,
// which is how a session menu that reads `shellSource` came to log
// `SessionMenu does not have a property called providerSource` on every open.
// The bridge is therefore named by whoever builds the controller, and
// `reducedMotion` is the only property added here — it is a presentation
// contract every one of these surfaces declares.
class OverlayController final : public QObject
{
    Q_OBJECT

public:
    // `source` is the bridge this component reads; which property it arrives
    // as comes from `overlaySourceProperty`. Four of these overlays read the
    // provider bridge as `providerSource` and the session menu reads a request
    // channel as `shellSource`, so the owner supplies the object and the list
    // supplies the name.
    OverlayController(
        QQmlEngine *engine,
        const QString &qmlComponentName,
        QObject *source,
        QObject *parent = nullptr
    );

    // Exactly what this controller would hand its component, and nothing else.
    // Exposed so a regression can compare it against what the QML file
    // declares, before a session is the thing that finds out.
    QVariantMap initialProperties() const;

    // False when the component itself failed to load — a broken QML file, not
    // a missing source. The overlay simply never opens; nothing crashes.
    bool isEnabled() const { return m_enabled; }
    bool isOpen() const;

public slots:
    void open();
    void close();
    void toggle();
    // The same overlay opened from a panel control rather than a keybind. The
    // opener's rectangle travels with it so the surface can grow out of the
    // control instead of appearing beside it; an empty rectangle means there was
    // no control, which is what a keybind is.
    void toggleFrom(
        QWindow *panel,
        const QRectF &globalOpener,
        const QRectF &globalAttachmentAnchor
    );

private slots:
    // A retired QML window may finish its close transition after a successor
    // is mapped. Only the window still adopted by the surface may close it.
    void overlayDismissed();

private:
    QWindow *createWindow();

    // Where the surface should grow from, while a panel-opened toggle is in
    // flight. Empty for a keybind, which has no origin on screen.
    QRectF m_opener;
    QRectF m_attachmentAnchor;
    QPointer<QWindow> m_openerPanel;
    PanelAttachmentLease m_attachmentLease;

    QQmlComponent m_component;
    QString m_componentName;
    QString m_sourceProperty;
    // Guarded rather than owned: the bridge outlives no overlay in practice,
    // but an overlay that opened after its source died would bind `undefined`
    // into a `required property`.
    QPointer<QObject> m_source;
    OverlaySurface *m_surface;
    bool m_enabled;
};
