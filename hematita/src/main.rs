mod actions;
mod activation;
mod analysis;
mod analysis_session;
mod analysis_view;
mod appearance;
mod browse;
mod kernel_text;
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
mod watchdog;

use cxx_qt_lib::{
    QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QQuickStyle, QString,
    QUrl, QVariant,
};

/// The freedesktop application ID: the installed `.desktop` basename, the icon
/// name, and — because Qt reports it as the Wayland `app_id` — what the
/// compositor matches a window against. All three must be this one string.
const APP_ID: &str = celestina_core::activation::HEMATITA.0;

fn main() {
    // Without a platform theme Qt has nobody to ask for dialogs and draws its
    // own outside this session's portal route. An explicit choice still wins.
    if std::env::var_os("QT_QPA_PLATFORMTHEME").is_none() {
        std::env::set_var("QT_QPA_PLATFORMTHEME", "xdgdesktopportal");
    }

    // The folder to browse on arrival, if one was handed (`Exec=hematita %f`),
    // read as OS text so a non-UTF-8 argument cannot panic. It crosses to a
    // running instance byte-exact (the shared hand-off makes it absolute
    // against this launch's working directory), and to this window as its
    // path key, absolute too; the hub ignores it if it is not a folder.
    let argv_paths: Vec<std::path::PathBuf> = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .into_iter()
        .collect();
    let start_path = argv_paths
        .first()
        .map(|arg| {
            celestina_core::pathkey::encode(
                &std::path::absolute(arg).unwrap_or_else(|_| arg.clone()),
            )
        })
        .unwrap_or_default();

    // The name is claimed before anything else exists, so two launches in the
    // same instant end as one window: the bus gives the name to one of them,
    // and the other asks that one to raise itself (and browse the folder) and
    // leaves without building a window.
    activation::claim(&argv_paths);

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
        let mut initial_properties = QMap::<QMapPair_QString_QVariant>::default();
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
        // Development only: scripts/qml-dev.sh lays the source QML out as an
        // import tree, so a QML change needs a restart instead of a build.
        // The tree goes first, ahead of the module compiled in.
        if let Some(import) = std::env::var_os("CELESTINA_QML_DEV_IMPORT") {
            engine
                .as_mut()
                .add_import_path(&QString::from(import.to_string_lossy().as_ref()));
        }
        let main = std::env::var("CELESTINA_QML_DEV_MAIN")
            .unwrap_or_else(|_| "qrc:/qt/qml/org/celestina/hematita/qml/Main.qml".to_owned());
        engine.load(&QUrl::from(&QString::from(main.as_str())));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
