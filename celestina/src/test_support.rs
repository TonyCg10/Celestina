//! Fixtures the crate's unit tests share.
//!
//! Since the cutover (CEL-2-M) `Settings::default()` means "every shipped
//! duty on", the session lock and the polkit agent included since
//! CEL-2-M-LOCK-POLKIT, so a test that wants nothing to start says so
//! explicitly through these instead of leaning on the defaults.

use celestina_shell::settings::{Duties, Duty, Settings};

/// The duties with every switch off ([`Duties::all_off`]): the state a test
/// builds when it wants to start exactly the duties it names and nothing
/// else.
pub(crate) fn quiet_duties() -> Duties {
    Duties::all_off()
}

/// The defaults with [`quiet_duties`]: a settings value that starts no
/// worker.
pub(crate) fn quiet_settings() -> Settings {
    Settings {
        duties: quiet_duties(),
        ..Settings::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_settings_differ_from_the_defaults_only_in_the_duties() {
        let quiet = quiet_settings();
        assert_eq!(quiet.duties, quiet_duties());
        assert_eq!(quiet.duties, Duties::all_off());
        assert_eq!(quiet.duties.clipboard, Duty::Off);
        // The two privileged duties default to on; the quiet set starts
        // neither.
        assert_eq!(quiet.duties.lock, Duty::Off);
        assert_eq!(quiet.duties.polkit, Duty::Off);
        assert_ne!(quiet.duties, Settings::default().duties);
        let rest = Settings {
            duties: Settings::default().duties,
            ..quiet
        };
        assert_eq!(rest, Settings::default());
    }
}
