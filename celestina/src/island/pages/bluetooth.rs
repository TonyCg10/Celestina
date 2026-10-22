//! The Bluetooth page (CEL-2-F): the adapter switch, one row per device
//! BlueZ lists (connected first, then paired), and an `Editar…` row that
//! opens the editing program. A paired device's click connects or
//! disconnects it; a device that is not paired is shown and says where it is
//! paired, because the shell never pairs, trusts or removes. The shell never
//! starts a discovery either: it only says when one is running.

use celestina_paint::{Role, Theme};
use celestina_shell::connectivity::{validate, Action, BtIcon, BtRow, BtSection, Editor};
use celestina_ui::{view, Key, View};

use super::{
    note_list,
    rows::{self, apply_status, Glyph, Line, Trailing},
    PageCx, PageView,
};
use crate::{connectivity::state::ConnectivityState, copy, island::state::PAGE_SIZE};

/// The Bluetooth page.
#[derive(Clone, Copy, Debug)]
pub struct Bluetooth;

/// The adapter switch's row.
#[must_use]
pub fn power_key() -> Key {
    Key::named("bt-power")
}

/// The `Editar…` row.
#[must_use]
pub fn edit_key() -> Key {
    Key::named("bt-edit")
}

/// The row of the device at `address`.
#[must_use]
pub fn row_key(address: &str) -> Key {
    Key::named(&format!("bt:{address}"))
}

fn icon(row: &BtRow) -> &'static str {
    match row.icon() {
        BtIcon::Audio => "music",
        BtIcon::Phone => "phone",
        BtIcon::Computer => "monitor",
        BtIcon::Keyboard | BtIcon::Pointer | BtIcon::Other => "bluetooth",
    }
}

/// The one thing a click on `row` asks for.
fn request(row: &BtRow) -> Action {
    if row.connected() {
        Action::DisconnectDevice(row.address().clone())
    } else {
        Action::ConnectDevice(row.address().clone())
    }
}

fn device_line(state: &ConnectivityState, row: &BtRow) -> Line {
    let asked = request(row);
    let allowed = validate(&asked, &state.inventory()).is_ok();
    let (word, role) = match (row.connected(), row.paired()) {
        (true, _) => (copy::BT_CONNECTED, Role::Accent),
        (false, true) => (copy::BT_PAIRED, Role::TextLo),
        (false, false) => (copy::BT_PAIR_ELSEWHERE, Role::TextFaint),
    };
    let mut line = Line {
        key: row_key(row.address().as_str()),
        tag: format!("bt:{}", row.address().as_str()),
        glyph: Glyph::Icon(icon(row)),
        tint: if row.connected() {
            Role::Accent
        } else {
            Role::TextHi
        },
        title: if row.alias().is_empty() {
            copy::BT_UNNAMED.to_owned()
        } else {
            row.alias().to_owned()
        },
        title_role: if row.paired() {
            Role::TextHi
        } else {
            Role::TextLo
        },
        detail: row
            .battery()
            .filter(|_| row.connected())
            .map(copy::battery_percent),
        trailing: Trailing::Text(word.to_owned(), role),
        action: allowed.then_some(asked),
        note: None,
    };
    apply_status(&mut line, state.device_status(row.address()));
    line
}

fn power_line(state: &ConnectivityState, section: &BtSection) -> Line {
    let on = section.powered();
    let asked = Action::SetBluetoothPowered(!on);
    let allowed = validate(&asked, &state.inventory()).is_ok();
    let mut line = Line {
        key: power_key(),
        tag: "bt-power".to_owned(),
        glyph: Glyph::Icon("bluetooth"),
        tint: if on { Role::Accent } else { Role::TextLo },
        title: copy::BT_SWITCH.to_owned(),
        title_role: Role::TextHi,
        detail: (on && section.discovering()).then(|| copy::BT_SEARCHING.to_owned()),
        trailing: Trailing::Switch {
            on,
            enabled: allowed,
        },
        action: allowed.then_some(asked),
        note: None,
    };
    apply_status(&mut line, state.powered_status());
    line
}

fn edit_line(state: &ConnectivityState) -> Line {
    let mut line = Line {
        key: edit_key(),
        tag: "bt-edit".to_owned(),
        glyph: Glyph::Icon("pencil"),
        tint: Role::TextHi,
        title: copy::EDIT.to_owned(),
        title_role: Role::TextHi,
        detail: Some(copy::EDIT_BLUETOOTH_DETAIL.to_owned()),
        trailing: Trailing::Nothing,
        action: Some(Action::OpenEditor(Editor::Bluetooth)),
        note: None,
    };
    if state.editor_missing(Editor::Bluetooth) {
        line.note = Some((copy::EDITOR_BLUETOOTH_MISSING.to_owned(), Role::Danger));
    }
    line
}

