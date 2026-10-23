//! The overview's view: a Haze card with the mode tabs on top and, in
//! Desktop mode, one strip per workspace of the page that holds the
//! selection (design §6, Overview; `celestina-style/DESIGN.md`).
//!
//! A strip draws the workspace the way `workspace_map` arranged it: each
//! column takes its share of the strip's width and each tile its share of
//! its column's height, so the picture is the layout, not a list. A tile is
//! the window's icon and title (cut by grapheme to what the tile can hold);
//! tiles are geometry only. A workspace the caller has a preview for (the
//! whole-output capture of the workspace that was on screen) shows it as a
//! rounded, dimmed backdrop of its strip, behind the tiles, fitted to the
//! output's aspect ratio. Windows floating over the workspace sit in a row
//! under the columns. Every colour is a `Role` of the theme.

use std::collections::HashMap;

use celestina_paint::{IconKey, PixmapId, Rect, Role, TextStyle, Theme, Weight};
use celestina_shell::workspace_map::Map;
use celestina_ui::{view, Key, View, ViewKind};
use taffy::AlignItems;

use super::state::{Mode, OverviewState, ShortcutLine, WorkspaceView};
use crate::{
    copy,
    island::pages::activity::{truncate_graphemes, MAX_TITLE_GRAPHEMES},
};

/// The widest the card grows, logical px.
pub const MAX_CARD_W: f32 = 1680.0;
/// The tallest the card grows, logical px.
pub const MAX_CARD_H: f32 = 1040.0;
/// The room kept between the card and the surface's edges.
pub const SURFACE_MARGIN: f32 = 48.0;
/// The tallest a workspace strip grows, so a lone workspace is not a wall.
pub const MAX_STRIP_H: f32 = 420.0;
/// A tile's icon.
pub const ICON_SIZE: f32 = 20.0;
/// The height of the row of floating windows under the columns.
pub const FLOATING_H: f32 = 56.0;
/// The widest a floating window's tile grows.
pub const FLOATING_TILE_W: f32 = 180.0;
/// The focus dot at the end of the focused window's title.
const DOT: f32 = 8.0;
/// The icon of a window whose application has none.
const FALLBACK_ICON: &str = "app-window";

/// How strongly a workspace preview shows through behind its tiles.
const PREVIEW_ALPHA: f32 = 0.4;

/// A workspace preview already uploaded to the GPU, as the view needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewPixmap {
    pub id: PixmapId,
    /// The image's size in pixels; only its aspect ratio is used.
    pub size: (u32, u32),
}

/// The surface: the root of the view, transparent, holding the card.
#[must_use]
pub fn surface_key() -> Key {
    Key::named("overview-surface")
}

/// The card.
#[must_use]
pub fn card_key() -> Key {
    Key::named("overview")
}

/// The tab of `mode`.
#[must_use]
pub fn tab_key(mode: Mode) -> Key {
    match mode {
        Mode::Desktop => Key::named("overview-tab-desktop"),
        Mode::Shortcuts => Key::named("overview-tab-shortcuts"),
    }
}

/// The header of workspace `id`.
#[must_use]
pub fn workspace_key(id: u64) -> Key {
    Key::indexed("overview-ws", id)
}

/// The strip that holds workspace `id`'s tiles.
#[must_use]
pub fn strip_key(id: u64) -> Key {
    Key::indexed("overview-strip", id)
}

/// The tile of window `id`.
#[must_use]
pub fn window_key(id: u64) -> Key {
    Key::indexed("overview-win", id)
}

/// The mode a click on `key` switches to, if `key` is a tab.
#[must_use]
pub fn tab_of(key: Key) -> Option<Mode> {
    [Mode::Desktop, Mode::Shortcuts]
        .into_iter()
        .find(|&m| tab_key(m) == key)
}

/// The card's logical size on a surface of `surface`: the surface less the
/// margin, never above the maximum.
#[must_use]
pub fn card_size(surface: (f32, f32)) -> (f32, f32) {
    (
        (surface.0 - 2.0 * SURFACE_MARGIN).clamp(0.0, MAX_CARD_W),
        (surface.1 - 2.0 * SURFACE_MARGIN).clamp(0.0, MAX_CARD_H),
    )
}

/// Where a card of `card` size sits in a surface of `surface`: centred, on
/// whole logical pixels, never above or left of the origin.
#[must_use]
pub fn card_rect(surface: (f32, f32), card: (f32, f32)) -> Rect {
    let offset = |outer: f32, inner: f32| ((outer - inner) / 2.0).floor().max(0.0);
    Rect {
        x: offset(surface.0, card.0),
        y: offset(surface.1, card.1),
        w: card.0,
        h: card.1,
    }
}

