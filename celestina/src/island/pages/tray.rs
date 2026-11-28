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
    path::{Path, PathBuf},
};

use celestina_paint::{IconKey, PixmapId, Role, Theme, Weight};
use celestina_ui::{view, Key, View, ViewKind};

use super::{note_list, PageCx, PageView};
use crate::{
    copy,
    island::{
        state::PAGE_SIZE,
        view::{centred, chrome_height, text_style},
    },
    sheet::tray_overflow::page_card_slot,
    tray::{
        card::{CardView, MARGINS},
        model::FALLBACK_ICON,
        pick_pixmap, ItemId, Pixmap, Status, TrayItem,
    },
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
    /// A file [`celestina_paint::peer_icon_file`] or
    /// [`celestina_paint::icon_in_dir`] accepted: canonical, a PNG or SVG,
    /// under the item's own theme directory or an XDG icon root.
    File(PathBuf),
    /// A pixmap uploaded to the painter.
    Pixmap(PixmapId),
    /// One of the bundled glyphs of `celestina-style/icons`, drawn in the
    /// muted text colour.
    Bundled(&'static str),
}

impl Default for RowIcon {
    fn default() -> Self {
        RowIcon::Bundled(FALLBACK_ICON)
    }
}

/// The view that draws `icon` in a box of [`ICON_SIZE`], the one the page's
/// rows and the bar's icons share.
#[must_use]
pub fn icon_view(key: Key, icon: &RowIcon) -> View {
    match icon {
        RowIcon::Theme(name) => View::new(
            key,
            ViewKind::Icon {
                key: IconKey::Theme(name.clone()),
                tint: None,
                size: ICON_SIZE,
            },
        ),
        RowIcon::File(path) => View::new(
            key,
            ViewKind::Icon {
                key: IconKey::File(path.clone()),
                tint: None,
                size: ICON_SIZE,
            },
        ),
        RowIcon::Bundled(name) => View::new(
            key,
            ViewKind::Icon {
                key: IconKey::Semantic(name),
                tint: Some(Role::TextLo),
                size: ICON_SIZE,
            },
        ),
        RowIcon::Pixmap(id) => view::image(key, *id, 1.0),
    }
    .size(ICON_SIZE, ICON_SIZE)
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
    /// Its `ItemIsMenu`: a left click opens the menu.
    pub item_is_menu: bool,
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

/// What the icon of an item is, before any pixmap is uploaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconSource<'a> {
    /// Drawn as it is.
    Icon(RowIcon),
    /// One of the item's pixmaps, to upload.
    Pixmap(&'a Pixmap),
}

/// The icon size, in device px, at output scale `scale`.
fn device_px(scale: f32) -> u32 {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    ((ICON_SIZE * scale).round() as u32).max(1)
}

/// One icon description (`name`, its resolved `file` and `pixmaps`), tried
/// in order:
/// 1. the `file` the tray host resolved off this thread (an absolute name
///    under an allowed root, or a relative one in the item's own theme
///    directory); this thread never touches the disk for it;
/// 2. a relative `name` in the user's theme (looked up here, when the list
///    changes, so a frame never looks an icon up to draw nothing);
/// 3. the pixmap nearest to the scaled size.
fn describe<'a>(
    name: &str,
    file: Option<&Path>,
    pixmaps: &'a [Pixmap],
    px: u32,
    has_theme: &dyn Fn(&str, u32) -> bool,
) -> Option<IconSource<'a>> {
    if let Some(file) = file {
        return Some(IconSource::Icon(RowIcon::File(file.to_owned())));
    }
    if !name.is_empty()
        && !Path::new(name).is_absolute()
        && !name.contains(['/', '\\', '\0'])
        && has_theme(name, px)
    {
        return Some(IconSource::Icon(RowIcon::Theme(name.to_owned())));
    }
    pick_pixmap(pixmaps, px).map(IconSource::Pixmap)
}

