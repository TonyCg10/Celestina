//! The Tray page (design §5.5): one row per status notifier item (its
//! icon, its title and, when it is not simply active, a status word). While
//! another program holds the status notifier watcher name the page shows the
//! explanation line and nothing else; with no items it shows the empty
//! state. The page only describes: a click, a middle click, the menu and
//! the wheel are decided by the Island from the rows' keys
//! ([`row_key`], [`index_of`]) and become [`crate::tray::TrayCommand`]s.

use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    hash::{Hash, Hasher},
};

use celestina_paint::{IconKey, PixmapId, Role, Weight};
use celestina_ui::{view, Key, View, ViewKind};

use super::{note_list, PageCx, PageView};
use crate::{
    copy,
    island::{
        state::PAGE_SIZE,
        view::{centred, chrome_height, text_style},
    },
    tray::{model::FALLBACK_ICON, ItemId, Pixmap, Status, TrayItem},
};

/// An item's icon, logical px.
pub const ICON_SIZE: f32 = 24.0;
/// The most rows the page draws, however tall the surface.
pub const MAX_ROWS: usize = 24;

/// What a row draws as its icon: already resolved to something the painter
/// can draw without touching the item's pixel bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowIcon {
    /// A name in the icon theme.
    Theme(String),
    /// A pixmap uploaded to the painter.
    Pixmap(PixmapId),
}

impl Default for RowIcon {
    fn default() -> Self {
        RowIcon::Theme(FALLBACK_ICON.to_owned())
    }
}

/// One item as the page shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayRow {
    /// Who serves the item: what a command is addressed to.
    pub id: ItemId,
    /// Its `Id` property: what `bar.tray_hidden` matches (empty when it
    /// has none, and then the item is never hidden).
    pub sni_id: String,
    /// Its title (empty when it has none).
    pub title: String,
    /// Its status.
    pub status: Status,
    /// Its icon.
    pub icon: RowIcon,
    /// Whether it serves a menu.
    pub has_menu: bool,
}

/// The item list the app keeps and the generation that tells an Island it
/// changed, so a frame never compares pixel bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayList {
    /// The items, in the host's order.
    pub items: Vec<TrayItem>,
    /// Increases whenever the list is replaced; starts at 1.
    pub generation: u64,
}

impl Default for TrayList {
    fn default() -> Self {
        TrayList {
            items: Vec::new(),
            generation: 1,
        }
    }
}

impl TrayList {
    /// Replaces the items; whether that changed anything.
    pub fn replace(&mut self, items: Vec<TrayItem>) -> bool {
        if self.items == items {
            return false;
        }
        self.items = items;
        self.generation += 1;
        true
    }
}

/// The pixmaps uploaded for the items' icons, uploaded once per distinct
/// image and released when the item goes or its image changes.
#[derive(Debug, Default)]
pub struct IconCache {
    uploaded: HashMap<ItemId, (u64, PixmapId)>,
}

/// A fingerprint of a pixmap, to tell a changed image from a re-sent one.
fn fingerprint(w: u32, h: u32, argb: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    (w, h, argb).hash(&mut hasher);
    hasher.finish()
}

/// Where pixmaps live and where theme icons are looked up: the painter.
pub trait Pixmaps {
    /// Uploads an ARGB32 image; `None` when it cannot.
    fn upload(&mut self, argb: &[u8], w: u32, h: u32) -> Option<PixmapId>;
    /// Frees an uploaded image.
    fn release(&mut self, id: PixmapId);
    /// Whether the icon theme has `name` at about `size` logical px.
    fn has_theme_icon(&self, name: &str, size: u32) -> bool;
}

/// Whether the theme has the item's icon name. The lookup happens here,
/// when the list changes, so a frame never looks an icon up to draw
/// nothing; an item publishing a private name (as libappindicator does)
/// draws its pixmap or the fallback instead.
fn themed(item: &TrayItem, pixmaps: &impl Pixmaps) -> bool {
    !item.icon_name.is_empty() && pixmaps.has_theme_icon(&item.icon_name, ICON_SIZE as u32)
}