/// The room under the tabs, inside the card's padding.
#[must_use]
pub fn body_size(card: (f32, f32), theme: &Theme) -> (f32, f32) {
    (
        (card.0 - 2.0 * theme.spacing.md).max(0.0),
        (card.1 - 3.0 * theme.spacing.md - theme.top_bar).max(0.0),
    )
}

/// The height of one workspace's strip when `count` workspaces share a body
/// of `body_h`: what is left after the gaps and the headers, never above
/// [`MAX_STRIP_H`].
#[must_use]
pub fn strip_height(body_h: f32, count: usize, theme: &Theme) -> f32 {
    if count == 0 {
        return 0.0;
    }
    let n = count as f32;
    let per = (body_h - theme.spacing.md * (n - 1.0)) / n;
    (per - theme.row_height - theme.spacing.xs).clamp(0.0, MAX_STRIP_H)
}

/// One tile's place in its strip, in strip coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileRect {
    pub id: u64,
    pub rect: Rect,
    /// A window floating over the workspace, in the row under the columns.
    pub floating: bool,
}

/// The tiles of `map` in a strip of `size`, `gap` apart: each column gets
/// its width share of the room the gaps leave and each tile its height share
/// of its column's; the floating windows share a row of [`FLOATING_H`] under
/// the columns (the whole strip when there are no columns). A window whose id
/// is not a number is left out.
#[must_use]
pub fn strip_layout(map: &Map, size: (f32, f32), gap: f32) -> Vec<TileRect> {
    let (w, h) = size;
    let floating_h = if map.floating.is_empty() {
        0.0
    } else {
        FLOATING_H.min(h)
    };
    let columns_h = if map.columns.is_empty() {
        0.0
    } else if floating_h > 0.0 {
        (h - floating_h - gap).max(0.0)
    } else {
        h
    };
    let mut out = Vec::new();
    let n = map.columns.len();
    let room_w = (w - gap * n.saturating_sub(1) as f32).max(0.0);
    let mut x = 0.0_f32;
    for column in &map.columns {
        let col_w = (column.width_share as f32 * room_w).max(0.0);
        let m = column.tiles.len();
        let room_h = (columns_h - gap * m.saturating_sub(1) as f32).max(0.0);
        let mut y = 0.0_f32;
        for tile in &column.tiles {
            let tile_h = (tile.height_share as f32 * room_h).max(0.0);
            if let Ok(id) = tile.window.id.parse() {
                out.push(TileRect {
                    id,
                    rect: Rect {
                        x,
                        y,
                        w: col_w,
                        h: tile_h,
                    },
                    floating: false,
                });
            }
            y += tile_h + gap;
        }
        x += col_w + gap;
    }
    let f = map.floating.len();
    if f > 0 {
        let tile_w = ((w - gap * (f - 1) as f32) / f as f32).clamp(0.0, FLOATING_TILE_W);
        let y = if map.columns.is_empty() {
            0.0
        } else {
            columns_h + gap
        };
        for (i, window) in map.floating.iter().enumerate() {
            if let Ok(id) = window.id.parse() {
                out.push(TileRect {
                    id,
                    rect: Rect {
                        x: i as f32 * (tile_w + gap),
                        y,
                        w: tile_w,
                        h: floating_h,
                    },
                    floating: true,
                });
            }
        }
    }
    out
}

fn style(size: f32, role: Role) -> TextStyle {
    TextStyle {
        size,
        weight: Weight::Regular,
        tabular: false,
        role,
    }
}

fn centred(mut v: View) -> View {
    v.style.align_items = AlignItems::CENTER;
    v
}

/// The overview for `state` in `theme` on a surface of `surface` size: a
/// transparent root as large as the surface, with the card centred in it.
/// `pixmaps` holds the uploaded previews by workspace id.
#[must_use]
pub fn overview(
    theme: &Theme,
    state: &OverviewState,
    surface: (f32, f32),
    pixmaps: &HashMap<u64, PreviewPixmap>,
) -> View {
    let size = card_size(surface);
    let at = card_rect(surface, size);
    view::col(surface_key())
        .size(surface.0, surface.1)
        .child(card(theme, state, size, pixmaps).absolute(at.x, at.y))
}

