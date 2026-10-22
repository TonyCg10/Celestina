//! The Network page (CEL-2-F): the Wi-Fi switch, one row per network the
//! system knows (active first, then saved, then by signal), and an `Editar…`
//! row that opens the editing program. A row's click asks for the one thing
//! it can do: join a saved network, or leave the active one. Networks the
//! shell may not join (VPNs, enterprise and WEP networks, unsaved ones) are
//! shown and say where they are configured. While NetworkManager is absent
//! the page shows only that, and nothing else is drawn or clickable.

use celestina_paint::{Role, Theme};
use celestina_shell::connectivity::{
    validate, Action, Editor, NetKind, NetRow, NetSection, NetSecurity, NetState,
};
use celestina_ui::{view, Key, View};

use super::{
    note_list,
    rows::{self, apply_status, Glyph, Line, Trailing},
    PageCx, PageView,
};
use crate::{connectivity::state::ConnectivityState, copy, island::state::PAGE_SIZE};

/// The Network page.
#[derive(Clone, Copy, Debug)]
pub struct Network;

/// The Wi-Fi switch's row.
#[must_use]
pub fn wifi_key() -> Key {
    Key::named("net-wifi")
}

/// The `Editar…` row.
#[must_use]
pub fn edit_key() -> Key {
    Key::named("net-edit")
}

/// The row of network `id`.
#[must_use]
pub fn row_key(id: &str) -> Key {
    Key::named(&format!("net:{id}"))
}

fn glyph(row: &NetRow) -> Glyph {
    match row.kind() {
        NetKind::Wifi => Glyph::Bars(row.signal()),
        NetKind::Ethernet => Glyph::Icon("link"),
        NetKind::Vpn => Glyph::Icon("cloud"),
    }
}

fn kind_word(row: &NetRow) -> &'static str {
    match (row.kind(), row.security()) {
        (NetKind::Ethernet, _) => copy::NET_WIRED,
        (NetKind::Vpn, _) => copy::NET_VPN,
        (NetKind::Wifi, NetSecurity::Open) => copy::NET_OPEN,
        (NetKind::Wifi, NetSecurity::Passphrase) => copy::NET_PASSPHRASE,
        (NetKind::Wifi, NetSecurity::Enterprise) => copy::NET_ENTERPRISE,
        (NetKind::Wifi, NetSecurity::Wep) => copy::NET_WEP,
    }
}

/// The one thing a click on `row` asks for.
fn request(row: &NetRow) -> Action {
    if row.state().is_active() {
        Action::Leave(row.id().clone())
    } else {
        Action::JoinSaved(row.id().clone())
    }
}

fn net_line(state: &ConnectivityState, row: &NetRow) -> Line {
    let key = row_key(row.id().as_str());
    let address = row.address().map(|a| a.to_string());
    let asked = request(row);
    let allowed = validate(&asked, &state.inventory()).is_ok();
    let (word, role) = match (row.state(), allowed) {
        (NetState::Connected, _) => (copy::NET_CONNECTED, Role::Accent),
        (NetState::Connecting, _) => (copy::NET_CONNECTING, Role::TextLo),
        (NetState::Disconnected, true) => (copy::NET_SAVED, Role::TextLo),
        (NetState::Disconnected, false) => (copy::NET_CONFIGURE_ELSEWHERE, Role::TextFaint),
    };
    let mut line = Line {
        key,
        tag: format!("net:{}", row.id().as_str()),
        glyph: glyph(row),
        tint: if row.state().is_active() {
            Role::Accent
        } else {
            Role::TextHi
        },
        title: if row.name().is_empty() {
            copy::NET_UNNAMED.to_owned()
        } else {
            row.name().to_owned()
        },
        title_role: if allowed || row.state().is_active() {
            Role::TextHi
        } else {
            Role::TextLo
        },
        detail: Some(copy::net_detail(kind_word(row), address.as_deref())),
        trailing: Trailing::Text(word.to_owned(), role),
        action: allowed.then_some(asked),
        note: None,
    };
    apply_status(&mut line, state.net_status(row.id()));
    line
}