impl IconCache {
    /// The rows for `items`: a theme icon when the theme has the item's
    /// name, else its pixmap, else the fallback. Pixmaps are uploaded to
    /// `pixmaps` only when new or changed, and the ones no item uses any
    /// more are released.
    pub fn rows(&mut self, items: &[TrayItem], pixmaps: &mut impl Pixmaps) -> Vec<TrayRow> {
        let mut seen = HashSet::new();
        let mut rows = Vec::with_capacity(items.len());
        let mut drawn: Vec<(ItemId, bool)> = Vec::with_capacity(items.len());
        for item in items {
            let themed = themed(item, pixmaps);
            let uses_pixmap = !themed && item.pixmap.is_some();
            drawn.push((item.id.clone(), uses_pixmap));
            let icon = match (&item.pixmap, uses_pixmap) {
                (Some(Pixmap { w, h, argb }), true) => {
                    let print = fingerprint(*w, *h, argb);
                    let kept = self.uploaded.get(&item.id).copied();
                    let shared = !seen.insert(item.id.clone());
                    match kept {
                        Some((old, id)) if old == print || shared => RowIcon::Pixmap(id),
                        _ => {
                            if let Some((_, stale)) = self.uploaded.remove(&item.id) {
                                pixmaps.release(stale);
                            }
                            match pixmaps.upload(argb, *w, *h) {
                                Some(id) => {
                                    self.uploaded.insert(item.id.clone(), (print, id));
                                    RowIcon::Pixmap(id)
                                }
                                None => RowIcon::default(),
                            }
                        }
                    }
                }
                _ if themed => RowIcon::Theme(item.icon_name.clone()),
                _ => RowIcon::default(),
            };
            rows.push(TrayRow {
                id: item.id.clone(),
                sni_id: item.sni_id.clone(),
                title: item.title.clone(),
                status: item.status,
                icon,
                has_menu: item.has_menu,
            });
        }
        let gone: Vec<ItemId> = self
            .uploaded
            .keys()
            .filter(|id| {
                !drawn
                    .iter()
                    .any(|(drawn_id, uses)| drawn_id == *id && *uses)
            })
            .cloned()
            .collect();
        for id in gone {
            if let Some((_, stale)) = self.uploaded.remove(&id) {
                pixmaps.release(stale);
            }
        }
        rows
    }

    /// Releases every pixmap (the Island goes away).
    pub fn release_all(&mut self, pixmaps: &mut impl Pixmaps) {
        for (_, (_, id)) in self.uploaded.drain() {
            pixmaps.release(id);
        }
    }

    /// How many pixmaps are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.uploaded.len()
    }

    /// Whether none is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.uploaded.is_empty()
    }
}

/// The row of the item at `index` of the list.
#[must_use]
pub fn row_key(index: usize) -> Key {
    Key::indexed("tray-item", index as u64)
}

/// The index of the item whose row is `key`, among `count` rows.
#[must_use]
pub fn index_of(key: Key, count: usize) -> Option<usize> {
    (0..count.min(MAX_ROWS)).find(|&i| row_key(i) == key)
}

/// The rows of `items` that are drawn on a page of `max_size`: what fits,
/// at most [`MAX_ROWS`]. The rest is not built, so nothing invisible can be
/// focused or clicked.
fn drawn<'a>(items: &'a [TrayRow], cx: &PageCx<'_>) -> &'a [TrayRow] {
    let room = (cx.max_size.1 - chrome_height(cx.theme)).max(0.0);
    let fit = (room / cx.theme.row_height).floor() as usize;
    &items[..items.len().min(fit.min(MAX_ROWS))]
}

/// The Tray page.
#[derive(Clone, Copy, Debug)]
pub struct Tray;

impl PageView for Tray {
    fn title(&self) -> &str {
        copy::PAGE_TRAY
    }

