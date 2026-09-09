//! `magnetita-peer` — the own protocol from a shell.
//!
//! ```text
//! magnetita-peer identity                    who this peer is
//! magnetita-peer pair 'magnetita://pair?…'   scan a QR by pasting it
//! magnetita-peer connect IP:PORT [--battery N] [--hold SECONDS]
//! magnetita-peer browse                      who Avahi sees
//! ```
//!
//! `MAGNETITA_PEER_DIR` overrides where the certificate and pins live.

use std::path::PathBuf;
use std::time::Duration;

use magnetita_link::trust::fingerprint_text;
use magnetita_peer::{describe, Peer};

fn dir() -> PathBuf {
    std::env::var_os("MAGNETITA_PEER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(Peer::dir_default)
}

fn usage() -> std::process::ExitCode {
    eprintln!("usage: magnetita-peer identity | pair URI | connect IP:PORT [--battery N] [--hold SECONDS] | browse");
    std::process::ExitCode::from(2)
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(verb) = args.first() else {
        return usage();
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("runtime: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let result = runtime.block_on(async {
        match verb.as_str() {
            "identity" => {
                let peer = Peer::open(&dir(), "magnetita-peer")?;
                println!(
                    "id {}\nfingerprint {}",
                    peer.device_id,
                    fingerprint_text(&peer.fingerprint)
                );
                for p in peer.pinned() {
                    println!("pinned {} {} {}", p.device_id, p.device_name, p.fingerprint);
                }
                Ok(())
            }
            "pair" => {
                let uri = args.get(1).ok_or_else(|| {
                    magnetita_link::LinkError::Connection("pair needs the QR text".into())
                })?;
                let mut peer = Peer::open(&dir(), "magnetita-peer")?;
                let (pinned, hello, session) = peer.pair(uri).await?;
                println!(
                    "paired {} ({}) fingerprint {}",
                    hello.device_id,
                    hello.device_name,
                    fingerprint_text(&pinned.peer_fingerprint)
                );
                session.close("paired");
                Ok(())
            }
            "connect" => {
                let address = args.get(1).and_then(|a| a.parse().ok()).ok_or_else(|| {
                    magnetita_link::LinkError::Connection("connect needs IP:PORT".into())
                })?;
                let battery = flag(&args, "--battery").and_then(|v| v.parse::<u8>().ok());
                let hold = flag(&args, "--hold")
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
                let peer = Peer::open(&dir(), "magnetita-peer")?;
                let (session, hello) = peer.connect(address).await?;
                println!("connected {} ({})", hello.device_id, hello.device_name);
                if let Some(level) = battery {
                    Peer::report_battery(&session, level, false).await?;
                    println!("battery {level} reported");
                }
                let until = tokio::time::Instant::now() + Duration::from_secs(hold);
                while tokio::time::Instant::now() < until {
                    match Peer::next(&session, until - tokio::time::Instant::now()).await? {
                        Some(env) => println!("{}", describe(&env)),
                        None => break,
                    }
                }
                session.close("done");
                Ok(())
            }
            "browse" => {
                for p in Peer::browse() {
                    println!("{} {}", p.device_id, p.address);
                }
                Ok(())
            }
            _ => Err(magnetita_link::LinkError::Connection("unknown verb".into())),
        }
    });
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}