fn wifi_line(state: &ConnectivityState, section: &NetSection) -> Line {
    let on = section.wifi_enabled();
    let blocked = section.wifi_hardware_blocked() && !on;
    let asked = Action::SetWifiEnabled(!on);
    let allowed = validate(&asked, &state.inventory()).is_ok();
    let mut line = Line {
        key: wifi_key(),
        tag: "net-wifi".to_owned(),
        glyph: Glyph::Icon("wifi"),
        tint: if on { Role::Accent } else { Role::TextLo },
        title: copy::NET_WIFI.to_owned(),
        title_role: Role::TextHi,
        detail: blocked.then(|| copy::NET_WIFI_BLOCKED.to_owned()),
        trailing: Trailing::Switch {
            on,
            enabled: allowed,
        },
        action: allowed.then_some(asked),
        note: None,
    };
    apply_status(&mut line, state.wifi_status());
    line
}

fn edit_line(state: &ConnectivityState) -> Line {
    let mut line = Line {
        key: edit_key(),
        tag: "net-edit".to_owned(),
        glyph: Glyph::Icon("pencil"),
        tint: Role::TextHi,
        title: copy::EDIT.to_owned(),
        title_role: Role::TextHi,
        detail: Some(copy::EDIT_NETWORK_DETAIL.to_owned()),
        trailing: Trailing::Nothing,
        action: Some(Action::OpenEditor(Editor::Network)),
        note: None,
    };
    if state.editor_missing(Editor::Network) {
        line.note = Some((copy::EDITOR_NETWORK_MISSING.to_owned(), Role::Danger));
    }
    line
}

/// The lines the page draws for `state` on a surface of `max_size`; `None`
/// while NetworkManager is absent.
fn shown(state: &ConnectivityState, theme: &Theme, max_size: (f32, f32)) -> Option<Vec<Line>> {
    let section = state.network().ready().copied()?;
    let mut body: Vec<Line> = section
        .rows()
        .iter()
        .map(|row| net_line(state, row))
        .collect();
    if body.is_empty() {
        let text = if section.wifi_enabled() {
            copy::NET_EMPTY
        } else {
            copy::NET_WIFI_OFF
        };
        body.push(Line::text("net-empty", text, Role::TextLo));
    }
    Some(rows::fit(
        theme,
        max_size,
        vec![wifi_line(state, section)],
        body,
        vec![edit_line(state)],
    ))
}

/// What a click or Enter on the node `key` asks of the shell: only a row
/// the page draws with an action (and so a request the shell would accept).
#[must_use]
pub fn action_for(
    state: &ConnectivityState,
    theme: &Theme,
    max_size: (f32, f32),
    key: Key,
) -> Option<Action> {
    rows::action_of(&shown(state, theme, max_size)?, key)
}

impl PageView for Network {
    fn title(&self) -> &str {
        copy::PAGE_NETWORK
    }

    fn view(&self, cx: &PageCx<'_>) -> View {
        let content = match shown(cx.connectivity, cx.theme, cx.max_size) {
            Some(lines) => rows::list(cx.theme, Key::named("network-rows"), &lines),
            None => note_list(cx.theme, Key::named("network-rows"), copy::NET_UNAVAILABLE),
        };
        view::col(Key::named("network")).child(content)
    }

