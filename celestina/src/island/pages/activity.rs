//! The Activity page (design §5.5): this output's workspaces as rows (name
//! or index, column count) with the windows of its active workspace
//! underneath (the `.desktop` icon of the application, the title cut at
//! [`MAX_TITLE_GRAPHEMES`]). A click asks the compositor to focus; the page
//! changes only when the compositor's event says it did.

use celestina_paint::{IconKey, Role, Theme, Weight};
use celestina_ui::{view, Key, View, ViewKind};
use unicode_segmentation::UnicodeSegmentation;

use super::{PageCx, PageView};
use crate::{
    copy,
    island::{
        state::PAGE_SIZE,
        view::{centred, chrome_height, text_style, DOT},
        workspaces::{rows_for, Row},
        IslandAction,
    },
    providers::niri::{NiriModel, Window},
};

/// The longest window title the page shows, in extended grapheme clusters,
/// the ellipsis included.
pub const MAX_TITLE_GRAPHEMES: usize = 48;
/// The most rows (workspaces and windows together) the page draws, however
/// tall the surface: past this the page is a wall of rows nobody scans.
pub const MAX_ROWS: usize = 24;
/// The line between the workspaces and the windows.
const DIVIDER_H: f32 = 1.0;
/// A window row's application icon.
pub const ICON_SIZE: f32 = 24.0;
/// The fallback icon of a window without a `.desktop` icon.
const FALLBACK_ICON: &str = "app-window";

/// The Activity page.
#[derive(Clone, Copy, Debug)]
pub struct Activity;

impl PageView for Activity {
    fn title(&self) -> &str {
        copy::PAGE_ACTIVITY
    }

    fn view(&self, cx: &PageCx<'_>) -> View {
        let theme = cx.theme;
        let list = view::col(Key::named("activity"));
        let Some(shown) = cx.model.and_then(|m| {
            let shown = visible(m, cx.output, theme, cx.max_size);
            (!shown.rows.is_empty()).then_some(shown)
        }) else {
            return list.child(view::text(
                Key::named("activity-empty"),
                copy::ACTIVITY_EMPTY,
                text_style(theme.type_scale.body, Weight::Regular, false, Role::TextLo),
            ));
        };
        let divider = (!shown.windows.is_empty()).then(|| {
            view::row(Key::named("windows-divider"))
                .height(DIVIDER_H)
                .fill(Role::Divider)
        });
        list.children(shown.rows.iter().map(|r| workspace_row(theme, r)))
            .children(divider)
            .children(shown.windows.into_iter().map(|w| window_row(theme, w)))
    }

    /// The default size, taller when the rows need it: the page grows with
    /// them up to the surface.
    fn wants(&self, cx: &PageCx<'_>) -> (f32, f32) {
        let Some(shown) = cx
            .model
            .map(|m| visible(m, cx.output, cx.theme, cx.max_size))
        else {
            return PAGE_SIZE;
        };
        let rows = shown.rows.len() + shown.windows.len();
        let divider = if shown.windows.is_empty() {
            0.0
        } else {
            DIVIDER_H
        };
        let needed = chrome_height(cx.theme) + rows as f32 * cx.theme.row_height + divider;
        (PAGE_SIZE.0, PAGE_SIZE.1.max(needed))
    }
}

/// The rows of one page: what is drawn, and so all that can take focus.
struct Visible<'a> {
    rows: Vec<Row>,
    windows: Vec<&'a Window>,
}

/// The rows of `output`'s page that are drawn: workspaces first, then the
/// active workspace's windows, at most [`MAX_ROWS`] together and at most
/// what fits in a page of `max_size` (the surface's limit). The rest is
/// not built, so nothing invisible can be focused or clicked.
fn visible<'a>(
    model: &'a NiriModel,
    output: &str,
    theme: &Theme,
    max_size: (f32, f32),
) -> Visible<'a> {
    let room = (max_size.1 - chrome_height(theme) - DIVIDER_H).max(0.0);
    let fit = (room / theme.row_height).floor() as usize;
    let cap = fit.min(MAX_ROWS);
    let mut rows = rows_for(model, output);
    rows.truncate(cap);
    let mut windows = active_windows(model, &rows_for(model, output));
    windows.truncate(cap - rows.len());
    Visible { rows, windows }
}

/// The row of workspace `id`.
#[must_use]
pub fn workspace_key(id: u64) -> Key {
    Key::indexed("ws", id)
}

/// The row of window `id`.
#[must_use]
pub fn window_key(id: u64) -> Key {
    Key::indexed("win", id)
}

