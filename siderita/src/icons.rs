use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;

/// One per-path visual override. Icon and accent are one atomic value so QML
/// never observes a new shape with the previous colour (or vice versa).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IconAppearance {
    pub icon: String,
    pub accent: String,
}

impl IconAppearance {
    fn is_empty(&self) -> bool {
        self.icon.is_empty() && self.accent.is_empty()
    }
}

/// Stable keys persisted on disk. Their actual colours live in
/// CelestinaTheme, so retuning the palette never rewrites user configuration.
pub const ACCENT_KEYS: &[&str] = &["blue", "cyan", "green", "violet", "coral", "amber"];

pub fn valid_accent(key: &str) -> bool {
    key.is_empty() || ACCENT_KEYS.contains(&key)
}

/// The XDG config file per-path custom icon overrides live in, if a config home
/// is resolvable. One `key\ticon-name\taccent-key` line each, where the first
/// field is the path key of ADR 0008; records written before that decision hold
/// the raw path and are migrated on load by `pathkey::normalize`.
fn config_file() -> Option<PathBuf> {
    Some(
        celestina_core::xdg::config_home()?
            .join("siderita")
            .join("icons.conf"),
    )
}

/// Parses both the current three-column appearance format and the original
/// two-column `path\ticon` format. Invalid accents fall back to automatic while
/// preserving the icon, so a typo cannot make a valid legacy override vanish.
fn parse(content: &str) -> HashMap<String, IconAppearance> {
    content
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            // Leading/trailing spaces are legal Linux filename characters.
            // Preserve the path byte-for-byte even though icon/accent keys are
            // normalized as human-edited identifiers.
            let path = parts.next().map(crate::pathkey::normalize)?;
            let icon = parts.next()?.trim();
            let accent = parts.next().unwrap_or_default().trim();
            if path.is_empty() {
                return None;
            }

            let appearance = IconAppearance {
                icon: icon.to_owned(),
                accent: if valid_accent(accent) {
                    accent.to_owned()
                } else {
                    String::new()
                },
            };
            (!appearance.is_empty()).then_some((path, appearance))
        })
        .collect()
}

/// Loads the saved appearance overrides (absolute path → icon + accent). Any
/// error yields an empty map — a custom appearance is a convenience, never
/// required for browsing files.
pub fn load() -> HashMap<String, IconAppearance> {
    let Some(path) = config_file() else {
        return HashMap::new();
    };
    let Ok(content) = fs::read_to_string(&path) else {
        return HashMap::new();
    };
    parse(&content)
}

fn serialize(overrides: &HashMap<String, IconAppearance>) -> String {
    let mut entries: Vec<(&String, &IconAppearance)> = overrides
        .iter()
        .filter(|(_, appearance)| !appearance.is_empty())
        .collect();
    entries.sort_by_key(|(path, _)| *path);

    let mut body = String::new();
    for (path, appearance) in entries {
        // Marked, so the reader knows this is a key and does not have to infer
        // it from the codec.
        body.push_str(&crate::pathkey::persist(path));
        body.push('\t');
        body.push_str(&appearance.icon);
        if !appearance.accent.is_empty() {
            body.push('\t');
            body.push_str(&appearance.accent);
        }
        body.push('\n');
    }
    body
}

/// Persists the overrides through the suite's atomic replacement: a sibling
/// temporary, synced, then renamed over the file. A crash cannot therefore
/// truncate every saved appearance halfway through a write. This touches only
/// Siderita's own config, never the user's files.
pub fn save(overrides: &HashMap<String, IconAppearance>) -> io::Result<()> {
    let Some(path) = config_file() else {
        return Ok(());
    };
    celestina_core::atomic_file::replace(&path, serialize(overrides).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::{parse, serialize, IconAppearance};
    use std::collections::HashMap;

    #[test]
    fn reads_legacy_icons_and_current_appearances() {
        let parsed =
            parse("/tmp/legacy\tfolder-code\n/tmp/colour\t\tcyan\n/tmp/both\tfile-text\tviolet\n");

        assert_eq!(
            parsed.get("/tmp/legacy"),
            Some(&IconAppearance {
                icon: "folder-code".into(),
                accent: String::new(),
            })
        );
        assert_eq!(parsed["/tmp/colour"].accent, "cyan");
        assert_eq!(parsed["/tmp/both"].icon, "file-text");
        assert_eq!(parsed["/tmp/both"].accent, "violet");
    }

    #[test]
    fn invalid_accents_fall_back_without_losing_an_icon() {
        let parsed = parse("/tmp/icon\tfolder-git-2\thotpink\n/tmp/empty\t\thotpink\n");

        assert_eq!(parsed["/tmp/icon"].icon, "folder-git-2");
        assert!(parsed["/tmp/icon"].accent.is_empty());
        assert!(!parsed.contains_key("/tmp/empty"));
    }

    #[test]
    fn serializes_in_stable_order_and_round_trips_colour_only_entries() {
        let mut appearances = HashMap::new();
        appearances.insert(
            "/z".into(),
            IconAppearance {
                icon: String::new(),
                accent: "amber".into(),
            },
        );
        appearances.insert(
            "/a".into(),
            IconAppearance {
                icon: "file-code".into(),
                accent: String::new(),
            },
        );

        let body = serialize(&appearances);
        // The path field is marked as a key; the mark is what tells a later
        // reader it is one instead of a raw path written before ADR 0008.
        assert_eq!(body, "key:/a\tfile-code\nkey:/z\t\tamber\n");
        assert_eq!(parse(&body), appearances);
    }

    #[test]
    fn round_trip_preserves_legal_outer_spaces_in_a_path() {
        let mut appearances = HashMap::new();
        // Outer spaces are legal filename characters; as a path key they are
        // escaped, which is exactly what keeps them from being trimmed away.
        appearances.insert(
            "/tmp/%20carpeta%20".into(),
            IconAppearance {
                icon: "folder".into(),
                accent: "green".into(),
            },
        );

        assert_eq!(parse(&serialize(&appearances)), appearances);
    }

    #[test]
    fn a_legacy_raw_path_record_migrates_to_its_key() {
        let parsed = parse("/tmp/mis fotos\tfolder\tgreen\n");
        assert!(parsed.contains_key("/tmp/mis%20fotos"), "{parsed:?}");
    }

    #[test]
    fn a_marked_record_survives_a_name_holding_a_literal_percent_escape() {
        // Written by this version: `/tmp/100%20` is a directory whose name ends
        // in the four characters `%20`, and the mark is what stops it being
        // read back as `/tmp/100 `.
        let mut appearances = HashMap::new();
        appearances.insert(
            "/tmp/100%2520".into(),
            IconAppearance {
                icon: "folder".into(),
                accent: "amber".into(),
            },
        );
        assert_eq!(parse(&serialize(&appearances)), appearances);
    }
}
