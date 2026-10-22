//! The Notifications page (design §5.5): the do-not-disturb switch, then
//! every live notification followed by the last ended ones, each with its
//! icon, summary, two lines of body, arrival time and, while live, a
//! dismiss button and the producer's action buttons; a dismiss-all button
//! tops the list while anything is live. While another program holds the
//! notifications name the page shows only that program's line and the
//! switch stays disabled: nothing here would be served.

use celestina_paint::{Role, Theme, Weight};
use celestina_ui::{view, Key, View, ViewKind};
use taffy::JustifyContent;

use super::{note_list, HistoryRow, NameOwner, PageCx, PageView};
use crate::{
    copy,
    island::{
        state::PAGE_SIZE,
        view::{centred, chrome_height, switch, text_style, BUTTON_H},
        IslandAction,
    },
    notifications::{
        banners::{icon_of, line, DISMISS_SIZE, ICON_SIZE},
        excerpt,
    },
    providers::notifications::NotificationsCommand,
};

/// The width reserved for a row's arrival time, logical px.
const TIME_W: f32 = 72.0;
/// The glyph inside a row's dismiss button.
const DISMISS_GLYPH: f32 = 16.0;
const DISMISS_ICON: &str = "x";

/// The Notifications page.
#[derive(Clone, Copy, Debug)]
pub struct Notifications;

/// The key of the do-not-disturb switch.
#[must_use]
pub fn switch_key() -> Key {
    Key::named("dnd-switch")
}

/// The key of the dismiss-all button.
#[must_use]
pub fn dismiss_all_key() -> Key {
    Key::named("notes-dismiss-all")
}

/// The key of the row of notification `id`.
#[must_use]
pub fn row_key(id: u32) -> Key {
    Key::indexed("note", u64::from(id))
}

/// The key of the dismiss button of notification `id`.
#[must_use]
pub fn dismiss_key(id: u32) -> Key {
    Key::indexed("note-dismiss", u64::from(id))
}

/// The key of the `index`th action button of notification `id`.
#[must_use]
pub fn action_key(id: u32, index: usize) -> Key {
    Key::indexed("note-action", (u64::from(id) << 8) | (index as u64 & 0xff))
}

impl PageView for Notifications {
    fn title(&self) -> &str {
        copy::PAGE_NOTIFICATIONS
    }

    fn view(&self, cx: &PageCx<'_>) -> View {
        let theme = cx.theme;
        let held = cx.owners.notifications.as_deref();
        let rows = match held {
            Some(owner) => note_list(
                theme,
                Key::named("notifications-rows"),
                &copy::notifications_owner(owner),
            ),
            None if cx.history.is_empty() => note_list(
                theme,
                Key::named("notifications-rows"),
                copy::NOTIFICATIONS_EMPTY,
            ),
            None => entries(theme, shown(cx.history, theme, cx.max_size)),
        };
        view::col(Key::named("notifications"))
            .gap(theme.spacing.sm)
            .child(dnd_switch(theme, cx.do_not_disturb, held.is_none()))
            .child(rows)
    }

    /// The default size, taller when the rows need it: the page grows with
    /// them up to the surface.
    fn wants(&self, cx: &PageCx<'_>) -> (f32, f32) {
        if cx.owners.notifications.is_some() {
            return PAGE_SIZE;
        }
        let rows = shown(cx.history, cx.theme, cx.max_size);
        let needed = chrome_height(cx.theme) + switch_block(cx.theme) + list_height(cx.theme, rows);
        (PAGE_SIZE.0, PAGE_SIZE.1.max(needed))
    }
}