/// What a click or Enter on the row `key` asks of the compositor, if it is
/// one of the rows `model` gives `output`'s page.
#[must_use]
pub fn action_for(
    model: &NiriModel,
    output: &str,
    theme: &Theme,
    max_size: (f32, f32),
    key: Key,
) -> Option<IslandAction> {
    let shown = visible(model, output, theme, max_size);
    if let Some(row) = shown.rows.iter().find(|r| workspace_key(r.id) == key) {
        return Some(IslandAction::FocusWorkspace(row.id));
    }
    shown
        .windows
        .into_iter()
        .find(|w| window_key(w.id) == key)
        .map(|w| IslandAction::FocusWindow(w.id))
}

/// The windows of the active workspace among `rows`, in the model's order.
fn active_windows<'a>(model: &'a NiriModel, rows: &[Row]) -> Vec<&'a Window> {
    let Some(active) = rows.iter().find(|r| r.active) else {
        return Vec::new();
    };
    model
        .windows
        .iter()
        .filter(|w| w.workspace_id == Some(active.id))
        .collect()
}

/// `title` as the page shows it: whole up to [`MAX_TITLE_GRAPHEMES`]
/// grapheme clusters, else its first `MAX_TITLE_GRAPHEMES - 1` and an
/// ellipsis, so the result never has more than the maximum.
#[must_use]
pub fn truncate_title(title: &str) -> String {
    let kept: Vec<&str> = title
        .graphemes(true)
        .take(MAX_TITLE_GRAPHEMES + 1)
        .collect();
    if kept.len() <= MAX_TITLE_GRAPHEMES {
        return title.to_owned();
    }
    let mut cut = kept[..MAX_TITLE_GRAPHEMES - 1].concat();
    cut.push('\u{2026}');
    cut
}

fn workspace_row(theme: &Theme, row: &Row) -> View {
    let mut v = centred(view::row(workspace_key(row.id)))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_md)
        .interactive()
        .focusable();
    if row.active {
        v = v.fill(Role::ControlFill);
    }
    let dot = if row.active {
        Role::Accent
    } else {
        Role::TextLo
    };
    v.child(
        view::row(Key::indexed("ws-dot", row.id))
            .fill(dot)
            .radius(DOT / 2.0)
            .size(DOT, DOT),
    )
    .child(
        view::text(
            Key::indexed("ws-name", row.id),
            &row.text,
            text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi),
        )
        .grow(),
    )
    .child(view::text(
        Key::indexed("ws-columns", row.id),
        &copy::column_count(row.columns),
        text_style(theme.type_scale.body, Weight::Regular, false, Role::TextLo),
    ))
}

