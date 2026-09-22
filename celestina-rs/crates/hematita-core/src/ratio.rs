//! One integer percentage for the whole crate.
//!
//! Tick and kibibyte counts are exact integers; turning them into floats to
//! divide would only add rounding to a number a bar shows as a whole percent.

/// `part` of `whole` as a whole percent, saturating at 100 and answering 0 for
/// a whole of nothing.
#[must_use]
pub fn percent_of(part: u64, whole: u64) -> u8 {
    if whole == 0 {
        return 0;
    }
    // `part.min(whole) * 100` can overflow a u64 near its top, so divide in
    // u128 and the result is provably within 0..=100.
    let scaled = u128::from(part.min(whole)) * 100 / u128::from(whole);
    u8::try_from(scaled).unwrap_or(100)
}

#[cfg(test)]
mod tests {
    use super::percent_of;

    #[test]
    fn a_percentage_never_leaves_its_range() {
        assert_eq!(percent_of(5_000, 1_000), 100);
        assert_eq!(percent_of(0, 0), 0);
        assert_eq!(percent_of(1, 3), 33);
        assert_eq!(percent_of(2, 3), 66);
        assert_eq!(percent_of(u64::MAX, u64::MAX), 100);
    }
}
