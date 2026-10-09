mod appearance;
mod commands;
mod controller;
mod devices;
mod lifecycle;
mod messages;
mod mirror_input;
mod mirror_view;
mod pairing;
mod projection;

use cxx_qt_lib::{
    QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QQuickStyle, QString,
    QUrl, QVariant,
};

/// The freedesktop application ID: the installed `.desktop` basename, the icon
/// name, and the Wayland `app_id` the compositor matches windows against.
const APP_ID: &str = "org.celestina.Magnetita";

fn main() {
    let mut app = QGuiApplication::new();

    if let Some(mut app) = app.as_mut() {
        app.as_mut()
            .set_application_name(&QString::from("Magnetita"));
        app.as_mut()
            .set_application_display_name(&QString::from("Magnetita"));
        app.as_mut()
            .set_organization_name(&QString::from("Celestina"));
        app.as_mut()
            .set_organization_domain(&QString::from("celestina.org"));
        QGuiApplication::set_desktop_file_name(&QString::from(APP_ID));
    }

    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        QQuickStyle::set_style(&QString::from("Basic"));
    }

    // `--mirror`: the daemon launched this instance for the mirror window
    // alone; the main window stays hidden and the process ends with the
    // mirror.
    let mirror_only = std::env::args().skip(1).any(|arg| arg == "--mirror");

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        // The shared media surface, registered once before any window exists.
        mirror_view::qobject::register_video_item(engine.as_mut());
        let mut initial_properties = QMap::<QMapPair_QString_QVariant>::default();
        initial_properties.insert(QString::from("mirrorOnly"), QVariant::from(&mirror_only));
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
            .unwrap_or_else(|_| "qrc:/qt/qml/org/celestina/magnetita/qml/Main.qml".to_owned());
        engine.load(&QUrl::from(&QString::from(main.as_str())));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
