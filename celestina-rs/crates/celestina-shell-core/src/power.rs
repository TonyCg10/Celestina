//! Which power profile the daemon is on, and which one comes next.
//!
//! `power-profiles-daemon` owns the list and the current choice; the panel
//! neither invents a profile nor assumes a change took. Cycling is a request
//! like any other: ask for the next one, then read back what the daemon says.

/// The profiles `powerprofilesctl list` offers, in the order it lists them.
/// Its lines are `  performance:` and `* balanced:` for the active one.
#[must_use]
pub fn parse_profiles(listing: &str) -> Vec<String> {
    listing
        .lines()
        .filter_map(|line| {
            let name = line.trim().trim_start_matches('*').trim();
            let name = name.strip_suffix(':')?;
            if name.is_empty() || name.contains(char::is_whitespace) {
                return None;
            }
            Some(name.to_owned())
        })
        .collect()
}

/// The profile the daemon marks as active.
#[must_use]
pub fn parse_active(listing: &str) -> Option<String> {
    listing.lines().find_map(|line| {
        let marked = line.trim_start().strip_prefix('*')?;
        marked.trim().strip_suffix(':').map(str::to_owned)
    })
}

/// The next profile to ask for, wrapping. `None` when there is nothing to
/// cycle through, or when the current one is not among them — a daemon
/// reporting a profile it does not offer is not one to guess for.
#[must_use]
pub fn next_profile(current: &str, profiles: &[String]) -> Option<String> {
    if profiles.len() < 2 {
        return None;
    }
    let at = profiles.iter().position(|profile| profile == current)?;
    Some(profiles[(at + 1) % profiles.len()].clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = "  performance:\n    CpuDriver:\tamd_pstate\n\
                           * balanced:\n    CpuDriver:\tamd_pstate\n\
                             power-saver:\n";

    #[test]
    fn the_daemon_owns_the_list_and_the_choice() {
        assert_eq!(
            parse_profiles(LISTING),
            ["performance", "balanced", "power-saver"]
        );
        assert_eq!(parse_active(LISTING).as_deref(), Some("balanced"));
    }

    #[test]
    fn an_indented_detail_line_is_not_a_profile() {
        // `CpuDriver:\tamd_pstate` ends in no colon and carries whitespace;
        // neither is a profile name.
        assert!(!parse_profiles(LISTING)
            .iter()
            .any(|name| name == "CpuDriver"));
    }

    #[test]
    fn cycling_wraps_and_refuses_what_it_cannot_place() {
        let profiles = parse_profiles(LISTING);

        assert_eq!(
            next_profile("performance", &profiles).as_deref(),
            Some("balanced")
        );
        assert_eq!(
            next_profile("power-saver", &profiles).as_deref(),
            Some("performance")
        );
        // A profile the daemon does not offer is not a place to cycle from.
        assert_eq!(next_profile("turbo", &profiles), None);
        assert_eq!(next_profile("balanced", &profiles[..1]), None);
    }

    #[test]
    fn a_daemon_that_says_nothing_offers_nothing() {
        assert!(parse_profiles("").is_empty());
        assert_eq!(parse_active(""), None);
    }
}
