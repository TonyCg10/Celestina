use std::cmp::Ordering;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;

/// Orders two entry names the way a person reads a listing: case-insensitively,
/// and with runs of digits compared as numbers — so `apple` comes before
/// `Zebra` and `file2` before `file10`, instead of the byte order that puts
/// every capital first and `file10` before `file2`.
///
/// Deliberately ASCII-only in what it folds: real Unicode collation would mean
/// carrying an ICU-sized table, which the dependency allowlist does not admit.
/// Bytes outside ASCII are compared as they are, so accented and non-UTF-8
/// names still order deterministically (and never panic) — they just sort after
/// the ASCII letters, exactly as they do today.
///
/// The comparison is total: names that differ only by case or by leading zeros
/// fall back to their raw bytes, so the order never depends on the input order.
#[must_use]
pub fn compare_names(left: &OsStr, right: &OsStr) -> Ordering {
    let (left, right) = (left.as_bytes(), right.as_bytes());
    let (mut i, mut j) = (0usize, 0usize);

    while i < left.len() && j < right.len() {
        let (a, b) = (left[i], right[j]);

        if a.is_ascii_digit() && b.is_ascii_digit() {
            let a_end = digits_end(left, i);
            let b_end = digits_end(right, j);
            // Compare the runs as numbers without parsing them: once the
            // leading zeros are gone, the longer run is the larger number, and
            // equal-length runs compare digit by digit. No overflow, however
            // many digits a name carries.
            let a_digits = trim_zeros(&left[i..a_end]);
            let b_digits = trim_zeros(&right[j..b_end]);
            let numeric = a_digits
                .len()
                .cmp(&b_digits.len())
                .then_with(|| a_digits.cmp(b_digits));
            if numeric != Ordering::Equal {
                return numeric;
            }
            i = a_end;
            j = b_end;
            continue;
        }

        let folded = a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase());
        if folded != Ordering::Equal {
            return folded;
        }
        i += 1;
        j += 1;
    }

    // Whatever is left: the shorter name comes first, and the raw bytes settle
    // anything the folded comparison called equal.
    (left.len() - i)
        .cmp(&(right.len() - j))
        .then_with(|| left.cmp(right))
}

fn digits_end(bytes: &[u8], from: usize) -> usize {
    let mut end = from;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    end
}

/// The run without its leading zeros — an all-zero run keeps one digit, so
/// `000` still compares as a number rather than as nothing.
fn trim_zeros(digits: &[u8]) -> &[u8] {
    let first_significant = digits
        .iter()
        .position(|byte| *byte != b'0')
        .unwrap_or(digits.len() - 1);
    &digits[first_significant..]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    fn sorted(names: &[&str]) -> Vec<String> {
        let mut names: Vec<OsString> = names.iter().map(OsString::from).collect();
        names.sort_by(|left, right| compare_names(left, right));
        names
            .iter()
            .map(|name| name.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn digit_runs_compare_as_numbers() {
        assert_eq!(
            sorted(&["file10", "file2", "file1", "file20"]),
            ["file1", "file2", "file10", "file20"]
        );
        // The number keeps its place in the middle of a name, too.
        assert_eq!(
            sorted(&["v10 final", "v2 final", "v2 draft"]),
            ["v2 draft", "v2 final", "v10 final"]
        );
    }

    #[test]
    fn case_does_not_split_the_alphabet() {
        assert_eq!(
            sorted(&["Zebra", "apple", "Banana"]),
            ["apple", "Banana", "Zebra"]
        );
    }

    #[test]
    fn long_numbers_do_not_overflow() {
        let big = "f99999999999999999999999999999";
        let bigger = "f99999999999999999999999999999999";
        assert_eq!(
            compare_names(OsStr::new(big), OsStr::new(bigger)),
            Ordering::Less
        );
    }

    #[test]
    fn leading_zeros_tie_break_but_never_reorder_values() {
        assert_eq!(sorted(&["f7", "f007", "f8"]), ["f007", "f7", "f8"]);
        assert_eq!(
            compare_names(OsStr::new("f0"), OsStr::new("f000")),
            Ordering::Less
        );
    }

    #[test]
    fn the_order_is_total_and_case_only_names_stay_distinct() {
        assert_eq!(
            compare_names(OsStr::new("README"), OsStr::new("readme")),
            Ordering::Less
        );
        assert_eq!(
            compare_names(OsStr::new("readme"), OsStr::new("README")),
            Ordering::Greater
        );
        assert_eq!(
            compare_names(OsStr::new("readme"), OsStr::new("readme")),
            Ordering::Equal
        );
    }

    #[test]
    fn a_prefix_comes_first() {
        assert_eq!(sorted(&["notes.txt", "notes"]), ["notes", "notes.txt"]);
    }

    #[test]
    fn invalid_utf8_names_order_without_panicking() {
        let weird = OsString::from_vec(vec![b'a', 0xFF, b'z']);
        let plain = OsString::from_vec(vec![b'a', b'b']);
        assert_eq!(compare_names(&plain, &weird), Ordering::Less);
        assert_eq!(compare_names(&weird, &weird.clone()), Ordering::Equal);
    }
}
