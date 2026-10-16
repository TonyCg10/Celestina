//! The command surface: the verbs `celestina <verb>` forwards over
//! `org.celestina.Shell1`, and the binary's command line (design §5.5,
//! "Opens" and "Closes").
//!
//! The same words travel on the bus as an array of strings, so
//! [`Verb::parse`] is also what the served object runs on input from any
//! peer: it is bounded and never echoes more than a few dozen characters
//! back.

use std::time::Duration;

use celestina_wayland::Config;

/// Arguments the binary reads at most; a longer command line is a usage
/// error before anything is parsed.
pub const MAX_ARGS: usize = 16;

/// The longest verb, `island open <page>`.
const MAX_VERB_WORDS: usize = 3;

/// How much of a rejected command a usage error repeats.
const MAX_ECHO_CHARS: usize = 64;

/// What `celestina --help` prints and a usage error follows with.
pub const USAGE: &str = "\
usage: celestina [--exit-after SECONDS] [--metrics] [--no-blur]
       celestina --check
       celestina VERB

verbs: island toggle
       island open activity|notifications|tray
       island close
       launcher toggle
       notifications toggle
       notifications dismiss-all
       quit";

/// An Island page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Activity,
    Notifications,
    Tray,
}

impl Page {
    pub const ALL: [Page; 3] = [Page::Activity, Page::Notifications, Page::Tray];

    /// The page's word in a verb.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Page::Activity => "activity",
            Page::Notifications => "notifications",
            Page::Tray => "tray",
        }
    }

    fn from_word(word: &str) -> Option<Page> {
        Page::ALL.into_iter().find(|p| p.word() == word)
    }
}

/// One command of `org.celestina.Shell1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    IslandToggle,
    IslandOpen(Page),
    IslandClose,
    LauncherToggle,
    NotificationsToggle,
    NotificationsDismissAll,
    Quit,
}

/// A command line or a verb that is not one of the shell's.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct UsageError(String);

impl Verb {
    /// The verb `args` spell, word for word.
    ///
    /// # Errors
    ///
    /// [`UsageError`] for anything that is not exactly one verb.
    pub fn parse(args: &[String]) -> Result<Verb, UsageError> {
        if args.len() > MAX_VERB_WORDS {
            return Err(UsageError(format!(
                "a verb has at most {MAX_VERB_WORDS} words, not {}",
                args.len()
            )));
        }
        let words: Vec<&str> = args.iter().map(String::as_str).collect();
        let verb = match words.as_slice() {
            ["island", "toggle"] => Verb::IslandToggle,
            ["island", "open", page] => match Page::from_word(page) {
                Some(page) => Verb::IslandOpen(page),
                None => return Err(unknown(&words)),
            },
            ["island", "close"] => Verb::IslandClose,
            ["launcher", "toggle"] => Verb::LauncherToggle,
            ["notifications", "toggle"] => Verb::NotificationsToggle,
            ["notifications", "dismiss-all"] => Verb::NotificationsDismissAll,
            ["quit"] => Verb::Quit,
            _ => return Err(unknown(&words)),
        };
        Ok(verb)
    }

    /// The words that spell the verb; [`Verb::parse`] reads them back.
    #[must_use]
    pub fn words(&self) -> Vec<&'static str> {
        match self {
            Verb::IslandToggle => vec!["island", "toggle"],
            Verb::IslandOpen(page) => vec!["island", "open", page.word()],
            Verb::IslandClose => vec!["island", "close"],
            Verb::LauncherToggle => vec!["launcher", "toggle"],
            Verb::NotificationsToggle => vec!["notifications", "toggle"],
            Verb::NotificationsDismissAll => vec!["notifications", "dismiss-all"],
            Verb::Quit => vec!["quit"],
        }
    }
}

/// `unknown verb "<words>"`, the words cut to [`MAX_ECHO_CHARS`].
fn unknown(words: &[&str]) -> UsageError {
    let joined = words.join(" ");
    let mut shown: String = joined.chars().take(MAX_ECHO_CHARS).collect();
    if shown.len() < joined.len() {
        shown.push('…');
    }
    UsageError(format!("unknown verb {shown:?}"))
}

/// What one run of the binary is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The shell itself.
    Shell(Config),
    /// Forward a verb to the running shell and exit.
    Client(Verb),
    /// Load the theme, the fonts and the appearance file, then exit.
    Check,
    /// Print [`USAGE`].
    Usage,
}

/// Reads the command line, without the program name.
///
/// # Errors
///
/// [`UsageError`] for an unknown flag or verb, a flag without its value, or
/// more than [`MAX_ARGS`] arguments.
pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Mode, UsageError> {
    let args: Vec<String> = args.into_iter().take(MAX_ARGS + 1).collect();
    if args.len() > MAX_ARGS {
        return Err(UsageError(format!("more than {MAX_ARGS} arguments")));
    }
    match args.first().map(String::as_str) {
        None => return Ok(Mode::Shell(Config::default())),
        Some(first) if !first.starts_with('-') => return Verb::parse(&args).map(Mode::Client),
        _ => {}
    }
    if let [only] = args.as_slice() {
        match only.as_str() {
            "--check" => return Ok(Mode::Check),
            "--help" | "-h" => return Ok(Mode::Usage),
            _ => {}
        }
    }
    let mut cfg = Config::default();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--metrics" => cfg.metrics = true,
            "--no-blur" => cfg.no_blur = true,
            "--exit-after" => {
                let value = rest
                    .next()
                    .ok_or_else(|| UsageError("--exit-after needs SECONDS".to_owned()))?;
                cfg.exit_after = Some(seconds(value)?);
            }
            other => {
                let shown: String = other.chars().take(MAX_ECHO_CHARS).collect();
                return Err(UsageError(format!("unknown argument {shown:?}")));
            }
        }
    }
    Ok(Mode::Shell(cfg))
}

