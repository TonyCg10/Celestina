//! The launcher's application groups (design §5.2) and the Spanish words
//! that find an application by its desktop categories.
//!
//! An application belongs to exactly one group, chosen by the first rule
//! that one of its categories satisfies: Games, Development, Multimedia,
//! Internet, Office, System; anything else is Utilities. The words of
//! [`aliases`] are the group's name and one phrase per known category; the
//! search takes them as extra keywords.

use crate::copy;

/// One of the launcher's groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    Internet,
    Multimedia,
    Games,
    Development,
    Office,
    System,
    Utilities,
}

impl Group {
    /// Every group in tab order.
    pub const ALL: [Group; 7] = [
        Group::Internet,
        Group::Multimedia,
        Group::Games,
        Group::Development,
        Group::Office,
        Group::System,
        Group::Utilities,
    ];

    /// The group's name, in Spanish.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Group::Internet => copy::LAUNCHER_GROUP_INTERNET,
            Group::Multimedia => copy::LAUNCHER_GROUP_MULTIMEDIA,
            Group::Games => copy::LAUNCHER_GROUP_GAMES,
            Group::Development => copy::LAUNCHER_GROUP_DEVELOPMENT,
            Group::Office => copy::LAUNCHER_GROUP_OFFICE,
            Group::System => copy::LAUNCHER_GROUP_SYSTEM,
            Group::Utilities => copy::LAUNCHER_GROUP_UTILITIES,
        }
    }

    /// The name of the group's icon in the shell's icon set.
    #[must_use]
    pub fn icon(self) -> &'static str {
        match self {
            Group::Internet => "globe",
            Group::Multimedia => "film",
            Group::Games => "gamepad-2",
            Group::Development => "file-code",
            Group::Office => "file-text",
            Group::System => "settings",
            Group::Utilities => "toolbox",
        }
    }

    /// The desktop categories that put an application in this group.
    fn categories(self) -> &'static [&'static str] {
        match self {
            Group::Games => &["Game"],
            Group::Development => &["Development"],
            Group::Multimedia => &["AudioVideo", "Audio", "Video", "Graphics"],
            Group::Internet => &["Network"],
            Group::Office => &["Office", "Education", "Science"],
            Group::System => &["System", "Settings"],
            Group::Utilities => &[],
        }
    }
}

/// The group of an application with these desktop categories.
#[must_use]
pub fn group_of(categories: &[String]) -> Group {
    /// The order the rules apply in (design §5.2).
    const PRIORITY: [Group; 6] = [
        Group::Games,
        Group::Development,
        Group::Multimedia,
        Group::Internet,
        Group::Office,
        Group::System,
    ];
    PRIORITY
        .into_iter()
        .find(|group| {
            categories
                .iter()
                .any(|category| group.categories().contains(&category.as_str()))
        })
        .unwrap_or(Group::Utilities)
}

/// The Spanish phrase that finds an application of this desktop category.
fn alias_of(category: &str) -> Option<&'static str> {
    Some(match category {
        "WebBrowser" => copy::LAUNCHER_ALIAS_WEB_BROWSER,
        "TerminalEmulator" => copy::LAUNCHER_ALIAS_TERMINAL_EMULATOR,
        "TextEditor" => copy::LAUNCHER_ALIAS_TEXT_EDITOR,
        "FileManager" => copy::LAUNCHER_ALIAS_FILE_MANAGER,
        "Player" => copy::LAUNCHER_ALIAS_PLAYER,
        "IDE" => copy::LAUNCHER_ALIAS_IDE,
        "InstantMessaging" => copy::LAUNCHER_ALIAS_INSTANT_MESSAGING,
        "Monitor" => copy::LAUNCHER_ALIAS_MONITOR,
        "Emulator" => copy::LAUNCHER_ALIAS_EMULATOR,
        "AudioVideoEditing" => copy::LAUNCHER_ALIAS_AUDIO_VIDEO_EDITING,
        "RasterGraphics" | "2DGraphics" => copy::LAUNCHER_ALIAS_GRAPHICS,
        _ => return None,
    })
}

