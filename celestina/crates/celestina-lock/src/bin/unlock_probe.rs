//! `unlock_probe`: the rehearsal-only twin of `celestina-lock` (CEL-2-H design
//! §4.13, R-8). Built only with the `fake-verifier` feature, run only in the
//! nest by `lock-rehearsal.sh`, never shipped.
//!
//! It is the same core with a "verifier" that always answers yes
//! (`/usr/bin/true`) and a password it types by itself 300 ms after `locked`,
//! so the nest can see the whole path: `locked`, the attempt, `unlocked`,
//! exit 0 and niri's "unlocking session" line. It is an unlock without a
//! password by construction: that is why it does not exist without the
//! feature, why `verify-production.sh` checks the shipped binary holds no probe
//! symbol, and why it must never be pointed at a session that matters.
#![forbid(unsafe_code)]

use std::process::ExitCode;
use std::time::Duration;

use celestina_lock::launch::{self, Launch, VerifierChoice};
use celestina_wayland::lock::ConnectPolicy;

fn main() -> ExitCode {
    launch::start(Launch {
        rehearsal: false,
        // An unlock without a password: only ever in a verified nest.
        connect: ConnectPolicy::detect(true),
        verifier: VerifierChoice::Fake {
            path: "/usr/bin/true".into(),
            args: Vec::new(),
            auto_type_after: Duration::from_millis(300),
        },
    })
}
