//! Reading `.desktop` files, the way freedesktop describes them.
//!
//! Two applications in this suite need this and want different fields from it:
//! the file manager asks which applications declare a MIME type, and the shell's
//! launcher asks what a person can start and what to call it. That is the same
//! recipe read twice, so it is read once here — the parser keeps every field
//! either of them uses, and each caller looks at what it needs.
//!
//! Only the `[Desktop Entry]` group is read. A file's later action groups
//! describe extra launchers within it, which is a different question from "what
//! is this application".
//!
//! # Reading and scanning
//!
//! [`parse`] takes a body a caller already holds and keeps every value exactly
//! as written. [`read`], [`scan`] and [`find`] are the file-facing owners the
//! audit asked for, and they apply the specification's value rules on top:
//!
//! - a file is read only when it is a regular file of at most
//!   [`MAX_ENTRY_BYTES`], so a FIFO or a huge file named `x.desktop` neither
//!   blocks nor exhausts the reader;
//! - string values are unescaped (`\s`, `\n`, `\t`, `\r`, `\\`) before anything
//!   reads them, so [`exec_argv`] sees the `Exec` the specification means, and
//!   list values split only on an unescaped `;` (`\;` is a literal semicolon);
//! - one shadowing rule: a desktop-file id belongs to the first directory in
//!   [`application_search_dirs`] order that holds a file of that name, whether
//!   or not that file reads or parses. A user's unreadable or `Hidden=true`
//!   override therefore hides the system entry of the same id;
//! - the directory list drops relative and empty `$XDG_DATA_DIRS` entries and
//!   falls back to the specification's default when none is left.
//!
//! All three block on the filesystem and belong on a worker thread, never in a
//! Qt-thread invokable.
//!
//! # Adoption
//!
//! No consumer calls [`read`], [`scan`] or [`find`] yet (ruling R-A3 keeps the
//! unit that introduced them purely additive, so [`parse`] and
//! [`application_dirs`] keep their behaviour). Siderita's `apps::apps_for_mime`
//! and `ownicon::own_icon` move to them and off the Qt thread in `SID-H1-C`
//! (P-15), the shell launcher's `scan_entries` in `SURF-1-F` (P-17), and
//! Hematita's per-id lookup in `HEM-H1-B` (P-18). When the last caller has
//! moved, [`application_dirs`] and the raw [`parse`] lose their callers.

use std::collections::HashSet;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::atomic_file;
use crate::CancellationToken;

/// One `.desktop` file's entry group.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DesktopEntry {
    /// The file name, e.g. `firefox.desktop`: the id every other desktop tool
    /// uses, and what a user override shadows.
    pub id: String,
    /// The unlocalized `Name`. Localized variants are deliberately ignored:
    /// picking one means picking a locale, which is the caller's business and
    /// not something to guess while parsing.
    pub name: String,
    pub generic_name: String,
    pub comment: String,
    pub exec: String,
    pub try_exec: String,
    pub icon: String,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub mimetypes: Vec<String>,
    pub only_show_in: Vec<String>,
    pub not_show_in: Vec<String>,
    pub is_application: bool,
    pub hidden: bool,
    pub no_display: bool,
    /// `Terminal=true`: the application expects to be started inside one.
    pub terminal: bool,
}

impl DesktopEntry {
    /// Whether this is an application a person should be offered.
    ///
    /// `Hidden` means the entry was deleted by a user override and is not an
    /// application at all; `NoDisplay` means it exists but is not for a menu.
    #[must_use]
    pub fn is_listable(&self) -> bool {
        self.is_application && !self.hidden && !self.no_display && !self.name.is_empty()
    }

    /// Whether this entry belongs in `desktop`'s menus. An entry that names the
    /// desktops it is for excludes every other one, and an entry that names the
    /// ones it is not for excludes those.
    #[must_use]
    pub fn shows_in(&self, desktop: &str) -> bool {
        let names = |list: &[String]| list.iter().any(|entry| entry.eq_ignore_ascii_case(desktop));

        if !self.only_show_in.is_empty() && !names(&self.only_show_in) {
            return false;
        }
        !names(&self.not_show_in)
    }

    #[must_use]
    pub fn handles(&self, mime: &str) -> bool {
        self.mimetypes.iter().any(|declared| declared == mime)
    }
}

