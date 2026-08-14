#include <algorithm>
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
#include "diagnosticjournal.h"
#include "niriclient.h"
#include "osdcontroller.h"
#include "lockcontroller.h"
#include "overlaycontroller.h"
#include "toastcontroller.h"
#include "panelmanager.h"
#include "panelmenucontroller.h"
#include "shellclient.h"
#include "shellprovidersclient.h"
#include "sessionactions.h"
#include "shellservice.h"
#include "surfacemanager.h"
#include "appiconprovider.h"
#include "trayiconprovider.h"
#include "wallpapermanager.h"
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

    // A folder choice belongs to the session's standard portal route. Respect
    // an explicit platform theme, but otherwise give Qt's dialog backend that
    // route before the GUI application chooses its platform integrations.
    if (!qEnvironmentVariableIsSet("QT_QPA_PLATFORMTHEME"))
        qputenv("QT_QPA_PLATFORMTHEME", QByteArrayLiteral("xdgdesktopportal"));

    QGuiApplication app(argc, argv);
    app.setApplicationName("celestina");
    app.setApplicationDisplayName("Celestina Desktop");
    app.setDesktopFileName("celestina");
    app.setOrganizationName("Celestina");
    app.setQuitOnLastWindowClosed(false);

    // The journal opens before anything else does, so a host that fails during
    // its own startup still leaves a record of having tried. The `run_id` is
    // generated here and exported now, before any helper exists, so every
    // process of this invocation writes lines that merge into one ordering.
    DiagnosticJournal::instance().open(QStringLiteral("host"));
    DiagnosticJournal::exportRunId();
    {
        // Arguments by class, never by value. A path or a session token passed
        // on a command line is not something this file may keep.
        const QStringList arguments = app.arguments().mid(1);
        DiagnosticJournal::Record start(
            DiagnosticJournal::Level::Critical,
            QStringLiteral("host.start")
        );
        start.text(QStringLiteral("version"), QStringLiteral(CELESTINA_VERSION))
            .text(
                QStringLiteral("mode"),
                arguments.contains(QStringLiteral("--pick-output"))
                    ? QStringLiteral("pick-output")
                    : QStringLiteral("panel")
            )
            .text(QStringLiteral("platform"), app.platformName())
            .number(QStringLiteral("argument_count"), arguments.size())
            .flag(
                QStringLiteral("has_unrecognized_arguments"),
                std::any_of(
                    arguments.cbegin(),
                    arguments.cend(),
                    [](const QString &argument) {
                        return argument != QStringLiteral("--pick-output");
                    }
                )
            );
        DiagnosticJournal::instance().record(start);
    }
    QObject::connect(&app, &QCoreApplication::aboutToQuit, [] {
        DiagnosticJournal::Record stop(
            DiagnosticJournal::Level::Critical,
            QStringLiteral("host.shutdown")
        );
        DiagnosticJournal::instance().record(stop);
        // Bounded and deterministic: the writer is asked to drain here rather
        // than abandoned, but an unresponsive filesystem cannot hold exit.
        DiagnosticJournal::instance().close();
    });

    QQmlEngine engine;

    // Make CelestinaStyle importable from the explicit production module, or
    // from the source-tree fallback used by the legacy developer launcher. Its
    // physical directory need not match the URI, so expose it under that name
    // through a runtime symlink and add the parent import path.
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
        const QString styleDirectory = qEnvironmentVariable(
            "CELESTINA_STYLE_PATH",
            QStringLiteral(CELESTINA_STYLE_DIR)
        );
        QFile::link(styleDirectory, styleLink);
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

    // The command channel is claimed before a single surface is mapped: a
    // second panel-mode process must defer before constructing any adapter,
    // provider or tray service. In particular, a rejected host must never run
    // an automatic DDC probe. A session without a bus keeps its panels and
    // loses only the channel — D-Bus degrades the service, never the shell.
    auto *shell = new ShellService(nullptr, &app);
    switch (shell->attach(QDBusConnection::sessionBus())) {
    case ShellService::Attachment::NameTaken:
        DiagnosticJournal::instance().record(
            CELESTINA_JOURNAL(Critical, "dbus.name.refused")
                .text(QStringLiteral("name"), ShellService::serviceName())
        );
        qCritical() << "Celestina is already running this session; deferring to "
                       "the shell that owns"
                    << ShellService::serviceName();
        return EXIT_FAILURE;
    case ShellService::Attachment::NoBus:
        DiagnosticJournal::instance().record(
            CELESTINA_JOURNAL(Warn, "dbus.absent")
                .text(QStringLiteral("name"), ShellService::serviceName())
        );
        break;
    case ShellService::Attachment::Owned:
        DiagnosticJournal::instance().record(
            CELESTINA_JOURNAL(Critical, "dbus.name.acquired")
                .text(QStringLiteral("name"), ShellService::serviceName())
        );
        qInfo() << "Celestina owns" << ShellService::serviceName()
                << "at" << ShellService::objectPath();
        break;
    }

    // Only the accepted host may start session helpers. They outlive the
    // engine and are exposed to every per-output panel. Niri state arrives
    // from the Rust helper and is marshalled on this GUI thread.
    auto *niri = new NiriClient(&app);
    shell->setNiriClient(niri);
    auto *phone = new DevicesClient(&app);
    // One helper carries every bar provider; this is the panel's only bridge
    // to it, shared by every output rather than created per widget.
    auto *providers = new ShellProvidersClient(&app);

    // The tray host and the provider that draws what it resolved share one
    // cache; the engine owns the provider, so neither of them owns the cache.
    auto trayIcons = QSharedPointer<TrayIconCache>::create();
    engine.addImageProvider(QStringLiteral("tray"), new TrayIconProvider(trayIcons));
    // Another application's own icon, for the surfaces that name applications
    // rather than tray items. It resolves and caches its own lookups.
    engine.addImageProvider(QStringLiteral("appicon"), new AppIconProvider());
    auto *tray = new TrayWatcher(trayIcons, &app);

    // The session's background: one surface per output, on the layer
    // everything else sits on. It is mapped before the panels so a screen is
    // never briefly panelled over an empty compositor backdrop.
    auto *wallpapers = new WallpaperManager(
        &app,
        &engine,
        providers,
        reducedMotionRequested(),
        &app
    );
    if (!wallpapers->start())
        qInfo() << "Celestina is running without a shell-drawn wallpaper.";

    // The panel's context menu is part of the panel; `CELESTINA_PANEL_MENU=0`
    // is the way back if it ever misbehaves on a session.
    auto *menu = new PanelMenuController(
        &engine,
        niri,
        &app
    );
    if (!menu->isEnabled())
        qInfo() << "Celestina is running without the panel context menu.";

    // The launcher and the clipboard history share one overlay lifecycle. A
    // keybind or session command centres them; a permanent panel opener may
    // additionally hand the same controller a real opener rectangle.
    auto *launcher = new OverlayController(
        &engine,
        QStringLiteral("LauncherOverlay"),
        providers,
        &app
    );
    if (!launcher->isEnabled())
        qWarning() << "Celestina is running without its launcher overlay.";
    shell->setLauncherController(launcher);

    auto *clipboard = new OverlayController(
        &engine,
        QStringLiteral("ClipboardOverlay"),
        providers,
        &app
    );
    if (!clipboard->isEnabled())
        qWarning() << "Celestina is running without its clipboard history overlay.";
    shell->setClipboardController(clipboard);
    // Session verbs change devices, and every device the shell can change is
    // behind the one provider helper.
    shell->setProvidersClient(providers);

    // The on-screen display follows the readings that helper publishes, not the
    // requests the shell made: a key that changed nothing raises nothing.
    auto *osd = new OsdController(&engine, providers, &app);
    if (!osd->isEnabled())
        qWarning() << "Celestina is running without its on-screen display.";

    // Toasts follow the notification server's own list; when another program
    // owns that name this controller simply never has anything to show.
    auto *toasts = new ToastController(&engine, providers, &app);
    if (!toasts->isEnabled())
        qWarning() << "Celestina is running without its toast stack.";

    // The keyboard path to everything a toast offers, opened from the panel's
    // unread indicator or from `celestina msg notifications-toggle`.
    auto *notificationCentre = new OverlayController(
        &engine,
        QStringLiteral("NotificationCenter"),
        providers,
        &app
    );
    if (!notificationCentre->isEnabled())
        qWarning() << "Celestina is running without its notification centre.";
    shell->setNotificationCentreController(notificationCentre);

    // One place to change what the panel reports, over the verbs that already
    // exist: `celestina msg control-centre-toggle`.
    auto *controlCentre = new OverlayController(
        &engine,
        QStringLiteral("ControlCentre"),
        providers,
        &app
    );
    if (!controlCentre->isEnabled())
        qWarning() << "Celestina is running without its control centre.";
    shell->setControlCentreController(controlCentre);

    // Ending the session: asked twice in the surface, and answered by whoever
    // owns the session rather than by this shell.
    // The one overlay that reads no provider: it asks the session to end, so
    // its bridge is the shell's own request channel.
    auto *sessionMenu = new OverlayController(
        &engine,
        QStringLiteral("SessionMenu"),
        new SessionActions(shell, &app),
        &app
    );
    if (!sessionMenu->isEnabled())
        qWarning() << "Celestina is running without its session menu.";
    shell->setSessionMenuController(sessionMenu);
    // The session lock, and with it the rule that nothing sleeps an uncovered
    // screen: `lock`, `suspend` and `lock-and-suspend` all go through it, and
    // it holds a logind delay inhibitor so a lid or an idle timer waits for
    // the cover too.
    auto *lock = new LockController(&app);
    shell->setLockController(lock);

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
        menu,
        &PanelMenuController::trayItemActivated,
        tray,
        &TrayWatcher::activate
    );
    QObject::connect(
        menu,
        &PanelMenuController::trayItemSecondaryActivated,
        tray,
        &TrayWatcher::secondaryActivate
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
    // The quiet surfaces attach to the bar and yield the top-right zone to
    // anything interactive already there. The probes are lambdas because the
    // occupants are controllers only this function knows; each surface also
    // counts the other quiet surface, so a display arriving while toasts are
    // up retreats, and the other way round.
    osd->setPanels(&panels);
    osd->setMenus(menu);
    toasts->setPanels(&panels);
    const auto interactiveCards =
        [menu, launcher, clipboard, notificationCentre, controlCentre,
         sessionMenu](QScreen *screen) {
            return QList<QRectF> {
                menu->openCardRectOnOutput(screen),
                launcher->openCardRectOnOutput(screen),
                clipboard->openCardRectOnOutput(screen),
                notificationCentre->openCardRectOnOutput(screen),
                controlCentre->openCardRectOnOutput(screen),
                sessionMenu->openCardRectOnOutput(screen),
            };
        };
    osd->setZoneProbe([interactiveCards, toasts](QScreen *screen) {
        QList<QRectF> cards = interactiveCards(screen);
        cards.append(toasts->openCardRectOnOutput(screen));
        return cards;
    });
    toasts->setCentreProbe([notificationCentre]() {
        return notificationCentre->isOpen();
    });
    toasts->setZoneProbe([interactiveCards, osd](QScreen *screen) {
        QList<QRectF> cards = interactiveCards(screen);
        cards.append(osd->openCardRectOnOutput(screen));
        return cards;
    });

    // A surface opening where a display already sits pushes it to its
    // fallback in real time, cards and clocks intact.
    QObject::connect(
        menu, &PanelMenuController::contextualSurfaceOpened,
        osd, &OsdController::retreatIfCovered);
    for (OverlayController *const opener :
         {launcher, clipboard, notificationCentre, controlCentre, sessionMenu}) {
        QObject::connect(
            opener, &OverlayController::contextualSurfaceOpened,
            osd, &OsdController::retreatIfCovered);
    }

    panels.setLauncher(launcher);
    panels.setNotificationCentre(notificationCentre);
    panels.setControlCentre(controlCentre);
    panels.setClipboard(clipboard);
    panels.setSessionMenu(sessionMenu);

    // And one contextual surface at a time. Whichever opened last says so and
    // every other retires — after the new one is up, never before it, because
    // a surface destroyed inside the click that asked for its replacement
    // takes that click with it. The panel's menus already share one surface,
    // so a menu opening only has to sweep the overlays; an overlay opening
    // sweeps the menu and its four siblings.
    QObject::connect(
        menu, &PanelMenuController::contextualSurfaceOpened,
        &panels, [&panels]() { panels.closeOverlaysExcept(nullptr); });
    for (OverlayController *const opener :
         {launcher, clipboard, notificationCentre, controlCentre, sessionMenu}) {
        QObject::connect(
            opener, &OverlayController::contextualSurfaceOpened,
            &panels, [&panels, opener]() {
                panels.closeContextualExcept(opener);
            });
    }
    if (!panels.start())
        return EXIT_FAILURE;

    return app.exec();
}
