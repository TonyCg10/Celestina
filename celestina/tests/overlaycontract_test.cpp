#include <QtTest>

#include <QQmlComponent>
#include <QQmlEngine>
#include <QStringList>
#include <QUrl>
#include <QQuickWindow>
#include <QSignalSpy>
#include <QVariantMap>

#include <memory>

#include "overlaycontroller.h"

namespace {
// Collects what Qt says while a component is created. Qt does not fail a
// component over an initial property it does not declare — it logs and carries
// on — so the log is the only place that mistake exists until a person opens
// the surface and finds it half-bound.
QStringList *captured = nullptr;

void collect(QtMsgType, const QMessageLogContext &, const QString &message)
{
    if (captured)
        captured->append(message);
}
} // namespace

// What each overlay is handed when it is created, and what a click outside its
// card does.
//
// This is the offscreen half of a defect that only ever showed up live: the
// host injected `providerSource` into every overlay component, and the session
// menu declares `shellSource` instead, so every open logged
// `SessionMenu does not have a property called providerSource`. The surface
// still drew, which is exactly why nothing caught it — the missed property was
// the one the component never wanted.
//
// It proves the contract inside the window, not the surface around it. Whether
// the compositor delivers an outside click to this window at all is what
// `OverlaySurface` now arranges by covering the output, and only a real Wayland
// session can show that; what is provable here is that the window answers such
// a click with `dismissed()` and leaves a click on the card alone.
class OverlayContractTest final : public QObject
{
    Q_OBJECT

private slots:
    void everyOverlayDeclaresTheBridgeTheListNamesForIt();
    void aPropertyTheComponentDoesNotDeclareIsVisibleAsAFailure();
    void aComponentThisShellDoesNotHaveNamesNoBridge();
    void aClickOutsideTheCardDismissesEveryOverlay();
    void aClickOnTheCardDismissesNothing();

private:
    // The overlays `main()` builds, by component name. Kept here as the set the
    // list must cover, so an overlay added without a bridge name fails this
    // case rather than a session.
    static QStringList overlays()
    {
        return {
            QStringLiteral("LauncherOverlay"),
            QStringLiteral("ClipboardOverlay"),
            QStringLiteral("NotificationCenter"),
            QStringLiteral("ControlCentre"),
            QStringLiteral("SessionMenu"),
        };
    }

    static QUrl sourceFor(const QString &component)
    {
        return QUrl::fromLocalFile(
            QStringLiteral(CELESTINA_QML_DIR "/") + component + QStringLiteral(".qml")
        );
    }

    static bool complainedAboutAProperty(const QStringList &messages)
    {
        for (const QString &message : messages) {
            if (message.contains(QStringLiteral("does not have a property called")))
                return true;
        }
        return false;
    }
};

void OverlayContractTest::everyOverlayDeclaresTheBridgeTheListNamesForIt()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &component : overlays()) {
        const QString bridge = overlaySourceProperty(component);
        QVERIFY2(!bridge.isEmpty(), qPrintable(component));

        QQmlComponent overlay(&engine, sourceFor(component));
        QVERIFY2(overlay.isReady(), qPrintable(overlay.errorString()));

        // A null bridge is enough: this is about which properties exist, and
        // every one of these surfaces reads its bridge through a guard that
        // already answers "nothing published yet".
        const QVariantMap properties {
            {QStringLiteral("reducedMotion"), true},
            {bridge, QVariant::fromValue<QObject *>(nullptr)},
        };

        QStringList messages;
        captured = &messages;
        QtMessageHandler previous = qInstallMessageHandler(collect);
        QObject *const root = overlay.createWithInitialProperties(properties);
        qInstallMessageHandler(previous);
        captured = nullptr;

        QVERIFY2(root != nullptr, qPrintable(overlay.errorString()));
        QVERIFY2(
            !complainedAboutAProperty(messages),
            qPrintable(component + QStringLiteral(": ") + messages.join(u'\n'))
        );
        delete root;
    }
}

