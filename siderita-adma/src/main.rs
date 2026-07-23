mod apps;
mod bookmarks;
mod controller;
mod dbus;
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

    // Register the native list model type before any QML is loaded.
    controller::qobject::register_entry_model();

    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from(
            "qrc:/qt/qml/org/celestina/siderita/qml/i1/MainI1.qml",
        ));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
