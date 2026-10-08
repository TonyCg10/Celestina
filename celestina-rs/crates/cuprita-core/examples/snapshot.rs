//! Prints what the real backends read on this machine, so a client can be
//! checked against the live system without the window. The network part
//! comes first; Bluetooth and audio join as their clients land.
//!
//! `cargo run -p cuprita-core --example snapshot [-- --watch]`: with
//! `--watch` it then prints a line for every coalesced NetworkManager change
//! until interrupted.

use cuprita_core::network::Network;
use cuprita_core::nm::{watch, NmNetwork};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut network = NmNetwork::connect_system()?;
    println!("{:#?}", network.snapshot()?);
    if std::env::args().any(|arg| arg == "--watch") {
        let (tx, rx) = std::sync::mpsc::channel();
        let _handle = watch(move || {
            let _ = tx.send(());
        })?;
        println!("watching NetworkManager; Ctrl+C to stop");
        for () in rx {
            let snapshot = network.snapshot()?;
            println!(
                "change: wifi_enabled={} rows={}",
                snapshot.wifi_enabled,
                snapshot.networks.len()
            );
        }
    }
    Ok(())
}
