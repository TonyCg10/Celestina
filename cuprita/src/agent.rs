//! The pairing agent's bridge to the window: BlueZ's requests reach
//! `BluetoothController` on the Qt thread (which shows `PairingDialog`), and
//! the dialog's answers travel back to the agent, which replies to BlueZ.
//! Registering the agent is blocking IO, so it happens on a thread of its own;
//! that thread then carries the requests for the life of the process.

use std::pin::Pin;
use std::sync::mpsc::{self, Sender};

use cxx_qt::CxxQtThread;

use cuprita_core::agent::AgentAnswer;

use crate::backend;
use crate::controller::bluetooth::qobject::BluetoothController;

/// Starts the agent and returns where the controller sends the answers. When
/// no agent can be registered (the fakes, or BlueZ refused) the answers go
/// nowhere and nothing ever asks.
pub fn start(qt: CxxQtThread<BluetoothController>) -> Sender<AgentAnswer> {
    let (answer, answers) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("cuprita-agent".to_owned())
        .spawn(move || {
            let (requests, incoming) = mpsc::channel();
            let Some(_agent) = backend::agent(requests, answers) else {
                return;
            };
            // The agent holds the sending half, so this runs until exit.
            for request in incoming {
                let queued = qt.queue(move |controller: Pin<&mut BluetoothController>| {
                    controller.raise_agent_request(request);
                });
                // The controller is gone: the window closed.
                if queued.is_err() {
                    break;
                }
            }
        });
    if let Err(error) = spawned {
        eprintln!("Cuprita: the pairing agent's thread could not start: {error}");
    }
    answer
}
