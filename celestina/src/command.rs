//! The command surface: the verbs `celestina <verb>` forwards over
//! `org.celestina.Shell1`, and the binary's command line (design §5.5,
//! "Opens" and "Closes").
//!
//! The same words travel on the bus as an array of strings, so
//! [`Verb::parse`] is also what the served object runs on input from any
//! peer: it is bounded and never echoes more than a few dozen characters
//! back.

use std::time::Duration;

use celestina_shell::settings::plain_name;
#[cfg(test)]
use celestina_shell::settings::MAX_NAME_BYTES;
use celestina_shell::wallpaper::{is_showable, Mode as FitMode};
use celestina_wayland::Config;

/// Arguments the binary reads at most; a longer command line is a usage
/// error before anything is parsed.
pub const MAX_ARGS: usize = 16;

/// The longest verb, `wallpaper set <output> <file>`.
const MAX_VERB_WORDS: usize = 4;

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
       wallpaper set OUTPUT|all FILE
       wallpaper next [OUTPUT]
       wallpaper prev [OUTPUT]
       wallpaper mode fill|fit|stretch|center
       wallpaper retry
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

/// What a wallpaper verb addresses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// Every output.
    All,
    /// The output of this name (bounded text, not interpreted here).
    Output(String),
}

/// One command of `org.celestina.Shell1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verb {
    IslandToggle,
    IslandOpen(Page),
    IslandClose,
    LauncherToggle,
    NotificationsToggle,
    NotificationsDismissAll,
    /// `wallpaper set <output|all> <file>`: `file` is a bare name in the
    /// wallpaper folder.
    WallpaperSet {
        target: Target,
        file: String,
    },
    /// `wallpaper next [output]`.
    WallpaperNext(Option<String>),
    /// `wallpaper prev [output]`.
    WallpaperPrev(Option<String>),
    /// `wallpaper mode <fill|fit|stretch|center>`.
    WallpaperMode(FitMode),
    /// `wallpaper retry`.
    WallpaperRetry,
    Quit,
}

/// The word of a fit mode in a verb.
fn mode_word(mode: FitMode) -> &'static str {
    match mode {
        FitMode::Fill => "fill",
        FitMode::Fit => "fit",
        FitMode::Stretch => "stretch",
        FitMode::Center => "center",
    }
}

fn mode_from_word(word: &str) -> Option<FitMode> {
    [
        FitMode::Fill,
        FitMode::Fit,
        FitMode::Stretch,
        FitMode::Center,
    ]
    .into_iter()
    .find(|m| mode_word(*m) == word)
}

/// An output name (or `all`): the settings' own plain-name predicate, so a
/// verb is never accepted that the settings clamp would then drop.
fn output_name(name: &str) -> bool {
    plain_name(name)
}

