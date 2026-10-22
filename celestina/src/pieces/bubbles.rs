//! The Bubbles piece: one bubble per window Melibea keeps minimised. It has
//! no page of its own. A click on a bubble restores that window; the
//! "+N" bubble that stands for the windows that do not fit opens the
//! selector, where every window is reachable (the selector and the verbs
//! `bubbles select` and `bubbles minimize` are the keyboard path).
//! With no windows, or with Melibea absent, refused or lost, it draws
//! nothing.

use celestina_paint::{IconKey, Role, Weight};
use celestina_shell::melibea::Window;
use celestina_ui::{view, Key, View, ViewKind};
use unicode_segmentation::UnicodeSegmentation;

use super::{BarCx, Gesture, PageEntry, Piece, PieceAction, PieceId, PieceInput};
use crate::{
    copy,
    island::{
        pages::activity::truncate_title,
        view::{centred, text_style},
    },
};

/// The most bubbles drawn; the rest are the "+N" bubble.
pub const MAX_VISIBLE: usize = 5;
/// A bubble's diameter, logical px.
const BUBBLE: f32 = 28.0;
/// A bubble's icon, logical px.
const ICON: f32 = 20.0;
/// The largest number the overflow bubble spells out.
const MAX_MORE: usize = 99;

/// The Bubbles piece.
#[derive(Clone, Copy, Debug)]
pub struct Bubbles;

/// The bubble of window `id`.
#[must_use]
pub fn bubble_key(id: u64) -> Key {
    Key::indexed("bubble", id)
}

/// The overflow bubble.
#[must_use]
pub fn more_key() -> Key {
    Key::named("bubbles-more")
}

/// The windows that get a bubble of their own.
fn shown<'a>(cx: &BarCx<'a>) -> &'a [Window] {
    let windows = &cx.bubbles.windows;
    &windows[..windows.len().min(MAX_VISIBLE)]
}

/// How many windows the overflow bubble stands for.
fn hidden(cx: &BarCx<'_>) -> usize {
    cx.bubbles.windows.len().saturating_sub(MAX_VISIBLE)
}

/// The first visible character of the window's title (its application id
/// when it has none), upper-cased, for a bubble without an icon; "?" when
/// there is nothing printable.
fn initial(window: &Window) -> String {
    [window.title.as_deref(), window.app_id.as_deref()]
        .into_iter()
        .flatten()
        .find_map(|text| {
            text.graphemes(true)
                .find(|g| g.chars().any(|c| !c.is_whitespace() && !c.is_control()))
        })
        .map_or_else(|| "?".to_owned(), str::to_uppercase)
}

/// The window's title as the bubble names it, bounded.
fn title(window: &Window) -> String {
    truncate_title(
        window
            .title
            .as_deref()
            .filter(|t| !t.is_empty())
            .or(window.app_id.as_deref())
            .unwrap_or(""),
    )
}

fn disc(key: Key) -> View {
    let mut v = centred(view::row(key))
        .size(BUBBLE, BUBBLE)
        .radius(BUBBLE / 2.0)
        .fill(Role::ControlFill)
        .interactive()
        .focusable();
    v.style.justify_content = taffy::JustifyContent::CENTER;
    v
}

fn bubble(cx: &BarCx<'_>, window: &Window) -> View {
    let theme = cx.theme;
    let face = match window.icon_name.as_deref().filter(|n| !n.is_empty()) {
        Some(name) => View::new(
            Key::indexed("bubble-icon", window.id),
            ViewKind::Icon {
                key: IconKey::Theme(name.to_owned()),
                tint: None,
                size: ICON,
            },
        )
        .size(ICON, ICON),
        None => view::text(
            Key::indexed("bubble-initial", window.id),
            &initial(window),
            text_style(theme.type_scale.body, Weight::Semibold, false, Role::TextHi),
        ),
    };
    disc(bubble_key(window.id)).child(face)
}