/// What a click or Enter on the node `key` asks of the shell, if it is the
/// switch of a page whose name `owners` shows as served, or a button of a
/// row the page draws.
#[must_use]
pub fn action_for(
    history: &[HistoryRow],
    owners: &NameOwner,
    theme: &Theme,
    max_size: (f32, f32),
    key: Key,
) -> Option<IslandAction> {
    if owners.notifications.is_some() {
        return None;
    }
    if key == switch_key() {
        return Some(IslandAction::ToggleDoNotDisturb);
    }
    let rows = shown(history, theme, max_size);
    if key == dismiss_all_key() && rows.first().is_some_and(|r| r.live) {
        return Some(IslandAction::Notification(NotificationsCommand::DismissAll));
    }
    for row in rows.iter().filter(|r| r.live) {
        if key == dismiss_key(row.id) {
            return Some(IslandAction::Notification(NotificationsCommand::Close(
                row.id,
            )));
        }
        if let Some(a) = (0..row.actions.len()).find(|i| key == action_key(row.id, *i)) {
            return Some(IslandAction::Notification(NotificationsCommand::Invoke(
                row.id,
                row.actions[a].key.clone(),
            )));
        }
    }
    None
}

/// The switch row and the gap under it.
fn switch_block(theme: &Theme) -> f32 {
    theme.row_height + theme.spacing.sm
}

/// The width the texts of a row may take.
fn text_width(theme: &Theme) -> f32 {
    PAGE_SIZE.0 - 2.0 * theme.spacing.lg - 2.0 * theme.spacing.md - ICON_SIZE - theme.spacing.md
}

/// One row's height: the header line, the two body lines and the buttons
/// it has, never less than a plain row.
fn entry_height(theme: &Theme, row: &HistoryRow) -> f32 {
    let mut h = line(theme.type_scale.row_title);
    if !row.body.is_empty() {
        h += theme.spacing.xs + 2.0 * line(theme.type_scale.body);
    }
    if !row.actions.is_empty() {
        h += theme.spacing.xs + BUTTON_H;
    }
    (h + 2.0 * theme.spacing.sm).max(theme.row_height)
}

/// The dismiss-all button's row and the gap under it.
fn dismiss_all_block(theme: &Theme) -> f32 {
    BUTTON_H + theme.spacing.xs
}

/// The height of the list of `rows`: the dismiss-all button when the first
/// is live, the rows and the gaps between them.
fn list_height(theme: &Theme, rows: &[HistoryRow]) -> f32 {
    let top = if rows.first().is_some_and(|r| r.live) {
        dismiss_all_block(theme)
    } else {
        0.0
    };
    let heights: f32 = rows.iter().map(|r| entry_height(theme, r)).sum();
    top + heights + rows.len().saturating_sub(1) as f32 * theme.spacing.xs
}

/// The leading rows that fit a page of `max_size`: the rest are not built,
/// so nothing invisible can be focused or clicked.
fn shown<'a>(rows: &'a [HistoryRow], theme: &Theme, max_size: (f32, f32)) -> &'a [HistoryRow] {
    let room = max_size.1 - chrome_height(theme) - switch_block(theme);
    let fit = (1..=rows.len())
        .take_while(|n| list_height(theme, &rows[..*n]) <= room)
        .last()
        .unwrap_or(0);
    &rows[..fit]
}

/// The do-not-disturb switch in the state `on` in force; `enabled` while
/// the shell serves the notifications (a control: focusable, Enter and
/// Space act), else out of the focus order and hit testing, in the faint
/// roles.
fn dnd_switch(theme: &Theme, on: bool, enabled: bool) -> View {
    let track = switch(Key::named("dnd-track"), on, enabled);
    let label_role = if enabled {
        Role::TextHi
    } else {
        Role::TextFaint
    };
    let mut row = centred(view::row(switch_key()))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_md)
        .child(
            view::text(
                Key::named("dnd-label"),
                copy::DO_NOT_DISTURB,
                text_style(theme.type_scale.body, Weight::Regular, false, label_role),
            )
            .grow(),
        )
        .child(track);
    if enabled {
        row = row.interactive().focusable();
    }
    row
}

/// The dismiss-all button over the list of `rows`, then the rows.
fn entries(theme: &Theme, rows: &[HistoryRow]) -> View {
    let all = rows.first().is_some_and(|r| r.live).then(|| {
        let mut bar = centred(view::row(Key::named("notes-bar"))).height(BUTTON_H);
        bar.style.justify_content = JustifyContent::FLEX_END;
        bar.child(button(
            theme,
            dismiss_all_key(),
            Key::named("notes-dismiss-all-label"),
            copy::NOTIFICATIONS_DISMISS_ALL,
        ))
    });
    view::col(Key::named("notifications-rows"))
        .gap(theme.spacing.xs)
        .children(all)
        .children(rows.iter().map(|r| entry(theme, r)))
}