fn window_row(theme: &Theme, window: &Window) -> View {
    let title = window
        .title
        .as_deref()
        .filter(|t| !t.is_empty())
        .or(window.app_id.as_deref())
        .map_or_else(|| copy::WINDOW_UNTITLED.to_owned(), truncate_title);
    let (key, tint) = match &window.icon {
        Some(name) => (IconKey::Theme(name.clone()), None),
        None => (IconKey::Semantic(FALLBACK_ICON), Some(Role::TextLo)),
    };
    let icon = View::new(
        Key::indexed("win-icon", window.id),
        ViewKind::Icon {
            key,
            tint,
            size: ICON_SIZE,
        },
    )
    .size(ICON_SIZE, ICON_SIZE);
    let mut v = centred(view::row(window_key(window.id)))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_md)
        .interactive()
        .focusable();
    if window.is_focused {
        v = v.fill(Role::ControlFill);
    }
    v.child(icon).child(
        view::text(
            Key::indexed("win-title", window.id),
            &title,
            text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi),
        )
        .grow(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::island::pages::wants_of;
    use crate::island::pages::NameOwner;
    use crate::providers::niri::Workspace;

    fn ws(
        id: u64,
        idx: u8,
        name: Option<&str>,
        output: &str,
        active: bool,
        columns: u16,
    ) -> Workspace {
        Workspace {
            id,
            idx,
            name: name.map(str::to_owned),
            output: Some(output.to_owned()),
            is_active: active,
            is_focused: active,
            columns,
        }
    }

    fn win(id: u64, title: &str, app_id: &str, workspace: u64, icon: Option<&str>) -> Window {
        Window {
            id,
            title: Some(title.to_owned()),
            app_id: Some(app_id.to_owned()),
            workspace_id: Some(workspace),
            is_focused: false,
            icon: icon.map(str::to_owned),
        }
    }

    /// Two outputs; DP-1's active workspace 11 holds windows 100 and 101,
    /// its other workspace 12 holds 102, and HDMI-A-1's 31 holds 103.
    fn model() -> NiriModel {
        NiriModel {
            workspaces: vec![
                ws(11, 1, None, "DP-1", true, 2),
                ws(12, 2, Some("web"), "DP-1", false, 1),
                ws(31, 1, None, "HDMI-A-1", true, 0),
            ],
            windows: vec![
                win(100, "main.rs", "kitty", 11, Some("kitty")),
                win(101, "Docs", "org.example.nothing", 11, None),
                win(102, "Hidden", "kitty", 12, Some("kitty")),
                win(103, "Elsewhere", "kitty", 31, Some("kitty")),
            ],
            outputs: vec!["DP-1".to_owned(), "HDMI-A-1".to_owned()],
        }
    }

    fn view_for(model: Option<&NiriModel>, output: &str) -> View {
        let owners = NameOwner::default();
        Activity.view(&PageCx {
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            theme: &celestina_paint::Theme::dark(),
            max_size: (760.0, 560.0),
            model,
            output,
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
            media: &crate::island::pages::media::NO_MEDIA,
        })
    }

    fn keys(v: &View) -> Vec<Key> {
        v.children.iter().map(|c| c.key).collect()
    }

    #[test]
    fn activity_rows_list_the_outputs_workspaces_then_the_active_workspaces_windows() {
        let m = model();
        let v = view_for(Some(&m), "DP-1");
        assert_eq!(
            keys(&v),
            [
                workspace_key(11),
                workspace_key(12),
                Key::named("windows-divider"),
                window_key(100),
                window_key(101),
            ]
        );
        // The other output's page has its own workspace and windows.
        let other = view_for(Some(&m), "HDMI-A-1");
        assert_eq!(
            keys(&other),
            [
                workspace_key(31),
                Key::named("windows-divider"),
                window_key(103)
            ]
        );
        // The workspace rows say their name or index and their columns.
        let texts = |key: Key| -> Vec<String> {
            v.children
                .iter()
                .find(|c| c.key == key)
                .map(|row| {
                    row.children
                        .iter()
                        .filter_map(|c| match &c.kind {
                            ViewKind::Text { text, .. } => Some(text.clone()),
                            _ => None,
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        assert_eq!(texts(workspace_key(11)), ["Espacio 1", "2 columnas"]);
        assert_eq!(texts(workspace_key(12)), ["web", "1 columna"]);
        assert_eq!(texts(window_key(100)), ["main.rs"]);
    }

    #[test]
    fn window_rows_carry_the_desktop_icon_or_the_window_fallback() {
        let m = model();
        let v = view_for(Some(&m), "DP-1");
        let icon_of = |id: u64| -> Option<IconKey> {
            v.children
                .iter()
                .find(|c| c.key == window_key(id))?
                .children
                .iter()
                .find_map(|c| match &c.kind {
                    ViewKind::Icon { key, .. } => Some(key.clone()),
                    _ => None,
                })
        };
        assert_eq!(icon_of(100), Some(IconKey::Theme("kitty".to_owned())));
        assert_eq!(icon_of(101), Some(IconKey::Semantic("app-window")));
    }

    #[test]
    fn without_a_model_or_workspaces_the_page_says_so() {
        for v in [view_for(None, "DP-1"), view_for(Some(&model()), "eDP-1")] {
            assert_eq!(keys(&v), [Key::named("activity-empty")]);
        }
    }

    #[test]
    fn clicks_focus_what_the_row_names_and_nothing_else() {
        let m = model();
        let theme = Theme::dark();
        let act = |key| action_for(&m, "DP-1", &theme, (760.0, 560.0), key);
        assert_eq!(
            act(workspace_key(12)),
            Some(IslandAction::FocusWorkspace(12))
        );
        assert_eq!(act(window_key(101)), Some(IslandAction::FocusWindow(101)));
        assert_eq!(act(window_key(102)), None, "not on the active workspace");
        assert_eq!(act(window_key(103)), None, "another output's");
        assert_eq!(act(workspace_key(31)), None);
        assert_eq!(act(Key::named("title")), None);
    }

    #[test]
    fn titles_are_cut_at_48_graphemes_with_an_ellipsis() {
        let short = "a".repeat(48);
        assert_eq!(truncate_title(&short), short);
        let long = "b".repeat(49);
        let cut = truncate_title(&long);
        assert_eq!(cut.graphemes(true).count(), 48);
        assert_eq!(cut, format!("{}\u{2026}", "b".repeat(47)));
        // A cluster is never split: 60 flags are 120 chars but 60 graphemes.
        let flags = "\u{1f1ea}\u{1f1f8}".repeat(60);
        let cut = truncate_title(&flags);
        assert_eq!(cut.graphemes(true).count(), 48);
        assert!(cut.ends_with('\u{2026}'));
        assert_eq!(cut.chars().count(), 47 * 2 + 1);
        // Combining marks stay with their letter.
        let accents = "e\u{301}".repeat(49);
        assert_eq!(truncate_title(&accents).chars().count(), 47 * 2 + 1);
        assert_eq!(truncate_title(""), "");
    }

    #[test]
    fn a_window_without_a_title_shows_its_app_id_then_the_untitled_label() {
        let theme = Theme::dark();
        let mut w = win(1, "", "kitty", 1, None);
        let text = |w: &Window| match &window_row(&theme, w).children[1].kind {
            ViewKind::Text { text, .. } => text.clone(),
            _ => String::new(),
        };
        assert_eq!(text(&w), "kitty");
        w.app_id = None;
        w.title = None;
        assert_eq!(text(&w), copy::WINDOW_UNTITLED);
    }

    #[test]
    fn the_page_asks_for_the_default_size() {
        assert_eq!(wants_of(&Activity, None, (760.0, 560.0)), PAGE_SIZE);
    }

    /// DP-1 with two workspaces and `n` windows on the active one.
    fn crowded(n: u64) -> NiriModel {
        let mut m = model();
        m.windows = (0..n)
            .map(|i| win(1000 + i, "w", "kitty", 11, None))
            .collect();
        m
    }

    fn view_in(m: &NiriModel, theme: &Theme, max_size: (f32, f32)) -> View {
        Activity.view(&PageCx {
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            theme,
            max_size,
            model: Some(m),
            output: "DP-1",
            owners: &NameOwner::default(),
            do_not_disturb: false,
            history: &[],
            items: &[],
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
            media: &crate::island::pages::media::NO_MEDIA,
        })
    }

    fn wants_in(m: &NiriModel, theme: &Theme, max_size: (f32, f32)) -> (f32, f32) {
        Activity.wants(&PageCx {
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            theme,
            max_size,
            model: Some(m),
            output: "DP-1",
            owners: &NameOwner::default(),
            do_not_disturb: false,
            history: &[],
            items: &[],
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
            media: &crate::island::pages::media::NO_MEDIA,
        })
    }

    #[test]
    fn the_page_grows_with_its_rows_up_to_the_surface() {
        let theme = Theme::dark();
        let chrome = chrome_height(&theme);
        let big = (760.0, 1000.0);
        // Few rows: the default height.
        assert_eq!(wants_in(&crowded(1), &theme, big), PAGE_SIZE);
        // 10 windows + 2 workspaces + the divider need more than 300.
        let h = wants_in(&crowded(10), &theme, big).1;
        assert_eq!(h, chrome + 12.0 * theme.row_height + DIVIDER_H);
        assert!(h > PAGE_SIZE.1);
        // The request keeps growing until the row cap, never past it.
        let h = wants_in(&crowded(100), &theme, (760.0, 2000.0)).1;
        assert_eq!(h, chrome + MAX_ROWS as f32 * theme.row_height + DIVIDER_H);
    }

    #[test]
    fn rows_that_do_not_fit_the_surface_are_not_built_or_focusable() {
        let theme = Theme::dark();
        let m = crowded(30);
        // A surface that fits 6 rows (and the divider) after the chrome.
        let max = (
            760.0,
            chrome_height(&theme) + DIVIDER_H + 6.0 * theme.row_height + 3.0,
        );
        let v = view_in(&m, &theme, max);
        let rows = v.children.len() - 1; // minus the divider
        assert_eq!(rows, 6, "two workspaces and four windows");
        assert_eq!(keys(&v)[0], workspace_key(11));
        assert_eq!(keys(&v)[1], workspace_key(12));
        assert_eq!(keys(&v).last(), Some(&window_key(1003)));
        assert!(wants_in(&m, &theme, max).1 <= max.1, "fits the surface");
        let act = |id| action_for(&m, "DP-1", &theme, max, window_key(id));
        assert_eq!(act(1003), Some(IslandAction::FocusWindow(1003)));
        assert_eq!(act(1004), None, "cut rows cannot be activated");
        // Workspaces win the room over windows.
        let tiny = (
            760.0,
            chrome_height(&theme) + DIVIDER_H + 1.0 * theme.row_height,
        );
        let v = view_in(&m, &theme, tiny);
        assert_eq!(keys(&v), [workspace_key(11)]);
        // And no surface lets the cap through.
        let v = view_in(&crowded(100), &theme, (760.0, 5000.0));
        assert_eq!(v.children.len(), MAX_ROWS + 1);
    }

    #[test]
    fn the_page_draws_with_the_theme_it_is_given() {
        let dark = Theme::dark();
        let mut other = Theme::dark();
        other.row_height = 52.0;
        let m = model();
        let height_of = |theme: &Theme| {
            view_in(&m, theme, (760.0, 560.0)).children[0]
                .style
                .size
                .height
        };
        assert_ne!(height_of(&dark), height_of(&other));
    }
}
