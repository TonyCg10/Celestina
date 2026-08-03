#include <cstdio>
#include <cstdlib>

#include <QCoreApplication>
#include <QDBusConnection>
#include <QDebug>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QPointer>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QScreen>
#include <QStandardPaths>
#include <QTimer>
#include <QVariantMap>

#include "devicesclient.h"
#include "niriclient.h"
#include "overlaycontroller.h"
#include "panelmanager.h"
#include "panelmenucontroller.h"
#include "shellclient.h"
#include "shellprovidersclient.h"
#include "shellservice.h"
#include "surfacemanager.h"
#include "trayiconprovider.h"
#include "traywatcher.h"

namespace {
bool reducedMotionRequested()
{
    return qEnvironmentVariableIsSet("CELESTINA_REDUCED_MOTION");
}

QVariantList outputScreenSnapshot()
{
    QVariantList screens;
    for (QScreen *screen : QGuiApplication::screens()) {
        const QRect geometry = screen->geometry();
        screens.append(QVariantMap {
            {QStringLiteral("name"), screen->name()},
            {QStringLiteral("width"), geometry.width()},
            {QStringLiteral("height"), geometry.height()},
            {QStringLiteral("devicePixelRatio"), screen->devicePixelRatio()},
        });
    }
    return screens;
}
}

// The session's "which screen do I share?" dialog.
//
// xdg-desktop-portal-wlr brings no dialog of its own: it runs a command and
// keeps whatever output name that command prints. Hosting that chooser here —
// rather than in a loose `qml` runtime — buys two things the session actually
// needs: a real stdout to answer on, and a stable Wayland app_id (`celestina`),
// which is what a window rule has to match on to float it instead of tiling it.
//
// It is only the chooser, not the portal. Serving ScreenCast itself waits for
// the Niri adapter (CP1) — capture belongs to whoever knows the outputs and the
// windows.
int runOutputChooser(QGuiApplication &app, QQmlEngine &engine)
{
    QQmlComponent component(&engine);
    component.loadFromModule("CelestinaDesktop", "OutputChooser");
    if (component.isError()) {
        for (const auto &error : component.errors())
            qWarning() << "celestina --pick-output:" << error.toString();
        return EXIT_FAILURE;
    }

    const QVariantMap initialProperties {
        {QStringLiteral("reducedMotion"), reducedMotionRequested()},
        {QStringLiteral("screens"), outputScreenSnapshot()},
    };
    QScopedPointer<QObject> chooser(
        component.createWithInitialProperties(initialProperties)
    );
    if (chooser.isNull()) {
        qWarning() << "celestina --pick-output: the chooser did not load";
        return EXIT_FAILURE;
    }

    const QPointer<QObject> liveChooser(chooser.data());
    const auto updateScreens = [liveChooser] {
        if (liveChooser)
            liveChooser->setProperty("screens", outputScreenSnapshot());
    };
    const auto watchScreen = [liveChooser, updateScreens](QScreen *screen) {
        QObject::connect(
            screen,
            &QScreen::geometryChanged,
            liveChooser.data(),
            [updateScreens](const QRect &) { updateScreens(); }
        );
        QObject::connect(
            screen,
            &QScreen::physicalDotsPerInchChanged,
            liveChooser.data(),
            [updateScreens](qreal) { updateScreens(); }
        );
    };
    for (QScreen *screen : QGuiApplication::screens())
        watchScreen(screen);
    QObject::connect(
        &app,
        &QGuiApplication::screenAdded,
        chooser.data(),
        [watchScreen, updateScreens](QScreen *screen) {
            watchScreen(screen);
            updateScreens();
        }
    );
    QObject::connect(
        &app,
        &QGuiApplication::screenRemoved,
        chooser.data(),
        [liveChooser, updateScreens](QScreen *) {
            if (liveChooser)
                QTimer::singleShot(0, liveChooser.data(), updateScreens);
        }
    );

    int status = EXIT_FAILURE;
    // The window answers by setting `chosen` (or `cancelled`); xdpw's simple
    // chooser protocol expects `Monitor: <output name>` on stdout.
    QObject::connect(chooser.data(), SIGNAL(chosenChanged()), &app, SLOT(quit()));
    QObject::connect(chooser.data(), SIGNAL(cancelledChanged()), &app, SLOT(quit()));
    app.setQuitOnLastWindowClosed(true);
    app.exec();

    const QString chosen = chooser->property("chosen").toString();
    if (!chosen.isEmpty()) {
        std::fputs("Monitor: ", stdout);
        std::fputs(qPrintable(chosen), stdout);
        std::fputc('\n', stdout);
        std::fflush(stdout);
        status = EXIT_SUCCESS;
    }
    // Cancelling exits non-zero *without* a name: for the backend that reads as
    // "the user does not want to share", which is not the same as a failure.
    return status;
}

