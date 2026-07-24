mod apps;
mod bookmarks;
mod controller;
mod dbus;
mod icons;
mod places;
mod properties;
mod search;
mod settings;
mod volumes;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};

fn main() {
    let mut app = QGuiApplication::new();

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