    fn view(&self, cx: &PageCx<'_>) -> View {
        let theme = cx.theme;
        if let Some(holder) = &cx.owners.tray {
            return view::col(Key::named("tray")).child(note_list(
                theme,
                Key::named("tray-rows"),
                &copy::tray_owner(&holder.unique, holder.comm.as_deref()),
            ));
        }
        let rows = drawn(cx.items, cx);
        if rows.is_empty() {
            return view::col(Key::named("tray")).child(note_list(
                theme,
                Key::named("tray-rows"),
                copy::TRAY_EMPTY,
            ));
        }
        view::col(Key::named("tray")).child(
            view::col(Key::named("tray-rows"))
                .children(rows.iter().enumerate().map(|(i, r)| item_row(cx, i, r))),
        )
    }

    /// The default size, taller when the rows need it, up to the surface.
    fn wants(&self, cx: &PageCx<'_>) -> (f32, f32) {
        if cx.owners.tray.is_some() {
            return PAGE_SIZE;
        }
        let rows = drawn(cx.items, cx).len();
        let needed = chrome_height(cx.theme) + rows as f32 * cx.theme.row_height;
        (PAGE_SIZE.0, PAGE_SIZE.1.max(needed))
    }
}

fn item_row(cx: &PageCx<'_>, index: usize, row: &TrayRow) -> View {
    let theme = cx.theme;
    let (title_role, weight) = match row.status {
        Status::Passive => (Role::TextFaint, Weight::Regular),
        Status::Active => (Role::TextHi, Weight::Regular),
        Status::NeedsAttention => (Role::TextHi, Weight::Semibold),
    };
    let icon_key = Key::indexed("tray-icon", index as u64);
    let icon = match &row.icon {
        RowIcon::Theme(name) => View::new(
            icon_key,
            ViewKind::Icon {
                key: IconKey::Theme(name.clone()),
                tint: None,
                size: ICON_SIZE,
            },
        ),
        RowIcon::Pixmap(id) => view::image(icon_key, *id, 1.0),
    }
    .size(ICON_SIZE, ICON_SIZE);
    let title = if row.title.is_empty() {
        copy::TRAY_ITEM_UNTITLED
    } else {
        row.title.as_str()
    };
    let status = match row.status {
        Status::Active => None,
        Status::Passive => Some((copy::TRAY_PASSIVE, Role::TextFaint)),
        Status::NeedsAttention => Some((copy::TRAY_NEEDS_ATTENTION, Role::Accent)),
    };
    centred(view::row(row_key(index)))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md)
        .radius(theme.radius_md)
        .interactive()
        .focusable()
        .child(icon)
        .child(
            view::text(
                Key::indexed("tray-title", index as u64),
                title,
                text_style(theme.type_scale.body, weight, false, title_role),
            )
            .grow(),
        )
        .children(status.map(|(label, role)| {
            view::text(
                Key::indexed("tray-status", index as u64),
                label,
                text_style(theme.type_scale.caption, Weight::Regular, false, role),
            )
        }))
}

#[cfg(test)]
mod tests {
    use celestina_ui::ViewKind;

    use super::*;
    use crate::island::pages::wants_of;
    use crate::island::pages::NameOwner;
    use crate::tray::NameHolder;

    fn page_with(owners: &NameOwner, items: &[TrayRow]) -> View {
        Tray.view(&PageCx {
            theme: &celestina_paint::Theme::dark(),
            max_size: (760.0, 560.0),
            model: None,
            output: "DP-1",
            owners,
            do_not_disturb: false,
            history: &[],
            items,
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
        })
    }

    fn page_for(owners: &NameOwner) -> View {
        page_with(owners, &[])
    }

    fn only_row_text(v: &View) -> Option<String> {
        let rows = &v.children[0];
        assert_eq!(rows.children.len(), 1, "exactly one row");
        match &rows.children[0].children[0].kind {
            ViewKind::Text { text, .. } => Some(text.clone()),
            _ => None,
        }
    }

