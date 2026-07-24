mod apps;
mod bookmarks;
mod controller;
mod dbus;
mod favorites;
mod folder_views;
mod icons;
mod places;
mod properties;
mod recent;
mod search;
mod settings;
mod volumes;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};

/// The freedesktop application ID: the basename of the installed `.desktop`
/// entry, the name of the installed icon, and — because Qt reports it as the
/// Wayland `app_id` — what the compositor matches a window against. All three
/// must be this one string or the launcher shows a generic icon for a window it
/// cannot tie back to its entry.
const APP_ID: &str = "org.celestina.Siderita";

fn main() {
    let mut app = QGuiApplication::new();

    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("Siderita"));
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

    // Pin the app's freedesktop icon theme before any QML (and thus any icon)
    // loads. This Wayland session has no DE to supply one, so named icons would
    // otherwise fall back to Adwaita/hicolor. Qogir is installed and covers
    // every name Siderita uses; -Dark suits the dark glass chrome. Change this
    // one string to retheme the whole app.
    controller::qobject::apply_icon_theme(&QString::from("Qogir-Dark"));

    // Register the native list model type before any QML is loaded.
    controller::qobject::register_entry_model();

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        // The thumbnail image provider must be on the engine before the QML that
        // references image://thumb/… is loaded.
        controller::qobject::register_thumbnail_provider(engine.as_mut());
        engine.load(&QUrl::from(
            "qrc:/qt/qml/org/celestina/siderita/qml/i1/MainI1.qml",
        ));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
