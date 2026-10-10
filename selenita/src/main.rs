mod activation;
mod appearance;
mod backend;
mod capture;
mod controller;
mod flags;
mod portal;
mod record;

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
    let asked = match flags::parse(std::env::args_os().skip(1)) {
        Ok(flags::Launch::Window) => None,
        Ok(flags::Launch::Screenshot(kind)) => Some(activation::Request::Capture(kind)),
        Ok(flags::Launch::Record) => Some(activation::Request::Recording(
            activation::RecordingAction::Toggle,
        )),
        Ok(flags::Launch::Stop) => {
            stop_recording();
            return;
        }
        Err(error) => {
            eprintln!("selenita: {error}");
            std::process::exit(2);
        }
    };

    // A key binding's request goes to the running instance when there is
    // one; otherwise this launch becomes the instance, does it and stays
    // open on the history.
    if let Some(request) = asked {
        let sent = match request {
            activation::Request::Capture(kind) => activation::send_capture(kind),
            activation::Request::Recording(action) => activation::send_recording(action),
        };
        match sent {
            Ok(true) => return,
            Ok(false) => {}
            // Without a bus no instance can be reached: this launch does it
            // itself.
            Err(error) => {
                eprintln!("selenita: no running instance took the request: {error}");
            }
        }
    }

    // Claim the single-instance name before any window exists: a launch that
    // finds Selenita running asks it to come forward and ends here, so two
    // quick launches never race to build two processes.
    activation::claim();
    if let Some(request) = asked {
        activation::request(request);
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
    // The window is gone: a recording under way is finished and published
    // (or the portal's dialog closed) before the process ends.
    record::quit(record::STOP_DEADLINE);
}

/// `selenita --stop`: asks the running instance to stop its recording; when
/// none answers (no instance, or no bus), touches the stop file the recording
/// worker watches. Never opens a window.
fn stop_recording() {
    match activation::send_recording(activation::RecordingAction::Stop) {
        Ok(true) => return,
        Ok(false) => {}
        Err(error) => eprintln!("selenita: no running instance took the stop: {error}"),
    }
    let file = match celestina_core::xdg::runtime_dir() {
        Ok(runtime) => selenita_core::record::stop_file(&runtime),
        Err(error) => {
            eprintln!("selenita: no runtime folder for the stop file: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = selenita_core::record::request_stop(&file) {
        eprintln!("selenita: cannot touch {}: {error}", file.display());
        std::process::exit(1);
    }
}