    fn row(name: &str, status: Status, icon: RowIcon) -> TrayRow {
        TrayRow {
            id: ItemId {
                bus_name: format!(":1.{}", name.len()),
                path: format!("/{name}"),
            },
            title: name.to_owned(),
            status,
            icon,
            has_menu: false,
            sni_id: String::new(),
        }
    }

    fn texts(v: &View) -> Vec<(Key, String, Role, Weight)> {
        let mut out = Vec::new();
        let mut stack = vec![v];
        while let Some(n) = stack.pop() {
            if let ViewKind::Text { text, style, .. } = &n.kind {
                out.push((n.key, text.clone(), style.role, style.weight));
            }
            stack.extend(n.children.iter().rev());
        }
        out
    }

    #[test]
    fn tray_page_shows_the_owner_line_while_another_program_holds_the_name() {
        let owners = NameOwner {
            notifications: None,
            tray: Some(NameHolder {
                unique: ":1.42".to_owned(),
                comm: Some("noctalia".to_owned()),
            }),
        };
        let line = only_row_text(&page_for(&owners));
        assert_eq!(line, Some(copy::tray_owner(":1.42", Some("noctalia"))));
        // The line is the copy's: Spanish around the holder's data, no
        // English in it.
        let line = line.unwrap_or_default();
        assert!(
            line.contains("noctalia") && line.contains(":1.42"),
            "{line}"
        );
        assert!(!line.contains("held"), "{line}");
        // Without a process name the unique name stands alone.
        let bare = copy::tray_owner(":1.42", None);
        assert!(bare.ends_with(":1.42") && !bare.contains('('), "{bare}");
    }

    #[test]
    fn the_owner_line_still_wins_over_items() {
        let owners = NameOwner {
            notifications: None,
            tray: Some(NameHolder {
                unique: ":1.42".to_owned(),
                comm: None,
            }),
        };
        let items = [row("nm", Status::Active, RowIcon::default())];
        let v = page_with(&owners, &items);
        assert_eq!(only_row_text(&v), Some(copy::tray_owner(":1.42", None)));
    }

    #[test]
    fn tray_page_without_an_owner_shows_the_empty_state() {
        assert_eq!(
            only_row_text(&page_for(&NameOwner::default())),
            Some(copy::TRAY_EMPTY.to_owned())
        );
    }

    #[test]
    fn the_page_has_one_interactive_focusable_row_per_item() {
        let items = [
            row("nm", Status::Active, RowIcon::Theme("network".to_owned())),
            row("vol", Status::Active, RowIcon::default()),
            row("bt", Status::Active, RowIcon::default()),
        ];
        let v = page_with(&NameOwner::default(), &items);
        let rows = &v.children[0].children;
        let keys: Vec<Key> = rows.iter().map(|r| r.key).collect();
        assert_eq!(keys, vec![row_key(0), row_key(1), row_key(2)]);
        assert!(rows.iter().all(|r| r.interactive && r.focusable));
        assert_eq!(index_of(row_key(2), 3), Some(2));
        assert_eq!(index_of(row_key(3), 3), None);
        assert_eq!(index_of(Key::named("close"), 3), None);
        // The icon of a theme-named item is the theme icon.
        let icon = &rows[0].children[0];
        assert!(matches!(
            &icon.kind,
            ViewKind::Icon { key: IconKey::Theme(n), .. } if n == "network"
        ));
    }