/// A wallpaper file name: a plain name this shell can show, exactly what the
/// settings clamp keeps.
fn file_name(name: &str) -> bool {
    plain_name(name) && is_showable(name)
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
            ["wallpaper", "set", target, file] if output_name(target) && file_name(file) => {
                Verb::WallpaperSet {
                    target: if *target == "all" {
                        Target::All
                    } else {
                        Target::Output((*target).to_owned())
                    },
                    file: (*file).to_owned(),
                }
            }
            ["wallpaper", "next"] => Verb::WallpaperNext(None),
            ["wallpaper", "next", out] if output_name(out) => {
                Verb::WallpaperNext(Some((*out).to_owned()))
            }
            ["wallpaper", "prev"] => Verb::WallpaperPrev(None),
            ["wallpaper", "prev", out] if output_name(out) => {
                Verb::WallpaperPrev(Some((*out).to_owned()))
            }
            ["wallpaper", "mode", word] => match mode_from_word(word) {
                Some(mode) => Verb::WallpaperMode(mode),
                None => return Err(unknown(&words)),
            },
            ["wallpaper", "retry"] => Verb::WallpaperRetry,
            ["quit"] => Verb::Quit,
            _ => return Err(unknown(&words)),
        };
        Ok(verb)
    }

    /// The words that spell the verb; [`Verb::parse`] reads them back.
    #[must_use]
    pub fn words(&self) -> Vec<String> {
        let own = |words: Vec<&str>| words.into_iter().map(str::to_owned).collect();
        match self {
            Verb::WallpaperSet { target, file } => {
                let target = match target {
                    Target::All => "all",
                    Target::Output(name) => name,
                };
                vec![
                    "wallpaper".to_owned(),
                    "set".to_owned(),
                    target.to_owned(),
                    file.clone(),
                ]
            }
            Verb::WallpaperNext(out) | Verb::WallpaperPrev(out) => {
                let mut words = vec![
                    "wallpaper".to_owned(),
                    if matches!(self, Verb::WallpaperNext(_)) {
                        "next"
                    } else {
                        "prev"
                    }
                    .to_owned(),
                ];
                words.extend(out.clone());
                words
            }
            Verb::WallpaperMode(mode) => own(vec!["wallpaper", "mode", mode_word(*mode)]),
            Verb::WallpaperRetry => own(vec!["wallpaper", "retry"]),
            other => own(other.fixed_words()),
        }
    }

    fn fixed_words(&self) -> Vec<&'static str> {
        match self {
            Verb::IslandToggle => vec!["island", "toggle"],
            Verb::IslandOpen(page) => vec!["island", "open", page.word()],
            Verb::IslandClose => vec!["island", "close"],
            Verb::LauncherToggle => vec!["launcher", "toggle"],
            Verb::NotificationsToggle => vec!["notifications", "toggle"],
            Verb::NotificationsDismissAll => vec!["notifications", "dismiss-all"],
            Verb::Quit => vec!["quit"],
            Verb::WallpaperSet { .. }
            | Verb::WallpaperNext(_)
            | Verb::WallpaperPrev(_)
            | Verb::WallpaperMode(_)
            | Verb::WallpaperRetry => Vec::new(),
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
            (
                Verb::WallpaperSet {
                    target: Target::All,
                    file: "a b.png".to_owned(),
                },
                "wallpaper set all a b.png",
            ),
            (
                Verb::WallpaperSet {
                    target: Target::Output("DP-1".to_owned()),
                    file: "sea.png".to_owned(),
                },
                "wallpaper set DP-1 sea.png",
            ),
            (Verb::WallpaperNext(None), "wallpaper next"),
            (
                Verb::WallpaperNext(Some("DP-1".to_owned())),
                "wallpaper next DP-1",
            ),
            (Verb::WallpaperPrev(None), "wallpaper prev"),
            (
                Verb::WallpaperPrev(Some("DP-1".to_owned())),
                "wallpaper prev DP-1",
            ),
            (Verb::WallpaperMode(FitMode::Fill), "wallpaper mode fill"),
            (Verb::WallpaperMode(FitMode::Fit), "wallpaper mode fit"),
            (
                Verb::WallpaperMode(FitMode::Stretch),
                "wallpaper mode stretch",
            ),
            (
                Verb::WallpaperMode(FitMode::Center),
                "wallpaper mode center",
            ),
            (Verb::WallpaperRetry, "wallpaper retry"),
        ];
        for (verb, spelled) in table {
            assert_eq!(verb.words().join(" "), spelled);
            let words = verb.words();
            assert_eq!(Verb::parse(&words), Ok(verb.clone()), "{spelled}");
            let words: Vec<&str> = spelled.split(' ').collect();
            // "a b.png" holds a space: compare through the word list instead.
            if !spelled.contains("a b.png") {
                assert_eq!(parse(&words), Ok(Mode::Client(verb)), "{spelled}");
            }
        }
    }

    #[test]
    fn wallpaper_verbs_reject_bad_payloads() {
        let long = "x".repeat(MAX_NAME_BYTES + 1);
        let fits = format!("{}.png", "x".repeat(MAX_NAME_BYTES - 4));
        for bad in [
            &["wallpaper", "set", "all", "dir/a.png"][..],
            &["wallpaper", "set", "all", ".sea.png"],
            &["wallpaper", "set", "all", "a\\b.png"],
            &["wallpaper", "set", "all", "notes.txt"],
            &["wallpaper", "set", "a/b", "a.png"],
            &["wallpaper", "set", ".x", "a.png"],
            &["wallpaper", "set", "all", "../a.png"],
            &["wallpaper", "set", "all", ".."],
            &["wallpaper", "set", "all", "."],
            &["wallpaper", "set", "all", "a\0.png"],
            &["wallpaper", "set", "all", ""],
            &["wallpaper", "set", "", "a.png"],
            &["wallpaper", "set", "all", &long],
            &["wallpaper", "set", &long, "a.png"],
            &["wallpaper", "set", "all"],
            &["wallpaper", "set"],
            &["wallpaper", "set", "all", "a.png", "extra"],
            &["wallpaper", "next", "DP-1", "extra"],
            &["wallpaper", "next", ""],
            &["wallpaper", "prev", "a\nb"],
            &["wallpaper", "mode", "tile"],
            &["wallpaper", "mode", "Fill"],
            &["wallpaper", "mode"],
            &["wallpaper", "retry", "now"],
            &["wallpaper"],
        ] {
            assert!(Verb::parse(&strings(bad)).is_err(), "{bad:?}");
        }
        assert!(Verb::parse(&strings(&["wallpaper", "set", "all", &fits])).is_ok());
        let err = Verb::parse(&strings(&["wallpaper", "mode", &long])).expect_err("mode");
        assert!(err.to_string().len() < 100, "{err}");
    }

    #[test]
    fn every_accepted_name_survives_the_settings_clamp() {
        use celestina_shell::settings::{Settings, Store};
        let names = [
            "a.png",
            "a b.png",
            "DP-1",
            "all",
            ".sea.png",
            "a\\b.png",
            "notes.txt",
            "a/b",
            "..",
            ".",
            "x\ny",
            "\u{e9}.webp",
            "A.JPG",
            "",
        ];
        for target in names {
            for file in names {
                let args = strings(&["wallpaper", "set", target, file]);
                let Ok(Verb::WallpaperSet { .. }) = Verb::parse(&args) else {
                    continue;
                };
                let store = Store::new(Settings::default());
                let pending = store.stage(|s| {
                    s.wallpaper
                        .outputs
                        .insert(target.to_owned(), file.to_owned());
                });
                assert_eq!(
                    pending
                        .wanted()
                        .wallpaper
                        .outputs
                        .get(target)
                        .map(String::as_str),
                    Some(file),
                    "{target:?} {file:?}"
                );
            }
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
