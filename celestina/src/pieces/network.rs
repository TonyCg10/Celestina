//! The network piece: Wi-Fi signal bars (or a wired or VPN glyph) for the
//! link that is up, dimmed when nothing is. A click opens the Network page.
//! Absent while NetworkManager is.

use celestina_paint::Role;
use celestina_shell::connectivity::{NetKind, NetSection, NetState};
use celestina_ui::{Key, View};

use super::{
    conn_glyph::{self, ICON},
    BarCx, Gesture, PageEntry, PageId, Piece, PieceAction, PieceId, PieceInput,
};
use crate::{
    copy,
    island::pages::{network::Network as Page, rows::signal_bars},
};

/// The network piece.
#[derive(Clone, Copy, Debug)]
pub struct Network;

fn node_key() -> Key {
    Key::named("network")
}

/// What the piece shows of the section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Glance {
    Wifi(u8),
    Ethernet,
    Vpn,
    Connecting,
    Idle,
    WifiOff,
}

/// The link that stands for the connection: the first active Wi-Fi or wired
/// row (the section lists the active ones first), else an active VPN, else
/// one being activated.
fn glance_of(section: &NetSection) -> Glance {
    let up = |row: &&celestina_shell::connectivity::NetRow| row.state() == NetState::Connected;
    if let Some(row) = section
        .rows()
        .iter()
        .filter(up)
        .find(|row| row.kind() != NetKind::Vpn)
    {
        return match row.kind() {
            NetKind::Wifi => Glance::Wifi(row.signal()),
            _ => Glance::Ethernet,
        };
    }
    if section.rows().iter().any(|row| up(&row)) {
        return Glance::Vpn;
    }
    if section
        .rows()
        .iter()
        .any(|row| row.state() == NetState::Connecting)
    {
        return Glance::Connecting;
    }
    if section.wifi_enabled() {
        Glance::Idle
    } else {
        Glance::WifiOff
    }
}

impl Piece for Network {
    fn id(&self) -> PieceId {
        PieceId("network")
    }

