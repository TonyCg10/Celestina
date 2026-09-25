mod actions;
mod activation;
mod analysis;
mod analysis_session;
mod analysis_view;
mod browse;
mod lists;
mod locations;
mod privilege;
mod processes;
mod publish;
mod resources;
mod sampler;
mod sensors;
mod services;
mod usage_worker;

use cxx_qt_lib::{
    QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QQuickStyle, QString,
    QUrl, QVariant,
};

/// The freedesktop application ID: the installed `.desktop` basename, the icon
/// name, and — because Qt reports it as the Wayland `app_id` — what the
/// compositor matches a window against. All three must be this one string.
const APP_ID: &str = "org.celestina.Hematita";

fn main() {
    // Without a platform theme Qt has nobody to ask for dialogs and draws its
    // own outside this session's portal route. An explicit choice still wins.
    if std::env::var_os("QT_QPA_PLATFORMTHEME").is_none() {
        std::env::set_var("QT_QPA_PLATFORMTHEME", "xdgdesktopportal");
    }

    // The folder to browse on arrival, if one was handed (`Exec=hematita %f`).
    // Read as OS text so a non-UTF-8 argument cannot panic; such a path
    // becomes lossy and the hub ignores it as not a folder.
    // Made absolute here, against this launch's working directory, before it
    // crosses to a running instance over D-Bus or onto the browse stack; a
    // path `absolute` cannot handle stays as given and `open_path` reports it.
    let start_path = std::env::args_os()
        .nth(1)
        .map(|arg| {
            std::path::absolute(&arg)
                .map_or(arg, std::path::PathBuf::into_os_string)
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_default();

    // A Hematita already running takes this launch: it raises itself (and
    // browses the folder) and this process leaves without building a window.
    if activation::hand_off(Some(start_path.as_str()).filter(|p| !p.is_empty())) {
        return;
    }

    let mut app = QGuiApplication::new();

    if let Some(mut app) = app.as_mut() {
        app.as_mut()
            .set_application_name(&QString::from("Hematita"));
        app.as_mut()
            .set_application_display_name(&QString::from("Hematita"));
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
        let reduced_motion = std::env::var_os("CELESTINA_REDUCED_MOTION").is_some();
        let mut initial_properties = QMap::<QMapPair_QString_QVariant>::default();
        initial_properties.insert(
            QString::from("reducedMotion"),
            QVariant::from(&reduced_motion),
        );
        // The smoke's shape gate, off in every other run.
        let smoke_shape = std::env::var_os("HEMATITA_SMOKE_SHAPE").is_some();
        initial_properties.insert(QString::from("smokeShape"), QVariant::from(&smoke_shape));
        // The smoke's section walk: without it the pages nobody selected are
        // never built, and a page that cannot construct would pass the gate.
        let smoke_sections = std::env::var_os("HEMATITA_SMOKE_SECTIONS").is_some();
        initial_properties.insert(
            QString::from("smokeSections"),
            QVariant::from(&smoke_sections),
        );
        // The folder the storage section browses on arrival; empty for none.
        initial_properties.insert(
            QString::from("startPath"),
            QVariant::from(&QString::from(start_path.as_str())),
        );
        engine.as_mut().set_initial_properties(&initial_properties);
        engine.load(&QUrl::from(
            "qrc:/qt/qml/org/celestina/hematita/qml/Main.qml",
        ));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
