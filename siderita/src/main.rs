mod apps;
mod bookmarks;
mod controller;
mod dbus;
mod devices;
mod editor;
mod favorites;
mod folder_views;
mod format;
mod icons;
mod media;
mod places;
mod portal;
mod preferences;
mod properties;
mod recent;
mod search;
mod settings;
mod volumes;

use cxx_qt_lib::{
    QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QQuickStyle, QString,
    QUrl, QVariant,
};

/// The freedesktop application ID: the basename of the installed `.desktop`
/// entry, the name of the installed icon, and — because Qt reports it as the
/// Wayland `app_id` — what the compositor matches a window against. All three
/// must be this one string or the launcher shows a generic icon for a window it
/// cannot tie back to its entry.
const APP_ID: &str = "org.celestina.Siderita";

fn main() {
    let mut app = QGuiApplication::new();

    if let Some(mut app) = app.as_mut() {
        app.as_mut()
            .set_application_name(&QString::from("Siderita"));
        app.as_mut()
            .set_application_display_name(&QString::from("Siderita"));
        app.as_mut()
            .set_organization_name(&QString::from("Celestina"));
        app.as_mut()
            .set_organization_domain(&QString::from("celestina.org"));
        QGuiApplication::set_desktop_file_name(&QString::from(APP_ID));
    }

    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        QQuickStyle::set_style(&QString::from("Basic"));
    }

    // Register the native list model type before any QML is loaded.
    controller::qobject::register_entry_model();
    // Same for the picker's transient-parent shim: the QML that opens a picker
    // declares one, so the type has to exist before that QML is read.
    portal::qobject::register_window_parent();

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        // The thumbnail image provider must be on the engine before the QML that
        // references image://thumb/… is loaded.
        controller::qobject::register_thumbnail_provider(engine.as_mut());
        // The media surface registers its own QML type and pins the scene
        // graph to OpenGL, which libmpv's render API needs; both must happen
        // before any window exists.
        media::qobject::register_video_item(engine.as_mut());
        let reduced_motion = std::env::var_os("CELESTINA_REDUCED_MOTION").is_some();
        let mut initial_properties = QMap::<QMapPair_QString_QVariant>::default();
        initial_properties.insert(
            QString::from("reducedMotion"),
            QVariant::from(&reduced_motion),
        );
        engine.as_mut().set_initial_properties(&initial_properties);
        engine.load(&QUrl::from(
            "qrc:/qt/qml/org/celestina/siderita/qml/Main.qml",
        ));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
