mod activation;
mod image;
mod library;
mod mpris;
mod player;

use cxx_qt_lib::{
    QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QQuickStyle, QString,
    QUrl, QVariant,
};

/// The freedesktop application ID: the installed `.desktop` basename, the icon
/// name, and — because Qt reports it as the Wayland `app_id` — what the
/// compositor matches a window against. All three must be this one string.
const APP_ID: &str = "org.celestina.Fluorita";

fn main() {
    let mut app = QGuiApplication::new();

    if let Some(mut app) = app.as_mut() {
        app.as_mut()
            .set_application_name(&QString::from("Fluorita"));
        app.as_mut()
            .set_application_display_name(&QString::from("Fluorita"));
        app.as_mut()
            .set_organization_name(&QString::from("Celestina"));
        app.as_mut()
            .set_organization_domain(&QString::from("celestina.org"));
        QGuiApplication::set_desktop_file_name(&QString::from(APP_ID));
    }

    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        QQuickStyle::set_style(&QString::from("Basic"));
    }

    let requested = activation::requested_media();

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        // The video surface must exist as a QML type before the QML that uses
        // it loads, and it pins the scene graph to OpenGL, which libmpv's
        // render API requires.
        player::qobject::register_video_item(engine.as_mut());
        let reduced_motion = std::env::var_os("CELESTINA_REDUCED_MOTION").is_some();
        let mut initial_properties = QMap::<QMapPair_QString_QVariant>::default();
        initial_properties.insert(
            QString::from("reducedMotion"),
            QVariant::from(&reduced_motion),
        );
        // Display only. The real path stays a raw `PathBuf` in `activation`,
        // because a lossy label can never be turned back into a file — and the
        // window has nothing to open yet in any case.
        initial_properties.insert(
            QString::from("requestedLabel"),
            QVariant::from(&QString::from(requested.label.as_str())),
        );
        initial_properties.insert(
            QString::from("requestedKind"),
            QVariant::from(&QString::from(requested.kind_label())),
        );
        // The path the player actually opens. It is lossy only if the name is
        // not UTF-8, and that case is refused by the engine rather than opened
        // as a different file.
        initial_properties.insert(
            QString::from("requestedPath"),
            QVariant::from(&QString::from(
                requested
                    .path
                    .as_deref()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default()
                    .as_str(),
            )),
        );
        engine.as_mut().set_initial_properties(&initial_properties);
        engine.load(&QUrl::from(
            "qrc:/qt/qml/org/celestina/fluorita/qml/Main.qml",
        ));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