fn semicolon_list(value: &str) -> Vec<String> {
    value
        .split(';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A value with the specification's escapes undone. `\;` is a literal
/// semicolon inside a list and is kept as written in a plain string, like any
/// escape the specification does not define.
fn unescape(value: &str, in_list: bool) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(';') if in_list => out.push(';'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// A list value split on its unescaped `;`, each item trimmed as written and
/// then unescaped, so an escaped leading space (`\s`) survives.
fn escaped_list(value: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut start = 0;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            ';' => {
                items.push(&value[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    items.push(&value[start..]);
    items
        .into_iter()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| unescape(item, true))
        .collect()
}

/// How a parse treats values: as written ([`parse`]) or with the
/// specification's escapes undone ([`read`]).
#[derive(Clone, Copy)]
enum Values {
    Raw,
    Unescaped,
}

impl Values {
    fn string(self, value: &str) -> String {
        match self {
            Self::Raw => value.to_owned(),
            Self::Unescaped => unescape(value, false),
        }
    }

    fn list(self, value: &str) -> Vec<String> {
        match self {
            Self::Raw => semicolon_list(value),
            Self::Unescaped => escaped_list(value),
        }
    }
}

/// Parses a `.desktop` file body. Returns `None` when there is no
/// `[Desktop Entry]` group at all, which is the one thing that makes a file not
/// a desktop entry.
///
/// Values are kept exactly as written: no escape is undone and a list splits on
/// every `;`. [`read`] applies the specification's escapes.
#[must_use]
pub fn parse(id: &str, content: &str) -> Option<DesktopEntry> {
    parse_with(id, content, Values::Raw)
}

fn parse_with(id: &str, content: &str, values: Values) -> Option<DesktopEntry> {
    let mut entry = DesktopEntry {
        id: id.to_owned(),
        ..DesktopEntry::default()
    };
    let mut in_group = false;
    let mut seen_group = false;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_group = line == "[Desktop Entry]";
            seen_group |= in_group;
            continue;
        }
        if !in_group || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());

        match key {
            "Name" => entry.name = values.string(value),
            "GenericName" => entry.generic_name = values.string(value),
            "Comment" => entry.comment = values.string(value),
            "Exec" => entry.exec = values.string(value),
            "TryExec" => entry.try_exec = values.string(value),
            "Icon" => entry.icon = values.string(value),
            "Type" => entry.is_application = value == "Application",
            "Hidden" => entry.hidden = value.eq_ignore_ascii_case("true"),
            "NoDisplay" => entry.no_display = value.eq_ignore_ascii_case("true"),
            "Terminal" => entry.terminal = value.eq_ignore_ascii_case("true"),
            "Categories" => entry.categories = values.list(value),
            "Keywords" => entry.keywords = values.list(value),
            "MimeType" => entry.mimetypes = values.list(value),
            "OnlyShowIn" => entry.only_show_in = values.list(value),
            "NotShowIn" => entry.not_show_in = values.list(value),
            _ => {}
        }
    }

    seen_group.then_some(entry)
}

/// Splits an `Exec` value into words the way the specification defines: shell
/// quoting rules apply *only* inside double quotes, and a backslash there
/// escapes only `` ` ``, `$`, `"`, `\` and a newline — nothing else, and never
/// outside a quoted run. This is not a shell grammar and no shell ever runs it;
/// that is the point. An `Exec` launched through `/bin/sh -c` would let a
/// `.desktop` file's `$()` or `;` do something no launcher click implies.
///
/// Malformed input — a quote never closed — yields the words read so far
/// rather than nothing: a launcher entry with a typo still starts something
/// close to what it named, instead of quietly refusing to run at all.
fn split_exec(exec: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = exec.trim().chars().peekable();

    while let Some(character) = chars.next() {
        match character {
            c if c.is_whitespace() => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            '"' => {
                while let Some(&inner) = chars.peek() {
                    if inner == '"' {
                        chars.next();
                        break;
                    }
                    if inner == '\\' {
                        chars.next();
                        match chars.peek() {
                            Some('`' | '$' | '"' | '\\') => {
                                current.push(chars.next().expect("peeked"));
                            }
                            _ => current.push('\\'),
                        }
                        continue;
                    }
                    current.push(inner);
                    chars.next();
                }
            }
            other => current.push(other),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Expands `Exec`'s field codes into a program and its arguments, with no file
/// or URL to launch — which is every field code an application menu entry ever
/// needs to fill in, since a launcher click names no target.
///
/// `%f`, `%F`, `%u` and `%U` are dropped rather than left as literal text: an
/// application asked to open "nothing" should see no argument, not the two
/// characters `%f`. `%i` becomes `--icon <Icon>` when the entry has one, `%c`
/// becomes the entry's name, `%k` the empty string (no file backs a running
/// process), and `%%` is the one field code the specification asks a
/// implementation to keep as text: a literal percent sign.
///
/// # Errors
///
/// Returns `None` for an `Exec` that names no program at all — an empty value,
/// or one that is only field codes and quoting.
#[must_use]
pub fn exec_argv(entry: &DesktopEntry) -> Option<Vec<String>> {
    let mut argv = Vec::new();

    for word in split_exec(&entry.exec) {
        let mut expanded = String::new();
        let mut chars = word.chars().peekable();
        while let Some(character) = chars.next() {
            if character != '%' {
                expanded.push(character);
                continue;
            }
            match chars.next() {
                Some('%') => expanded.push('%'),
                Some('f' | 'F' | 'u' | 'U' | 'k') => {}
                Some('c') => expanded.push_str(&entry.name),
                Some('i') if !entry.icon.is_empty() => {
                    if !expanded.is_empty() {
                        argv.push(std::mem::take(&mut expanded));
                    }
                    argv.push("--icon".to_owned());
                    argv.push(entry.icon.clone());
                }
                Some('i') => {}
                // An unrecognized code is not one this implementation invents
                // a meaning for; it is dropped along with its `%`.
                Some(_) | None => {}
            }
        }
        if !expanded.is_empty() {
            argv.push(expanded);
        }
    }

    (!argv.is_empty()).then_some(argv)
}

/// The XDG application directories, most specific first, so a user override of
/// a system id wins.
///
/// Kept as it was for its current callers: a relative `$XDG_DATA_DIRS` entry is
/// kept (and resolves against the working directory), and a set but empty
/// variable yields no system directory. [`application_search_dirs`] follows
/// the specification and is what [`scan`] and [`find`] callers pass.
#[must_use]
pub fn application_dirs() -> Vec<PathBuf> {
    legacy_application_dirs(crate::xdg::data_home(), std::env::var_os("XDG_DATA_DIRS"))
}

fn legacy_application_dirs(
    data_home: Option<PathBuf>,
    data_dirs: Option<OsString>,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(data_home) = data_home {
        dirs.push(data_home.join("applications"));
    }

    let data_dirs = data_dirs
        .map(|raw| raw.to_string_lossy().into_owned())
        .unwrap_or_else(|| DEFAULT_DATA_DIRS.to_owned());
    for dir in data_dirs.split(':').filter(|part| !part.is_empty()) {
        dirs.push(Path::new(dir).join("applications"));
    }

    dirs
}

/// The specification's `$XDG_DATA_DIRS` when the variable is unset or empty.
const DEFAULT_DATA_DIRS: &str = "/usr/local/share:/usr/share";

/// The XDG application directories, most specific first, as the specification
/// defines them: `$XDG_DATA_HOME/applications`, then each absolute entry of
/// `$XDG_DATA_DIRS`. Relative and empty entries are dropped (a relative one
/// would resolve against the working directory), and when the variable is
/// unset, empty or holds no absolute entry the default
/// `/usr/local/share:/usr/share` applies. Entries are read as bytes, so a
/// non-UTF-8 directory name survives.
#[must_use]
pub fn application_search_dirs() -> Vec<PathBuf> {
    search_dirs_from(
        crate::xdg::data_home(),
        std::env::var_os("XDG_DATA_DIRS").as_deref(),
    )
}

fn search_dirs_from(data_home: Option<PathBuf>, data_dirs: Option<&OsStr>) -> Vec<PathBuf> {
    use std::os::unix::ffi::OsStrExt;

    let absolute = |raw: &OsStr| -> Vec<PathBuf> {
        raw.as_bytes()
            .split(|&byte| byte == b':')
            .map(|entry| Path::new(OsStr::from_bytes(entry)))
            .filter(|entry| entry.is_absolute())
            .map(Path::to_path_buf)
            .collect()
    };
    let mut system = data_dirs.map(absolute).unwrap_or_default();
    if system.is_empty() {
        system = absolute(OsStr::new(DEFAULT_DATA_DIRS));
    }
    data_home
        .into_iter()
        .chain(system)
        .map(|dir| dir.join("applications"))
        .collect()
}

/// The largest `.desktop` file [`read`] accepts. Real entries are a few
/// kilobytes even with dozens of translations.
pub const MAX_ENTRY_BYTES: u64 = 64 * 1024;

/// Why [`read`] has no entry for a path.
#[derive(Debug)]
pub enum ReadError {
    /// Nothing exists at the path.
    Missing { path: PathBuf },
    /// The file is not a regular file, is larger than [`MAX_ENTRY_BYTES`], or
    /// could not be read.
    File(atomic_file::ReadError),
    /// The file name is not UTF-8, so it is no desktop-file id.
    BadName { path: PathBuf },
    /// The body is not UTF-8, which the specification requires.
    NotUtf8 { path: PathBuf },
    /// The body has no `[Desktop Entry]` group.
    NotAnEntry { path: PathBuf },
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path } => write!(formatter, "{} does not exist", path.display()),
            Self::File(error) => error.fmt(formatter),
            Self::BadName { path } => {
                write!(
                    formatter,
                    "{} has a file name that is not UTF-8",
                    path.display()
                )
            }
            Self::NotUtf8 { path } => write!(formatter, "{} is not UTF-8", path.display()),
            Self::NotAnEntry { path } => {
                write!(formatter, "{} has no [Desktop Entry] group", path.display())
            }
        }
    }
}

impl Error for ReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::File(error) => Some(error),
            _ => None,
        }
    }
}

