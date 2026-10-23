//! The `TracerPid` field of `/proc/<pid>/status` (design §4.2 "Not traced at
//! start", TM-12).
//!
//! The lock and the verifier each set the process non-dumpable and then read
//! this field: a non-zero tracer attached in the instant before the call is
//! not detached by `PR_SET_DUMPABLE 0`, so the process ends before any secret
//! exists. The syscalls stay in each binary (they map a failure to a different
//! exit code and this crate has no dependency to make them with); the parse is
//! the same in both and lives here once.

/// The value of the `TracerPid:` line, if it holds exactly a decimal number.
///
/// Only the line whose name is `TracerPid` counts, never a value inside
/// another field. Anything else (no such line, no number, a sign, a hex
/// prefix) is `None`: not knowing is not a pass, and the caller refuses to
/// run.
#[must_use]
pub fn tracer_pid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| line.strip_prefix("TracerPid:"))
        .and_then(|value| value.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNTRACED: &str =
        "Name:\tcelestina-lock\nState:\tR (running)\nTgid:\t4242\nPid:\t4242\nPPid:\t1\nTracerPid:\t0\nUid:\t1000\t1000\t1000\t1000\n";

    #[test]
    fn tracer_pid_reads_the_named_line() {
        assert_eq!(tracer_pid(UNTRACED), Some(0));
        assert_eq!(
            tracer_pid(&UNTRACED.replace("TracerPid:\t0", "TracerPid:\t31337")),
            Some(31337)
        );
    }

    #[test]
    fn a_missing_or_garbled_line_is_not_a_number() {
        for status in [
            "",
            "Name:\tx\n",
            "TracerPid:\n",
            "TracerPid:\tabc\n",
            "TracerPid:\t-1\n",
            "TracerPid:\t0x0\n",
            "TracerPid:\t99999999999999999999\n",
        ] {
            assert_eq!(tracer_pid(status), None, "{status:?}");
        }
        // A value inside another field is not the field.
        assert_eq!(tracer_pid("Name:\tTracerPid:\t0\nPPid:\t0\n"), None);
    }
}
