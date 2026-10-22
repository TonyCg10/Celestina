//! The zone capsules as views: one [`View`] per non-empty zone, built from
//! the pieces table and the layout's zones (spec §6.4).
//!
//! A zone lists piece ids. An id the table does not have is ignored and
//! logged once per layout generation; an id listed twice (in any zone) keeps
//! its first place; a zone keeps at most [`MAX_ZONE_IDS`] ids; a piece with
//! nothing to show is absent, and a zone whose pieces are all absent has no
//! capsule. The centre zone is split at the literal id [`ISLAND_MARK`]: ids
//! before it flank the Island on the left, ids after it (or all of them,
//! with no mark) on the right.

use std::collections::{hash_map::Entry, HashMap, HashSet};

use celestina_paint::Theme;
use celestina_shell::settings::{BarLayout, MAX_ZONE_IDS};
use celestina_ui::{view, Key, View};

use crate::{
    island::{state::CAPSULE_H, view::CAPSULE_PADDING},
    pieces::{self, BarCx},
};

/// The id in the centre zone that stands for the Island.
pub const ISLAND_MARK: &str = "island";

/// The zone capsules, each `None` when it has nothing to show, and which
/// piece drew each node in them.
#[derive(Debug, Default)]
pub struct ZoneViews {
    pub start: Option<View>,
    pub centre_left: Option<View>,
    pub centre_right: Option<View>,
    pub end: Option<View>,
    /// Every key of a piece's view, with the id of the piece that drew it;
    /// a key two pieces draw is the first one's.
    owners: HashMap<Key, String>,
}

impl ZoneViews {
    /// The id of the piece that drew the node `key`: the one piece input
    /// on that node goes to.
    #[must_use]
    pub fn owner(&self, key: Key) -> Option<&str> {
        self.owners.get(&key).map(String::as_str)
    }
}

/// Records `view` and every node under it as `id`'s; a key already owned
/// stays with its first piece, and that is logged once per build.
fn own(owners: &mut HashMap<Key, String>, view: &View, id: &str, clashed: &mut bool) {
    match owners.entry(view.key) {
        Entry::Vacant(slot) => {
            slot.insert(id.to_owned());
        }
        Entry::Occupied(first) if first.get() != id && !*clashed => {
            *clashed = true;
            tracing::warn!(first = %first.get(), second = %id, "bar: two pieces draw the same node key; the first one gets its input");
        }
        Entry::Occupied(_) => {}
    }
    for child in &view.children {
        own(owners, child, id, clashed);
    }
}

/// Builds the zones' views for one layout, remembering which unknown ids it
/// has already logged so each is reported once per layout generation.
#[derive(Debug, Default)]
pub struct Zones {
    unknown_logged: HashSet<String>,
}

impl Zones {
    #[must_use]
    pub fn new() -> Zones {
        Zones::default()
    }

    /// A new layout is in force (the file changed): unknown ids are reported
    /// again.
    pub fn new_generation(&mut self) {
        self.unknown_logged.clear();
    }

    /// The zones of `layout` as views, from the pieces table.
    pub fn build(&mut self, layout: &BarLayout, cx: &BarCx<'_>) -> ZoneViews {
        self.build_with(layout, cx, &|id| pieces::piece(id).map(|p| p.bar(cx)))
    }

    /// [`Zones::build`] over any resolver: `None` for an unknown id,
    /// `Some(None)` for a piece with nothing to show.
    fn build_with(
        &mut self,
        layout: &BarLayout,
        cx: &BarCx<'_>,
        resolve: &dyn Fn(&str) -> Option<Option<View>>,
    ) -> ZoneViews {
        let mut seen: HashSet<String> = HashSet::new();
        let mut owners = HashMap::new();
        let mut clashed = false;
        let (centre_left, centre_right) = split_centre(&layout.centre);
        let mut zone = |name: &str, ids: &[String]| -> Option<View> {
            let mut children = Vec::new();
            for id in ids.iter().take(MAX_ZONE_IDS) {
                if !seen.insert(id.clone()) {
                    continue;
                }
                match resolve(id) {
                    Some(Some(piece)) => {
                        own(&mut owners, &piece, id, &mut clashed);
                        children.push(piece);
                    }
                    Some(None) => {}
                    None => {
                        if self.unknown_logged.insert(id.clone()) {
                            tracing::warn!(id = %id.escape_debug(), "bar: ignoring a piece the shell does not have");
                        }
                    }
                }
            }
            capsule(cx.theme, name, children)
        };
        let (start, centre_left, centre_right, end) = (
            zone("start", &layout.start),
            zone("centre-left", centre_left),
            zone("centre-right", centre_right),
            zone("end", &layout.end),
        );
        ZoneViews {
            start,
            centre_left,
            centre_right,
            end,
            owners,
        }
    }
}

