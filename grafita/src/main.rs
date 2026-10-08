mod activation;
mod encoding;
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

    // Without a platform theme Qt has nobody to ask for a file dialog and
    // draws its own floating window outside this session's portal route. The
    // portal theme sends `FileDialog` through
    // `org.freedesktop.impl.portal.FileChooser`, which this session routes to
    // Siderita. An explicit environment choice still wins.
    if std::env::var_os("QT_QPA_PLATFORMTHEME").is_none() {
        std::env::set_var("QT_QPA_PLATFORMTHEME", "xdgdesktopportal");
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
        // Development only: scripts/qml-dev.sh lays the source QML out as an
        // import tree, so a QML change needs a restart instead of a build.
        // The tree goes first, ahead of the module compiled in.
        if let Some(import) = std::env::var_os("CELESTINA_QML_DEV_IMPORT") {
            engine
                .as_mut()
                .add_import_path(&QString::from(import.to_string_lossy().as_ref()));
        }
        let main = std::env::var("CELESTINA_QML_DEV_MAIN")
            .unwrap_or_else(|_| "qrc:/qt/qml/org/celestina/grafita/qml/Main.qml".to_owned());
        engine.load(&QUrl::from(&QString::from(main.as_str())));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}

/// The first argument that names a local file, in the form the window's
/// `openPath` reads back unchanged, or an empty string.
///
/// A name that is not UTF-8 is made absolute first, because only an absolute
/// path has the `file://` form that carries its bytes through a QString.
fn initial_path() -> String {
    initial_path_buf()
        .map(|path| match std::env::current_dir() {
            Ok(directory) if path.is_relative() => directory.join(path),
            _ => path,
        })
        .and_then(|path| url::qml_argument(&path))
        .unwrap_or_default()
}

/// The first argument that names a local file.
///
/// Options are skipped rather than treated as filenames, and only the first
/// document is taken: Grafita opens one document per launch, and silently
/// ignoring the rest would be worse than a window the user can see.
///
/// Read as `OsString`s: `std::env::args` panics on an argument that is not
/// UTF-8, and a file name that is not UTF-8 is still a file Grafita can edit.
fn initial_path_buf() -> Option<std::path::PathBuf> {
    std::env::args_os()
        .skip(1)
        .find(|argument| !argument.as_encoded_bytes().starts_with(b"-"))
        .and_then(|argument| url::local_path_os(&argument))
}