/// A text button of the page.
fn button(theme: &Theme, key: Key, label_key: Key, label: &str) -> View {
    centred(view::row(key))
        .height(BUTTON_H)
        .padding_xy(theme.spacing.md, 0.0)
        .radius(theme.radius_button)
        .fill(Role::ControlFill)
        .interactive()
        .focusable()
        .child(view::text(
            label_key,
            label,
            text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi),
        ))
}

/// One notification's row.
fn entry(theme: &Theme, n: &HistoryRow) -> View {
    let id = u64::from(n.id);
    let text_w = text_width(theme);
    let (icon_key, tint) = icon_of(n.image.as_ref());
    let icon = View::new(
        Key::indexed("note-icon", id),
        ViewKind::Icon {
            key: icon_key,
            tint,
            size: ICON_SIZE,
        },
    )
    .size(ICON_SIZE, ICON_SIZE);
    let (title_role, body_role) = if n.live {
        (Role::TextHi, Role::TextLo)
    } else {
        (Role::TextLo, Role::TextFaint)
    };
    let summary_size = theme.type_scale.row_title;
    let summary_w = text_w - TIME_W - theme.spacing.sm - if n.live { DISMISS_SIZE } else { 0.0 };
    let summary = view::col(Key::indexed("note-summary", id))
        .height(line(summary_size))
        .clip()
        .grow()
        .child(
            view::text(
                Key::indexed("note-summary-text", id),
                &n.summary,
                text_style(summary_size, Weight::Semibold, false, title_role),
            )
            .max_width(summary_w.max(0.0)),
        );
    let when = view::text(
        Key::indexed("note-when", id),
        &n.when,
        text_style(
            theme.type_scale.caption,
            Weight::Regular,
            false,
            Role::TextFaint,
        ),
    )
    .max_width(TIME_W);
    let dismiss = n.live.then(|| {
        let mut b = centred(view::row(dismiss_key(n.id)))
            .size(DISMISS_SIZE, DISMISS_SIZE)
            .radius(theme.radius_sm)
            .interactive()
            .focusable()
            .child(
                view::icon(
                    Key::indexed("note-dismiss-icon", id),
                    DISMISS_ICON,
                    DISMISS_GLYPH,
                )
                .tint(Role::TextLo),
            );
        b.style.justify_content = JustifyContent::CENTER;
        b
    });
    let header = centred(view::row(Key::indexed("note-header", id)))
        .gap(theme.spacing.sm)
        .child(summary)
        .child(when)
        .children(dismiss);
    let body_size = theme.type_scale.body;
    let body = (!n.body.is_empty()).then(|| {
        view::col(Key::indexed("note-body", id))
            .height(2.0 * line(body_size))
            .clip()
            .child(
                view::text(
                    Key::indexed("note-body-text", id),
                    excerpt(&n.body),
                    text_style(body_size, Weight::Regular, false, body_role),
                )
                .max_width(text_w),
            )
    });
    let buttons = (!n.actions.is_empty()).then(|| {
        view::row(Key::indexed("note-buttons", id))
            .gap(theme.spacing.sm)
            .clip()
            .children(n.actions.iter().enumerate().map(|(i, a)| {
                button(
                    theme,
                    action_key(n.id, i),
                    Key::indexed("note-action-label", (id << 8) | (i as u64 & 0xff)),
                    &a.label,
                )
            }))
    });
    let mut content = view::col(Key::indexed("note-content", id))
        .grow()
        .gap(theme.spacing.xs)
        .child(header)
        .children(body)
        .children(buttons);
    content.style.justify_content = JustifyContent::CENTER;
    centred(view::row(row_key(n.id)))
        .height(entry_height(theme, n))
        .padding_xy(theme.spacing.md, theme.spacing.sm)
        .gap(theme.spacing.md)
        .radius(theme.radius_md)
        .child(icon)
        .child(content)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use celestina_ui::{Interaction, Tree, UiEvent};
    use celestina_wayland::{InputEvent, Modifiers};

    use super::*;
    use crate::island::pages::{wants_of, RowAction};

    const MAX: (f32, f32) = (760.0, 560.0);
    const OWNER: &str = ":1.42 (noctalia)";

    fn held() -> NameOwner {
        NameOwner {
            notifications: Some(OWNER.to_owned()),
            tray: None,
        }
    }

    fn page_with(owners: &NameOwner, history: &[HistoryRow], max_size: (f32, f32)) -> View {
        Notifications.view(&PageCx {
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            theme: &celestina_paint::Theme::dark(),
            max_size,
            model: None,
            output: "DP-1",
            owners,
            do_not_disturb: false,
            history,
            items: &[],
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
        })
    }

    fn page_for(owners: &NameOwner) -> View {
        page_with(owners, &[], MAX)
    }

    fn text_of(v: &View) -> Option<&str> {
        match &v.kind {
            ViewKind::Text { text, .. } => Some(text),
            _ => None,
        }
    }

    fn live(id: u32, actions: &[&str]) -> HistoryRow {
        HistoryRow {
            id,
            app_name: "app".to_owned(),
            summary: format!("summary {id}"),
            body: format!("body {id}"),
            live: true,
            when: "12:00".to_owned(),
            actions: actions
                .iter()
                .map(|k| RowAction {
                    key: (*k).to_owned(),
                    label: k.to_uppercase(),
                })
                .collect(),
            ..HistoryRow::default()
        }
    }

    fn ended(id: u32) -> HistoryRow {
        HistoryRow {
            live: false,
            actions: Vec::new(),
            ..live(id, &[])
        }
    }

    /// The first descendant of `v` keyed `key`.
    fn find(v: &View, key: Key) -> Option<&View> {
        if v.key == key {
            return Some(v);
        }
        v.children.iter().find_map(|c| find(c, key))
    }

    #[test]
    fn notifications_page_shows_the_owner_line_while_noctalia_holds_the_name() {
        let owner = "org.freedesktop.Notifications is held by :1.42 (noctalia)";
        let owners = NameOwner {
            notifications: Some(owner.to_owned()),
            tray: None,
        };
        let v = page_for(&owners);
        let rows = &v.children[1];
        assert_eq!(rows.key, Key::named("notifications-rows"));
        assert_eq!(rows.children.len(), 1, "exactly one row");
        let line = text_of(&rows.children[0].children[0]);
        assert_eq!(line, Some(copy::notifications_owner(owner).as_str()));
        assert!(line.is_some_and(|l| l.contains(owner)));
        // The switch is there, off the focus order and the hit test.
        let sw = &v.children[0];
        assert_eq!(sw.key, switch_key());
        assert!(!sw.interactive && !sw.focusable, "disabled");
        assert_eq!(text_of(&sw.children[0]), Some(copy::DO_NOT_DISTURB));
    }

    #[test]
    fn the_owner_line_wins_over_any_rows() {
        let history = [live(1, &["a"]), ended(2)];
        let v = page_with(&held(), &history, MAX);
        let rows = &v.children[1];
        assert_eq!(rows.children.len(), 1, "only the owner line");
        assert!(find(&v, row_key(1)).is_none() && find(&v, dismiss_all_key()).is_none());
        assert!(!v.children[0].interactive && !v.children[0].focusable);
        assert_eq!(wants_in(&held(), &history, (760.0, 2000.0)), PAGE_SIZE);
    }

    fn track_of(page: &View) -> &View {
        &page.children[0].children[1]
    }

    #[test]
    fn the_switch_shows_the_do_not_disturb_setting_in_force_and_is_enabled_only_when_served() {
        let theme = celestina_paint::Theme::dark();
        for owners in [NameOwner::default(), held()] {
            let enabled = owners.notifications.is_none();
            for on in [false, true] {
                let v = Notifications.view(&PageCx {
                    clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
                    theme: &theme,
                    max_size: MAX,
                    model: None,
                    output: "DP-1",
                    owners: &owners,
                    do_not_disturb: on,
                    history: &[],
                    items: &[],
                    sysmon: None,
                    weather: None,
                    today: None,
                    month_offset: 0,
                    utc_offset: 0,
                    connectivity: &crate::connectivity::state::ConnectivityState::default(),
                });
                assert_eq!(
                    *track_of(&v),
                    switch(Key::named("dnd-track"), on, enabled),
                    "on {on}, owners {owners:?}"
                );
                let sw = &v.children[0];
                assert_eq!((sw.interactive, sw.focusable), (enabled, enabled));
            }
        }
    }

    #[test]
    fn notifications_page_without_an_owner_and_nothing_to_show_says_so() {
        let v = page_for(&NameOwner::default());
        let rows = &v.children[1];
        assert_eq!(rows.children.len(), 1);
        assert_eq!(
            text_of(&rows.children[0].children[0]),
            Some(copy::NOTIFICATIONS_EMPTY)
        );
    }

    #[test]
    fn the_page_lists_live_then_ended_rows_with_their_texts() {
        let history = [live(7, &[]), live(6, &[]), ended(5), ended(4)];
        let v = page_with(&NameOwner::default(), &history, MAX);
        let rows = &v.children[1];
        let keys: Vec<Key> = rows.children.iter().map(|c| c.key).collect();
        assert_eq!(
            keys,
            [
                Key::named("notes-bar"),
                row_key(7),
                row_key(6),
                row_key(5),
                row_key(4)
            ]
        );
        let summary =
            |id: u32| find(&v, Key::indexed("note-summary-text", u64::from(id))).and_then(text_of);
        let body =
            |id: u32| find(&v, Key::indexed("note-body-text", u64::from(id))).and_then(text_of);
        let when = |id: u32| find(&v, Key::indexed("note-when", u64::from(id))).and_then(text_of);
        assert_eq!(summary(7), Some("summary 7"));
        assert_eq!(body(5), Some("body 5"));
        assert_eq!(when(4), Some("12:00"));
    }

    #[test]
    fn only_live_rows_have_buttons() {
        let history = [live(2, &["reply", "mute"]), ended(1)];
        let v = page_with(&NameOwner::default(), &history, MAX);
        let button = |key| find(&v, key).filter(|b| b.interactive && b.focusable);
        assert!(button(dismiss_key(2)).is_some());
        assert!(button(action_key(2, 0)).is_some() && button(action_key(2, 1)).is_some());
        assert!(button(action_key(2, 2)).is_none());
        assert!(
            find(&v, dismiss_key(1)).is_none(),
            "ended rows have no dismiss"
        );
        assert!(find(&v, action_key(1, 0)).is_none());
        assert!(button(dismiss_all_key()).is_some());
        let label = find(&v, Key::indexed("note-action-label", (2 << 8) | 1)).and_then(text_of);
        assert_eq!(label, Some("MUTE"));
        // Nothing live: no dismiss-all.
        let v = page_with(&NameOwner::default(), &[ended(1)], MAX);
        assert!(find(&v, dismiss_all_key()).is_none());
        assert!(!find(&v, row_key(1)).is_some_and(|r| r.interactive || r.focusable));
    }

    #[test]
    fn clicks_map_to_the_right_island_action() {
        let theme = celestina_paint::Theme::dark();
        let history = [live(2, &["reply", "mute"]), ended(1)];
        let act = |owners: &NameOwner, key| action_for(&history, owners, &theme, MAX, key);
        let free = NameOwner::default();
        let cmd = |c| Some(IslandAction::Notification(c));
        assert_eq!(
            act(&free, dismiss_key(2)),
            cmd(NotificationsCommand::Close(2))
        );
        assert_eq!(
            act(&free, action_key(2, 1)),
            cmd(NotificationsCommand::Invoke(2, "mute".to_owned()))
        );
        assert_eq!(
            act(&free, dismiss_all_key()),
            cmd(NotificationsCommand::DismissAll)
        );
        assert_eq!(
            act(&free, switch_key()),
            Some(IslandAction::ToggleDoNotDisturb)
        );
        assert_eq!(act(&free, dismiss_key(1)), None, "ended");
        assert_eq!(act(&free, action_key(2, 2)), None, "no third button");
        assert_eq!(act(&free, row_key(2)), None);
        // While another program holds the name nothing acts.
        for key in [dismiss_key(2), dismiss_all_key(), switch_key()] {
            assert_eq!(act(&held(), key), None);
        }
    }

    /// 7 px per char, 16 px tall.
    fn fake_measure(s: &str, _: &celestina_paint::TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
    }

    fn key_event(keysym: u32) -> InputEvent {
        InputEvent::Key {
            keysym,
            utf8: None,
            modifiers: Modifiers::default(),
            pressed: true,
            serial: 1,
        }
    }

    #[test]
    fn the_switch_takes_focus_and_enter_or_space_activate_it() {
        const TAB: u32 = 0xff09;
        const RETURN: u32 = 0xff0d;
        const SPACE: u32 = 0x20;
        let mut tree = Tree::new(false, &celestina_paint::Theme::dark());
        tree.apply(page_with(&NameOwner::default(), &[], MAX), Instant::now());
        tree.layout_measured(MAX, fake_measure);
        let mut input = Interaction::new();
        assert!(input.key(&mut tree, &key_event(TAB)).is_empty());
        for sym in [RETURN, SPACE] {
            assert_eq!(
                input.key(&mut tree, &key_event(sym)),
                vec![UiEvent::Activate(switch_key())]
            );
        }
        // Disabled, it never takes focus.
        let mut tree = Tree::new(false, &celestina_paint::Theme::dark());
        tree.apply(page_with(&held(), &[], MAX), Instant::now());
        tree.layout_measured(MAX, fake_measure);
        let mut input = Interaction::new();
        input.key(&mut tree, &key_event(TAB));
        assert!(input.key(&mut tree, &key_event(RETURN)).is_empty());
    }

    fn wants_in(owners: &NameOwner, history: &[HistoryRow], max_size: (f32, f32)) -> (f32, f32) {
        Notifications.wants(&PageCx {
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            theme: &celestina_paint::Theme::dark(),
            max_size,
            model: None,
            output: "DP-1",
            owners,
            do_not_disturb: false,
            history,
            items: &[],
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
        })
    }

    #[test]
    fn the_page_asks_for_the_default_size_and_grows_with_its_rows() {
        assert_eq!(wants_of(&Notifications, None, MAX), PAGE_SIZE);
        assert_eq!(Notifications.title(), copy::PAGE_NOTIFICATIONS);
        let theme = celestina_paint::Theme::dark();
        let rows: Vec<HistoryRow> = (1..=8).map(|i| live(i, &["a"])).collect();
        let big = (760.0, 3000.0);
        let (w, h) = wants_in(&NameOwner::default(), &rows, big);
        assert_eq!(w, PAGE_SIZE.0);
        assert_eq!(
            h,
            chrome_height(&theme) + switch_block(&theme) + list_height(&theme, &rows)
        );
        assert!(h > PAGE_SIZE.1);
        // A page with one ended row stays at the default.
        let few = wants_in(&NameOwner::default(), &[ended(1)], big);
        assert_eq!(few, PAGE_SIZE);
    }

    #[test]
    fn rows_past_the_surface_are_neither_built_nor_acted_on() {
        let theme = celestina_paint::Theme::dark();
        let rows: Vec<HistoryRow> = (1..=30).map(|i| live(i, &["a"])).collect();
        let max = (760.0, 400.0);
        let v = page_with(&NameOwner::default(), &rows, max);
        let built = v.children[1].children.len() - 1; // minus the bar
        assert!(built > 0 && built < 30, "built {built}");
        let last = u32::try_from(built).unwrap_or(0);
        assert!(find(&v, row_key(last)).is_some());
        assert!(find(&v, row_key(last + 1)).is_none());
        let act = |id| action_for(&rows, &NameOwner::default(), &theme, max, dismiss_key(id));
        assert!(act(last).is_some());
        assert_eq!(act(last + 1), None);
        assert!(wants_in(&NameOwner::default(), &rows, max).1 <= max.1);
    }
}
