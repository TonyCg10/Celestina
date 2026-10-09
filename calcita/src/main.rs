mod activation;
mod appearance;
mod controller;

use std::path::PathBuf;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};

/// The freedesktop application ID. The installed `.desktop` basename, the
/// icon name and the Wayland `app_id` the compositor matches must all be this
/// one string, which the suite's activation names own.
const APP_ID: &str = celestina_core::activation::CALCITA.0;

/// The QML entry point compiled into the binary.
const MAIN_QML: &str = "qrc:/qt/qml/org/celestina/calcita/qml/Main.qml";

fn main() {
    // A platform theme routes dialogs through this session's portal; without
    // one Qt draws its own. An explicit choice in the environment still wins.
    if std::env::var_os("QT_QPA_PLATFORMTHEME").is_none() {
        std::env::set_var("QT_QPA_PLATFORMTHEME", "xdgdesktopportal");
    }

    // The documents named on the command line (`Exec=calcita %U`), kept as
    // the bytes they arrived in. Flags are not part of Calcita's interface.
    let argv_paths: Vec<PathBuf> = std::env::args_os()
        .skip(1)
        .filter(|argument| !argument.to_string_lossy().starts_with('-'))
        .map(PathBuf::from)
        .collect();

    // Claim the single-instance name before any window exists: a launch that
    // finds Calcita running hands its paths over and ends here, so two quick
    // launches never race to build two processes.
    activation::claim(&argv_paths);

    let mut app = QGuiApplication::new();
    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("Calcita"));
        app.as_mut()
            .set_application_display_name(&QString::from("Calcita"));
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
        // Development only: scripts/qml-dev.sh serves the source QML as an
        // import tree, so a QML edit needs a restart rather than a build.
        if let Some(import) = std::env::var_os("CELESTINA_QML_DEV_IMPORT") {
            engine
                .as_mut()
                .add_import_path(&QString::from(import.to_string_lossy().as_ref()));
        }
        let main = std::env::var("CELESTINA_QML_DEV_MAIN").unwrap_or_else(|_| MAIN_QML.to_owned());
        engine.load(&QUrl::from(&QString::from(main.as_str())));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