    #[test]
    fn status_styles_the_row_attention_emphasised_passive_dimmed() {
        let items = [
            row("active", Status::Active, RowIcon::default()),
            row("attention", Status::NeedsAttention, RowIcon::default()),
            row("passive", Status::Passive, RowIcon::default()),
        ];
        let all = texts(&page_with(&NameOwner::default(), &items));
        let find = |key: Key| all.iter().find(|t| t.0 == key);
        let title = |i: u64| Key::indexed("tray-title", i);
        let status = |i: u64| Key::indexed("tray-status", i);
        assert_eq!(
            find(title(0)).map(|t| (t.2, t.3)),
            Some((Role::TextHi, Weight::Regular))
        );
        assert!(find(status(0)).is_none(), "an active item has no status");
        assert_eq!(
            find(status(1)).map(|t| (t.1.clone(), t.2)),
            Some((copy::TRAY_NEEDS_ATTENTION.to_owned(), Role::Accent))
        );
        assert_eq!(find(title(1)).map(|t| t.3), Some(Weight::Semibold));
        assert_eq!(find(title(2)).map(|t| t.2), Some(Role::TextFaint));
        assert_eq!(
            find(status(2)).map(|t| t.1.clone()),
            Some(copy::TRAY_PASSIVE.to_owned())
        );
    }

    #[test]
    fn an_untitled_item_says_so_and_a_pixmap_item_draws_its_image() {
        let mut untitled = row("x", Status::Active, RowIcon::Pixmap(PixmapId(7)));
        untitled.title.clear();
        let v = page_with(&NameOwner::default(), &[untitled]);
        let r = &v.children[0].children[0];
        assert!(matches!(&r.children[0].kind, ViewKind::Image { id, .. } if *id == PixmapId(7)));
        assert_eq!(
            texts(&v).first().map(|t| t.1.clone()),
            Some(copy::TRAY_ITEM_UNTITLED.to_owned())
        );
    }

    #[test]
    fn only_the_rows_that_fit_are_built_and_the_page_grows_for_them() {
        let many: Vec<TrayRow> = (0..40)
            .map(|i| row(&format!("i{i}"), Status::Active, RowIcon::default()))
            .collect();
        let v = page_with(&NameOwner::default(), &many);
        assert!(v.children[0].children.len() <= MAX_ROWS);
        assert!(!v.children[0].children.is_empty());
        let wants = Tray.wants(&PageCx {
            theme: &celestina_paint::Theme::dark(),
            max_size: (760.0, 560.0),
            model: None,
            output: "DP-1",
            owners: &NameOwner::default(),
            do_not_disturb: false,
            history: &[],
            items: &many,
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
        });
        assert_eq!(wants.0, PAGE_SIZE.0);
        assert!(wants.1 >= PAGE_SIZE.1);
    }

    fn pixmap_item(name: &str, fill: u8) -> TrayItem {
        TrayItem {
            id: ItemId {
                bus_name: name.to_owned(),
                path: "/StatusNotifierItem".to_owned(),
            },
            title: name.to_owned(),
            pixmap: Some(Pixmap {
                w: 1,
                h: 1,
                argb: vec![fill; 4],
            }),
            ..TrayItem::default()
        }
    }

    /// Hands out ids from 1 and remembers what happened; the theme has the
    /// icons in `theme`.
    #[derive(Default)]
    struct Fake {
        next: u32,
        refuse: bool,
        uploads: usize,
        released: Vec<PixmapId>,
        theme: Vec<&'static str>,
        lookups: std::cell::Cell<usize>,
    }

    impl Pixmaps for Fake {
        fn upload(&mut self, _: &[u8], _: u32, _: u32) -> Option<PixmapId> {
            if self.refuse {
                return None;
            }
            self.next += 1;
            self.uploads += 1;
            Some(PixmapId(self.next))
        }

        fn release(&mut self, id: PixmapId) {
            self.released.push(id);
        }

        fn has_theme_icon(&self, name: &str, _: u32) -> bool {
            self.lookups.set(self.lookups.get() + 1);
            self.theme.contains(&name)
        }
    }

    #[test]
    fn rows_carry_the_items_own_id() {
        let mut item = pixmap_item("a", 1);
        item.sni_id = "nm-applet".to_owned();
        let rows = IconCache::default().rows(&[item], &mut Fake::default());
        assert_eq!(rows[0].sni_id, "nm-applet");
    }