/// The centre zone's ids either side of the Island: before the mark, after
/// it. With no mark they are all on the right. The mark itself is in neither
/// half, and a second one is ignored.
#[must_use]
pub fn split_centre(centre: &[String]) -> (&[String], &[String]) {
    match centre.iter().position(|id| id == ISLAND_MARK) {
        Some(i) => (&centre[..i], &centre[i + 1..]),
        None => (&[], centre),
    }
}

/// The capsule of zone `name` around its pieces, or `None` with no piece.
/// Unplaced: the Island sets the rect from `bar::layout`.
fn capsule(theme: &Theme, name: &str, pieces: Vec<View>) -> Option<View> {
    if pieces.is_empty() {
        return None;
    }
    Some(
        view::row(zone_key(zone_index(name)))
            .height(CAPSULE_H)
            .padding_xy(CAPSULE_PADDING, 0.0)
            .gap(theme.spacing.sm)
            .haze(theme.radius_lg)
            .children(pieces),
    )
}

/// The key of zone capsule `index` (0 start, 1 centre left, 2 centre right,
/// 3 end): what the Island finds the capsules by.
#[must_use]
pub fn zone_key(index: u64) -> Key {
    Key::indexed("bar-zone", index)
}

/// A stable index per zone name, so each capsule keeps its identity (hover,
/// focus) across rebuilds.
fn zone_index(name: &str) -> u64 {
    match name {
        "start" => 0,
        "centre-left" => 1,
        "centre-right" => 2,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use celestina_ui::ViewKind;

    use super::*;
    use crate::island::pages::NameOwner;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn layout(start: &[&str], centre: &[&str], end: &[&str]) -> BarLayout {
        BarLayout {
            start: ids(start),
            centre: ids(centre),
            end: ids(end),
            tray_hidden: Vec::new(),
        }
    }

    /// A resolver with three pieces: `a` and `b` draw a 10 px box keyed by
    /// their name, `quiet` has nothing to show; anything else is unknown.
    fn fake(id: &str) -> Option<Option<View>> {
        match id {
            "a" | "b" => Some(Some(view::row(Key::named(id)).size(10.0, 10.0))),
            "quiet" => Some(None),
            _ => None,
        }
    }

    fn with_cx<R>(f: impl FnOnce(&BarCx<'_>) -> R) -> R {
        let theme = Theme::dark();
        let owners = NameOwner::default();
        let cx = BarCx {
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            theme: &theme,
            output: "DP-1",
            model: None,
            owners: &owners,
            do_not_disturb: false,
            history: &[],
            items: &[],
            tray_hidden: &[],
            sysmon: None,
            weather: None,
            today: None,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            levels: crate::pieces::Levels::default(),
        };
        f(&cx)
    }

    fn child_keys(v: &View) -> Vec<Key> {
        v.children.iter().map(|c| c.key).collect()
    }

    #[test]
    fn a_zone_becomes_one_haze_capsule_with_its_pieces_in_order() {
        with_cx(|cx| {
            let z = Zones::new().build_with(&layout(&["a", "b"], &[], &[]), cx, &fake);
            let start = z.start.expect("start");
            assert_eq!(child_keys(&start), [Key::named("a"), Key::named("b")]);
            assert!(matches!(start.kind, ViewKind::Box { haze: true, .. }));
            assert!(z.centre_left.is_none() && z.centre_right.is_none() && z.end.is_none());
        });
    }

    #[test]
    fn empty_zones_and_all_absent_zones_have_no_capsule() {
        with_cx(|cx| {
            let z = Zones::new().build_with(&layout(&[], &[], &["quiet", "ghost"]), cx, &fake);
            assert!(z.start.is_none() && z.end.is_none());
        });
    }

    #[test]
    fn unknown_ids_are_ignored_and_logged_once_per_generation() {
        with_cx(|cx| {
            let mut zones = Zones::new();
            let l = layout(&["a", "ghost", "ghost"], &[], &["ghost", "b"]);
            let z = zones.build_with(&l, cx, &fake);
            assert_eq!(child_keys(&z.start.expect("start")), [Key::named("a")]);
            assert_eq!(child_keys(&z.end.expect("end")), [Key::named("b")]);
            assert_eq!(zones.unknown_logged.len(), 1);
            // Rebuilding with the same layout does not log it again.
            zones.build_with(&l, cx, &fake);
            assert_eq!(zones.unknown_logged.len(), 1);
            // A new generation reports it again.
            zones.new_generation();
            assert!(zones.unknown_logged.is_empty());
            zones.build_with(&l, cx, &fake);
            assert_eq!(zones.unknown_logged.len(), 1);
        });
    }

    #[test]
    fn a_piece_listed_twice_shows_once_in_its_first_place() {
        with_cx(|cx| {
            let l = layout(&["a", "a"], &["a", "b"], &["b"]);
            let z = Zones::new().build_with(&l, cx, &fake);
            assert_eq!(child_keys(&z.start.expect("start")), [Key::named("a")]);
            assert_eq!(
                child_keys(&z.centre_right.expect("right")),
                [Key::named("b")]
            );
            assert!(z.end.is_none(), "b was already placed in the centre");
        });
    }

    #[test]
    fn the_centre_splits_at_the_island_mark() {
        let c = ids(&["a", "island", "b", "island"]);
        let (left, right) = split_centre(&c);
        assert_eq!((left, right), (&c[..1], &c[2..]));
        let none = ids(&["a", "b"]);
        let (left, right) = split_centre(&none);
        assert!(left.is_empty());
        assert_eq!(right, none.as_slice());
        with_cx(|cx| {
            let z = Zones::new().build_with(&layout(&[], &["a", "island", "b"], &[]), cx, &fake);
            assert_eq!(child_keys(&z.centre_left.expect("left")), [Key::named("a")]);
            assert_eq!(
                child_keys(&z.centre_right.expect("right")),
                [Key::named("b")]
            );
        });
    }

    #[test]
    fn a_hostile_zone_is_bounded() {
        with_cx(|cx| {
            // Sixteen distinct unknown ids, then a known one past the bound,
            // and a thousand known duplicates.
            let mut start: Vec<String> = (0..16).map(|i| format!("ghost{i}")).collect();
            start.push("a".to_owned());
            let mut zones = Zones::new();
            let z = zones.build_with(
                &BarLayout {
                    start,
                    centre: Vec::new(),
                    end: vec!["b".to_owned(); 1000],
                    tray_hidden: Vec::new(),
                },
                cx,
                &fake,
            );
            assert!(z.start.is_none(), "the 17th id is beyond the bound");
            assert_eq!(zones.unknown_logged.len(), 16);
            assert_eq!(child_keys(&z.end.expect("end")), [Key::named("b")]);
        });
    }

    #[test]
    fn capsules_keep_their_identity_across_rebuilds() {
        with_cx(|cx| {
            let l = layout(&["a"], &[], &["b"]);
            let one = Zones::new().build_with(&l, cx, &fake);
            let two = Zones::new().build_with(&l, cx, &fake);
            assert_eq!(one.start.map(|v| v.key), two.start.map(|v| v.key));
            assert_ne!(
                Zones::new().build_with(&l, cx, &fake).start.map(|v| v.key),
                two.end.map(|v| v.key)
            );
        });
    }

    #[test]
    fn every_node_of_a_piece_is_owned_by_it_and_a_shared_key_by_the_first() {
        // `a` draws a node `dot` of its own and `x`; `b` draws `x` too.
        let shared = |id: &str| -> Option<Option<View>> {
            let node = |name: &str| view::row(Key::named(name)).size(5.0, 5.0);
            match id {
                "a" => Some(Some(
                    view::row(Key::named("a")).children([node("dot"), node("x")]),
                )),
                "b" => Some(Some(view::row(Key::named("b")).children([node("x")]))),
                _ => None,
            }
        };
        with_cx(|cx| {
            let z = Zones::new().build_with(&layout(&["a"], &[], &["b"]), cx, &shared);
            assert_eq!(z.owner(Key::named("dot")), Some("a"));
            assert_eq!(z.owner(Key::named("a")), Some("a"));
            assert_eq!(z.owner(Key::named("b")), Some("b"));
            assert_eq!(
                z.owner(Key::named("x")),
                Some("a"),
                "the first piece keeps it"
            );
            // A capsule's own node, and a node nobody drew, have no owner.
            assert_eq!(z.owner(zone_key(0)), None);
            assert_eq!(z.owner(Key::named("ghost")), None);
        });
    }

    #[test]
    fn the_real_table_builds_without_panicking() {
        with_cx(|cx| {
            let z = Zones::new().build(&BarLayout::default(), cx);
            // Whatever the pieces draw today, a capsule exists iff it has children.
            for v in [&z.start, &z.centre_left, &z.centre_right, &z.end]
                .into_iter()
                .flatten()
            {
                assert!(!v.children.is_empty());
            }
        });
    }

    #[test]
    fn the_default_end_zone_names_real_pieces_and_draws_only_what_answers() {
        use crate::connectivity::state::{sample, ConnectivityState};
        use crate::pieces::tests_support::with_state;

        let end = layout(&[], &[], &["tray", "network", "bluetooth"]);
        // Nothing answers: no capsule, and nothing is logged as unknown.
        let mut zones = Zones::new();
        let bare = ConnectivityState::default();
        let views = with_state(&bare, |cx| zones.build(&end, cx));
        assert!(views.end.is_none());
        assert!(
            zones.unknown_logged.is_empty(),
            "{:?}",
            zones.unknown_logged
        );
        // NetworkManager and BlueZ answer: both pieces share the capsule.
        let ready = sample::ready();
        let views = with_state(&ready, |cx| zones.build(&end, cx));
        let capsule = views.end.expect("end capsule");
        assert_eq!(capsule.children.len(), 2);
        assert!(zones.unknown_logged.is_empty());
    }
}