/// The card of `size` for `state`.
#[must_use]
pub fn card(
    theme: &Theme,
    state: &OverviewState,
    size: (f32, f32),
    pixmaps: &HashMap<u64, PreviewPixmap>,
) -> View {
    let tabs = view::row(Key::named("overview-tabs"))
        .height(theme.top_bar)
        .gap(theme.spacing.sm)
        .children([
            tab(theme, Mode::Desktop, state.mode, copy::OVERVIEW_TAB_DESKTOP),
            tab(
                theme,
                Mode::Shortcuts,
                state.mode,
                copy::OVERVIEW_TAB_SHORTCUTS,
            ),
        ]);
    let body = match state.mode {
        Mode::Desktop => desktop(theme, state, body_size(size, theme), pixmaps),
        Mode::Shortcuts => shortcuts(theme, state, body_size(size, theme)),
    };
    view::col(card_key())
        .haze(theme.radius_lg)
        .size(size.0, size.1)
        .padding(theme.spacing.md)
        .gap(theme.spacing.md)
        .child(tabs)
        .child(body)
}

fn tab(theme: &Theme, mode: Mode, current: Mode, label: &str) -> View {
    let mut v = centred(view::row(tab_key(mode)))
        .height(theme.top_bar)
        .padding_xy(theme.spacing.lg, 0.0)
        .radius(theme.top_bar / 2.0)
        .interactive();
    let role = if mode == current {
        v = v.fill(Role::ControlFill);
        Role::TextHi
    } else {
        Role::TextLo
    };
    v.child(view::text(
        Key::indexed("overview-tab-label", mode as u64),
        label,
        style(theme.type_scale.body, role),
    ))
}

/// The width of the chord column of a Shortcuts row.
pub const CHORD_W: f32 = 260.0;

/// How many Shortcuts rows fit a body of `body_h`: one per row height, at
/// least one.
#[must_use]
pub fn shortcut_rows(body_h: f32, theme: &Theme) -> usize {
    ((body_h / theme.row_height).floor().max(1.0)) as usize
}

/// Shortcuts mode: the rows of the groups from the scroll offset on, as many
/// as the body holds. A heading is the group's title; a bind is its chord
/// and what it does. Before the bindings arrive the body is empty.
#[must_use]
pub fn shortcuts(theme: &Theme, state: &OverviewState, body: (f32, f32)) -> View {
    let root = view::col(Key::named("overview-shortcuts"));
    let lines = state.shortcut_lines();
    let rows = shortcut_rows(body.1, theme);
    root.children(
        lines
            .iter()
            .enumerate()
            .skip(state.scroll_offset())
            .take(rows)
            .map(|(i, line)| shortcut_row(theme, i as u64, line)),
    )
}

fn shortcut_row(theme: &Theme, index: u64, line: &ShortcutLine) -> View {
    let row = centred(view::row(Key::indexed("overview-sc-row", index)))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.md);
    match line {
        ShortcutLine::Heading(title) => row.child(view::text(
            Key::indexed("overview-sc-heading", index),
            title,
            style(theme.type_scale.row_title, Role::TextHi),
        )),
        ShortcutLine::Note(text) => row.child(view::text(
            Key::indexed("overview-sc-note", index),
            text,
            style(theme.type_scale.body, Role::TextLo),
        )),
        ShortcutLine::Bind { chord, label } => row
            .child(
                view::text(
                    Key::indexed("overview-sc-chord", index),
                    chord,
                    style(theme.type_scale.body, Role::TextHi),
                )
                .width(CHORD_W),
            )
            .child(
                view::text(
                    Key::indexed("overview-sc-label", index),
                    label,
                    style(theme.type_scale.body, Role::TextLo),
                )
                .grow(),
            ),
    }
}

/// Desktop mode: one strip per workspace of the page, or one line when the
/// output has none.
fn desktop(
    theme: &Theme,
    state: &OverviewState,
    body: (f32, f32),
    pixmaps: &HashMap<u64, PreviewPixmap>,
) -> View {
    let root = view::col(Key::named("overview-desktop")).gap(theme.spacing.md);
    let page = state.page();
    if page.is_empty() {
        return root.child(view::text(
            Key::named("overview-empty"),
            copy::OVERVIEW_EMPTY,
            style(theme.type_scale.body, Role::TextLo),
        ));
    }
    let strip_h = strip_height(body.1, page.len(), theme);
    root.children(page.clone().filter_map(|i| {
        let ws = state.workspaces.get(i)?;
        let chosen =
            (i == state.selected.workspace).then_some((state.selected.column, state.selected.row));
        Some(workspace(theme, ws, (body.0, strip_h), chosen, pixmaps))
    }))
}