/// The lines the page draws for `state` on a surface of `max_size`; `None`
/// while BlueZ or its adapter is absent.
fn shown(state: &ConnectivityState, theme: &Theme, max_size: (f32, f32)) -> Option<Vec<Line>> {
    let section = state.bluetooth().ready().copied()?;
    let mut body: Vec<Line> = section
        .rows()
        .iter()
        .map(|row| device_line(state, row))
        .collect();
    if body.is_empty() {
        let text = if section.powered() {
            copy::BT_EMPTY
        } else {
            copy::BT_OFF
        };
        body.push(Line::text("bt-empty", text, Role::TextLo));
    }
    Some(rows::fit(
        theme,
        max_size,
        vec![power_line(state, section)],
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

impl PageView for Bluetooth {
    fn title(&self) -> &str {
        copy::PAGE_BLUETOOTH
    }

    fn view(&self, cx: &PageCx<'_>) -> View {
        let content = match shown(cx.connectivity, cx.theme, cx.max_size) {
            Some(lines) => rows::list(cx.theme, Key::named("bluetooth-rows"), &lines),
            None => note_list(cx.theme, Key::named("bluetooth-rows"), copy::BT_UNAVAILABLE),
        };
        view::col(Key::named("bluetooth")).child(content)
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
        connectivity::{state::sample, ConnectivityEvent},
        island::pages::NameOwner,
    };

    const BIG: (f32, f32) = (760.0, 560.0);
    const HEADSET: &str = "AA:BB:CC:DD:EE:01";
    const KEYBOARD: &str = "AA:BB:CC:DD:EE:02";
    const STRANGER: &str = "AA:BB:CC:DD:EE:03";

    fn cx<'a>(
        state: &'a ConnectivityState,
        owners: &'a NameOwner,
        max: (f32, f32),
        theme: &'a Theme,
    ) -> PageCx<'a> {
        PageCx {
            theme,
            max_size: max,
            model: None,
            output: "DP-1",
            owners,
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
        }
    }

    fn page(state: &ConnectivityState, max: (f32, f32)) -> View {
        let (owners, theme) = (NameOwner::default(), Theme::dark());
        Bluetooth.view(&cx(state, &owners, max, &theme))
    }

    fn wants(state: &ConnectivityState, max: (f32, f32)) -> (f32, f32) {
        let (owners, theme) = (NameOwner::default(), Theme::dark());
        Bluetooth.wants(&cx(state, &owners, max, &theme))
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

    fn sections(powered: bool, rows: Vec<BtRow>) -> ConnectivityEvent {
        ConnectivityEvent::Bluetooth(Service::Ready(BtSection::new(powered, false, rows)))
    }

    #[test]
    fn an_absent_adapter_shows_its_explanation_and_nothing_to_click() {
        let state = ConnectivityState::default();
        let v = page(&state, BIG);
        assert_eq!(texts(&v), [copy::BT_UNAVAILABLE]);
        assert!(nodes(&v).iter().all(|n| !n.interactive && !n.focusable));
        assert_eq!(act(&state, power_key()), None);
        assert_eq!(wants(&state, BIG), PAGE_SIZE);
    }

    #[test]
    fn devices_say_their_state_and_paired_ones_connect_or_disconnect() {
        let state = sample::ready();
        let said = texts(&page(&state, BIG));
        for expected in [
            copy::BT_SWITCH,
            "Headset",
            copy::BT_CONNECTED,
            &copy::battery_percent(80),
            "Keyboard",
            copy::BT_PAIRED,
            "Stranger",
            copy::BT_PAIR_ELSEWHERE,
            copy::EDIT,
        ] {
            assert!(said.iter().any(|t| t == expected), "{expected}: {said:?}");
        }
        assert_eq!(
            act(&state, row_key(HEADSET)),
            Some(Action::DisconnectDevice(sample::address(HEADSET)))
        );
        assert_eq!(
            act(&state, row_key(KEYBOARD)),
            Some(Action::ConnectDevice(sample::address(KEYBOARD)))
        );
        assert_eq!(act(&state, row_key(STRANGER)), None, "never pairs");
        assert_eq!(
            act(&state, power_key()),
            Some(Action::SetBluetoothPowered(false))
        );
        assert_eq!(
            act(&state, edit_key()),
            Some(Action::OpenEditor(Editor::Bluetooth))
        );
    }

    #[test]
    fn keys_are_unique_and_only_actionable_rows_take_focus() {
        let state = sample::ready();
        let v = page(&state, BIG);
        let all = nodes(&v);
        let keys: HashSet<Key> = all.iter().map(|n| n.key).collect();
        assert_eq!(keys.len(), all.len());
        let focusable: HashSet<Key> = all.iter().filter(|n| n.focusable).map(|n| n.key).collect();
        let want: HashSet<Key> =
            [power_key(), row_key(HEADSET), row_key(KEYBOARD), edit_key()].into();
        assert_eq!(focusable, want);
    }

    #[test]
    fn every_action_the_page_offers_passes_the_domains_validation() {
        let state = sample::ready();
        let lines = shown(&state, &Theme::dark(), BIG).expect("lines");
        for action in lines.iter().filter_map(|l| l.action.as_ref()) {
            assert!(validate(action, &state.inventory()).is_ok(), "{action:?}");
        }
    }

    #[test]
    fn pending_and_failure_show_on_their_own_row() {
        let mut state = sample::ready();
        assert_eq!(
            state.begin(&Action::ConnectDevice(sample::address(KEYBOARD))),
            Ok(())
        );
        assert!(texts(&page(&state, BIG))
            .iter()
            .any(|t| t == copy::APPLYING));
        assert_eq!(act(&state, row_key(KEYBOARD)), None);
        assert!(act(&state, row_key(HEADSET)).is_some());
        state.apply(&ConnectivityEvent::Resolved(Resolution::Failed(
            Expected::BtConnected {
                address: sample::address(KEYBOARD),
                connected: true,
            },
            Failure::NotFound,
        )));
        let said = texts(&page(&state, BIG));
        assert!(said.iter().any(|t| t == copy::FAIL_NOT_FOUND), "{said:?}");
        assert!(act(&state, row_key(KEYBOARD)).is_some());
    }

    #[test]
    fn an_off_adapter_and_an_empty_list_say_why() {
        let mut state = ConnectivityState::default();
        state.apply(&sections(false, Vec::new()));
        let said = texts(&page(&state, BIG));
        assert!(said.iter().any(|t| t == copy::BT_OFF), "{said:?}");
        assert_eq!(
            act(&state, power_key()),
            Some(Action::SetBluetoothPowered(true))
        );
        state.apply(&sections(true, Vec::new()));
        assert!(texts(&page(&state, BIG))
            .iter()
            .any(|t| t == copy::BT_EMPTY));
    }

    #[test]
    fn discovery_is_only_reported() {
        let mut state = ConnectivityState::default();
        state.apply(&ConnectivityEvent::Bluetooth(Service::Ready(
            BtSection::new(true, true, Vec::new()),
        )));
        let said = texts(&page(&state, BIG));
        assert!(said.iter().any(|t| t == copy::BT_SEARCHING), "{said:?}");
    }

    #[test]
    fn the_page_never_draws_past_the_surface_or_the_row_bound() {
        let rows: Vec<BtRow> = (0..40)
            .filter_map(|n| {
                BtRow::new(
                    &format!("AA:BB:CC:DD:{:02X}:{:02X}", n / 256, n % 256),
                    &format!("Device {n}"),
                    BtIcon::Other,
                    true,
                    false,
                    true,
                    None,
                )
            })
            .collect();
        let mut state = ConnectivityState::default();
        state.apply(&sections(true, rows));
        let theme = Theme::dark();
        for height in [0.0, 120.0, 200.0, 300.0, 420.0, 560.0, 4000.0] {
            let max = (760.0, height);
            let lines = shown(&state, &theme, max).expect("lines");
            let units: usize = lines
                .iter()
                .map(|l| 1 + usize::from(l.note.is_some()))
                .sum();
            assert!(units <= rows::room(&theme, max), "{height}");
            let (_, wanted) = wants(&state, max);
            assert!(wanted <= height.max(PAGE_SIZE.1), "{height}: {wanted}");
            let devices = lines.iter().filter(|l| l.tag.starts_with("bt:")).count();
            assert!(devices <= celestina_shell::connectivity::MAX_ROWS);
        }
    }
}
