//! `magnetita-peer` — the own protocol from a shell.
//!
//! ```text
//! magnetita-peer identity                    who this peer is
//! magnetita-peer pair 'magnetita://pair?…'   scan a QR by pasting it
//! magnetita-peer connect IP:PORT [--battery N] [--clipboard TEXT] [--notify APP|TITLE|BODY] [--send-file PATH] [--media PLAYER|TITLE|ARTIST] [--hold SECONDS]
//! magnetita-peer browse                      who Avahi sees
//! ```
//!
//! `MAGNETITA_PEER_DIR` overrides where the certificate and pins live.

use std::path::PathBuf;
use std::time::Duration;

use magnetita_link::trust::fingerprint_text;
use magnetita_link::LinkError;
use magnetita_peer::{browse, clipboard_text, describe, dir_default, Incoming, Phone};
use magnetita_proto::daily::media::MediaState;
use magnetita_proto::daily::notifications::{Action, NotificationPosted};

fn dir() -> PathBuf {
    std::env::var_os("MAGNETITA_PEER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(dir_default)
}

fn usage() -> std::process::ExitCode {
    eprintln!("usage: magnetita-peer identity | pair URI | connect IP:PORT [--battery N] [--clipboard TEXT] [--notify APP|TITLE|BODY] [--send-file PATH] [--media PLAYER|TITLE|ARTIST] [--hold SECONDS] | browse");
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
            if let Some(spec) = flag(args, "--media") {
                // PLAYER|TITLE|ARTIST as a playing state.
                let mut parts = spec.splitn(3, '|');
                let state = MediaState {
                    player: parts.next().unwrap_or("peer").into(),
                    title: parts.next().unwrap_or("").into(),
                    artist: parts.next().unwrap_or("").into(),
                    album: String::new(),
                    playing: true,
                    position_ms: 0,
                    length_ms: 0,
                    can_seek: false,
                    can_next: true,
                    can_previous: true,
                    volume: 50,
                };
                session.send_media_state(&state).await?;
                println!("media state sent");
            }
            let sending = match flag(args, "--send-file") {
                Some(path) => {
                    let path = PathBuf::from(path);
                    let size = std::fs::metadata(&path)?.len();
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("file")
                        .to_owned();
                    let id = session.offer_file(&name, size, "").await?;
                    println!("offered {name} ({size} bytes) as transfer {id}");
                    Some((id, path))
                }
                None => None,
            };
            let downloads = dir().join("downloads");
            let until = tokio::time::Instant::now() + Duration::from_secs(hold);
            while tokio::time::Instant::now() < until {
                match session.next(until - tokio::time::Instant::now()).await? {
                    Some(Incoming::Envelope(env)) => {
                        let share = magnetita_peer::share_fields(&env);
                        let offset = share.offset;
                        match (env.capability, env.kind, share.transfer) {
                            (5, 1, Some(id)) => {
                                session
                                    .accept_file(id, downloads.clone())
                                    .await
                                    .map(tokio::spawn)?;
                                println!("accepted transfer {id} into {}", downloads.display());
                            }
                            (5, 2, Some(id))
                                if sending.as_ref().is_some_and(|(mine, _)| *mine == id) =>
                            {
                                let (_, path) = sending.as_ref().unwrap();
                                let offset = offset.unwrap_or(0);
                                let bytes = std::fs::read(path)?;
                                for chunk in bytes[offset.min(bytes.len() as u64) as usize..]
                                    .chunks(64 * 1024)
                                {
                                    session.write_transfer(id, chunk).await?;
                                }
                                session.finish_transfer(id).await?;
                                println!("sent from offset {offset}");
                            }
                            (6, 1, _) => {
                                let m = magnetita_peer::media_fields(&env);
                                if let Some(state) = m.state {
                                    println!(
                                        "media: {} playing={} {} - {}",
                                        state.player, state.playing, state.artist, state.title
                                    );
                                }
                            }
                            (6, 2, _) => {
                                let m = magnetita_peer::media_fields(&env);
                                if let Some(c) = m.command {
                                    println!("media command: {} {:?}", c.player, c.button);
                                }
                            }
                            _ => match clipboard_text(&env) {
                                Some(text) => println!("clipboard: {text}"),
                                None => println!("{}", describe(&env)),
                            },
                        }
                    }
                    Some(Incoming::FileReceived {
                        transfer,
                        path,
                        complete,
                    }) => println!(
                        "transfer {transfer} {}: {}",
                        if complete { "complete" } else { "interrupted" },
                        path.display()
                    ),
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
