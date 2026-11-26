//! The subtitle row at the top of every sheet body (CEL-D-008): the open
//! piece's icon and text, which show with the body and leave with the
//! window (the piece itself crossfades in the bar, CEL-D-010). The row is
//! built from the piece's [`Face`], so the text is the string the piece's bar
//! view shows; a piece without text gives the menu's name instead.

use celestina_paint::{Role, Theme, Weight};
use celestina_ui::{view, Key, View};

use crate::{
    island::view::{centred, text_style},
    pieces::Face,
};

/// The subtitle's height, logical px.
pub const SUBTITLE_H: f32 = 40.0;
/// The icon's size.
pub const ICON: f32 = 22.0;
/// The space between the icon and the text.
const GAP: f32 = 10.0;

/// The row.
#[must_use]
pub fn row_key() -> Key {
    Key::named("sheet-subtitle")
}

/// The icon.
#[must_use]
pub fn icon_key() -> Key {
    Key::named("sheet-subtitle-icon")
}

/// The text.
#[must_use]
pub fn text_key() -> Key {
    Key::named("sheet-subtitle-text")
}

/// The subtitle of `face`: its icon at 22 px, then its text (or `fallback`,
/// the menu's name, when the piece has none) at `row_title`, semibold, and
/// the `trailing` view (a page sheet's overflow button) at the right.
#[must_use]
pub fn subtitle(theme: &Theme, face: &Face, fallback: &str, trailing: Option<View>) -> View {
    let text = face.text.as_deref().unwrap_or(fallback);
    centred(view::row(row_key()))
        .height(SUBTITLE_H)
        .padding_xy(theme.spacing.xs, 0.0)
        .gap(GAP)
        .child(view::icon(icon_key(), face.icon, ICON).tint(Role::TextHi))
        .child(
            view::text(
                text_key(),
                text,
                text_style(
                    theme.type_scale.row_title,
                    Weight::Semibold,
                    face.text.is_some(),
                    Role::TextHi,
                ),
            )
            .grow(),
        )
        .children(trailing)
}

#[cfg(test)]
mod tests {
    use celestina_paint::{Rect, TextStyle};
    use celestina_ui::{Tree, ViewKind};

    use super::*;
    use crate::copy;

    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (s.chars().count() as f32 * 7.0, 16.0)
    }

    fn laid_out(v: View, w: f32) -> Tree {
        let mut tree = Tree::new(true, &Theme::dark());
        let root = view::col(Key::named("root")).size(w, SUBTITLE_H).child(v);
        tree.apply(root, std::time::Instant::now());
        tree.layout_measured((w, SUBTITLE_H), fake_measure);
        tree
    }

    fn rect_of(tree: &Tree, key: Key) -> Rect {
        tree.rect(tree.node_by_key(key).expect("node"))
    }

    fn find(v: &View, key: Key) -> Option<&View> {
        if v.key == key {
            return Some(v);
        }
        v.children.iter().find_map(|c| find(c, key))
    }

    #[test]
    fn the_subtitle_is_40_high_with_a_22_px_icon_and_semibold_row_title_text() {
        let theme = Theme::dark();
        let face = Face {
            icon: "sun",
            text: Some(copy::osd_percent(20)),
        };
        let v = subtitle(&theme, &face, copy::PIECE_BRIGHTNESS, None);
        let text = find(&v, text_key()).expect("text");
        let ViewKind::Text {
            text: shown, style, ..
        } = &text.kind
        else {
            panic!("the text node is text");
        };
        assert_eq!(shown, &copy::osd_percent(20));
        assert_eq!(style.size, theme.type_scale.row_title);
        assert_eq!(style.size, 15.0);
        assert_eq!(style.weight, Weight::Semibold);
        assert_eq!(style.role, Role::TextHi);
        let tree = laid_out(v, 320.0);
        let row = rect_of(&tree, row_key());
        assert_eq!((row.x, row.y, row.w, row.h), (0.0, 0.0, 320.0, SUBTITLE_H));
        let icon = rect_of(&tree, icon_key());
        assert_eq!((icon.w, icon.h), (22.0, 22.0));
        assert_eq!(icon.x, theme.spacing.xs, "padding x 4");
        assert_eq!(icon.y, (SUBTITLE_H - 22.0) / 2.0, "centred");
        let text = rect_of(&tree, text_key());
        assert_eq!(text.x, icon.x + icon.w + 10.0, "gap 10");
        assert_eq!(text.x + text.w, 320.0 - theme.spacing.xs, "growing");
    }

    /// The text and icon of the subtitle `v` holds.
    fn shown(v: &View) -> (String, &'static str) {
        let text = find(v, text_key()).expect("text");
        let ViewKind::Text { text, .. } = &text.kind else {
            panic!("the text node is text");
        };
        let icon = find(v, icon_key()).expect("icon");
        let ViewKind::Icon {
            key: celestina_paint::IconKey::Semantic(name),
            ..
        } = &icon.kind
        else {
            panic!("the icon node is a semantic icon");
        };
        (text.clone(), name)
    }

    #[test]
    fn a_piece_without_text_shows_the_menu_name() {
        let theme = Theme::dark();
        let session = crate::session_menu::view(&theme, crate::session_menu::Mode::List, 0.0);
        assert_eq!(shown(&session), (copy::PIECE_SESSION.to_owned(), "power"));
        let ws = crate::sheet::workspaces::workspaces_view(&theme, &[], &[]);
        assert_eq!(
            shown(&ws),
            (copy::WORKSPACES_FALLBACK.to_owned(), "view-grid")
        );
        // With text, the text wins over the name.
        let face = Face {
            icon: "gauge",
            text: Some("CPU 23% · RAM 41%".to_owned()),
        };
        assert_eq!(
            shown(&subtitle(&theme, &face, "Sistema", None)),
            ("CPU 23% · RAM 41%".to_owned(), "gauge")
        );
    }

    #[test]
    fn a_trailing_view_sits_at_the_right_edge() {
        let theme = Theme::dark();
        let face = Face {
            icon: "link",
            text: None,
        };
        let more = view::row(Key::named("probe")).size(40.0, 28.0);
        let tree = laid_out(subtitle(&theme, &face, "Red", Some(more)), 420.0);
        let probe = rect_of(&tree, Key::named("probe"));
        assert_eq!(probe.x + probe.w, 420.0 - theme.spacing.xs);
        let text = rect_of(&tree, text_key());
        assert!(text.x + text.w <= probe.x);
    }
}
