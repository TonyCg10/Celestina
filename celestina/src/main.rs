//! The Celestina shell. CEL-1-A registers the project; the shell itself
//! arrives in CEL-1-B. Until then the binary answers `--check` so the
//! production entries have something to verify.
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--check"] {
        return ExitCode::SUCCESS;
    }
    eprintln!("celestina: the shell is not delivered yet (CEL-1-B); only --check is accepted");
    ExitCode::from(2)
}
