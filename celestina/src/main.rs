//! The `celestina` binary: the shell, its own client and its check.
//!
//! - `celestina [--exit-after S] [--metrics] [--no-blur]` runs the shell:
//!   it claims `org.celestina.Shell` (a second shell exits 0), starts the
//!   providers and runs the loop until a signal, `--exit-after` or `quit`.
//! - `celestina <verb>` forwards the verb to the running shell and exits 0,
//!   or 1 when it could not.
//! - `celestina --check` loads the theme, the fonts and the appearance file
//!   and exits, without a compositor.
//!
//! Diagnostics go to stderr, filtered by `RUST_LOG` (default `info`).
#![forbid(unsafe_code)]

use std::{
    cell::{OnceCell, RefCell},
    io::IsTerminal,
    process::ExitCode,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use celestina::{
    app::App,
    bus::{self, BusError},
    clipboard::ClipboardDuty,
    command::{self, Mode, Verb, USAGE},
    connectivity::ConnectivityHandle,
    energy::EnergyHandle,
    error_chain,
    glance::GlanceHandle,
    island::clock,
    launcher::recent::RecentLaunches,
    message::Message,
    providers::notifications::NotificationsCommand,
    providers::{self, settings::ShellSettings, wallpaper::WallpaperCommand, Worker},
};
use celestina_paint::{default_font_dirs, TextSystem, Theme};
use celestina_shell::settings::Duty;
use celestina_wayland::Config;
use tracing_subscriber::EnvFilter;

/// How long the exit waits for each worker.
const JOIN_WAIT: Duration = Duration::from_secs(2);

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .init();
    let args = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned());
    match command::parse_args(args) {
        Ok(Mode::Usage) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Mode::Check) => check(),
        Ok(Mode::Client(verb)) => client(verb),
        Ok(Mode::Shell(cfg)) => shell(cfg),
        Err(err) => {
            eprintln!("celestina: {err}\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn check() -> ExitCode {
    let theme = Theme::dark();
    if let Err(err) = TextSystem::new(&default_font_dirs()) {
        eprintln!("celestina: {}", error_chain(&err));
        return ExitCode::FAILURE;
    }
    let appearance = celestina_settings::load();
    tracing::info!(
        accent = ?theme.accent,
        reduced_motion = appearance.reduced_motion,
        text_scale = appearance.text_scale.as_str(),
        "check passed: theme, fonts and appearance load"
    );
    ExitCode::SUCCESS
}

fn client(verb: Verb) -> ExitCode {
    match bus::send_verb(&verb) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!(
                "celestina: {}: {}",
                verb.words().join(" "),
                error_chain(&err)
            );
            ExitCode::FAILURE
        }
    }
}

