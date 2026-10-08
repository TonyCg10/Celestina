mod backend;
mod controller;
mod models;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};

/// The freedesktop application ID: the installed `.desktop` basename, the icon
/// name, and, because Qt reports it as the Wayland `app_id`, what the
/// compositor matches a window against. All three must be this one string.
const APP_ID: &str = "org.celestina.Cuprita";

fn main() {
    // Without a platform theme Qt has nobody to ask for dialogs and draws its
    // own outside this session's portal route. An explicit choice still wins.
    if std::env::var_os("QT_QPA_PLATFORMTHEME").is_none() {
        std::env::set_var("QT_QPA_PLATFORMTHEME", "xdgdesktopportal");
    }

    // Claim the single-instance name before any window exists: the loser of
    // a race hands its launch to the winner and leaves here, so two launches
    // a moment apart never build two windows.
    if let controller::activation::Claim::HandedOff = controller::activation::claim() {
        return;
    }

    let mut app = QGuiApplication::new();

    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("Cuprita"));
        app.as_mut()
            .set_application_display_name(&QString::from("Cuprita"));
        app.as_mut()
            .set_organization_name(&QString::from("Celestina"));
        app.as_mut()
            .set_organization_domain(&QString::from("celestina.org"));
        QGuiApplication::set_desktop_file_name(&QString::from(APP_ID));
    }

    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        QQuickStyle::set_style(&QString::from("Basic"));
    }

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        // Development only: scripts/qml-dev.sh lays the source QML out as an
        // import tree, so a QML change needs a restart instead of a build.
        if let Some(import) = std::env::var_os("CELESTINA_QML_DEV_IMPORT") {
            engine
                .as_mut()
                .add_import_path(&QString::from(import.to_string_lossy().as_ref()));
        }
        let main = std::env::var("CELESTINA_QML_DEV_MAIN")
            .unwrap_or_else(|_| "qrc:/qt/qml/org/celestina/cuprita/qml/Main.qml".to_owned());
        engine.load(&QUrl::from(&QString::from(main.as_str())));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