/// Reads one `.desktop` file, bounded, with the specification's escapes
/// undone. The entry's id is the file name.
///
/// # Errors
///
/// The [`ReadError`] saying why the path holds no usable entry.
pub fn read(path: &Path) -> Result<DesktopEntry, ReadError> {
    let id = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| ReadError::BadName {
            path: path.to_path_buf(),
        })?;
    let bytes = atomic_file::read_bounded(path, MAX_ENTRY_BYTES)
        .map_err(ReadError::File)?
        .ok_or_else(|| ReadError::Missing {
            path: path.to_path_buf(),
        })?;
    let text = String::from_utf8(bytes).map_err(|_| ReadError::NotUtf8 {
        path: path.to_path_buf(),
    })?;
    parse_with(id, &text, Values::Unescaped).ok_or_else(|| ReadError::NotAnEntry {
        path: path.to_path_buf(),
    })
}

/// Whether `id` can name a desktop file in an application directory.
fn is_desktop_id(id: &str) -> bool {
    id.len() > ".desktop".len() && id.ends_with(".desktop") && !id.contains('/')
}

/// What one application scan found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    /// The entry that owns each id, in directory order and then by id. Entries
    /// that are not listable (`Hidden`, `NoDisplay`, not an application) are
    /// included, because they are still what owns the id; a menu filters with
    /// [`DesktopEntry::is_listable`].
    pub entries: Vec<DesktopEntry>,
    /// More ids existed than the scan was allowed to read.
    pub truncated: bool,
}

