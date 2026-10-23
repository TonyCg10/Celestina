//! `celestina-lock`: the session lock (CEL-2-H). See the library for the
//! design; this file only reads argv and hands over.
//!
//! argv is exactly nothing, or `--rehearsal`. Anything else is a usage error
//! (exit 2). There is no option to name a verifier, a service or a log level:
//! the verifier is the binary beside this one, pinned by descriptor, and the
//! service is compiled in.
#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::process::ExitCode;

use celestina_lock::launch::{self, Launch, VerifierChoice};
use celestina_lock_proto::LockExit;
use celestina_wayland::lock::ConnectPolicy;

/// What argv asks for, if it is something this program knows. An argument
/// that is not UTF-8 is a usage error like any other (it never panics).
fn parse(args: &[OsString]) -> Option<bool> {
    match args {
        [] => Some(false),
        [flag] if flag == "--rehearsal" => Some(true),
        _ => None,
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let Some(rehearsal) = parse(&args) else {
        eprintln!("usage: celestina-lock [--rehearsal]");
        return ExitCode::from(u8::try_from(LockExit::Usage.code()).unwrap_or(2));
    };
    launch::start(Launch {
        rehearsal,
        verifier: VerifierChoice::BesideSelf,
        // `--rehearsal` needs a verified nest, marker or not.
        connect: ConnectPolicy::detect(rehearsal),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

    fn words(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn an_argument_that_is_not_utf8_is_a_usage_error_not_a_panic() {
        let bad = vec![OsString::from_vec(vec![0x2d, 0xff, 0xfe])];
        assert_eq!(parse(&bad), None);
    }

    #[test]
    fn argv_is_nothing_or_rehearsal_and_nothing_else() {
        assert_eq!(parse(&words(&[])), Some(false));
        assert_eq!(parse(&words(&["--rehearsal"])), Some(true));
        for bad in [
            &["--rehearsal", "--rehearsal"][..],
            &["--service", "login"],
            &["--verifier", "/bin/true"],
            &["rehearsal"],
            &["-h"],
            &[""],
        ] {
            assert_eq!(parse(&words(bad)), None, "{bad:?}");
        }
    }
}