    #[test]
    fn a_name_the_theme_lacks_falls_back_to_the_pixmap_then_the_fallback_icon() {
        let mut cache = IconCache::default();
        let mut fake = Fake {
            theme: vec!["mail"],
            ..Fake::default()
        };
        let mut themed = pixmap_item("a", 1);
        themed.icon_name = "mail".to_owned();
        let mut private = pixmap_item("b", 2);
        private.icon_name = "steam_tray_mono".to_owned();
        let nothing = TrayItem {
            icon_name: "no-such-icon".to_owned(),
            ..TrayItem::default()
        };
        let rows = cache.rows(&[themed, private, nothing], &mut fake);
        assert_eq!(rows[0].icon, RowIcon::Theme("mail".to_owned()));
        assert_eq!(rows[1].icon, RowIcon::Pixmap(PixmapId(1)));
        assert_eq!(
            rows[2].icon,
            RowIcon::default(),
            "no pixmap to fall back to"
        );
        assert_eq!(fake.uploads, 1, "the themed item's pixmap stays unused");
        // The lookups happen here, once per named item, not per frame.
        assert_eq!(fake.lookups.get(), 3);
    }

    #[test]
    fn a_pixmap_is_uploaded_once_and_released_when_changed_or_gone() {
        let mut cache = IconCache::default();
        let mut fake = Fake::default();
        let items = [pixmap_item("a", 1), pixmap_item("b", 2)];
        let first = cache.rows(&items, &mut fake);
        assert_eq!(cache.len(), 2);
        let again = cache.rows(&items, &mut fake);
        assert_eq!(first, again, "re-sent items upload nothing");
        assert_eq!(fake.uploads, 2);
        let changed = [pixmap_item("a", 9), pixmap_item("b", 2)];
        let after = cache.rows(&changed, &mut fake);
        assert_ne!(first[0].icon, after[0].icon);
        assert_eq!(first[1].icon, after[1].icon);
        assert_eq!(
            fake.released,
            vec![PixmapId(1)],
            "the changed image's old texture"
        );
        let gone = cache.rows(&changed[1..], &mut fake);
        assert_eq!(gone.len(), 1);
        assert_eq!(cache.len(), 1);
        assert_eq!(fake.released.len(), 2, "and the gone item's");
        cache.release_all(&mut fake);
        assert!(cache.is_empty());
        assert_eq!(fake.released.len(), 3);
        assert_eq!(fake.uploads, 3);
    }

    #[test]
    fn a_failed_upload_falls_back_to_the_semantic_icon() {
        let mut cache = IconCache::default();
        let mut fake = Fake {
            refuse: true,
            ..Fake::default()
        };
        let rows = cache.rows(&[pixmap_item("a", 1)], &mut fake);
        assert_eq!(rows[0].icon, RowIcon::default());
        assert!(cache.is_empty());
    }

    #[test]
    fn a_pixmap_item_that_switches_to_a_name_releases_its_pixmap() {
        let mut cache = IconCache::default();
        let mut fake = Fake {
            theme: vec!["mail"],
            ..Fake::default()
        };
        let mut a = pixmap_item("a", 1);
        cache.rows(&[a.clone()], &mut fake);
        a.pixmap = None;
        a.icon_name = "mail".to_owned();
        let rows = cache.rows(&[a], &mut fake);
        assert_eq!(rows[0].icon, RowIcon::Theme("mail".to_owned()));
        assert_eq!(fake.released, vec![PixmapId(1)]);
    }

    #[test]
    fn replacing_the_list_bumps_the_generation_only_for_a_real_change() {
        let mut list = TrayList::default();
        let g = list.generation;
        assert!(!list.replace(Vec::new()));
        assert_eq!(list.generation, g);
        assert!(list.replace(vec![pixmap_item("a", 1)]));
        assert_eq!(list.generation, g + 1);
        assert!(!list.replace(vec![pixmap_item("a", 1)]));
    }

    #[test]
    fn the_page_asks_for_the_default_size() {
        assert_eq!(wants_of(&Tray, None, (760.0, 560.0)), PAGE_SIZE);
        assert_eq!(Tray.title(), copy::PAGE_TRAY);
    }
}