/// One workspace: its header over its strip. `chosen` is the selected
/// (column, row) of the grid when the selection is in this workspace.
fn workspace(
    theme: &Theme,
    ws: &WorkspaceView,
    strip: (f32, f32),
    chosen: Option<(usize, usize)>,
    pixmaps: &HashMap<u64, PreviewPixmap>,
) -> View {
    let selected_id = chosen
        .and_then(|(c, r)| ws.grid.get(c).and_then(|col| col.get(r)))
        .copied();
    let header_selected = chosen.is_some() && ws.grid.is_empty();
    let mut header = centred(view::row(workspace_key(ws.id)))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .gap(theme.spacing.sm)
        .radius(theme.radius_md)
        .interactive()
        .child(view::text(
            Key::indexed("overview-ws-label", ws.id),
            &ws.label,
            style(
                theme.type_scale.row_title,
                if ws.active {
                    Role::TextHi
                } else {
                    Role::TextLo
                },
            ),
        ));
    if header_selected {
        header = header.fill(Role::ControlFill);
    }
    if ws.map.hidden > 0 {
        header = header.child(view::text(
            Key::indexed("overview-ws-hidden", ws.id),
            &copy::overview_hidden(ws.map.hidden),
            style(theme.type_scale.row_secondary, Role::TextFaint),
        ));
    }
    let tiles = strip_layout(&ws.map, strip, theme.spacing.sm);
    let body = view::col(strip_key(ws.id)).size(strip.0, strip.1);
    let body = match pixmaps
        .get(&ws.id)
        .and_then(|p| preview(theme, ws.id, p, strip))
    {
        Some(backdrop) => body.child(backdrop),
        None => body,
    };
    let body = if tiles.is_empty() {
        body.child(view::text(
            Key::indexed("overview-ws-empty", ws.id),
            copy::OVERVIEW_WORKSPACE_EMPTY,
            style(theme.type_scale.body, Role::TextFaint),
        ))
    } else {
        body.children(
            tiles
                .iter()
                .map(|t| tile(theme, ws, t, selected_id == Some(t.id))),
        )
    };
    view::col(Key::indexed("overview-section", ws.id))
        .gap(theme.spacing.xs)
        .child(header)
        .child(body)
}

/// The most graphemes of a title a tile of `width` holds beside its icon
/// (and the focus dot): an estimate from the type size, never above the
/// page's own maximum and never below one.
fn title_room(width: f32, theme: &Theme, pad: f32, focused: bool) -> usize {
    let dot = if focused { DOT + theme.spacing.xs } else { 0.0 };
    let text_w = width - 2.0 * pad - ICON_SIZE - theme.spacing.xs - dot;
    let glyph = (theme.type_scale.body * 0.6).max(1.0);
    ((text_w / glyph).floor().max(1.0) as usize).min(MAX_TITLE_GRAPHEMES)
}

/// One window's tile at `at` in its strip.
fn tile(theme: &Theme, ws: &WorkspaceView, at: &TileRect, selected: bool) -> View {
    let pad = theme.spacing.sm;
    let gap = theme.spacing.xs;
    let info = ws.info(at.id);
    let focused = info.is_some_and(|i| i.focused);
    let raw = info.map_or("", |i| i.title.as_str());
    let title = if raw.is_empty() {
        copy::OVERVIEW_WINDOW_UNTITLED.to_owned()
    } else {
        truncate_graphemes(raw, title_room(at.rect.w, theme, pad, focused))
    };
    let (glyph, tint) = match info.and_then(|i| i.icon.clone()) {
        Some(name) if !name.is_empty() => (IconKey::Theme(name), None),
        _ => (IconKey::Semantic(FALLBACK_ICON), Some(Role::TextLo)),
    };
    let header = centred(view::row(Key::indexed("overview-win-head", at.id)))
        .height(ICON_SIZE)
        .gap(gap)
        .child(
            View::new(
                Key::indexed("overview-win-icon", at.id),
                ViewKind::Icon {
                    key: glyph,
                    tint,
                    size: ICON_SIZE,
                },
            )
            .size(ICON_SIZE, ICON_SIZE),
        )
        .child(
            view::text(
                Key::indexed("overview-win-title", at.id),
                &title,
                style(
                    theme.type_scale.body,
                    if focused { Role::TextHi } else { Role::TextLo },
                ),
            )
            .grow(),
        )
        .children(focused.then(|| {
            view::row(Key::indexed("overview-win-focus", at.id))
                .size(DOT, DOT)
                .radius(DOT / 2.0)
                .fill(Role::Accent)
        }));
    view::col(window_key(at.id))
        .size(at.rect.w, at.rect.h)
        .padding(pad)
        .gap(gap)
        .radius(theme.radius_md)
        .clip()
        .interactive()
        .fill(if selected {
            Role::ControlHover
        } else {
            Role::ControlFill
        })
        .absolute(at.rect.x, at.rect.y)
        .child(header)
}

