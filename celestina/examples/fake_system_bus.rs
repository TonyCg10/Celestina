//! Serves fake UPower, power-profiles-daemon, NetworkManager and BlueZ on a
//! private `dbus-daemon`, for the nested smoke (CEL-2-F). It prints one line,
//! `DBUS_SYSTEM_BUS_ADDRESS=<address>`, and then reads one command per line
//! from its standard input until it closes, killing the daemon on the way out.
//! Hold the pipe open for as long as the shell runs, and set the variable for
//! the shell only. Commands (each answers one line, `calls` answers a count
//! line and then the calls): `battery <percentage> <state>`, `profile <name>`,
//! `wireless`, `serving`, `calls`, `clear`.
//!
//! Options: `--ppd upower|hadess|both|absent` (default `upower`) and
//! `--without upower|nm|bluez` (repeatable). Nothing here touches the real
//! system or session bus.

use std::io::Write;
use std::process::ExitCode;

use celestina_fake_system as fakes;

fn main() -> ExitCode {
    let mut services = fakes::Services::all();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = args.next();
        match (arg.as_str(), value.as_deref()) {
            ("--ppd", Some("upower")) => services.ppd = fakes::Ppd::Upower,
            ("--ppd", Some("hadess")) => services.ppd = fakes::Ppd::Hadess,
            ("--ppd", Some("both")) => services.ppd = fakes::Ppd::Both,
            ("--ppd", Some("absent")) => services.ppd = fakes::Ppd::Absent,
            ("--without", Some("upower")) => services.upower = false,
            ("--without", Some("nm")) => services.network_manager = false,
            ("--without", Some("bluez")) => services.bluez = false,
            _ => {
                eprintln!("usage: fake_system_bus [--ppd upower|hadess|both|absent] [--without upower|nm|bluez]...");
                return ExitCode::from(2);
            }
        }
    }
    let system = match fakes::FakeSystem::start("fake-system-bus", services) {
        Ok(system) => system,
        Err(error) => {
            eprintln!("fake_system_bus: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut out = std::io::stdout();
    if writeln!(out, "DBUS_SYSTEM_BUS_ADDRESS={}", system.address())
        .and_then(|()| out.flush())
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    // One command per line; end of input ends the run.
    for line in std::io::stdin().lines() {
        let Ok(line) = line else { break };
        let words: Vec<&str> = line.split_whitespace().collect();
        let reply = match words.as_slice() {
            ["battery", percentage, state] => match (percentage.parse(), state.parse()) {
                (Ok(percentage), Ok(state)) => system
                    .set_battery(percentage, state)
                    .map_or_else(|e| format!("error {e}"), |()| "ok".to_owned()),
                _ => "error usage: battery <percentage> <state>".to_owned(),
            },
            ["profile", name] => system
                .set_active_profile(name)
                .map_or_else(|e| format!("error {e}"), |()| "ok".to_owned()),
            ["wireless"] => system
                .wireless_enabled()
                .map_or_else(|e| format!("error {e}"), |on| format!("wireless {on}")),
            ["calls"] => {
                let calls = system.calls().all();
                format!("calls {}\n{}", calls.len(), calls.join("\n"))
            }
            ["serving"] => format!("serving {}", system.serving().join(" ")),
            ["clear"] => {
                system.calls().clear();
                "ok".to_owned()
            }
            _ => "error unknown command".to_owned(),
        };
        if writeln!(out, "{reply}").and_then(|()| out.flush()).is_err() {
            break;
        }
    }
    ExitCode::SUCCESS
}
