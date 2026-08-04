mod activation;
mod preferences;
mod session;
mod syntax;
mod url;

use cxx_qt_lib::{
    QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QQuickStyle, QString,
    QUrl, QVariant,
};

/// The freedesktop application ID: the installed `.desktop` basename, the icon
/// name, and — because Qt reports it as the Wayland `app_id` — what the
/// compositor matches a window against. All three must be this one string.
const APP_ID: &str = "org.celestina.Grafita";

fn main() {
    // A Grafita already running takes this document into a tab; this launch
    // then has nothing to show and leaves without building a window. Opening a
    // second file should not open a second editor.
    if let Some(path) = initial_path_buf() {
        if activation::hand_off(&path) {
            return;
        }
    }

    let mut app = QGuiApplication::new();

    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("Grafita"));
        app.as_mut()
            .set_application_display_name(&QString::from("Grafita"));
        app.as_mut()
            .set_organization_name(&QString::from("Celestina"));
        app.as_mut()
            .set_organization_domain(&QString::from("celestina.org"));
        QGuiApplication::set_desktop_file_name(&QString::from(APP_ID));
    }

    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        QQuickStyle::set_style(&QString::from("Basic"));
    }

    // The syntax highlighter is a hand-written C++ QObject (see
    // cpp/highlighter.h for why), registered before the QML that uses it loads.
    syntax::register_highlighter();

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        let reduced_motion = std::env::var_os("CELESTINA_REDUCED_MOTION").is_some();
        let mut initial_properties = QMap::<QMapPair_QString_QVariant>::default();
        initial_properties.insert(
            QString::from("reducedMotion"),
            QVariant::from(&reduced_motion),
        );
        // The document to open, resolved here so the window never has to parse
        // a command line. Whether it is *editable* is decided later, by its
        // bytes — a name Grafita cannot classify is still opened and answered
        // honestly.
        initial_properties.insert(
            QString::from("initialPath"),
            QVariant::from(&QString::from(initial_path().as_str())),
        );
        engine.as_mut().set_initial_properties(&initial_properties);
        engine.load(&QUrl::from(
            "qrc:/qt/qml/org/celestina/grafita/qml/Main.qml",
        ));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}

/// The first argument that names a local file, or an empty string.
///
/// Options are skipped rather than treated as filenames, and only the first
/// document is taken: Grafita opens one document, so silently ignoring the
/// rest would be worse than a window the user can see is showing one file.
fn initial_path() -> String {
    initial_path_buf()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The first argument that names a local file.
///
/// Options are skipped rather than treated as filenames, and only the first
/// document is taken: Grafita opens one document per launch, and silently
/// ignoring the rest would be worse than a window the user can see.
fn initial_path_buf() -> Option<std::path::PathBuf> {
    std::env::args()
        .skip(1)
        .find(|argument| !argument.starts_with('-'))
        .and_then(|argument| url::local_path(&argument))
}
