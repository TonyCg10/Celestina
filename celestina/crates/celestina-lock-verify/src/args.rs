//! The verifier's argv (design §4.3 (c)): exactly
//! `--protocol 1 --service <celestina-lock|celestina-lock-rehearsal>`.
//!
//! The service is a closed enum of `celestina-lock-proto`; no string from argv
//! ever reaches `pam_start`. The user is not an argument either: it is the name
//! of the real uid.
#![forbid(unsafe_code)]

use std::ffi::OsString;

use celestina_lock_proto::{Service, Verdict, PROTOCOL_VERSION};

/// The service the request names.
///
/// `args` is argv without the program name.
///
/// # Errors
///
/// [`Verdict::BadRequest`] for anything but the four exact words, in order: an
/// unknown protocol, an unknown service, a missing or extra argument, a
/// non-UTF-8 word.
pub fn parse<I>(args: I) -> Result<Service, Verdict>
where
    I: IntoIterator<Item = OsString>,
{
    let mut words = args.into_iter();
    let mut next = || words.next().and_then(|word| word.into_string().ok());
    let (flag, version, service_flag, service) = (next(), next(), next(), next());
    if words.next().is_some() {
        return Err(Verdict::BadRequest);
    }
    match (flag.as_deref(), version, service_flag.as_deref(), service) {
        (Some("--protocol"), Some(version), Some("--service"), Some(service))
            if version == PROTOCOL_VERSION.to_string() =>
        {
            Service::parse(&service).ok_or(Verdict::BadRequest)
        }
        _ => Err(Verdict::BadRequest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

    fn parse_words(words: &[&str]) -> Result<Service, Verdict> {
        parse(words.iter().map(OsString::from))
    }

    #[test]
    fn the_two_services_are_accepted() {
        assert_eq!(
            parse_words(&["--protocol", "1", "--service", "celestina-lock"]),
            Ok(Service::Production)
        );
        assert_eq!(
            parse_words(&["--protocol", "1", "--service", "celestina-lock-rehearsal"]),
            Ok(Service::Rehearsal)
        );
    }

    #[test]
    fn unknown_protocol_and_unknown_service_are_bad_request() {
        for words in [
            &["--protocol", "2", "--service", "celestina-lock"][..],
            &["--protocol", "0", "--service", "celestina-lock"],
            &["--protocol", "01", "--service", "celestina-lock"],
            &["--protocol", "+1", "--service", "celestina-lock"],
            &["--protocol", "", "--service", "celestina-lock"],
            &["--protocol", "1", "--service", "login"],
            &["--protocol", "1", "--service", "other"],
            &["--protocol", "1", "--service", "system-auth"],
            &["--protocol", "1", "--service", ""],
            &["--protocol", "1", "--service", "celestina-lock "],
            &["--protocol", "1", "--service", "/etc/pam.d/celestina-lock"],
            &["--protocol", "1", "--service", "../login"],
        ] {
            assert_eq!(parse_words(words), Err(Verdict::BadRequest), "{words:?}");
        }
    }

    #[test]
    fn missing_extra_reordered_and_alien_arguments_are_bad_request() {
        for words in [
            &[][..],
            &["--protocol"],
            &["--protocol", "1"],
            &["--protocol", "1", "--service"],
            &["--service", "celestina-lock", "--protocol", "1"],
            &["--protocol", "1", "--service", "celestina-lock", "extra"],
            &[
                "--protocol",
                "1",
                "--service",
                "celestina-lock",
                "--user",
                "root",
            ],
            &["--protocol=1", "--service=celestina-lock"],
            &["-protocol", "1", "-service", "celestina-lock"],
            &[
                "--protocol",
                "1",
                "--user",
                "alice",
                "--service",
                "celestina-lock",
            ],
        ] {
            assert_eq!(parse_words(words), Err(Verdict::BadRequest), "{words:?}");
        }
    }

    #[test]
    fn a_word_that_is_not_utf8_is_bad_request() {
        let args = [
            OsString::from("--protocol"),
            OsString::from("1"),
            OsString::from("--service"),
            OsString::from_vec(b"celestina-lock\xff".to_vec()),
        ];
        assert_eq!(parse(args), Err(Verdict::BadRequest));
        let args = [
            OsString::from_vec(vec![0xff]),
            OsString::from("1"),
            OsString::from("--service"),
            OsString::from("celestina-lock"),
        ];
        assert_eq!(parse(args), Err(Verdict::BadRequest));
    }
}
