//! The few editing choices that outlive a window.
//!
//! Grafita has no settings dialog and does not want one: a preference lands
//! here only when the user can already change it with a key. Today those are
//! the text size, which Ctrl + and Ctrl − move, and whether long lines wrap,
//! which Alt + Z turns off and on. Both would be an irritation to set again on
//! every launch.
//!
//! The file is `key = value`, one per line, in `$XDG_CONFIG_HOME/grafita/
//! preferences`. Like [`crate::recent`], reading a broken one is not an error —
//! an unreadable preference is the default, never a refusal to start — and
//! writing is best-effort, because a preference that cannot be saved must not
//! stop an edit.

use std::path::PathBuf;

use celestina_core::{atomic_file, xdg};

/// The text size Grafita starts at, matching the theme's caption size — the
/// value the editor used before it was adjustable.
pub const DEFAULT_FONT_SIZE: u32 = 11;

/// Below this the caret is larger than the glyphs; above it a line of code
/// stops fitting. Anything outside is clamped rather than refused, so a
/// hand-edited file still opens the editor.
pub const MIN_FONT_SIZE: u32 = 7;
/// See [`MIN_FONT_SIZE`].
pub const MAX_FONT_SIZE: u32 = 42;

/// Long lines wrap unless the user says otherwise. Grafita opens prose as
/// readily as code, and prose with a horizontal scroll bar is unreadable.
pub const DEFAULT_WRAP: bool = true;

/// What the editor remembers between launches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Preferences {
    font_size: u32,
    wrap: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            font_size: DEFAULT_FONT_SIZE,
            wrap: DEFAULT_WRAP,
        }
    }
}

impl Preferences {
    /// Reads the stored preferences, or the defaults.
    ///
    /// A missing, unreadable or malformed file is the defaults: there is
    /// nothing the user could do about it and nothing is lost.
    #[must_use]
    pub fn load() -> Self {
        let Some(path) = storage() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        Self::parse(&text)
    }

    #[must_use]
    fn parse(text: &str) -> Self {
        let mut preferences = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            // An unknown key is left alone rather than dropped from the parse:
            // a newer Grafita's preference should survive an older one reading
            // the file — which it does, because writing only ever rewrites the
            // keys this version knows, and the reader ignores the rest.
            match key.trim() {
                "font_size" => {
                    if let Ok(size) = value.trim().parse::<u32>() {
                        preferences.set_font_size(size);
                    }
                }
                // Only the two spellings this file is written with. Anything
                // else is a value nobody wrote, so the default stands rather
                // than a guess being made about what was meant.
                "wrap" => match value.trim() {
                    "true" => preferences.wrap = true,
                    "false" => preferences.wrap = false,
                    _ => {}
                },
                _ => {}
            }
        }
        preferences
    }

    /// The editor's text size, in pixels.
    #[must_use]
    pub const fn font_size(&self) -> u32 {
        self.font_size
    }

    /// Sets the text size, clamped to what stays legible.
    pub const fn set_font_size(&mut self, size: u32) {
        self.font_size = if size < MIN_FONT_SIZE {
            MIN_FONT_SIZE
        } else if size > MAX_FONT_SIZE {
            MAX_FONT_SIZE
        } else {
            size
        };
    }

    /// Moves the text size by `steps` pixels and answers the size now in
    /// effect, so a host can tell a real change from a keypress at the limit.
    pub const fn nudge_font_size(&mut self, steps: i32) -> u32 {
        // Saturating on both sides: the clamp below is what decides the
        // result, and it should never be reached through a wrapped number.
        let wanted = (self.font_size as i64).saturating_add(steps as i64);
        self.set_font_size(if wanted < 0 { 0 } else { wanted as u32 });
        self.font_size
    }

    /// Whether long lines wrap to the width of the surface.
    #[must_use]
    pub const fn wrap(&self) -> bool {
        self.wrap
    }

    /// Turns wrapping off and on.
    pub const fn toggle_wrap(&mut self) {
        self.wrap = !self.wrap;
    }

    /// Writes the preferences back. Best-effort, for the reason at the top.
    pub fn store(&self) {
        let Some(path) = storage() else {
            return;
        };
        let text = format!("font_size = {}\nwrap = {}\n", self.font_size, self.wrap);
        let _ = atomic_file::replace(&path, text.as_bytes());
    }
}

/// Where the preferences live. Config, not data: this is a choice the user
/// made, and it is the kind of file they may reasonably want to edit or copy.
fn storage() -> Option<PathBuf> {
    Some(xdg::config_home()?.join("grafita").join("preferences"))
}

#[cfg(test)]
mod tests {
    use super::{Preferences, DEFAULT_FONT_SIZE, DEFAULT_WRAP, MAX_FONT_SIZE, MIN_FONT_SIZE};

    #[test]
    fn an_unreadable_line_leaves_the_default_standing() {
        let preferences =
            Preferences::parse("rubbish\n# a comment\nfont_size = eight\nwrap = maybe\n");

        assert_eq!(preferences.font_size(), DEFAULT_FONT_SIZE);
        assert_eq!(preferences.wrap(), DEFAULT_WRAP);
    }

    #[test]
    fn a_stored_size_is_read_back() {
        let preferences = Preferences::parse("font_size = 17\n");

        assert_eq!(preferences.font_size(), 17);
    }

    #[test]
    fn wrapping_is_stored_and_read_back_in_both_states() {
        assert!(!Preferences::parse("wrap = false\n").wrap());
        assert!(Preferences::parse("wrap = true\n").wrap());

        // A file this version wrote must read back as itself.
        let mut written = Preferences::default();
        written.toggle_wrap();
        written.set_font_size(23);
        let read_back = Preferences::parse(&format!(
            "font_size = {}\nwrap = {}\n",
            written.font_size(),
            written.wrap()
        ));

        assert_eq!(read_back, written);
    }

    #[test]
    fn a_size_outside_the_legible_range_is_clamped_rather_than_refused() {
        assert_eq!(
            Preferences::parse("font_size = 0").font_size(),
            MIN_FONT_SIZE
        );
        assert_eq!(
            Preferences::parse("font_size = 4000").font_size(),
            MAX_FONT_SIZE
        );
    }

    #[test]
    fn nudging_stops_at_the_limits_and_reports_the_size_in_effect() {
        let mut preferences = Preferences::default();

        assert_eq!(preferences.nudge_font_size(1), DEFAULT_FONT_SIZE + 1);
        assert_eq!(preferences.nudge_font_size(-1), DEFAULT_FONT_SIZE);
        assert_eq!(preferences.nudge_font_size(-1000), MIN_FONT_SIZE);
        assert_eq!(preferences.nudge_font_size(1000), MAX_FONT_SIZE);
    }
}
