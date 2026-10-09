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
mod send;

use std::path::PathBuf;

use cxx_qt_lib::{
    QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QQuickStyle, QString,
    QStringList, QUrl, QVariant,
};

/// The freedesktop application ID: the installed `.desktop` basename, the icon
/// name, and the Wayland `app_id` the compositor matches windows against.
const APP_ID: &str = "org.celestina.Magnetita";

/// What a `--send` launch does once the bus has answered.
enum SendStep {
    /// Done, or refused: exit with this code and open no window.
    Exit(i32),
    /// Several devices: the window asks which.
    Choose(send::Chooser),
}

/// Runs `magnetita --send` up to the point a person must choose, on `main`
/// before Qt exists: the bus calls block, and no window is open to freeze.
fn send_step(request: Result<send::SendRequest, send::SendError>) -> SendStep {
    let request = match request {
        Ok(request) => request,
        Err(send::SendError::NoPaths) => {
            eprintln!("magnetita: --send needs at least one file");
            return SendStep::Exit(2);
        }
    };
    if let Some(path) = send::first_non_file(&request.paths) {
        eprintln!("magnetita: not a file: {}", path.display());
        return SendStep::Exit(1);
    }
    let devices = match devices::list_devices() {
        Ok(devices) => devices,
        Err(error) => {
            eprintln!("magnetita: cannot list the devices: {error}");
            return SendStep::Exit(1);
        }
    };
    let count = request.paths.len();
    match send::plan(
        devices
            .iter()
            .map(|device| (device.id.as_str(), device.connected)),
    ) {
        send::Plan::NoDevice => {
            eprintln!("magnetita: no device is connected");
            SendStep::Exit(1)
        }
        send::Plan::One(id) if request.dry_run => {
            println!("magnetita: would send {count} file(s) to the one connected device {id}");
            SendStep::Exit(0)
        }
        send::Plan::One(id) => match send::send_files(&id, &request.paths) {
            Ok(()) => SendStep::Exit(0),
            Err(error) => {
                eprintln!("magnetita: cannot send: {error}");
                SendStep::Exit(1)
            }
        },
        send::Plan::Choose if request.dry_run => {
            println!("magnetita: would ask which connected device takes {count} file(s)");
            SendStep::Exit(0)
        }
        send::Plan::Choose => SendStep::Choose(send::Chooser {
            devices: devices
                .into_iter()
                .filter(|device| device.connected)
                .map(|device| (device.id, device.name))
                .collect(),
            paths: request.paths,
        }),
    }
}

fn main() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    let mut chooser: Option<send::Chooser> = None;
    if let Some(request) = send::parse(std::env::args_os().skip(1), &cwd) {
        match send_step(request) {
            SendStep::Exit(code) => std::process::exit(code),
            SendStep::Choose(pick) => {
                send::set_chooser(pick.clone());
                chooser = Some(pick);
            }
        }
    }
    // Only the explicit send action sends: a file given to the main entry
    // opens the window as before.
    for ignored in send::ignored_arguments(std::env::args_os().skip(1)) {
        eprintln!(
            "magnetita: ignoring {} (use --send to send it)",
            PathBuf::from(ignored).display()
        );
    }

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
    let mirror_only = std::env::args_os().skip(1).any(|arg| arg == "--mirror");

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        // The shared media surface, registered once before any window exists.
        mirror_view::qobject::register_video_item(engine.as_mut());
        let mut initial_properties = QMap::<QMapPair_QString_QVariant>::default();
        initial_properties.insert(QString::from("mirrorOnly"), QVariant::from(&mirror_only));
        // `--send` with several devices: the window is the chooser alone.
        let send_names: QStringList = chooser
            .as_ref()
            .map(|pick| {
                pick.devices
                    .iter()
                    .map(|(_, name)| QString::from(name.as_str()))
                    .collect()
            })
            .unwrap_or_default();
        let send_count = chooser.as_ref().map_or(0, |pick| {
            i32::try_from(pick.paths.len()).unwrap_or(i32::MAX)
        });
        initial_properties.insert(
            QString::from("sendDeviceNames"),
            QVariant::from(&send_names),
        );
        initial_properties.insert(QString::from("sendCount"), QVariant::from(&send_count));
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

    // The chooser sent on a worker while its window was up; the exit code
    // says whether the person's pick reached the phone.
    if chooser.is_some() {
        // A transfer still in flight finishes, or fails, before the exit code.
        send::wait_chosen();
        std::process::exit(i32::from(send::failed()));
    }
}