int main(int argc, char *argv[])
{
    // `celestina msg` is a transient client of whatever shell owns the
    // session: no window, no Wayland connection, and never the bus name. The
    // decision is made before the application object exists, because that is
    // what chooses between a GUI process and a command.
    if (argc > 1 && qstrcmp(argv[1], "msg") == 0) {
        QCoreApplication app(argc, argv);
        app.setApplicationName("celestina");
        return runShellMessage(app.arguments().mid(2));
    }

    QGuiApplication app(argc, argv);
    app.setApplicationName("celestina");
    app.setApplicationDisplayName("Celestina Desktop");
    app.setDesktopFileName("celestina");
    app.setOrganizationName("Celestina");
    app.setQuitOnLastWindowClosed(false);

    QQmlEngine engine;

    // Make CelestinaStyle importable from source. The style tree's directory is
    // named `celestina-style`, but its module URI is `CelestinaStyle`, so expose
    // it under that name via a runtime symlink and add the import path. Self-
    // provisioning here means the panel and the chooser both resolve the style
    // without a wrapper pre-setting QML_IMPORT_PATH.
    {
        QString runtime =
            QStandardPaths::writableLocation(QStandardPaths::RuntimeLocation);
        if (runtime.isEmpty())
            runtime = QDir::tempPath();
        const QString importRoot =
            QDir(runtime).filePath(QStringLiteral("celestina-shell-import"));
        QDir().mkpath(importRoot);
        const QString styleLink =
            importRoot + QStringLiteral("/CelestinaStyle");
        // Always relink — a stale link (an older checkout, another build) would
        // silently feed the shell the wrong CelestinaStyle. QFile::remove clears
        // the symlink itself, not its target, and is a no-op when absent.
        QFile::remove(styleLink);
        QFile::link(QStringLiteral(CELESTINA_STYLE_DIR), styleLink);
        engine.addImportPath(importRoot);
    }

    if (app.arguments().contains(QStringLiteral("--pick-output")))
        return runOutputChooser(app, engine);

    // Panel mode is layer-shell or nothing. Off Wayland, LayerShellQt declines
    // to create the surface and Qt maps an ordinary window instead — the shell
    // would then report panels it does not have, which is worse than not
    // starting.
    switch (layerShellSupport(app.platformName())) {
    case LayerShellSupport::Unavailable:
        qCritical().noquote()
            << "Celestina's panel needs a Wayland session with layer-shell "
               "support; this one reports the platform plugin"
            << app.platformName();
        return EXIT_FAILURE;
    case LayerShellSupport::Headless:
        qWarning() << "Celestina is running headless: its windows exist but no "
                      "layer surface does, so nothing seen here is evidence "
                      "about a compositor.";
        break;
    case LayerShellSupport::Available:
        break;
    }

    // Session providers outlive the engine and are exposed to every per-output
    // panel. Niri state arrives from the Rust helper and is marshalled by the
    // thin Qt adapter on this GUI thread.
    auto *niri = new NiriClient(&app);
    auto *phone = new DevicesClient(&app);
    // One helper carries every bar provider; this is the panel's only bridge
    // to it, shared by every output rather than created per widget.
    auto *providers = new ShellProvidersClient(&app);

    // The tray host and the provider that draws what it resolved share one
    // cache; the engine owns the provider, so neither of them owns the cache.
    auto trayIcons = QSharedPointer<TrayIconCache>::create();
    engine.addImageProvider(QStringLiteral("tray"), new TrayIconProvider(trayIcons));
    auto *tray = new TrayWatcher(trayIcons, &app);

    // The command channel is claimed before a single surface is mapped: a
    // second panel-mode process must defer to the owner, not flash a duplicate
    // panel first. A session without a bus keeps its panels and loses only the
    // channel — D-Bus degrades the service, never the shell.
    auto *shell = new ShellService(niri, &app);
    switch (shell->attach(QDBusConnection::sessionBus())) {
    case ShellService::Attachment::NameTaken:
        qCritical() << "Celestina is already running this session; deferring to "
                       "the shell that owns"
                    << ShellService::serviceName();
        return EXIT_FAILURE;
    case ShellService::Attachment::NoBus:
        break;
    case ShellService::Attachment::Owned:
        qInfo() << "Celestina owns" << ShellService::serviceName()
                << "at" << ShellService::objectPath();
        break;
    }

    // The panel's context menu is part of the panel; `CELESTINA_PANEL_MENU=0`
    // is the way back if it ever misbehaves on a session.
    auto *menu = new PanelMenuController(&engine, niri, &app);
    if (!menu->isEnabled())
        qInfo() << "Celestina is running without the panel context menu.";

    // The launcher and the clipboard history: two keybind-driven overlays,
    // opened and closed the same way, each loading its own QML component. See
    // `OverlayController`'s own doc for why one class serves both.
    auto *launcher =
        new OverlayController(&engine, providers, QStringLiteral("LauncherOverlay"), &app);
    if (!launcher->isEnabled())
        qWarning() << "Celestina is running without its launcher overlay.";
    shell->setLauncherController(launcher);

    auto *clipboard =
        new OverlayController(&engine, providers, QStringLiteral("ClipboardOverlay"), &app);
    if (!clipboard->isEnabled())
        qWarning() << "Celestina is running without its clipboard history overlay.";
    shell->setClipboardController(clipboard);

    // The menu controller draws menus; the tray host holds the conversation
    // with the application that owns one. Wiring them here keeps the controller
    // from knowing that a tray exists at all.
    QObject::connect(
        menu,
        &PanelMenuController::trayMenuNeeded,
        tray,
        &TrayWatcher::requestMenu
    );
    QObject::connect(
        menu,
        &PanelMenuController::trayEntryTriggered,
        tray,
        &TrayWatcher::triggerMenuEntry
    );
    QObject::connect(
        tray,
        &TrayWatcher::menuReady,
        menu,
        &PanelMenuController::trayMenuReady
    );

    // The manager receives the session's motion preference rather than reading
    // the environment itself: bootstrap owns the process environment, and the
    // chooser above reads the same value from the same place.
    PanelManager panels(
        &app,
        &engine,
        niri,
        phone,
        providers,
        tray,
        menu,
        reducedMotionRequested()
    );
    if (!panels.start())
        return EXIT_FAILURE;

    return app.exec();
}