/// The backdrop of a strip of `strip` size: the workspace's preview, fitted
/// whole and centred, rounded and dimmed, or `None` when the strip is too
/// small for it.
fn preview(theme: &Theme, id: u64, pixmap: &PreviewPixmap, strip: (f32, f32)) -> Option<View> {
    let (w, h) = fit(pixmap.size, strip)?;
    Some(
        view::col(Key::indexed("overview-ws-preview", id))
            .size(w, h)
            .radius(theme.radius_md)
            .clip()
            .absolute(((strip.0 - w) / 2.0).floor(), ((strip.1 - h) / 2.0).floor())
            .child(
                view::image(
                    Key::indexed("overview-ws-preview-img", id),
                    pixmap.id,
                    PREVIEW_ALPHA,
                )
                .size(w, h),
            ),
    )
}

/// The size of an image of `image` pixels fitted whole inside `room`, or
/// `None` when the room is under 8 px either way or the image has no area.
fn fit(image: (u32, u32), room: (f32, f32)) -> Option<(f32, f32)> {
    if room.0 < 8.0 || room.1 < 8.0 || image.0 == 0 || image.1 == 0 {
        return None;
    }
    let (iw, ih) = (image.0 as f32, image.1 as f32);
    let scale = (room.0 / iw).min(room.1 / ih);
    Some(((iw * scale).floor().max(1.0), (ih * scale).floor().max(1.0)))
}

/// The accessible name of window `id`'s tile.
#[must_use]
pub fn window_name(ws: &WorkspaceView, id: u64) -> Option<String> {
    let info = ws.info(id)?;
    let title = if info.title.is_empty() {
        copy::OVERVIEW_WINDOW_UNTITLED
    } else {
        &info.title
    };
    Some(copy::overview_window_name(title, info.focused))
}