    /// The default size, taller when the rows need it, up to the surface.
    fn wants(&self, cx: &PageCx<'_>) -> (f32, f32) {
        match shown(cx.connectivity, cx.theme, cx.max_size) {
            Some(lines) => (PAGE_SIZE.0, PAGE_SIZE.1.max(rows::needed(cx.theme, &lines))),
            None => PAGE_SIZE,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use celestina_shell::connectivity::{Expected, Failure, Resolution, Service};
    use celestina_ui::ViewKind;

    use super::*;
    use crate::{
        connectivity::{
            state::{sample, RowStatus},
            ConnectivityEvent,
        },
        island::pages::{wants_of, NameOwner},
    };

    const BIG: (f32, f32) = (760.0, 560.0);

    fn page(state: &ConnectivityState, max: (f32, f32)) -> View {
        let owners = NameOwner::default();
        Network.view(&PageCx {
            theme: &Theme::dark(),
            max_size: max,
            model: None,
            output: "DP-1",
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,

            connectivity: state,
            phone: &celestina_shell::phone::PhoneState::Absent,
        })
    }

    fn nodes(v: &View) -> Vec<&View> {
        let mut out = Vec::new();
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            out.push(n);
            stack.extend(n.children.iter().rev());
        }
        out
    }

    fn texts(v: &View) -> Vec<String> {
        nodes(v)
            .into_iter()
            .filter_map(|n| match &n.kind {
                ViewKind::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn act(state: &ConnectivityState, key: Key) -> Option<Action> {
        action_for(state, &Theme::dark(), BIG, key)
    }

    #[test]
    fn an_absent_service_shows_its_explanation_and_nothing_to_click() {
        let state = ConnectivityState::default();
        let v = page(&state, BIG);
        assert_eq!(texts(&v), [copy::NET_UNAVAILABLE]);
        assert!(nodes(&v).iter().all(|n| !n.interactive && !n.focusable));
        assert_eq!(act(&state, wifi_key()), None);
        assert_eq!(act(&state, edit_key()), None);
        assert_eq!(wants_of(&Network, None, BIG), PAGE_SIZE);
    }

    #[test]
    fn the_rows_say_their_state_and_each_has_the_one_action_it_can_do() {
        let state = sample::ready();
        let v = page(&state, BIG);
        let said = texts(&v);
        for expected in [
            copy::NET_WIFI,
            "HomeNet",
            copy::NET_CONNECTED,
            "Cafe",
            copy::NET_SAVED,
            "Work",
            "Office VPN",
            copy::NET_CONFIGURE_ELSEWHERE,
            copy::EDIT,
        ] {
            assert!(said.iter().any(|t| t == expected), "{expected}: {said:?}");
        }
        assert!(
            said.iter().any(|t| t.contains("192.168.1.20")),
            "the active link's address shows: {said:?}"
        );
        assert_eq!(
            act(&state, row_key("home")),
            Some(Action::Leave(sample::id("home")))
        );
        assert_eq!(
            act(&state, row_key("cafe")),
            Some(Action::JoinSaved(sample::id("cafe")))
        );
        assert_eq!(act(&state, row_key("work")), None, "enterprise, unsaved");
        assert_eq!(act(&state, row_key("vpn")), None, "never a VPN");
        assert_eq!(act(&state, wifi_key()), Some(Action::SetWifiEnabled(false)));
        assert_eq!(
            act(&state, edit_key()),
            Some(Action::OpenEditor(Editor::Network))
        );
        assert_eq!(act(&state, Key::named("nowhere")), None);
    }

    #[test]
    fn every_action_the_page_offers_passes_the_domains_validation() {
        let state = sample::ready();
        let lines = shown(&state, &Theme::dark(), BIG).expect("lines");
        let offered: Vec<&Action> = lines.iter().filter_map(|l| l.action.as_ref()).collect();
        assert!(offered.len() >= 4);
        for action in offered {
            assert!(validate(action, &state.inventory()).is_ok(), "{action:?}");
        }
    }

    #[test]
    fn keys_are_unique_and_only_actionable_rows_take_focus() {
        let state = sample::ready();
        let v = page(&state, BIG);
        let all = nodes(&v);
        let keys: HashSet<Key> = all.iter().map(|n| n.key).collect();
        assert_eq!(keys.len(), all.len(), "one key per node");
        let focusable: HashSet<Key> = all.iter().filter(|n| n.focusable).map(|n| n.key).collect();
        let want: HashSet<Key> = [wifi_key(), row_key("home"), row_key("cafe"), edit_key()].into();
        assert_eq!(focusable, want);
    }

    #[test]
    fn a_pending_row_shows_it_and_stops_offering_its_action() {
        let mut state = sample::ready();
        assert_eq!(state.begin(&Action::JoinSaved(sample::id("cafe"))), Ok(()));
        assert!(texts(&page(&state, BIG))
            .iter()
            .any(|t| t == copy::APPLYING));
        assert_eq!(act(&state, row_key("cafe")), None);
        assert_eq!(
            act(&state, row_key("home")),
            Some(Action::Leave(sample::id("home"))),
            "other rows are unaffected"
        );
    }

    #[test]
    fn a_failure_is_a_sentence_by_kind_under_its_row_and_the_row_stays_usable() {
        let mut state = sample::ready();
        let expected = Expected::NetActive {
            id: sample::id("cafe"),
            active: true,
        };
        state.apply(&ConnectivityEvent::Resolved(Resolution::Failed(
            expected,
            Failure::NeedsSecret,
        )));
        assert_eq!(
            state.net_status(&sample::id("cafe")),
            RowStatus::Failed(Failure::NeedsSecret)
        );
        let said = texts(&page(&state, BIG));
        assert!(
            said.iter().any(|t| t == copy::FAIL_NEEDS_SECRET),
            "{said:?}"
        );
        assert!(
            said.iter().all(|t| !t.contains("Resolution")),
            "no debug text in the page"
        );
        assert_eq!(
            act(&state, row_key("cafe")),
            Some(Action::JoinSaved(sample::id("cafe")))
        );
    }

    #[test]
    fn an_empty_network_list_says_why() {
        let mut state = ConnectivityState::default();
        let none =
            |on| ConnectivityEvent::Network(Service::Ready(NetSection::new(on, false, Vec::new())));
        state.apply(&none(true));
        assert!(texts(&page(&state, BIG))
            .iter()
            .any(|t| t == copy::NET_EMPTY));
        state.apply(&none(false));
        let said = texts(&page(&state, BIG));
        assert!(said.iter().any(|t| t == copy::NET_WIFI_OFF), "{said:?}");
        assert_eq!(
            act(&state, wifi_key()),
            Some(Action::SetWifiEnabled(true)),
            "the radio can be turned on from here"
        );
    }

    #[test]
    fn a_missing_editor_is_said_under_its_row() {
        let mut state = sample::ready();
        state.set_editor_missing(Editor::Network, true);
        let said = texts(&page(&state, BIG));
        assert!(said.iter().any(|t| t == copy::EDITOR_NETWORK_MISSING));
    }

    #[test]
    fn the_page_never_draws_past_the_surface_or_the_row_bound() {
        let rows: Vec<NetRow> = (0..40)
            .map(|n| {
                sample::net(
                    &format!("n{n}"),
                    NetKind::Wifi,
                    &format!("Net {n}"),
                    NetState::Disconnected,
                    NetSecurity::Open,
                    true,
                )
            })
            .collect();
        let mut state = ConnectivityState::default();
        state.apply(&ConnectivityEvent::Network(Service::Ready(
            NetSection::new(true, false, rows),
        )));
        let theme = Theme::dark();
        for height in [0.0, 120.0, 200.0, 300.0, 420.0, 560.0, 4000.0] {
            let max = (760.0, height);
            let lines = shown(&state, &theme, max).expect("lines");
            let units: usize = lines
                .iter()
                .map(|l| 1 + usize::from(l.note.is_some()))
                .sum();
            assert!(units <= rows::room(&theme, max), "{height}");
            let (_, wanted) = wants_of_state(&state, max);
            assert!(wanted <= height.max(PAGE_SIZE.1), "{height}: {wanted}");
            // The rows drawn are the first ones, in order, and only they
            // can be clicked.
            let network_rows = lines.iter().filter(|l| l.tag.starts_with("net:")).count();
            assert!(network_rows <= celestina_shell::connectivity::MAX_ROWS);
            if network_rows < 24 {
                let hidden = row_key(&format!("n{network_rows}"));
                assert_eq!(action_for(&state, &theme, max, hidden), None);
            }
        }
    }

    fn wants_of_state(state: &ConnectivityState, max: (f32, f32)) -> (f32, f32) {
        Network.wants(&PageCx {
            theme: &Theme::dark(),
            max_size: max,
            model: None,
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
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,

            connectivity: state,
            phone: &celestina_shell::phone::PhoneState::Absent,
        })
    }

    #[test]
    fn a_hostile_name_is_shown_as_a_name_and_nothing_more() {
        let mut state = ConnectivityState::default();
        let row = sample::net(
            "x",
            NetKind::Wifi,
            "\u{202e}Evil %s {0} \n network",
            NetState::Disconnected,
            NetSecurity::Open,
            true,
        );
        state.apply(&ConnectivityEvent::Network(Service::Ready(
            NetSection::new(true, false, [row]),
        )));
        let said = texts(&page(&state, BIG));
        assert!(said.iter().any(|t| t.contains("Evil")), "{said:?}");
        assert!(said
            .iter()
            .all(|t| !t.contains('\u{202e}') && !t.contains('\n')));
    }
}
