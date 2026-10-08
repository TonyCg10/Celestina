//! Prints what the real backends read on this machine, so a client can be
//! checked against the live system without the window: the network first,
//! then Bluetooth; audio joins as its client lands.
//!
//! `cargo run -p cuprita-core --example snapshot [-- FLAGS]`:
//! - `--watch` then prints a line for every coalesced NetworkManager and
//!   BlueZ change until interrupted;
//! - `--discover` runs a five-second Bluetooth search and prints the devices
//!   it leaves;
//! - `--agent` registers the pairing agent, prints that it did and
//!   unregisters it (it answers nothing: no pairing is started).

use std::time::Duration;

use cuprita_core::bluetooth::Bluetooth;
use cuprita_core::bluez::{self, BluezBluetooth};
use cuprita_core::network::Network;
use cuprita_core::nm::{self, NmNetwork};

enum Change {
    Network,
    Bluetooth,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let flag = |name: &str| std::env::args().any(|arg| arg == name);
    let mut network = NmNetwork::connect_system()?;
    println!("{:#?}", network.snapshot()?);
    let mut bluetooth = BluezBluetooth::connect_system()?;
    println!("{:#?}", bluetooth.snapshot()?);

    if flag("--discover") {
        bluetooth.set_discovering(true)?;
        println!("searching for five seconds");
        std::thread::sleep(Duration::from_secs(5));
        let during = bluetooth.snapshot()?;
        bluetooth.set_discovering(false)?;
        println!(
            "during the search: discovering={} devices={}",
            during.discovering,
            during.devices.len()
        );
        let after = bluetooth.snapshot()?;
        println!("after: discovering={}", after.discovering);
    }

    if flag("--agent") {
        let (requests, _incoming) = std::sync::mpsc::channel();
        let (_answer, answers) = std::sync::mpsc::channel();
        let handle = bluez::serve_agent(requests, answers)?;
        println!("agent registered at {} as the default", bluez::AGENT_PATH);
        drop(handle);
        println!("agent unregistered");
    }

    if flag("--watch") {
        let (tx, rx) = std::sync::mpsc::channel();
        let to_network = tx.clone();
        let _network = nm::watch(move || {
            let _ = to_network.send(Change::Network);
        })?;
        let _bluetooth = bluez::watch(move || {
            let _ = tx.send(Change::Bluetooth);
        })?;
        println!("watching NetworkManager and BlueZ; Ctrl+C to stop");
        for change in rx {
            match change {
                Change::Network => {
                    let snapshot = network.snapshot()?;
                    println!(
                        "network change: wifi_enabled={} rows={}",
                        snapshot.wifi_enabled,
                        snapshot.networks.len()
                    );
                }
                Change::Bluetooth => {
                    let snapshot = bluetooth.snapshot()?;
                    println!(
                        "bluetooth change: powered={} discovering={} devices={}",
                        snapshot.powered,
                        snapshot.discovering,
                        snapshot.devices.len()
                    );
                }
            }
        }
    }
    Ok(())
}
