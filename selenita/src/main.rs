mod activation;
mod appearance;
mod backend;
mod capture;
mod controller;
mod flags;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};

/// The freedesktop application ID. The installed `.desktop` basename, the
/// icon name and the Wayland `app_id` the compositor matches must all be this
/// one string, which the suite's activation names own.
const APP_ID: &str = celestina_core::activation::SELENITA.0;

/// The QML entry point compiled into the binary.
const MAIN_QML: &str = "qrc:/qt/qml/org/celestina/selenita/qml/Main.qml";

fn main() {
    // A platform theme routes dialogs through this session's portal; without
    // one Qt draws its own. An explicit choice in the environment still wins.
    if std::env::var_os("QT_QPA_PLATFORMTHEME").is_none() {
        std::env::set_var("QT_QPA_PLATFORMTHEME", "xdgdesktopportal");
    }

    // Selenita opens no files: apart from the key-binding flags the command
    // line is ignored, so a path handed to it never triggers anything.
    let screenshot = match flags::parse(std::env::args_os().skip(1)) {
        Ok(flags::Launch::Window) => None,
        Ok(flags::Launch::Screenshot(kind)) => Some(kind),
        Ok(flags::Launch::Reserved(flag)) => {
            eprintln!("selenita: {flag} arrives with screen recording (SEL-1-B)");
            std::process::exit(2);
        }
        Err(error) => {
            eprintln!("selenita: {error}");
            std::process::exit(2);
        }
    };

    // A key binding's capture goes to the running instance when there is
    // one; otherwise this launch becomes the instance, takes it and stays
    // open on the history.
    if let Some(kind) = screenshot {
        match activation::send_capture(kind) {
            Ok(true) => return,
            Ok(false) => {}
            // Without a bus no instance can be reached: this launch takes
            // the capture itself.
            Err(error) => {
                eprintln!("selenita: no running instance took the capture: {error}");
            }
        }
    }

    // Claim the single-instance name before any window exists: a launch that
    // finds Selenita running asks it to come forward and ends here, so two
    // quick launches never race to build two processes.
    activation::claim();
    if let Some(kind) = screenshot {
        activation::request_capture(kind);
    }

    let mut app = QGuiApplication::new();
    if let Some(mut app) = app.as_mut() {
        app.as_mut()
            .set_application_name(&QString::from("Selenita"));
        app.as_mut()
            .set_application_display_name(&QString::from("Selenita"));
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
        // Development only: a caller may serve the source QML as an import
        // tree (CELESTINA_QML_DEV_IMPORT, CELESTINA_QML_DEV_MAIN), so a QML
        // edit needs a restart rather than a build.
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