/// A finite, non-negative number of seconds.
fn seconds(value: &str) -> Result<Duration, UsageError> {
    let invalid = || {
        let shown: String = value.chars().take(MAX_ECHO_CHARS).collect();
        UsageError(format!("invalid --exit-after value {shown:?}"))
    };
    let secs: f64 = value.parse().map_err(|_| invalid())?;
    if !secs.is_finite() || secs < 0.0 || secs > f64::from(u32::MAX) {
        return Err(invalid());
    }
    Ok(Duration::from_secs_f64(secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_owned()).collect()
    }

    fn parse(words: &[&str]) -> Result<Mode, UsageError> {
        parse_args(strings(words))
    }

    #[test]
    fn every_verb_round_trips_through_words_and_parse() {
        let table = [
            (Verb::IslandToggle, "island toggle"),
            (Verb::IslandOpen(Page::Activity), "island open activity"),
            (
                Verb::IslandOpen(Page::Notifications),
                "island open notifications",
            ),
            (Verb::IslandOpen(Page::Tray), "island open tray"),
            (Verb::IslandClose, "island close"),
            (Verb::LauncherToggle, "launcher toggle"),
            (Verb::NotificationsToggle, "notifications toggle"),
            (Verb::NotificationsDismissAll, "notifications dismiss-all"),
            (Verb::Quit, "quit"),
        ];
        for (verb, spelled) in table {
            assert_eq!(verb.words().join(" "), spelled);
            let words: Vec<String> = verb.words().into_iter().map(str::to_owned).collect();
            assert_eq!(Verb::parse(&words), Ok(verb), "{spelled}");
            let words: Vec<&str> = spelled.split(' ').collect();
            assert_eq!(parse(&words), Ok(Mode::Client(verb)), "{spelled}");
        }
    }

    #[test]
    fn unknown_verbs_are_usage_errors() {
        for bad in [
            &["island", "fly"][..],
            &["island", "open", "menu"],
            &["island", "open"],
            &["island"],
            &["Island", "toggle"],
            &["quit", "now"],
        ] {
            assert!(Verb::parse(&strings(bad)).is_err(), "{bad:?}");
            assert!(parse(bad).is_err(), "{bad:?}");
        }
        assert_eq!(
            Verb::parse(&strings(&["island", "fly"])),
            Err(UsageError("unknown verb \"island fly\"".to_owned()))
        );
        // `celestina` followed by 200 arguments: refused by count, and the
        // message does not repeat them.
        let many: Vec<String> = std::iter::repeat_n("island".to_owned(), 200).collect();
        let err = parse_args(many.clone()).expect_err("too many arguments");
        assert!(err.to_string().len() < 100, "{err}");
        let err = Verb::parse(&many).expect_err("too many words");
        assert!(err.to_string().len() < 100, "{err}");
        // A long word is cut in the message.
        let long = "x".repeat(10_000);
        let err = Verb::parse(&[long]).expect_err("unknown");
        assert!(err.to_string().len() < 100, "{err}");
    }

    #[test]
    fn shell_mode_flags() {
        assert_eq!(parse(&[]), Ok(Mode::Shell(Config::default())));
        assert_eq!(
            parse(&["--exit-after", "3"]),
            Ok(Mode::Shell(Config {
                exit_after: Some(Duration::from_secs(3)),
                ..Config::default()
            }))
        );
        assert_eq!(
            parse(&["--metrics"]),
            Ok(Mode::Shell(Config {
                metrics: true,
                ..Config::default()
            }))
        );
        assert_eq!(
            parse(&["--no-blur"]),
            Ok(Mode::Shell(Config {
                no_blur: true,
                ..Config::default()
            }))
        );
        assert_eq!(
            parse(&["--exit-after", "2.5", "--metrics", "--no-blur"]),
            Ok(Mode::Shell(Config {
                exit_after: Some(Duration::from_millis(2500)),
                no_blur: true,
                metrics: true,
            }))
        );
        assert_eq!(parse(&["--check"]), Ok(Mode::Check));
        assert_eq!(parse(&["--help"]), Ok(Mode::Usage));
        assert_eq!(parse(&["-h"]), Ok(Mode::Usage));
        for bad in [
            &["--exit-after"][..],
            &["--exit-after", "soon"],
            &["--exit-after", "-1"],
            &["--exit-after", "inf"],
            &["--exit-after", "NaN"],
            &["--bogus"],
            &["--check", "--metrics"],
            &["--metrics", "island", "toggle"],
        ] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }
}