/// The scan stopped because its token was cancelled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanCancelled;

impl fmt::Display for ScanCancelled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the application scan was cancelled")
    }
}

impl Error for ScanCancelled {}

/// Reads every desktop-file id in `dirs` (normally [`application_search_dirs`])
/// once, with the one shadowing rule of the module documentation.
///
/// At most `max_entries` ids are claimed and read; when more exist the result
/// says it is truncated, and which ones were kept is then unspecified. The
/// token is checked between directory entries, so a cancelled scan stops
/// within one file read.
///
/// # Errors
///
/// [`ScanCancelled`] when `cancellation` was cancelled before the scan ended.
pub fn scan(
    dirs: &[PathBuf],
    cancellation: &CancellationToken,
    max_entries: usize,
) -> Result<Scan, ScanCancelled> {
    let mut claimed: HashSet<String> = HashSet::new();
    let mut found = Scan::default();

    'dirs: for dir in dirs {
        let Ok(listing) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut ids = Vec::new();
        for item in listing {
            if cancellation.is_cancelled() {
                return Err(ScanCancelled);
            }
            let Ok(item) = item else {
                continue;
            };
            let name = item.file_name();
            let Some(id) = name.to_str() else {
                continue;
            };
            if !is_desktop_id(id)
                || claimed.contains(id)
                || item.file_type().is_ok_and(|kind| kind.is_dir())
            {
                continue;
            }
            if ids.len() >= max_entries {
                found.truncated = true;
                break;
            }
            ids.push(id.to_owned());
        }
        ids.sort();

        for id in ids {
            if cancellation.is_cancelled() {
                return Err(ScanCancelled);
            }
            if claimed.len() >= max_entries {
                found.truncated = true;
                break 'dirs;
            }
            let path = dir.join(&id);
            claimed.insert(id);
            if let Ok(entry) = read(&path) {
                found.entries.push(entry);
            }
        }
    }

    Ok(found)
}

