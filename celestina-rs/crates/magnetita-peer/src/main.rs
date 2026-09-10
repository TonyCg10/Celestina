//! `magnetita-peer` — the own protocol from a shell.
//!
//! ```text
//! magnetita-peer identity                    who this peer is
//! magnetita-peer pair 'magnetita://pair?…'   scan a QR by pasting it
//! magnetita-peer connect IP:PORT [--battery N] [--clipboard TEXT] [--notify APP|TITLE|BODY] [--hold SECONDS]
//! magnetita-peer browse                      who Avahi sees
//! ```
//!
//! `MAGNETITA_PEER_DIR` overrides where the certificate and pins live.

use std::path::PathBuf;
use std::time::Duration;

use magnetita_link::trust::fingerprint_text;
use magnetita_link::LinkError;
use magnetita_peer::{browse, clipboard_text, describe, dir_default, Phone};
use magnetita_proto::daily::notifications::{Action, NotificationPosted};

fn dir() -> PathBuf {
    std::env::var_os("MAGNETITA_PEER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(dir_default)
}

fn usage() -> std::process::ExitCode {
    eprintln!("usage: magnetita-peer identity | pair URI | connect IP:PORT [--battery N] [--clipboard TEXT] [--notify APP|TITLE|BODY] [--hold SECONDS] | browse");
    std::process::ExitCode::from(2)
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

async fn run(args: &[String]) -> Result<(), LinkError> {
    match args[0].as_str() {
        "identity" => {
            let phone = Phone::open(&dir(), "magnetita-peer")?;
            println!(
                "id {}\nfingerprint {}",
                phone.device_id,
                fingerprint_text(&phone.fingerprint)
            );
            for p in phone.pinned() {
                println!("pinned {} {} {}", p.device_id, p.device_name, p.fingerprint);
            }
            Ok(())
        }
        "pair" => {
            let uri = args
                .get(1)
                .ok_or_else(|| LinkError::Connection("pair needs the QR text".into()))?;
            let phone = Phone::open(&dir(), "magnetita-peer")?;
            let (pinned, session) = phone.pair(uri).await?;
            println!(
                "paired {} ({}) fingerprint {}",
                session.desktop.device_id,
                session.desktop.device_name,
                fingerprint_text(&pinned.peer_fingerprint)
            );
            session.close("paired");
            Ok(())
        }
        "connect" => {
            let address = args
                .get(1)
                .and_then(|a| a.parse().ok())
                .ok_or_else(|| LinkError::Connection("connect needs IP:PORT".into()))?;
            let battery = flag(args, "--battery").and_then(|v| v.parse::<u8>().ok());
            let hold = flag(args, "--hold")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            let phone = Phone::open(&dir(), "magnetita-peer")?;
            let session = phone.connect(address).await?;
            println!(
                "connected {} ({})",
                session.desktop.device_id, session.desktop.device_name
            );
            if let Some(level) = battery {
                session.report_battery(level, false).await?;
                println!("battery {level} reported");
            }
            if let Some(text) = flag(args, "--clipboard") {
                session.send_clipboard(text).await?;
                println!("clipboard sent ({} bytes)", text.len());
            }
            if let Some(spec) = flag(args, "--notify") {
                // APP|TITLE|BODY, one button, replyable: enough to see it land.
                let mut parts = spec.splitn(3, '|');
                let app = parts.next().unwrap_or("magnetita-peer");
                let title = parts.next().unwrap_or("");
                let body = parts.next().unwrap_or("");
                session
                    .send_notification(&NotificationPosted {
                        key: "peer|1".into(),
                        app_name: app.into(),
                        title: title.into(),
                        body: body.into(),
                        timestamp_ms: 0,
                        replyable: true,
                        actions: vec![Action { label: "OK".into() }],
                        icon: None,
                    })
                    .await?;
                println!("notification sent");
            }
            let until = tokio::time::Instant::now() + Duration::from_secs(hold);
            while tokio::time::Instant::now() < until {
                match session.next(until - tokio::time::Instant::now()).await? {
                    Some(env) => match clipboard_text(&env) {
                        Some(text) => println!("clipboard: {text}"),
                        None => println!("{}", describe(&env)),
                    },
                    None => break,
                }
            }
            session.close("done");
            Ok(())
        }
        "browse" => {
            for p in browse() {
                println!("{} {}", p.device_id, p.address);
            }
            Ok(())
        }
        _ => Err(LinkError::Connection("unknown verb".into())),
    }
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return usage();
    }
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("runtime: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run(&args)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::FAILURE
        }
    }
}