// The case above only means something if it can fail. This is the exact
// injection the host used to perform on every overlay, against the one
// component that never declared it.
void OverlayContractTest::aPropertyTheComponentDoesNotDeclareIsVisibleAsAFailure()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    QQmlComponent menu(&engine, sourceFor(QStringLiteral("SessionMenu")));
    QVERIFY2(menu.isReady(), qPrintable(menu.errorString()));

    QStringList messages;
    captured = &messages;
    QtMessageHandler previous = qInstallMessageHandler(collect);
    QObject *const root = menu.createWithInitialProperties({
        {QStringLiteral("reducedMotion"), true},
        {QStringLiteral("shellSource"), QVariant::fromValue<QObject *>(nullptr)},
        {QStringLiteral("providerSource"), QVariant::fromValue<QObject *>(nullptr)},
    });
    qInstallMessageHandler(previous);
    captured = nullptr;

    QVERIFY(complainedAboutAProperty(messages));
    delete root;
}

void OverlayContractTest::aComponentThisShellDoesNotHaveNamesNoBridge()
{
    QVERIFY(overlaySourceProperty(QStringLiteral("Panel")).isEmpty());
    QVERIFY(overlaySourceProperty(QString()).isEmpty());
}


namespace {
// The surface covers the output, so the window is bigger than the card. This is
// what a compositor configure leaves behind, applied by hand because nothing
// here has a compositor.
constexpr int outputWidth = 1280;
constexpr int outputHeight = 800;
} // namespace

void OverlayContractTest::aClickOutsideTheCardDismissesEveryOverlay()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &component : overlays()) {
        QQmlComponent overlay(&engine, sourceFor(component));
        QVERIFY2(overlay.isReady(), qPrintable(overlay.errorString()));

        std::unique_ptr<QObject> root(overlay.createWithInitialProperties({
            {QStringLiteral("reducedMotion"), true},
            {overlaySourceProperty(component), QVariant::fromValue<QObject *>(nullptr)},
        }));
        auto *window = qobject_cast<QQuickWindow *>(root.get());
        QVERIFY2(window, qPrintable(component));

        window->resize(outputWidth, outputHeight);
        window->show();
        QVERIFY(QTest::qWaitForWindowExposed(window));

        QSignalSpy dismissed(window, SIGNAL(dismissed()));
        QVERIFY(dismissed.isValid());
        // The far corner of the output: as far outside the centred card as the
        // surface goes.
        QTest::mouseClick(window, Qt::LeftButton, {}, QPoint(4, 4));
        QCOMPARE(dismissed.count(), 1);
    }
}

void OverlayContractTest::aClickOnTheCardDismissesNothing()
{
    QQmlEngine engine;
    engine.addImportPath(QStringLiteral(CELESTINA_STYLE_IMPORT_ROOT));

    for (const QString &component : overlays()) {
        QQmlComponent overlay(&engine, sourceFor(component));
        QVERIFY2(overlay.isReady(), qPrintable(overlay.errorString()));

        std::unique_ptr<QObject> root(overlay.createWithInitialProperties({
            {QStringLiteral("reducedMotion"), true},
            {overlaySourceProperty(component), QVariant::fromValue<QObject *>(nullptr)},
        }));
        auto *window = qobject_cast<QQuickWindow *>(root.get());
        QVERIFY2(window, qPrintable(component));

        window->resize(outputWidth, outputHeight);
        window->show();
        QVERIFY(QTest::qWaitForWindowExposed(window));

        QSignalSpy dismissed(window, SIGNAL(dismissed()));
        QVERIFY(dismissed.isValid());
        // Just inside the card's right edge: card, but padding rather than a
        // control, so what stops the click is the card's own catch-all rather
        // than a button that would have swallowed it anyway.
        const int cardWidth = window->property("cardWidth").toInt();
        QVERIFY(cardWidth > 0);
        QTest::mouseClick(
            window,
            Qt::LeftButton,
            {},
            QPoint(outputWidth / 2 + cardWidth / 2 - 3, outputHeight / 2)
        );
        QCOMPARE(dismissed.count(), 0);
    }
}

QTEST_MAIN(OverlayContractTest)
#include "overlaycontract_test.moc"
