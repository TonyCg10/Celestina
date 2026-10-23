//! Which descriptors a process was handed by accident (round 4, finding I-α).
//!
//! The shell receives the logind delay inhibitor from the system bus as a file
//! descriptor, and the library that receives it does not ask the kernel for
//! `MSG_CMSG_CLOEXEC`. For as long as that message lives, the shell holds an
//! inheritable copy of the descriptor, and any child the shell starts in that
//! time (the lock, its verifier, a launched application) inherits it: the
//! inhibitor then lasts as long as the child. For the lock that means a
//! machine that waits `InhibitDelayMaxSec` before every suspend while the
//! session is locked.
//!
//! The lock and the verifier therefore close every descriptor they were not
//! given on purpose, first thing they do. The syscalls (listing
//! `/proc/self/fd`, closing) stay in each binary, as for the tracer check; the
//! decision of *which* numbers are accidents is the same in both and lives here
//! once.

/// The lowest descriptor number that can be an accident: 0, 1 and 2 are the
/// standard streams, which the parent always chooses.
pub const FIRST_INHERITED: i32 = 3;

/// The numbers among `names` (the entries of `/proc/self/fd`) that are
/// inherited by accident: decimal, at least [`FIRST_INHERITED`] and not in
/// `keep`, the descriptors the process was handed on purpose (the Wayland
/// socket). A name that is not a plain decimal number is ignored, never an
/// error. The result is sorted and has no duplicates.
#[must_use]
pub fn inherited_descriptors<'a>(
    names: impl IntoIterator<Item = &'a str>,
    keep: &[i32],
) -> Vec<i32> {
    let mut found: Vec<i32> = names
        .into_iter()
        .filter(|name| !name.is_empty() && name.bytes().all(|b| b.is_ascii_digit()))
        .filter_map(|name| name.parse::<i32>().ok())
        .filter(|number| *number >= FIRST_INHERITED && !keep.contains(number))
        .collect();
    found.sort_unstable();
    found.dedup();
    found
}

/// The descriptor `WAYLAND_SOCKET` names, if the value is a plain decimal
/// number of at least [`FIRST_INHERITED`]: ASCII digits only, so no sign
/// (round 7: the toolkit's `str::parse::<i32>` also reads `+5`, which this
/// function refuses, failing closed), no whitespace and no prefix. That
/// socket is the one descriptor the lock is handed on purpose besides its
/// standard streams, and this is the single owner of the rule: the connection
/// guard of `celestina-wayland` and the lock's launcher both call it.
#[must_use]
pub fn wayland_fd(value: Option<&str>) -> Option<i32> {
    let text = value?;
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let number = text.parse::<i32>().ok()?;
    (number >= FIRST_INHERITED).then_some(number)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_streams_and_the_kept_descriptors_stay() {
        let names = ["0", "1", "2", "3", "7", "12", "57"];
        assert_eq!(inherited_descriptors(names, &[]), [3, 7, 12, 57]);
        assert_eq!(inherited_descriptors(names, &[7]), [3, 12, 57]);
        assert_eq!(
            inherited_descriptors(names, &[3, 7, 12, 57]),
            Vec::<i32>::new()
        );
    }

    #[test]
    fn what_is_not_a_plain_number_is_ignored() {
        let names = ["", "-4", "+5", "0x10", "9a", " 6", "99999999999", "8", "8"];
        assert_eq!(inherited_descriptors(names, &[]), [8]);
    }

    #[test]
    fn the_wayland_fd_is_a_plain_number_above_the_standard_streams() {
        assert_eq!(wayland_fd(Some("5")), Some(5));
        assert_eq!(wayland_fd(Some("007")), Some(7));
        for bad in [
            None,
            // Round 5, m-3: the value is taken exactly as the toolkit takes
            // it, so surrounding whitespace is not a descriptor number.
            Some(" 5\n"),
            Some("5\n"),
            Some(" 5"),
            Some(""),
            Some("0"),
            Some("2"),
            Some("-1"),
            Some("x"),
            Some("5 6"),
            // Round 7: no sign, although `str::parse::<i32>` reads `+5`.
            Some("+5"),
            Some("+0005x"),
            Some("99999999999"),
        ] {
            assert_eq!(wayland_fd(bad), None, "{bad:?}");
        }
    }
}