/// The accessible name of `ws`'s header.
#[must_use]
pub fn workspace_name(ws: &WorkspaceView) -> String {
    copy::overview_workspace_name(&ws.label, ws.window_count())
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use celestina_shell::keybinds::{Bind, BindGroup};
    use celestina_ui::Tree;

    use super::*;
    use crate::{
        overview::state::{fixtures::*, Scroll},
        providers::keybinds::{Keybinds, KeybindsUnavailable},
    };

    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
    }

    const SURFACE: (f32, f32) = (1920.0, 1080.0);

    fn laid_out(state: &OverviewState, pixmaps: &HashMap<u64, PreviewPixmap>) -> (Tree, Rect) {
        let theme = Theme::dark();
        let mut tree = Tree::new(false, &theme);
        tree.apply(overview(&theme, state, SURFACE, pixmaps), Instant::now());
        tree.layout_measured(SURFACE, fake_measure);
        (tree, card_rect(SURFACE, card_size(SURFACE)))
    }

    fn rect_of(tree: &Tree, key: Key) -> Option<Rect> {
        tree.node_by_key(key).map(|id| tree.rect(id))
    }

    #[test]
    fn tile_geometry_follows_the_shares_of_the_map() {
        let state = OverviewState::new(&model(), "DP-1");
        let tiles = strip_layout(&state.workspaces[0].map, (1000.0, 400.0), 10.0);
        let by = |id: u64| tiles.iter().find(|t| t.id == id).copied();
        // Columns: widths 400 / 800 → shares 1/3 and 2/3 of 1000 - 10 gap.
        let (a, b, c) = (by(10), by(11), by(12));
        let (Some(a), Some(b), Some(c)) = (a, b, c) else {
            panic!("tiles 10, 11 and 12 are laid out");
        };
        assert!((a.rect.w - 330.0).abs() < 0.01 && (b.rect.w - 660.0).abs() < 0.01);
        assert!((b.rect.x - 340.0).abs() < 0.01 && a.rect.x == 0.0);
        // The columns' room is the strip less the floating row and a gap.
        let columns_h = 400.0 - FLOATING_H - 10.0;
        assert!((a.rect.h - columns_h).abs() < 0.01);
        // Two equal-height tiles split their column, a gap between them.
        assert!((b.rect.h - (columns_h - 10.0) / 2.0).abs() < 0.01);
        assert!((c.rect.y - (b.rect.h + 10.0)).abs() < 0.01);
        assert!(!a.floating);
        // The floating window sits under the columns.
        let Some(f) = by(13) else {
            panic!("the floating window is laid out");
        };
        assert!(f.floating && (f.rect.y - (columns_h + 10.0)).abs() < 0.01);
        assert_eq!(f.rect.h, FLOATING_H);
        assert_eq!(f.rect.w, FLOATING_TILE_W);
        // Nothing leaves the strip.
        assert!(tiles
            .iter()
            .all(|t| t.rect.x + t.rect.w <= 1000.01 && t.rect.y + t.rect.h <= 400.01));
    }

    #[test]
    fn an_empty_map_has_no_tiles_and_floating_only_fills_the_strip() {
        let empty = OverviewState::new(&model(), "DP-1");
        assert!(strip_layout(&empty.workspaces[1].map, (500.0, 200.0), 8.0).is_empty());
        let mut m = model();
        m.windows.retain(|w| w.id == 13);
        let only = OverviewState::new(&m, "DP-1");
        let tiles = strip_layout(&only.workspaces[0].map, (500.0, 200.0), 8.0);
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].rect.y, 0.0);
        // A zero-size strip never yields a negative size.
        let squashed = strip_layout(&only.workspaces[0].map, (0.0, 0.0), 8.0);
        assert!(squashed.iter().all(|t| t.rect.w >= 0.0 && t.rect.h >= 0.0));
    }

    #[test]
    fn strips_share_the_body_and_never_exceed_the_cap() {
        let theme = Theme::dark();
        assert_eq!(strip_height(900.0, 0, &theme), 0.0);
        assert_eq!(strip_height(900.0, 1, &theme), MAX_STRIP_H);
        let three = strip_height(900.0, 3, &theme);
        assert!(three > 0.0 && three < MAX_STRIP_H);
        assert_eq!(strip_height(10.0, 8, &theme), 0.0, "never negative");
    }

    #[test]
    fn the_view_has_a_section_per_workspace_and_a_tile_per_window() {
        let state = OverviewState::new(&model(), "DP-1");
        let (tree, card) = laid_out(&state, &HashMap::new());
        assert_eq!(rect_of(&tree, card_key()), Some(card));
        for id in [1, 2, 3] {
            assert!(tree.node_by_key(workspace_key(id)).is_some(), "ws {id}");
        }
        for id in [10, 11, 12, 13, 30] {
            let r = rect_of(&tree, window_key(id));
            assert!(
                r.is_some_and(|r| r.w > 0.0
                    && r.h > 0.0
                    && r.x >= card.x
                    && r.x + r.w <= card.x + card.w
                    && r.y + r.h <= card.y + card.h),
                "tile {id} is inside the card"
            );
        }
        assert!(tree.node_by_key(window_key(99)).is_none());
        assert!(
            tree.node_by_key(Key::indexed("overview-ws-empty", 2))
                .is_some(),
            "an empty workspace says so"
        );
        // Columns run left to right in layout too.
        let (a, b) = (
            rect_of(&tree, window_key(10)),
            rect_of(&tree, window_key(11)),
        );
        assert!(a.zip(b).is_some_and(|(a, b)| a.x < b.x));
    }

    #[test]
    fn a_workspace_shows_its_preview_behind_the_tiles_only_when_it_has_one() {
        let state = OverviewState::new(&model(), "DP-1");
        let none = card(&Theme::dark(), &state, card_size(SURFACE), &HashMap::new());
        let has = |v: &View, key: Key| {
            let mut stack = vec![v];
            while let Some(n) = stack.pop() {
                if n.key == key {
                    return true;
                }
                stack.extend(n.children.iter());
            }
            false
        };
        let backdrop = |id: u64| Key::indexed("overview-ws-preview-img", id);
        assert!(!has(&none, backdrop(1)));
        let pixmaps = HashMap::from([(
            1,
            PreviewPixmap {
                id: PixmapId(7),
                size: (640, 360),
            },
        )]);
        let with = card(&Theme::dark(), &state, card_size(SURFACE), &pixmaps);
        assert!(has(&with, backdrop(1)));
        assert!(!has(&with, backdrop(2)), "only the workspace that has one");
        let (tree, _) = laid_out(&state, &pixmaps);
        let (strip, pic) = (
            rect_of(&tree, strip_key(1)),
            rect_of(&tree, Key::indexed("overview-ws-preview", 1)),
        );
        assert!(strip.zip(pic).is_some_and(|(s, p)| p.x >= s.x
            && p.y >= s.y
            && p.x + p.w <= s.x + s.w + 0.5
            && p.y + p.h <= s.y + s.h + 0.5
            && (p.w / p.h - 640.0 / 360.0).abs() < 0.05));
    }

    #[test]
    fn the_preview_sits_behind_the_tiles_and_keeps_their_text() {
        let state = OverviewState::new(&model(), "DP-1");
        let pixmaps = HashMap::from([(
            1,
            PreviewPixmap {
                id: PixmapId(7),
                size: (640, 360),
            },
        )]);
        let v = card(&Theme::dark(), &state, card_size(SURFACE), &pixmaps);
        let mut stack = vec![&v];
        let mut order = None;
        while let Some(n) = stack.pop() {
            if n.key == strip_key(1) {
                order = Some(n.children.iter().map(|c| c.key).collect::<Vec<_>>());
            }
            stack.extend(n.children.iter());
        }
        let order = order.unwrap_or_default();
        assert_eq!(order.first(), Some(&Key::indexed("overview-ws-preview", 1)));
        assert!(order.contains(&window_key(11)), "tiles are still drawn");
    }

    #[test]
    fn fitting_keeps_the_aspect_and_refuses_no_room() {
        assert_eq!(fit((320, 160), (100.0, 100.0)), Some((100.0, 50.0)));
        assert_eq!(fit((100, 400), (100.0, 100.0)), Some((25.0, 100.0)));
        assert_eq!(fit((0, 10), (100.0, 100.0)), None);
        assert_eq!(fit((10, 10), (7.0, 100.0)), None);
    }

    #[test]
    fn the_focused_window_has_the_accent_dot_and_the_selected_tile_the_hover_fill() {
        let mut m = model();
        m.windows[1].is_focused = true;
        let state = OverviewState::new(&m, "DP-1");
        let v = card(&Theme::dark(), &state, card_size(SURFACE), &HashMap::new());
        let find = |key: Key| {
            let mut stack = vec![&v];
            while let Some(n) = stack.pop() {
                if n.key == key {
                    return Some(n.clone());
                }
                stack.extend(n.children.iter());
            }
            None
        };
        assert!(find(Key::indexed("overview-win-focus", 11)).is_some());
        assert!(find(Key::indexed("overview-win-focus", 10)).is_none());
        let fill = |id: u64| match find(window_key(id)).map(|n| n.kind) {
            Some(ViewKind::Box { fill, .. }) => fill,
            _ => None,
        };
        assert_eq!(fill(11), Some(Role::ControlHover), "the selection");
        assert_eq!(fill(10), Some(Role::ControlFill));
        assert!(find(window_key(10)).is_some_and(|n| n.interactive && !n.focusable));
    }

    #[test]
    fn titles_are_cut_by_grapheme_to_what_the_tile_holds() {
        let mut m = model();
        m.windows[0].title = Some("e\u{301}".repeat(80));
        let state = OverviewState::new(&m, "DP-1");
        let v = card(&Theme::dark(), &state, card_size(SURFACE), &HashMap::new());
        let mut stack = vec![&v];
        let mut found = None;
        while let Some(n) = stack.pop() {
            if n.key == Key::indexed("overview-win-title", 10) {
                found = Some(n.clone());
            }
            stack.extend(n.children.iter());
        }
        let Some(ViewKind::Text { text, .. }) = found.map(|n| n.kind) else {
            panic!("tile 10 has a title");
        };
        assert!(text.ends_with('\u{2026}'));
        assert!(text.chars().filter(|c| *c != '\u{301}').count() <= MAX_TITLE_GRAPHEMES);
        assert!(!text.starts_with('\u{301}'), "a mark never starts the text");
    }

    #[test]
    fn an_output_without_workspaces_shows_one_line() {
        let state = OverviewState::new(&model(), "HDMI-A-1");
        let (tree, _) = laid_out(&state, &HashMap::new());
        assert!(tree.node_by_key(Key::named("overview-empty")).is_some());
        assert!(tree.node_by_key(window_key(10)).is_none());
    }

    fn with_keybinds(keybinds: Option<Keybinds>) -> OverviewState {
        let mut state = OverviewState::new(&model(), "DP-1");
        state.set_mode(Mode::Shortcuts);
        if let Some(keybinds) = keybinds {
            state.set_keybinds(keybinds);
        }
        state
    }

    fn groups_of(n: usize) -> Keybinds {
        Keybinds::Groups(vec![BindGroup {
            name: None,
            enter: None,
            binds: (0..n)
                .map(|i| Bind {
                    chord: format!("Mod+{i}"),
                    title: None,
                    action: format!("do-{i}"),
                    program: None,
                })
                .collect(),
        }])
    }

    fn text_of(v: &View, key: Key) -> Option<String> {
        if v.key == key {
            if let ViewKind::Text { text, .. } = &v.kind {
                return Some(text.clone());
            }
        }
        v.children.iter().find_map(|c| text_of(c, key))
    }

    fn texts(v: &View, prefix: &'static str, count: u64) -> Vec<String> {
        (0..count)
            .filter_map(|i| text_of(v, Key::indexed(prefix, i)))
            .collect()
    }

    fn body(state: &OverviewState) -> View {
        let theme = Theme::dark();
        shortcuts(&theme, state, body_size(card_size(SURFACE), &theme))
    }

    #[test]
    fn shortcuts_mode_lists_groups_with_a_heading_and_chord_label_rows() {
        let state = with_keybinds(Some(groups_of(3)));
        let (tree, _) = laid_out(&state, &HashMap::new());
        assert!(tree.node_by_key(Key::named("overview-shortcuts")).is_some());
        assert!(tree.node_by_key(window_key(10)).is_none());
        let v = body(&state);
        assert_eq!(
            texts(&v, "overview-sc-heading", 8),
            [copy::SHORTCUTS_NORMAL_TITLE]
        );
        assert_eq!(
            texts(&v, "overview-sc-chord", 8),
            ["Mod+0", "Mod+1", "Mod+2"]
        );
        assert_eq!(texts(&v, "overview-sc-label", 8), ["do-0", "do-1", "do-2"]);
        assert_eq!(tab_of(tab_key(Mode::Shortcuts)), Some(Mode::Shortcuts));
        assert_eq!(tab_of(card_key()), None);
    }

    #[test]
    fn the_scroll_offset_picks_the_first_row_and_the_body_height_the_count() {
        let theme = Theme::dark();
        let mut state = with_keybinds(Some(groups_of(100)));
        let rows = shortcut_rows(body_size(card_size(SURFACE), &theme).1, &theme);
        state.set_viewport(rows);
        let v = body(&state);
        assert_eq!(v.children.len(), rows, "as many rows as the body holds");
        assert_eq!(texts(&v, "overview-sc-chord", 200).len(), rows - 1);
        state.scroll_by(Scroll::PageDown);
        let first = state.scroll_offset();
        assert!(first > 0);
        let v = body(&state);
        assert!(text_of(&v, Key::indexed("overview-sc-heading", 0)).is_none());
        assert_eq!(
            text_of(&v, Key::indexed("overview-sc-chord", first as u64)),
            Some(format!("Mod+{}", first - 1))
        );
    }

    #[test]
    fn unavailable_and_empty_show_one_explanatory_line() {
        let state = with_keybinds(Some(Keybinds::Unavailable(KeybindsUnavailable::Missing)));
        assert_eq!(
            texts(&body(&state), "overview-sc-note", 4),
            [copy::SHORTCUTS_MISSING]
        );
        let state = with_keybinds(Some(Keybinds::Groups(Vec::new())));
        assert_eq!(
            texts(&body(&state), "overview-sc-note", 4),
            [copy::SHORTCUTS_EMPTY]
        );
        let state = with_keybinds(None);
        assert!(
            body(&state).children.is_empty(),
            "nothing before the answer"
        );
    }

    #[test]
    fn names_say_the_title_and_that_a_window_is_current() {
        let mut m = model();
        m.windows[0].is_focused = true;
        m.windows[3].title = None;
        let state = OverviewState::new(&m, "DP-1");
        let ws = &state.workspaces[0];
        assert_eq!(
            window_name(ws, 10),
            Some(copy::overview_window_name("w10", true))
        );
        assert_eq!(
            window_name(ws, 13),
            Some(copy::overview_window_name(
                copy::OVERVIEW_WINDOW_UNTITLED,
                false
            ))
        );
        assert_eq!(window_name(ws, 99), None);
        assert_eq!(
            workspace_name(ws),
            copy::overview_workspace_name(&ws.label, 4)
        );
    }
}
