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
    cell::OnceCell,
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
    command::{self, Mode, Verb, USAGE},
    error_chain,
    island::clock,
    providers::{self, Worker},
};
use celestina_paint::{default_font_dirs, TextSystem, Theme};
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
    let fatal = Rc::new(OnceCell::new());
    let app = App::new(
        fonts,
        appearance,
        Rc::clone(&niri),
        Rc::clone(&fatal),
        cfg.metrics,
    );
    let mut workers: Vec<Worker> = Vec::new();
    let mut follower = None;
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
        match providers::niri::spawn(sender.clone(), &shutdown) {
            Ok((provider, stream, requests)) => {
                // The slot is empty: `setup` runs once.
                let _ = niri.set(provider);
                workers.push(stream);
                workers.push(requests);
            }
            Err(err) => tracing::warn!(%err, "no niri provider; the capsule shows its fallback"),
        }
        follower = Some(providers::settings::follow_appearance(sender));
    });

    shutdown.store(true, Ordering::Release);
    drop(follower);
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
