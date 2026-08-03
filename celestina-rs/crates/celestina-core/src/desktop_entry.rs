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

use std::path::{Path, PathBuf};

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

/// Parses a `.desktop` file body. Returns `None` when there is no
/// `[Desktop Entry]` group at all, which is the one thing that makes a file not
/// a desktop entry.
#[must_use]
pub fn parse(id: &str, content: &str) -> Option<DesktopEntry> {
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
            "Name" => entry.name = value.to_owned(),
            "GenericName" => entry.generic_name = value.to_owned(),
            "Comment" => entry.comment = value.to_owned(),
            "Exec" => entry.exec = value.to_owned(),
            "TryExec" => entry.try_exec = value.to_owned(),
            "Icon" => entry.icon = value.to_owned(),
            "Type" => entry.is_application = value == "Application",
            "Hidden" => entry.hidden = value.eq_ignore_ascii_case("true"),
            "NoDisplay" => entry.no_display = value.eq_ignore_ascii_case("true"),
            "Terminal" => entry.terminal = value.eq_ignore_ascii_case("true"),
            "Categories" => entry.categories = semicolon_list(value),
            "Keywords" => entry.keywords = semicolon_list(value),
            "MimeType" => entry.mimetypes = semicolon_list(value),
            "OnlyShowIn" => entry.only_show_in = semicolon_list(value),
            "NotShowIn" => entry.not_show_in = semicolon_list(value),
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
#[must_use]
pub fn application_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(data_home) = crate::xdg::data_home() {
        dirs.push(data_home.join("applications"));
    }

    let data_dirs = std::env::var_os("XDG_DATA_DIRS")
        .map(|raw| raw.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    for dir in data_dirs.split(':').filter(|part| !part.is_empty()) {
        dirs.push(Path::new(dir).join("applications"));
    }

    dirs
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
    fn the_user_directory_comes_before_the_system_ones() {
        let dirs = application_dirs();

        assert!(dirs.len() >= 2);
        assert!(dirs.last().expect("a system dir").starts_with("/"));
    }
}