/// The words that find an application with these categories: its group's
/// name and one phrase per category that has one.
#[must_use]
pub fn aliases(categories: &[String]) -> Vec<&'static str> {
    let mut words: Vec<&'static str> = categories
        .iter()
        .filter_map(|category| alias_of(category))
        .collect();
    words.push(group_of(categories).label());
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cats(text: &str) -> Vec<String> {
        text.split(';')
            .filter(|c| !c.is_empty())
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn the_first_matching_rule_decides() {
        assert_eq!(group_of(&cats("Utility;Game;Graphics")), Group::Games);
        assert_eq!(group_of(&cats("Utility;Development")), Group::Development);
        assert_eq!(group_of(&cats("Network;Development")), Group::Development);
        assert_eq!(group_of(&cats("Network;WebBrowser")), Group::Internet);
        assert_eq!(group_of(&cats("AudioVideo;Player")), Group::Multimedia);
        assert_eq!(group_of(&cats("Audio")), Group::Multimedia);
        assert_eq!(group_of(&cats("Video")), Group::Multimedia);
        assert_eq!(group_of(&cats("Graphics;Network")), Group::Multimedia);
        assert_eq!(group_of(&cats("Office")), Group::Office);
        assert_eq!(group_of(&cats("Education")), Group::Office);
        assert_eq!(group_of(&cats("Science")), Group::Office);
        assert_eq!(group_of(&cats("Settings")), Group::System);
        assert_eq!(group_of(&cats("System;Utility")), Group::System);
    }

    #[test]
    fn what_no_rule_names_is_a_utility() {
        assert_eq!(group_of(&cats("X-WayDroid-App")), Group::Utilities);
        assert_eq!(group_of(&cats("Utility;TextEditor")), Group::Utilities);
        assert_eq!(group_of(&[]), Group::Utilities);
    }

    #[test]
    fn the_tabs_run_in_order_with_their_icons_and_labels() {
        let labels: Vec<&str> = Group::ALL.iter().map(|g| g.label()).collect();
        assert_eq!(
            labels,
            [
                copy::LAUNCHER_GROUP_INTERNET,
                copy::LAUNCHER_GROUP_MULTIMEDIA,
                copy::LAUNCHER_GROUP_GAMES,
                copy::LAUNCHER_GROUP_DEVELOPMENT,
                copy::LAUNCHER_GROUP_OFFICE,
                copy::LAUNCHER_GROUP_SYSTEM,
                copy::LAUNCHER_GROUP_UTILITIES,
            ]
        );
        let icons: Vec<&str> = Group::ALL.iter().map(|g| g.icon()).collect();
        assert_eq!(
            icons,
            [
                "globe",
                "film",
                "gamepad-2",
                "file-code",
                "file-text",
                "settings",
                "toolbox"
            ]
        );
    }

    #[test]
    fn aliases_hold_the_group_and_the_category_words() {
        let words = aliases(&cats("Network;WebBrowser"));
        assert!(words.contains(&copy::LAUNCHER_GROUP_INTERNET));
        assert!(words.contains(&copy::LAUNCHER_ALIAS_WEB_BROWSER));
        let words = aliases(&cats("Development;IDE"));
        assert!(words.contains(&copy::LAUNCHER_GROUP_DEVELOPMENT));
        assert!(words.contains(&copy::LAUNCHER_ALIAS_IDE));
    }

    #[test]
    fn every_known_category_has_its_word_and_unknown_ones_have_none() {
        let table = [
            ("WebBrowser", copy::LAUNCHER_ALIAS_WEB_BROWSER),
            ("TerminalEmulator", copy::LAUNCHER_ALIAS_TERMINAL_EMULATOR),
            ("TextEditor", copy::LAUNCHER_ALIAS_TEXT_EDITOR),
            ("FileManager", copy::LAUNCHER_ALIAS_FILE_MANAGER),
            ("Player", copy::LAUNCHER_ALIAS_PLAYER),
            ("IDE", copy::LAUNCHER_ALIAS_IDE),
            ("InstantMessaging", copy::LAUNCHER_ALIAS_INSTANT_MESSAGING),
            ("Monitor", copy::LAUNCHER_ALIAS_MONITOR),
            ("Emulator", copy::LAUNCHER_ALIAS_EMULATOR),
            (
                "AudioVideoEditing",
                copy::LAUNCHER_ALIAS_AUDIO_VIDEO_EDITING,
            ),
            ("RasterGraphics", copy::LAUNCHER_ALIAS_GRAPHICS),
            ("2DGraphics", copy::LAUNCHER_ALIAS_GRAPHICS),
        ];
        for (category, word) in table {
            assert!(aliases(&cats(category)).contains(&word), "{category}");
        }
        assert_eq!(
            aliases(&cats("X-WayDroid-App")),
            [copy::LAUNCHER_GROUP_UTILITIES]
        );
        assert_eq!(aliases(&[]), [copy::LAUNCHER_GROUP_UTILITIES]);
    }
}