/// The entry that owns desktop-file id `id` (for example `firefox.desktop`) in
/// `dirs`, under the same shadowing rule as [`scan`]: the first directory
/// holding a file of that name decides, and `None` means no directory has one
/// or the one that decides does not read as an entry.
#[must_use]
pub fn find(dirs: &[PathBuf], id: &str) -> Option<DesktopEntry> {
    if !is_desktop_id(id) {
        return None;
    }
    for dir in dirs {
        let path = dir.join(id);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if !metadata.is_dir() => return read(&path).ok(),
            Ok(_) | Err(_) => continue,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry_with_exec(exec: &str) -> DesktopEntry {
        DesktopEntry {
            exec: exec.to_owned(),
            ..DesktopEntry::default()
        }
    }

    #[test]
    fn a_plain_command_needs_no_quoting_at_all() {
        assert_eq!(
            exec_argv(&entry_with_exec("kitty")),
            Some(vec!["kitty".to_owned()])
        );
        assert_eq!(
            exec_argv(&entry_with_exec("firefox --new-window")),
            Some(vec!["firefox".to_owned(), "--new-window".to_owned()])
        );
    }

    #[test]
    fn file_and_url_codes_vanish_with_no_target_to_fill_them() {
        assert_eq!(
            exec_argv(&entry_with_exec("firefox %u")),
            Some(vec!["firefox".to_owned()])
        );
        assert_eq!(
            exec_argv(&entry_with_exec("codium %F")),
            Some(vec!["codium".to_owned()])
        );
    }

    #[test]
    fn the_icon_code_becomes_a_named_flag_only_when_there_is_an_icon() {
        let mut with_icon = entry_with_exec("app %i --flag");
        with_icon.icon = "app-icon".to_owned();
        assert_eq!(
            exec_argv(&with_icon),
            Some(vec![
                "app".to_owned(),
                "--icon".to_owned(),
                "app-icon".to_owned(),
                "--flag".to_owned(),
            ])
        );

        assert_eq!(
            exec_argv(&entry_with_exec("app %i --flag")),
            Some(vec!["app".to_owned(), "--flag".to_owned()])
        );
    }

    #[test]
    fn a_literal_percent_survives_and_the_name_code_expands() {
        let mut entry = entry_with_exec("app --title=%c --ratio=50%%");
        entry.name = "App".to_owned();
        assert_eq!(
            exec_argv(&entry),
            Some(vec![
                "app".to_owned(),
                "--title=App".to_owned(),
                "--ratio=50%".to_owned()
            ])
        );
    }

    #[test]
    fn double_quotes_group_one_argument_and_only_escape_what_the_spec_lists() {
        assert_eq!(
            exec_argv(&entry_with_exec(r#"app "an argument with spaces""#)),
            Some(vec!["app".to_owned(), "an argument with spaces".to_owned()])
        );
        // Inside quotes, a backslash before `"` is the literal quote; a
        // backslash before anything else stays a backslash.
        assert_eq!(
            exec_argv(&entry_with_exec(r#"app "she said \"hi\", \\n""#)),
            Some(vec!["app".to_owned(), r#"she said "hi", \n"#.to_owned()])
        );
    }

    #[test]
    fn an_exec_with_no_program_at_all_is_not_launchable() {
        assert_eq!(exec_argv(&entry_with_exec("")), None);
        assert_eq!(exec_argv(&entry_with_exec("%f %u")), None);
        assert_eq!(exec_argv(&entry_with_exec("   ")), None);
    }

    #[test]
    fn an_unterminated_quote_still_yields_what_it_read() {
        assert_eq!(
            exec_argv(&entry_with_exec(r#"app "unterminated"#)),
            Some(vec!["app".to_owned(), "unterminated".to_owned()])
        );
    }

    const FIREFOX: &str = "[Desktop Entry]\n\
                           Type=Application\n\
                           Name=Firefox\n\
                           Name[es]=Zorro de fuego\n\
                           GenericName=Navegador web\n\
                           Exec=firefox %u\n\
                           Icon=firefox\n\
                           Categories=Network;WebBrowser;\n\
                           Keywords=internet;navegador;\n\
                           MimeType=text/html;text/xml;\n\
                           \n\
                           [Desktop Action new-window]\n\
                           Name=Ventana nueva\n\
                           Exec=firefox --new-window\n";

    #[test]
    fn reads_the_fields_both_callers_need() {
        let entry = parse("firefox.desktop", FIREFOX).expect("an entry");

        assert_eq!(entry.id, "firefox.desktop");
        assert_eq!(entry.name, "Firefox");
        assert_eq!(entry.generic_name, "Navegador web");
        assert_eq!(entry.exec, "firefox %u");
        assert_eq!(entry.icon, "firefox");
        assert_eq!(entry.categories, ["Network", "WebBrowser"]);
        assert_eq!(entry.keywords, ["internet", "navegador"]);
        assert!(entry.handles("text/html"));
        assert!(entry.is_listable());
        assert!(!entry.terminal);
    }

    #[test]
    fn a_localized_name_is_not_the_name() {
        // Picking one means picking a locale, which is the caller's business.
        let entry = parse("firefox.desktop", FIREFOX).expect("an entry");

        assert_eq!(entry.name, "Firefox");
    }

    #[test]
    fn only_the_entry_group_is_read() {
        let entry = parse("firefox.desktop", FIREFOX).expect("an entry");

        // The action group also has a Name and an Exec, and neither is the
        // application's.
        assert_ne!(entry.name, "Ventana nueva");
        assert_eq!(entry.exec, "firefox %u");
    }

    #[test]
    fn a_file_with_no_entry_group_is_not_a_desktop_entry() {
        assert!(parse("x.desktop", "[Desktop Action a]\nName=X\n").is_none());
        assert!(parse("x.desktop", "").is_none());
        // A group that exists but says nothing is still an entry — an empty one.
        assert!(parse("x.desktop", "[Desktop Entry]\n").is_some());
    }

    #[test]
    fn what_a_person_should_not_be_offered() {
        let hidden = parse(
            "x.desktop",
            "[Desktop Entry]\nType=Application\nName=X\nHidden=true\n",
        )
        .expect("an entry");
        let no_display = parse(
            "x.desktop",
            "[Desktop Entry]\nType=Application\nName=X\nNoDisplay=TRUE\n",
        )
        .expect("an entry");
        let nameless = parse("x.desktop", "[Desktop Entry]\nType=Application\n").expect("an entry");
        let a_link = parse(
            "x.desktop",
            "[Desktop Entry]\nType=Link\nName=X\nURL=https://example.invalid\n",
        )
        .expect("an entry");

        assert!(!hidden.is_listable());
        assert!(!no_display.is_listable());
        assert!(!nameless.is_listable());
        assert!(!a_link.is_listable());
    }

    #[test]
    fn an_entry_may_be_meant_for_another_desktop() {
        let gnome_only = parse(
            "x.desktop",
            "[Desktop Entry]\nType=Application\nName=X\nOnlyShowIn=GNOME;\n",
        )
        .expect("an entry");
        let not_here = parse(
            "x.desktop",
            "[Desktop Entry]\nType=Application\nName=X\nNotShowIn=niri;KDE;\n",
        )
        .expect("an entry");
        let anywhere =
            parse("x.desktop", "[Desktop Entry]\nType=Application\nName=X\n").expect("an entry");

        assert!(!gnome_only.shows_in("niri"));
        assert!(gnome_only.shows_in("GNOME"));
        assert!(!not_here.shows_in("niri"));
        // The comparison is case-insensitive, as the specification says.
        assert!(!not_here.shows_in("NIRI"));
        assert!(anywhere.shows_in("niri"));
    }

    #[test]
    fn the_legacy_directory_list_keeps_its_order_and_its_gaps() {
        let user = Some(PathBuf::from("/home/u/.local/share"));

        assert_eq!(
            legacy_application_dirs(user.clone(), Some("relative:/usr/share::".into())),
            [
                PathBuf::from("/home/u/.local/share/applications"),
                PathBuf::from("relative/applications"),
                PathBuf::from("/usr/share/applications"),
            ]
        );
        // A set but empty variable yields no system directory at all.
        assert_eq!(
            legacy_application_dirs(user, Some(OsString::new())),
            [PathBuf::from("/home/u/.local/share/applications")]
        );
        assert_eq!(
            legacy_application_dirs(None, None),
            [
                PathBuf::from("/usr/local/share/applications"),
                PathBuf::from("/usr/share/applications"),
            ]
        );
    }

    #[test]
    fn the_search_dirs_put_the_user_first_and_follow_the_specification() {
        let user = Some(PathBuf::from("/home/u/.local/share"));
        let defaults = [
            PathBuf::from("/home/u/.local/share/applications"),
            PathBuf::from("/usr/local/share/applications"),
            PathBuf::from("/usr/share/applications"),
        ];

        assert_eq!(
            search_dirs_from(
                user.clone(),
                Some(OsStr::new("/opt/share:relative::/usr/share"))
            ),
            [
                PathBuf::from("/home/u/.local/share/applications"),
                PathBuf::from("/opt/share/applications"),
                PathBuf::from("/usr/share/applications"),
            ]
        );
        assert_eq!(search_dirs_from(user.clone(), None), defaults);
        assert_eq!(
            search_dirs_from(user.clone(), Some(OsStr::new(""))),
            defaults
        );
        assert_eq!(
            search_dirs_from(user, Some(OsStr::new("relative:"))),
            defaults
        );
        assert_eq!(
            search_dirs_from(None, Some(OsStr::new("/usr/share"))),
            [PathBuf::from("/usr/share/applications")]
        );
    }

    #[test]
    fn the_search_dirs_keep_non_utf8_directory_names() {
        use std::os::unix::ffi::OsStrExt;
        let dirs = search_dirs_from(None, Some(OsStr::from_bytes(b"/opt/\xff")));
        assert_eq!(dirs[0].as_os_str().as_bytes(), b"/opt/\xff/applications");
    }

    #[test]
    fn string_values_are_unescaped_before_exec_is_split() {
        let body = "[Desktop Entry]\n\
                    Name=\\sTwo\\sWords\\n\\tand\\\\ more\\q\n\
                    Exec=app \"\\\\$HOME\"\n";
        let entry = parse_with("x.desktop", body, Values::Unescaped).expect("an entry");

        assert_eq!(entry.name, " Two Words\n\tand\\ more\\q");
        assert_eq!(entry.exec, r#"app "\$HOME""#);
        assert_eq!(
            exec_argv(&entry),
            Some(vec!["app".to_owned(), "$HOME".to_owned()])
        );

        // The raw parse keeps what was written, and its Exec is the one the
        // audit found wrong: the string escape is never undone.
        let raw = parse("x.desktop", body).expect("an entry");
        assert_eq!(
            exec_argv(&raw),
            Some(vec!["app".to_owned(), r"\$HOME".to_owned()])
        );
    }

    #[test]
    fn lists_split_only_on_an_unescaped_semicolon() {
        let body = "[Desktop Entry]\nKeywords=semi\\;colon;plain; \\sspaced;back\\\\;\n";

        let entry = parse_with("x.desktop", body, Values::Unescaped).expect("an entry");
        assert_eq!(entry.keywords, ["semi;colon", "plain", " spaced", "back\\"]);

        let raw = parse("x.desktop", body).expect("an entry");
        assert_eq!(
            raw.keywords,
            ["semi\\", "colon", "plain", "\\sspaced", "back\\\\"]
        );
    }

    fn write_entry(dir: &Path, id: &str, body: &str) {
        std::fs::create_dir_all(dir).expect("dir");
        std::fs::write(dir.join(id), body).expect("entry");
    }

    fn app(name: &str) -> String {
        format!("[Desktop Entry]\nType=Application\nName={name}\nExec={name}\n")
    }

    #[test]
    fn reading_a_file_bounds_it_and_names_what_is_wrong() {
        let scratch = crate::scratch::Scratch::new("desktop-read");
        let dir = scratch.path();
        write_entry(dir, "good.desktop", &app("Good"));
        write_entry(dir, "none.desktop", "[Desktop Action a]\nName=X\n");
        std::fs::write(dir.join("latin1.desktop"), b"[Desktop Entry]\nName=\xf1\n")
            .expect("latin1");
        let huge = format!(
            "[Desktop Entry]\nName=Huge\nComment={}\n",
            "x".repeat(usize::try_from(MAX_ENTRY_BYTES).expect("small"))
        );
        write_entry(dir, "huge.desktop", &huge);

        let good = read(&dir.join("good.desktop")).expect("an entry");
        assert_eq!(good.id, "good.desktop");
        assert_eq!(good.name, "Good");
        assert!(matches!(
            read(&dir.join("none.desktop")),
            Err(ReadError::NotAnEntry { .. })
        ));
        assert!(matches!(
            read(&dir.join("latin1.desktop")),
            Err(ReadError::NotUtf8 { .. })
        ));
        assert!(matches!(
            read(&dir.join("huge.desktop")),
            Err(ReadError::File(atomic_file::ReadError::TooLarge { .. }))
        ));
        assert!(matches!(
            read(&dir.join("missing.desktop")),
            Err(ReadError::Missing { .. })
        ));
        assert!(matches!(
            read(dir),
            Err(ReadError::File(atomic_file::ReadError::NotRegular { .. }))
        ));
    }

    #[test]
    fn a_fifo_named_like_an_entry_is_refused_without_blocking() {
        let scratch = crate::scratch::Scratch::new("desktop-fifo");
        let fifo = scratch.path().join("x.desktop");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .is_ok_and(|status| status.success());
        if !made {
            eprintln!("mkfifo is unavailable; the FIFO case is not exercised");
            return;
        }
        assert!(matches!(
            read(&fifo),
            Err(ReadError::File(atomic_file::ReadError::NotRegular { .. }))
        ));
    }

    fn ids(scan: &Scan) -> Vec<&str> {
        scan.entries.iter().map(|entry| entry.id.as_str()).collect()
    }

    #[test]
    fn a_scan_gives_each_id_to_its_most_specific_directory() {
        let scratch = crate::scratch::Scratch::new("desktop-scan");
        let user = scratch.path().join("user");
        let system = scratch.path().join("system");
        write_entry(
            &user,
            "hidden.desktop",
            "[Desktop Entry]\nType=Application\nName=Mine\nHidden=true\n",
        );
        write_entry(&user, "broken.desktop", "not an entry");
        write_entry(&user, "notes.txt", &app("Ignored"));
        std::fs::create_dir_all(user.join("folder.desktop")).expect("a directory");
        write_entry(&system, "hidden.desktop", &app("System"));
        write_entry(&system, "broken.desktop", &app("Shadowed"));
        write_entry(&system, "zeta.desktop", &app("Zeta"));
        write_entry(&system, "alpha.desktop", &app("Alpha"));
        write_entry(&system, "folder.desktop", &app("Folder"));

        let found = scan(&[user, system], &CancellationToken::new(), 100).expect("not cancelled");

        assert!(!found.truncated);
        assert_eq!(
            ids(&found),
            [
                "hidden.desktop",
                "alpha.desktop",
                "folder.desktop",
                "zeta.desktop"
            ]
        );
        // The user's Hidden override owns the id, so the system entry is gone.
        assert_eq!(found.entries[0].name, "Mine");
        assert!(!found.entries[0].is_listable());
    }

    #[test]
    fn a_scan_reads_at_most_its_cap_and_says_so() {
        let scratch = crate::scratch::Scratch::new("desktop-cap");
        let dir = scratch.path().join("apps");
        for name in ["a", "b", "c"] {
            write_entry(&dir, &format!("{name}.desktop"), &app(name));
        }

        let capped =
            scan(std::slice::from_ref(&dir), &CancellationToken::new(), 2).expect("not cancelled");
        assert!(capped.truncated);
        assert_eq!(capped.entries.len(), 2);

        let whole = scan(&[dir], &CancellationToken::new(), 3).expect("not cancelled");
        assert!(!whole.truncated);
        assert_eq!(ids(&whole), ["a.desktop", "b.desktop", "c.desktop"]);
    }

    #[test]
    fn a_cancelled_scan_answers_cancelled() {
        let scratch = crate::scratch::Scratch::new("desktop-cancel");
        let dir = scratch.path().join("apps");
        write_entry(&dir, "a.desktop", &app("A"));
        let token = CancellationToken::new();
        token.cancel();

        assert_eq!(scan(&[dir], &token, 10), Err(ScanCancelled));
    }

    #[test]
    fn finding_an_id_uses_the_same_shadowing_rule() {
        let scratch = crate::scratch::Scratch::new("desktop-find");
        let user = scratch.path().join("user");
        let system = scratch.path().join("system");
        write_entry(&user, "broken.desktop", "not an entry");
        write_entry(&system, "broken.desktop", &app("Shadowed"));
        write_entry(&system, "firefox.desktop", &app("Firefox"));
        let dirs = [user, system];

        assert_eq!(
            find(&dirs, "firefox.desktop").map(|entry| entry.name),
            Some("Firefox".to_owned())
        );
        assert_eq!(find(&dirs, "broken.desktop"), None);
        assert_eq!(find(&dirs, "missing.desktop"), None);
        assert_eq!(find(&dirs, "../system/firefox.desktop"), None);
        assert_eq!(find(&dirs, ".desktop"), None);
        assert_eq!(find(&dirs, "firefox"), None);
    }
}