fn shell(cfg: Config) -> ExitCode {
    let fonts = match TextSystem::new(&default_font_dirs()) {
        Ok(fonts) => fonts,
        Err(err) => {
            tracing::error!("{}", error_chain(&err));
            return ExitCode::FAILURE;
        }
    };
    let appearance = celestina_settings::load();
    // The one `date +%z` of the run.
    clock::utc_offset();
    if cfg.no_blur {
        tracing::info!("--no-blur: blur regions are not sent (attribution run)");
    }

    let shutdown = Arc::new(AtomicBool::new(false));
    let niri = Rc::new(OnceCell::new());
    let notifications = Rc::new(OnceCell::new());
    let tray = Rc::new(OnceCell::new());
    let fatal = Rc::new(OnceCell::new());
    let own_sender = Rc::new(OnceCell::new());
    let settings = ShellSettings::load();
    let quiet = settings.current().do_not_disturb;
    let wallpaper = Rc::new(OnceCell::new());
    let energy = Rc::new(OnceCell::new());
    let glance = Rc::new(OnceCell::new());
    let clipboard = Rc::new(OnceCell::new());
    let connectivity = Rc::new(RefCell::new(None));
    let settings_read = settings.current().clone();
    let settings_protected = settings.is_protected();
    let follow_from = settings.follow_from();
    let app = App::new(
        fonts,
        appearance,
        settings,
        Rc::clone(&niri),
        Rc::clone(&notifications),
        Rc::clone(&fatal),
        cfg.metrics,
    )
    .with_launcher(RecentLaunches::load(), Rc::clone(&own_sender))
    .with_tray(Rc::clone(&tray))
    .with_wallpaper(Rc::clone(&wallpaper))
    .with_energy(Rc::clone(&energy))
    .with_glance(Rc::clone(&glance))
    .with_clipboard(Rc::clone(&clipboard))
    .with_connectivity(Rc::clone(&connectivity));
    let mut workers: Vec<Worker> = Vec::new();
    let mut indexer = None;
    let mut follower = None;
    let mut shell_follower = None;
    let result = celestina_wayland::run(cfg.clone(), app, |sender, _| {
        // The name first: a second shell leaves before it starts anything.
        // Its layer surfaces have no buffer yet, so nothing was shown.
        match bus::claim_and_serve(sender.clone(), &shutdown) {
            Ok(worker) => workers.push(worker),
            Err(BusError::Running) => {
                eprintln!("celestina: the shell is running");
                std::process::exit(0);
            }
            Err(err) => tracing::warn!("no command surface: {}", error_chain(&err)),
        }
        let mut compositor = None;
        match providers::niri::spawn(sender.clone(), &shutdown) {
            Ok((provider, stream, requests)) => {
                // The energy duties' compositor actions use the same queue.
                compositor = Some(provider.requests.clone());
                // The slot is empty: `setup` runs once.
                let _ = niri.set(provider);
                workers.push(stream);
                workers.push(requests);
            }
            Err(err) => tracing::warn!(%err, "no niri provider; the capsule shows its fallback"),
        }
        // Every duty is off by default: with the defaults no worker starts.
        let handle = EnergyHandle::new(sender.clone(), compositor);
        handle.apply(&settings_read);
        // The slot is empty: `setup` runs once.
        let _ = energy.set(handle);
        // The system monitor and the weather run only for what is placed;
        // the weather never without a location in `shell.json`.
        let handle = GlanceHandle::new(sender.clone());
        handle.apply(&settings_read);
        // The slot is empty: `setup` runs once.
        let _ = glance.set(handle);
        // Off by default: with the duty off the history file is deleted (by
        // a reaper thread), with it on the worker loads the persisted
        // history on its own thread. Protected defaults (an unreadable
        // shell.json) delete nothing until a clean read says off.
        let duty = ClipboardDuty::new(sender.clone());
        duty.apply(&settings_read, settings_protected);
        // The slot is empty: `setup` runs once.
        let _ = clipboard.set(duty);
        // Connectivity has no duty (CEL-2-F, D9): its workers only read, and
        // a piece shows them when the author places it.
        let publisher = sender.clone();
        let handle = ConnectivityHandle::start(move |event| {
            publisher.send(Message::Connectivity(event)).is_ok()
        });
        // Nothing else holds the slot while `setup` runs.
        match connectivity.try_borrow_mut() {
            Ok(mut slot) => *slot = Some(handle),
            Err(_) => {
                tracing::warn!("the connectivity slot is in use; its workers stop");
                handle.stop();
            }
        }
        match providers::desktop::spawn(sender.clone(), &shutdown) {
            Ok(worker) => indexer = Some(worker),
            Err(err) => tracing::warn!(%err, "no desktop index; the launcher offers nothing"),
        }
        // The slot is empty: `setup` runs once.
        let _ = own_sender.set(sender.clone());
        match providers::notifications::spawn(sender.clone(), &shutdown) {
            Ok((provider, worker)) => {
                // The do-not-disturb setting in force at start; the menu
                // sends its changes. Queued before the worker serves.
                if provider
                    .send(NotificationsCommand::SetQuiet(quiet))
                    .is_err()
                {
                    tracing::warn!(
                        "the notification worker did not take the do-not-disturb setting"
                    );
                }
                // The slot is empty: `setup` runs once.
                let _ = notifications.set(provider);
                workers.push(worker);
            }
            Err(err) => {
                tracing::warn!(%err, "no notification server; notifications are not served")
            }
        }
        // The host claims the watcher name only if it is free: with another
        // program holding it, the Tray page says so and nothing else runs.
        match providers::tray::spawn(sender.clone(), &shutdown) {
            Ok((provider, worker)) => {
                let _ = tray.set(provider);
                workers.push(worker);
            }
            Err(err) => tracing::warn!(%err, "no tray host; the Tray page stays empty"),
        }
        // The wallpaper worker; with the duty on, its first scan is asked
        // now, and its answer asks for the outputs already announced.
        match providers::wallpaper::spawn(sender.clone(), &shutdown) {
            Ok((provider, worker)) => {
                if settings_read.duties.wallpaper == Duty::On {
                    let folder = settings_read.wallpaper.folder.clone();
                    if provider
                        .request(WallpaperCommand::Rescan { folder })
                        .is_err()
                    {
                        tracing::warn!("the wallpaper worker did not take its first scan");
                    }
                }
                let _ = wallpaper.set(provider);
                workers.push(worker);
            }
            Err(err) => tracing::warn!(%err, "no wallpaper worker; no wallpaper is painted"),
        }
        // `shell.json` live: the duties (bar, wallpaper and energy), the bar's
        // layout, the energy switches and do-not-disturb follow the file.
        match providers::settings::follow_shell(follow_from, sender.clone(), &shutdown) {
            Ok(watch) => shell_follower = Some(watch),
            Err(err) => {
                tracing::warn!("the shell settings are not followed: {}", error_chain(&err))
            }
        }
        follower = Some(providers::settings::follow_appearance(sender));
    });

    shutdown.store(true, Ordering::Release);
    // Gamma restored, the inhibitor closed, the idle notifications destroyed.
    if let Some(handle) = energy.get() {
        handle.stop();
    }
    if let Some(handle) = glance.get() {
        handle.stop();
    }
    // The history file stays: only the duty being off deletes it.
    if let Some(duty) = clipboard.get() {
        duty.shutdown();
    }
    // The connectivity workers watch a flag of their own: stopped and joined
    // here, each within its bound.
    // The loop has returned: nothing else holds the slot.
    let taken = connectivity
        .try_borrow_mut()
        .ok()
        .and_then(|mut slot| slot.take());
    if let Some(handle) = taken {
        if !handle.stop() {
            tracing::warn!("a connectivity worker did not stop in time; it is left behind");
        }
    }
    drop(follower);
    drop(shell_follower);
    if let Some(indexer) = indexer {
        indexer.join_within(JOIN_WAIT);
    }
    for worker in workers {
        worker.join_within(JOIN_WAIT);
    }
    if let Some(why) = fatal.get() {
        eprintln!("celestina: {why}");
        return ExitCode::FAILURE;
    }
    match result {
        Ok(report) => {
            tracing::info!(frames = report.frames, "the shell stopped");
            if let Some(summary) = report.metrics {
                println!("{summary}");
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            tracing::error!("{}", error_chain(&err));
            ExitCode::FAILURE
        }
    }
}