    fn name(&self) -> &'static str {
        copy::PAGE_NETWORK
    }

    fn bar(&self, cx: &BarCx<'_>) -> Option<View> {
        let section = cx.connectivity.network().ready().copied()?;
        let glyph = Key::named("network-glyph");
        let child = match glance_of(section) {
            Glance::Wifi(signal) => conn_glyph::boxed(
                glyph,
                signal_bars("network", signal, ICON * 0.7, Role::TextHi, Role::TextFaint),
            ),
            Glance::Ethernet => conn_glyph::icon(glyph, "link", Role::TextHi),
            Glance::Vpn => conn_glyph::icon(glyph, "cloud", Role::TextHi),
            Glance::Connecting => conn_glyph::icon(glyph, "wifi", Role::Accent),
            Glance::Idle => conn_glyph::icon(glyph, "wifi", Role::TextFaint),
            Glance::WifiOff => conn_glyph::icon(glyph, "unplug", Role::TextFaint),
        };
        Some(conn_glyph::tap(cx.theme, node_key(), [child]))
    }

    fn page(&self) -> Option<PageEntry> {
        Some(PageEntry {
            id: PageId::NETWORK,
            view: &Page,
        })
    }

    /// Opens the page; the wheel and the other buttons do nothing.
    fn activate(&self, input: &PieceInput<'_>) -> PieceAction {
        match (input.key == node_key(), input.gesture) {
            (true, Gesture::Primary) => PieceAction::OpenPage(PageId::NETWORK),
            _ => PieceAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_shell::connectivity::{NetSecurity, Service};
    use celestina_ui::ViewKind;

    use super::*;
    use crate::{
        connectivity::{
            state::{sample, ConnectivityState},
            ConnectivityEvent,
        },
        pieces::{tests_support::with_state, Axis},
    };

    fn state_with(section: NetSection) -> ConnectivityState {
        let mut state = ConnectivityState::default();
        state.apply(&ConnectivityEvent::Network(Service::Ready(section)));
        state
    }

    fn row(kind: NetKind, state: NetState, id: &str) -> celestina_shell::connectivity::NetRow {
        sample::net(id, kind, id, state, NetSecurity::Open, true)
    }

    fn keys(v: &View) -> Vec<Key> {
        let mut out = Vec::new();
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            out.push(n.key);
            stack.extend(n.children.iter().rev());
        }
        out
    }

    fn icon_name(v: &View) -> Option<&'static str> {
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            if let ViewKind::Icon {
                key: celestina_paint::IconKey::Semantic(name),
                ..
            } = &n.kind
            {
                return Some(name);
            }
            stack.extend(n.children.iter());
        }
        None
    }

    #[test]
    fn the_piece_is_absent_until_networkmanager_answers() {
        let state = ConnectivityState::default();
        with_state(&state, |cx| {
            assert!(Network.bar(cx).is_none());
        });
        let gone = {
            let mut s = state_with(sample::network());
            s.apply(&ConnectivityEvent::Network(Service::Unavailable));
            s
        };
        with_state(&gone, |cx| assert!(Network.bar(cx).is_none()));
    }

    #[test]
    fn each_state_has_its_own_glyph() {
        let cases = [
            (
                NetSection::new(true, false, [row(NetKind::Wifi, NetState::Connected, "w")]),
                Glance::Wifi(70),
            ),
            (
                NetSection::new(
                    true,
                    false,
                    [row(NetKind::Ethernet, NetState::Connected, "e")],
                ),
                Glance::Ethernet,
            ),
            (
                NetSection::new(true, false, [row(NetKind::Vpn, NetState::Connected, "v")]),
                Glance::Vpn,
            ),
            (
                NetSection::new(true, false, [row(NetKind::Wifi, NetState::Connecting, "c")]),
                Glance::Connecting,
            ),
            (
                NetSection::new(
                    true,
                    false,
                    [row(NetKind::Wifi, NetState::Disconnected, "d")],
                ),
                Glance::Idle,
            ),
            (NetSection::new(false, false, Vec::new()), Glance::WifiOff),
        ];
        let mut shapes = Vec::new();
        for (section, glance) in cases {
            assert_eq!(glance_of(&section), glance);
            let state = state_with(section);
            with_state(&state, |cx| {
                let v = Network.bar(cx).expect("piece");
                assert!(v.interactive && v.focusable);
                assert_eq!(v.key, Key::named("network"));
                shapes.push((icon_name(&v), keys(&v).len()));
            });
        }
        assert_eq!(shapes[1].0, Some("link"));
        assert_eq!(shapes[2].0, Some("cloud"));
        assert_eq!(shapes[5].0, Some("unplug"));
    }

    #[test]
    fn a_wired_link_wins_over_a_vpn() {
        let section = NetSection::new(
            true,
            false,
            [
                row(NetKind::Vpn, NetState::Connected, "VPNNAME"),
                row(NetKind::Ethernet, NetState::Connected, "WIREDNAME"),
            ],
        );
        assert_eq!(glance_of(&section), Glance::Ethernet);
    }

    #[test]
    fn keys_are_unique() {
        let state = state_with(sample::network());
        with_state(&state, |cx| {
            let ks = keys(&Network.bar(cx).expect("piece"));
            let set: std::collections::HashSet<Key> = ks.iter().copied().collect();
            assert_eq!(set.len(), ks.len());
        });
    }

    #[test]
    fn a_primary_input_opens_the_page_and_nothing_else_does_anything() {
        let state = state_with(sample::network());
        with_state(&state, |cx| {
            let act = |key, gesture| Network.activate(&PieceInput { key, gesture, cx });
            assert_eq!(
                act(node_key(), Gesture::Primary),
                PieceAction::OpenPage(PageId::NETWORK)
            );
            for gesture in [
                Gesture::Middle,
                Gesture::Context,
                Gesture::Scroll(Axis::Vertical, 2),
            ] {
                assert_eq!(act(node_key(), gesture), PieceAction::None);
            }
            assert_eq!(
                act(Key::named("other"), Gesture::Primary),
                PieceAction::None
            );
        });
    }

    #[test]
    fn the_piece_opens_the_network_page() {
        assert_eq!(Network.page().map(|e| e.id), Some(PageId::NETWORK));
        assert_eq!(Network.id().as_str(), "network");
    }
}
