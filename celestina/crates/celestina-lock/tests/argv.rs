//! The real binary answers a bad argv with a usage error, never a panic
//! (finding M3): `std::env::args()` panics on a non-UTF-8 argument.

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::process::{Command, Stdio};

#[test]
fn a_non_utf8_argument_exits_2_without_panicking() {
    let output = Command::new(env!("CARGO_BIN_EXE_celestina-lock"))
        .arg(OsString::from_vec(vec![0x2d, 0xff, 0xfe]))
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .expect("run the lock");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
}