/// What `item` draws at output scale `scale`, the one place the bar's piece
/// and the Tray page get their icons from (through the rows built by
/// [`IconCache::rows`]). While the item needs attention its attention icon
/// replaces the normal one when it has a usable one; the normal order is the
/// item's theme directory, the user's theme (`has_theme(name, device_px)`),
/// the pixmap nearest to `24 * scale`, then the bundled fallback.
#[must_use]
pub fn resolve_tray_icon<'a>(
    item: &'a TrayItem,
    scale: f32,
    has_theme: &dyn Fn(&str, u32) -> bool,
) -> IconSource<'a> {
    let px = device_px(scale);
    let attention = (item.status == Status::NeedsAttention)
        .then(|| {
            describe(
                &item.attention_icon_name,
                item.attention_icon_file.as_deref(),
                &item.attention_pixmaps,
                px,
                has_theme,
            )
        })
        .flatten();
    attention
        .or_else(|| {
            describe(
                &item.icon_name,
                item.icon_file.as_deref(),
                &item.pixmaps,
                px,
                has_theme,
            )
        })
        .unwrap_or_else(|| IconSource::Icon(RowIcon::default()))
}

impl IconCache {
    /// The rows for `items` at output scale `scale`, each icon as
    /// [`resolve_tray_icon`] says. Pixmaps are uploaded to `pixmaps` only
    /// when new or changed, and the ones no item uses any more are released.
    pub fn rows(
        &mut self,
        items: &[TrayItem],
        scale: f32,
        pixmaps: &mut impl Pixmaps,
    ) -> Vec<TrayRow> {
        let mut seen = HashSet::new();
        let mut rows = Vec::with_capacity(items.len());
        let mut drawn: Vec<(ItemId, bool)> = Vec::with_capacity(items.len());
        for item in items {
            let source = {
                let pixmaps: &dyn Pixmaps = &*pixmaps;
                resolve_tray_icon(item, scale, &|name, size| {
                    pixmaps.has_theme_icon(name, size)
                })
            };
            let uses_pixmap = matches!(source, IconSource::Pixmap(_));
            drawn.push((item.id.clone(), uses_pixmap));
            let icon = match source {
                IconSource::Icon(icon) => icon,
                IconSource::Pixmap(Pixmap { w, h, argb }) => {
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
            };
            rows.push(TrayRow {
                id: item.id.clone(),
                sni_id: item.sni_id.clone(),
                title: item.title.clone(),
                status: item.status,
                icon,
                has_menu: item.has_menu,
                item_is_menu: item.item_is_menu,
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

/// How many rows the page draws for `cx`: what fits, at most [`MAX_ROWS`].
#[must_use]
pub(crate) fn drawn_len(cx: &PageCx<'_>) -> usize {
    drawn(cx.items, cx).len()
}

/// How many rows the page could draw for `cx` with no card on it: what
/// fits, at most [`MAX_ROWS`]. Where an app's row may be found again when
/// the list changes under an open card.
#[must_use]
pub(crate) fn fitting_len(cx: &PageCx<'_>) -> usize {
    let room = (cx.max_size.1 - chrome_height(cx.theme)).max(0.0);
    let fit = (room / cx.theme.row_height).floor() as usize;
    cx.items.len().min(fit.min(MAX_ROWS))
}

/// The rows of `items` that are drawn on a page of `max_size`: what fits,
/// at most [`MAX_ROWS`]. The rest is not built, so nothing invisible can be
/// focused or clicked.
fn drawn<'a>(items: &'a [TrayRow], cx: &PageCx<'_>) -> &'a [TrayRow] {
    // An open card keeps room under its row: the rows that do not fit under
    // its box are not drawn, so nothing out of sight takes the focus.
    let kept = cx
        .tray_card
        .filter(CardView::shown)
        .map_or(usize::MAX, |card| card.rows);
    &items[..fitting_len(cx).min(kept)]
}

/// The Tray page.
#[derive(Clone, Copy, Debug)]
pub struct Tray;

impl PageView for Tray {
    fn icon(&self) -> &'static str {
        "system-tray"
    }

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
        let mut children = Vec::with_capacity(rows.len() + 1);
        for (i, r) in rows.iter().enumerate() {
            let mine = cx.tray_card.filter(|c| c.index == i);
            let look = RowLook {
                held: mine.is_some_and(|c| c.held),
                open: mine.is_some_and(|c| c.answered),
            };
            children.push(item_row_looking(theme, i, r, display_title(r), look));
            if let Some(card) = mine {
                children.push(page_card_slot(theme, card.shown().then_some(card.box_h)));
            }
        }
        if cx.tray_card.is_none_or(|c| c.index >= rows.len()) {
            // Always in the tree, out of the layout: a box never enters or
            // leaves with a fade of the page's tree.
            children.push(page_card_slot(theme, None));
        }
        view::col(Key::named("tray")).child(view::col(Key::named("tray-rows")).children(children))
    }

    /// The default size, taller when the rows need it, up to the surface.
    fn wants(&self, cx: &PageCx<'_>) -> (f32, f32) {
        if cx.owners.tray.is_some() {
            return PAGE_SIZE;
        }
        let rows = drawn(cx.items, cx).len();
        // The open menu's card under its row: its box and the 6 px around it.
        let card = cx
            .tray_card
            .filter(CardView::shown)
            .map_or(0.0, |c| c.box_h + MARGINS);
        let needed = chrome_height(cx.theme) + rows as f32 * cx.theme.row_height + card;
        (PAGE_SIZE.0, PAGE_SIZE.1.max(needed))
    }
}

/// The title a row shows when the item has none.
#[must_use]
pub(crate) fn display_title(row: &TrayRow) -> &str {
    if row.title.is_empty() {
        copy::TRAY_ITEM_UNTITLED
    } else {
        row.title.as_str()
    }
}

/// The status word a row shows and its colour; none for an active item.
#[must_use]
pub(crate) fn status_word(row: &TrayRow) -> Option<(&'static str, Role)> {
    match row.status {
        Status::Active => None,
        Status::Passive => Some((copy::TRAY_PASSIVE, Role::TextFaint)),
        Status::NeedsAttention => Some((copy::TRAY_NEEDS_ATTENTION, Role::Accent)),
    }
}

/// The weight a row's title has: bold while the item needs attention.
#[must_use]
pub(crate) fn title_weight(row: &TrayRow) -> Weight {
    match row.status {
        Status::NeedsAttention => Weight::Semibold,
        Status::Passive | Status::Active => Weight::Regular,
    }
}

/// How a row looks while the app's menu hangs from it as a card
/// (CEL-D-016).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RowLook {
    /// The row is the card's: it shows its press fill (`Role::ControlHover`),
    /// from the press until the card starts to fold.
    pub held: bool,
    /// The app has answered and the card is open: the title is semibold.
    pub open: bool,
}

/// The row of the item at `index` of the list: its icon, `title` (the page
/// passes [`display_title`]; the tray's overflow window cuts it to the room
/// the row has) and, when it is not simply active, a status word; `look` is
/// how the open menu's card changes it.
pub(crate) fn item_row_looking(
    theme: &Theme,
    index: usize,
    row: &TrayRow,
    title: &str,
    look: RowLook,
) -> View {
    let weight = if look.open {
        Weight::Semibold
    } else {
        title_weight(row)
    };
    let title_role = match row.status {
        Status::Passive => Role::TextFaint,
        Status::Active | Status::NeedsAttention => Role::TextHi,
    };
    let icon_key = Key::indexed("tray-icon", index as u64);
    let icon = icon_view(icon_key, &row.icon);
    let mut line = centred(view::row(row_key(index)));
    if look.held {
        line = line.fill(Role::ControlHover);
    }
    line.height(theme.row_height)
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
        .children(status_word(row).map(|(label, role)| {
            let mut text = view::text(
                Key::indexed("tray-status", index as u64),
                label,
                text_style(theme.type_scale.caption, Weight::Regular, false, role),
            );
            text.style.flex_shrink = 0.0;
            text
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
            content_w: None,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            theme: &celestina_paint::Theme::dark(),
            max_size: (760.0, 560.0),
            model: None,
            output: "DP-1",
            owners,
            do_not_disturb: false,
            history: &[],
            items,
            tray_card: None,
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
            item_is_menu: false,
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
        // The card's slot ends the rows, hidden, so a box never fades in.
        let all = &v.children[0].children;
        assert_eq!(all.len(), 4);
        assert_eq!(all[3].key, Key::named("tray-card-slot"));
        let rows = &all[..3];
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
            content_w: None,
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            theme: &celestina_paint::Theme::dark(),
            max_size: (760.0, 560.0),
            model: None,
            output: "DP-1",
            owners: &NameOwner::default(),
            do_not_disturb: false,
            history: &[],
            items: &many,
            tray_card: None,
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            connectivity: &crate::connectivity::state::ConnectivityState::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
            media: &crate::island::pages::media::NO_MEDIA,
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
            pixmaps: vec![Pixmap {
                w: 1,
                h: 1,
                argb: vec![fill; 4],
            }],
            ..TrayItem::default()
        }
    }

    /// A square pixmap of side `side`.
    fn square(side: u32) -> Pixmap {
        Pixmap {
            w: side,
            h: side,
            argb: vec![0; (side * side * 4) as usize],
        }
    }

    /// A scratch directory under the temp dir, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Scratch {
            let dir = std::env::temp_dir().join(format!(
                "celestina-tray-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).expect("scratch dir");
            Scratch(dir)
        }

        fn file(&self, rel: &str) -> PathBuf {
            let path = self.0.join(rel);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("dirs");
            std::fs::write(&path, b"x").expect("file");
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn icon_of(item: &TrayItem, scale: f32, theme: &[&str]) -> IconSource<'static> {
        let theme: Vec<String> = theme.iter().map(|s| (*s).to_owned()).collect();
        // The host resolves the files when it reads the item; so do the tests.
        let mut item = item.clone();
        let dir = item.icon_theme_path.clone();
        item.icon_file = crate::tray::item::icon_file_of(&item.icon_name, dir.as_deref());
        item.attention_icon_file =
            crate::tray::item::icon_file_of(&item.attention_icon_name, dir.as_deref());
        match resolve_tray_icon(&item, scale, &|name, _| theme.iter().any(|t| t == name)) {
            IconSource::Icon(icon) => IconSource::Icon(icon),
            IconSource::Pixmap(p) => IconSource::Icon(RowIcon::Pixmap(PixmapId(p.w))),
        }
    }

    #[test]
    fn icon_from_item_theme_path_wins() {
        let dir = Scratch::new("theme-path");
        let file = dir.file("app/mail.png");
        let item = TrayItem {
            icon_name: "mail".to_owned(),
            icon_theme_path: Some(dir.0.join("app")),
            pixmaps: vec![square(24)],
            ..TrayItem::default()
        };
        // The user's theme has `mail` too; the item's own directory wins.
        let real = file.canonicalize().expect("canonical");
        assert_eq!(
            icon_of(&item, 1.0, &["mail"]),
            IconSource::Icon(RowIcon::File(real))
        );
        // Without the file there, the user's theme is next, then the pixmap.
        let elsewhere = TrayItem {
            icon_theme_path: Some(dir.0.join("empty")),
            ..item.clone()
        };
        assert_eq!(
            icon_of(&elsewhere, 1.0, &["mail"]),
            IconSource::Icon(RowIcon::Theme("mail".to_owned()))
        );
        assert_eq!(
            icon_of(&elsewhere, 1.0, &[]),
            IconSource::Icon(RowIcon::Pixmap(PixmapId(24)))
        );
    }

    #[test]
    fn absolute_icon_name_under_a_root_is_used() {
        let dir = Scratch::new("abs-in");
        let file = dir.file("app/icons/tray.svg");
        let item = TrayItem {
            icon_name: file.to_string_lossy().into_owned(),
            icon_theme_path: Some(dir.0.join("app")),
            ..TrayItem::default()
        };
        assert_eq!(
            icon_of(&item, 1.0, &[]),
            IconSource::Icon(RowIcon::File(file.canonicalize().expect("canonical")))
        );
    }

    #[test]
    fn absolute_icon_name_outside_the_roots_is_refused() {
        let dir = Scratch::new("abs-out");
        let secret = dir.file("elsewhere/secret.png");
        dir.file("app/inside.png");
        let link = dir.0.join("app/link.png");
        std::os::unix::fs::symlink(&secret, &link).expect("symlink");
        let hostile = |name: String| TrayItem {
            icon_name: name,
            icon_theme_path: Some(dir.0.join("app")),
            ..TrayItem::default()
        };
        for name in [
            "/etc/shadow".to_owned(),
            "../../x.png".to_owned(),
            secret.to_string_lossy().into_owned(),
            link.to_string_lossy().into_owned(),
            format!("{}/app/../elsewhere/secret.png", dir.0.display()),
        ] {
            // Refused: the fallback, even when the theme claims every name.
            assert_eq!(
                icon_of(&hostile(name.clone()), 1.0, &[&name]),
                IconSource::Icon(RowIcon::default()),
                "{name}"
            );
        }
    }

    #[test]
    fn a_broad_theme_dir_makes_no_root() {
        // `IconThemePath` "/" must not turn every absolute name into an
        // allowed one: the host resolves nothing under it.
        let dir = Scratch::new("broad");
        let file = dir.file("anywhere/secret.png");
        let item = TrayItem {
            icon_name: file.to_string_lossy().into_owned(),
            icon_theme_path: Some(PathBuf::from("/")),
            ..TrayItem::default()
        };
        assert_eq!(
            icon_of(&item, 1.0, &[]),
            IconSource::Icon(RowIcon::default())
        );
    }

    #[test]
    fn pixmap_nearest_to_scaled_size() {
        let item = TrayItem {
            pixmaps: vec![square(32), square(48), square(64)],
            ..TrayItem::default()
        };
        assert_eq!(
            icon_of(&item, 1.0, &[]),
            IconSource::Icon(RowIcon::Pixmap(PixmapId(32))),
            "24 px: 32 is nearest"
        );
        assert_eq!(
            icon_of(&item, 2.0, &[]),
            IconSource::Icon(RowIcon::Pixmap(PixmapId(48))),
            "scale 2 is the 48 px candidate"
        );
        assert_eq!(
            icon_of(&item, 3.0, &[]),
            IconSource::Icon(RowIcon::Pixmap(PixmapId(64)))
        );
        // A non-finite scale counts as 1.
        assert_eq!(icon_of(&item, f32::NAN, &[]), icon_of(&item, 1.0, &[]));
    }

    #[test]
    fn attention_icon_replaces_the_icon_when_needs_attention() {
        let mut item = TrayItem {
            icon_name: "mail".to_owned(),
            attention_icon_name: "mail-unread".to_owned(),
            ..TrayItem::default()
        };
        let theme = ["mail", "mail-unread"];
        assert_eq!(
            icon_of(&item, 1.0, &theme),
            IconSource::Icon(RowIcon::Theme("mail".to_owned())),
            "active: the normal icon"
        );
        item.status = Status::NeedsAttention;
        assert_eq!(
            icon_of(&item, 1.0, &theme),
            IconSource::Icon(RowIcon::Theme("mail-unread".to_owned()))
        );
        // The attention pixmap stands in when the name is not usable.
        item.attention_icon_name.clear();
        item.attention_pixmaps = vec![square(24)];
        assert_eq!(
            icon_of(&item, 1.0, &theme),
            IconSource::Icon(RowIcon::Pixmap(PixmapId(24)))
        );
        // Nothing usable of its own: the normal icon.
        item.attention_pixmaps.clear();
        assert_eq!(
            icon_of(&item, 1.0, &theme),
            IconSource::Icon(RowIcon::Theme("mail".to_owned()))
        );
    }

    #[test]
    fn missing_icon_draws_the_bundled_fallback() {
        let item = TrayItem {
            icon_name: "no-such-icon".to_owned(),
            ..TrayItem::default()
        };
        let icon = RowIcon::Bundled("app-window");
        assert_eq!(icon_of(&item, 1.0, &[]), IconSource::Icon(icon.clone()));
        assert_eq!(RowIcon::default(), icon);
        let drawn = icon_view(Key::named("i"), &icon);
        assert!(matches!(
            &drawn.kind,
            ViewKind::Icon {
                key: IconKey::Semantic("app-window"),
                tint: Some(Role::TextLo),
                ..
            }
        ));
        // The glyph is bundled with the style assets.
        assert!(
            celestina_paint::default_icon_dirs()
                .iter()
                .any(|d| d.join("app-window.svg").is_file()),
            "celestina-style/icons/app-window.svg"
        );
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
        let rows = IconCache::default().rows(&[item], 1.0, &mut Fake::default());
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
        let rows = cache.rows(&[themed, private, nothing], 1.0, &mut fake);
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
        let first = cache.rows(&items, 1.0, &mut fake);
        assert_eq!(cache.len(), 2);
        let again = cache.rows(&items, 1.0, &mut fake);
        assert_eq!(first, again, "re-sent items upload nothing");
        assert_eq!(fake.uploads, 2);
        let changed = [pixmap_item("a", 9), pixmap_item("b", 2)];
        let after = cache.rows(&changed, 1.0, &mut fake);
        assert_ne!(first[0].icon, after[0].icon);
        assert_eq!(first[1].icon, after[1].icon);
        assert_eq!(
            fake.released,
            vec![PixmapId(1)],
            "the changed image's old texture"
        );
        let gone = cache.rows(&changed[1..], 1.0, &mut fake);
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
        let rows = cache.rows(&[pixmap_item("a", 1)], 1.0, &mut fake);
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
        cache.rows(&[a.clone()], 1.0, &mut fake);
        a.pixmaps.clear();
        a.icon_name = "mail".to_owned();
        let rows = cache.rows(&[a], 1.0, &mut fake);
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