impl Bubbles {
    /// The accessible name of the bubble `key` of the widget built from
    /// `cx`: what restoring that window does; `None` for any other node.
    #[must_use]
    pub fn label(cx: &BarCx<'_>, key: Key) -> Option<String> {
        if key == more_key() && hidden(cx) > 0 {
            return Some(copy::bubbles_more(hidden(cx)));
        }
        shown(cx)
            .iter()
            .find(|w| bubble_key(w.id) == key)
            .map(|w| copy::bubble_restore(&title(w)))
    }
}

impl Piece for Bubbles {
    fn id(&self) -> PieceId {
        PieceId("bubbles")
    }

    fn name(&self) -> &'static str {
        copy::PIECE_BUBBLES
    }

    /// One bubble per minimised window, at most [`MAX_VISIBLE`], and a
    /// "+N" bubble for the rest. Nothing without windows.
    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        if !cx.bubbles.has_windows() {
            return None;
        }
        let more = (hidden(cx) > 0).then(|| {
            let n = hidden(cx).min(MAX_MORE);
            disc(more_key()).child(view::text(
                Key::named("bubbles-more-count"),
                &format!("+{n}"),
                text_style(
                    cx.theme.type_scale.caption,
                    Weight::Semibold,
                    true,
                    Role::TextHi,
                ),
            ))
        });
        Some(
            centred(view::row(Key::named("bubbles")))
                .gap(cx.theme.spacing.sm)
                .children(shown(cx).iter().map(|w| bubble(cx, w)))
                .children(more),
        )
    }

    fn page(&self) -> Option<PageEntry> {
        None
    }

    /// A click restores the window of the bubble, or opens the selector
    /// from the overflow bubble; the other buttons and the wheel do
    /// nothing.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        if input.gesture != Gesture::Primary {
            return PieceAction::None;
        }
        if input.key == more_key() {
            return if hidden(input.cx) > 0 {
                PieceAction::OpenSelector
            } else {
                PieceAction::None
            };
        }
        shown(input.cx)
            .iter()
            .find(|w| bubble_key(w.id) == input.key)
            .map_or(PieceAction::None, |w| PieceAction::RestoreWindow(w.id))
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::Theme;

    use super::*;
    use crate::{
        bubbles::{Bubbles as State, Link},
        island::pages::NameOwner,
        pieces::{piece, Axis},
    };

    fn window(id: u64, title: Option<&str>, app_id: Option<&str>, icon: Option<&str>) -> Window {
        Window {
            id,
            app_id: app_id.map(str::to_owned),
            title: title.map(str::to_owned),
            icon_name: icon.map(str::to_owned),
        }
    }

    fn state(n: u64, link: Link) -> State {
        State {
            windows: (1..=n)
                .map(|id| window(id, Some(&format!("w{id}")), Some("app"), None))
                .collect(),
            link,
        }
    }

    fn with<R>(bubbles: &State, f: impl FnOnce(&BarCx<'_>) -> R) -> R {
        let theme = Theme::dark();
        let owners = NameOwner::default();
        f(&BarCx {
            theme: &theme,
            output: "DP-1",
            model: None,
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            tray_hidden: &[],
            phone: &celestina_shell::phone::PhoneState::Absent,
            bubbles,
            sysmon: None,
            weather: None,
            today: None,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            levels: crate::pieces::Levels::default(),
        })
    }

    fn built(bubbles: &State) -> Option<View> {
        with(bubbles, |cx| Bubbles.bar(cx))
    }

    #[test]
    fn nothing_without_windows_or_without_a_live_link() {
        assert!(built(&state(0, Link::Live)).is_none());
        for link in [Link::Absent, Link::Refused, Link::Unavailable] {
            assert!(built(&state(3, link)).is_none(), "{link:?}");
        }
        assert!(built(&State::default()).is_none());
    }

    #[test]
    fn one_bubble_per_window_up_to_the_cap() {
        for n in 1..=5u64 {
            let v = built(&state(n, Link::Live)).expect("bubbles");
            let keys: Vec<Key> = v.children.iter().map(|c| c.key).collect();
            let want: Vec<Key> = (1..=n).map(bubble_key).collect();
            assert_eq!(keys, want, "{n} windows");
            assert!(v.children.iter().all(|c| c.interactive && c.focusable));
        }
    }

    #[test]
    fn past_the_cap_an_overflow_bubble_counts_the_rest() {
        let v = built(&state(6, Link::Live)).expect("bubbles");
        assert_eq!(v.children.len(), MAX_VISIBLE + 1);
        let more = v.children.last().expect("more");
        assert_eq!(more.key, more_key());
        match &more.children[0].kind {
            ViewKind::Text { text, .. } => assert_eq!(text, "+1"),
            other => panic!("{other:?}"),
        }
        let v = built(&state(40, Link::Live)).expect("bubbles");
        assert_eq!(v.children.len(), MAX_VISIBLE + 1);
        let v = built(&state(1000, Link::Live)).expect("bubbles");
        match &v.children.last().expect("more").children[0].kind {
            ViewKind::Text { text, .. } => assert_eq!(text, "+99"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_icon_comes_from_the_icon_name_else_the_initial() {
        let s = State {
            windows: vec![
                window(1, Some("Terminal"), Some("kitty"), Some("kitty-icon")),
                window(2, Some("firefox page"), Some("firefox"), None),
                window(3, None, Some("code"), None),
                window(4, Some("   "), None, None),
                window(5, Some("\u{1f600} party"), None, None),
            ],
            link: Link::Live,
        };
        let v = built(&s).expect("bubbles");
        let face = |i: usize| &v.children[i].children[0];
        assert!(
            matches!(&face(0).kind, ViewKind::Icon { key: IconKey::Theme(n), .. } if n == "kitty-icon")
        );
        let letters: Vec<String> = (1..5)
            .map(|i| match &face(i).kind {
                ViewKind::Text { text, .. } => text.clone(),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(letters, ["F", "C", "?", "\u{1f600}"]);
    }

    #[test]
    fn hostile_titles_stay_bounded() {
        let s = State {
            windows: vec![window(1, Some(&"\u{1f600}".repeat(10_000)), None, None)],
            link: Link::Live,
        };
        let label = with(&s, |cx| Bubbles::label(cx, bubble_key(1))).expect("label");
        assert!(label.chars().count() < 100, "{}", label.chars().count());
        assert!(label.starts_with("Restaurar "));
        let untitled = State {
            windows: vec![window(2, None, None, None)],
            link: Link::Live,
        };
        assert_eq!(
            with(&untitled, |cx| Bubbles::label(cx, bubble_key(2))),
            Some(copy::bubble_restore(copy::WINDOW_UNTITLED))
        );
    }

    fn act(s: &State, key: Key, gesture: Gesture) -> PieceAction {
        with(s, |cx| Bubbles.activate(&PieceInput { key, gesture, cx }))
    }

    #[test]
    fn a_click_restores_the_window_and_the_overflow_opens_the_selector() {
        let s = state(7, Link::Live);
        assert_eq!(
            act(&s, bubble_key(3), Gesture::Primary),
            PieceAction::RestoreWindow(3)
        );
        assert_eq!(
            act(&s, more_key(), Gesture::Primary),
            PieceAction::OpenSelector
        );
        // A window that has no bubble (the sixth) is not restorable here.
        assert_eq!(act(&s, bubble_key(6), Gesture::Primary), PieceAction::None);
        assert_eq!(act(&s, bubble_key(99), Gesture::Primary), PieceAction::None);
        // No overflow bubble below the cap.
        let few = state(5, Link::Live);
        assert_eq!(act(&few, more_key(), Gesture::Primary), PieceAction::None);
        for gesture in [
            Gesture::Middle,
            Gesture::Context,
            Gesture::Scroll(Axis::Vertical, 1),
            Gesture::Scroll(Axis::Horizontal, 1),
        ] {
            assert_eq!(act(&s, bubble_key(3), gesture), PieceAction::None);
            assert_eq!(act(&s, more_key(), gesture), PieceAction::None);
        }
    }

    #[test]
    fn the_piece_has_no_page_and_the_table_holds_it() {
        assert!(Bubbles.page().is_none());
        assert!(piece("bubbles").is_some());
        assert!(!Bubbles.name().is_empty());
    }
}
