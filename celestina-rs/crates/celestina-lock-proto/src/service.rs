//! The PAM services the verifier may be asked for, and where their files are
//! (design §4.2, §4.11).
//!
//! `--service` is a closed enum: no string from argv ever reaches `pam_start`.

use std::path::PathBuf;

use crate::Verdict;

/// The directories PAM reads service files from, in the order it tries them.
pub const PAM_DIRECTORIES: [&str; 2] = ["/etc/pam.d", "/usr/lib/pam.d"];

/// The two services, and no other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    /// `celestina-lock`, the service the author installs.
    Production,
    /// `celestina-lock-rehearsal`, which must not exist on the machine: the
    /// nest rehearsal asks for it so that PAM falls to `other` and denies,
    /// outside `faillock`.
    Rehearsal,
}

impl Service {
    /// The service a word names, if it is one of the two, exactly.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "celestina-lock" => Some(Self::Production),
            "celestina-lock-rehearsal" => Some(Self::Rehearsal),
            _ => None,
        }
    }

    /// The service name PAM is started with.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Production => "celestina-lock",
            Self::Rehearsal => "celestina-lock-rehearsal",
        }
    }

    /// Where the service's file would be, in the order PAM looks.
    #[must_use]
    pub fn paths(self) -> [PathBuf; 2] {
        PAM_DIRECTORIES.map(|directory| PathBuf::from(directory).join(self.name()))
    }
}

/// Whether the machine's PAM files allow `service` to run, given the paths
/// that exist among its candidates. The caller `stat`s [`Service::paths`] and
/// passes what it found; this decides, so the rule is tested without root or
/// `/etc`.
///
/// # Errors
///
/// [`Verdict::ServiceMissing`] when the production service has no file;
/// [`Verdict::BadRequest`] when the rehearsal service has one.
pub fn service_check(service: Service, existing: &[PathBuf]) -> Result<(), Verdict> {
    let present = service
        .paths()
        .iter()
        .any(|candidate| existing.contains(candidate));
    match (service, present) {
        (Service::Production, false) => Err(Verdict::ServiceMissing),
        (Service::Rehearsal, true) => Err(Verdict::BadRequest),
        (Service::Production, true) | (Service::Rehearsal, false) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn service_parse_is_a_closed_enum() {
        assert_eq!(Service::parse("celestina-lock"), Some(Service::Production));
        assert_eq!(
            Service::parse("celestina-lock-rehearsal"),
            Some(Service::Rehearsal)
        );
        for word in [
            "",
            "login",
            "other",
            "system-auth",
            "sudo",
            "Celestina-Lock",
            "celestina-lock ",
            " celestina-lock",
            "celestina-lock\n",
            "celestina-lock-rehearsal2",
            "celestina-lock/../login",
            "/etc/pam.d/celestina-lock",
            "production",
            "rehearsal",
        ] {
            assert_eq!(Service::parse(word), None, "{word:?}");
        }
        for service in [Service::Production, Service::Rehearsal] {
            assert_eq!(Service::parse(service.name()), Some(service));
        }
    }

    #[test]
    fn the_service_files_are_in_the_two_pam_directories() {
        assert_eq!(
            Service::Production.paths(),
            [
                PathBuf::from("/etc/pam.d/celestina-lock"),
                PathBuf::from("/usr/lib/pam.d/celestina-lock")
            ]
        );
        assert_eq!(
            Service::Rehearsal.paths(),
            [
                PathBuf::from("/etc/pam.d/celestina-lock-rehearsal"),
                PathBuf::from("/usr/lib/pam.d/celestina-lock-rehearsal")
            ]
        );
    }

    #[test]
    fn t_n1_service_check_over_given_path_lists() {
        // Production: the file must exist in either directory.
        assert_eq!(
            service_check(Service::Production, &[]),
            Err(Verdict::ServiceMissing)
        );
        assert_eq!(
            service_check(Service::Production, &paths(&["/etc/pam.d/sudo"])),
            Err(Verdict::ServiceMissing)
        );
        assert_eq!(
            service_check(
                Service::Production,
                &paths(&["/etc/pam.d/celestina-lock-rehearsal"])
            ),
            Err(Verdict::ServiceMissing),
            "the rehearsal file is not the production one"
        );
        assert_eq!(
            service_check(Service::Production, &paths(&["/etc/pam.d/celestina-lock"])),
            Ok(())
        );
        assert_eq!(
            service_check(
                Service::Production,
                &paths(&["/usr/lib/pam.d/celestina-lock"])
            ),
            Ok(())
        );
        // Rehearsal: the file must not exist anywhere.
        assert_eq!(service_check(Service::Rehearsal, &[]), Ok(()));
        assert_eq!(
            service_check(Service::Rehearsal, &paths(&["/etc/pam.d/celestina-lock"])),
            Ok(()),
            "the production file does not affect the rehearsal"
        );
        assert_eq!(
            service_check(
                Service::Rehearsal,
                &paths(&["/etc/pam.d/celestina-lock-rehearsal"])
            ),
            Err(Verdict::BadRequest)
        );
        assert_eq!(
            service_check(
                Service::Rehearsal,
                &paths(&["/usr/lib/pam.d/celestina-lock-rehearsal"])
            ),
            Err(Verdict::BadRequest)
        );
    }

    #[test]
    fn service_check_compares_whole_paths_not_suffixes() {
        assert_eq!(
            service_check(
                Service::Production,
                &paths(&["/tmp/etc/pam.d/celestina-lock", "etc/pam.d/celestina-lock"])
            ),
            Err(Verdict::ServiceMissing)
        );
    }
}
