//! The notices (CEL-D-011): the Island growing into the newest notification,
//! up to two older ones waiting under it as sheets, on the focused output's
//! own `celestina-toasts` layer surface. This file is what that surface
//! paints and hears: the notification cards (the content of a notice), what
//! a click on one means, the painter of an [`IslandNotice`]'s look and the
//! surface's regions. The motion is the model's (`island_notice.rs`); the
//! app creates and destroys the surface and hands the capsule over
//! (`handover.rs`).

use std::time::{Duration, Instant};

use celestina_paint::{
    silhouette_slices, IconKey, Painter, Rect, Role, Theme, Weight, LINE_HEIGHT,
};
use celestina_shell::notifications::{Image, Notification};
use celestina_ui::{view, Interaction, Key, Tree, UiEvent, View, ViewKind, BTN_LEFT};
use celestina_wayland::{
    Anchoring, FrameCx, InputEvent, KeyboardInteractivity, Layer, LayerSpec, OutputId,
};
use taffy::JustifyContent;

use super::{
    excerpt,
    island_notice::{
        FaceKind, Gesture, IslandNotice, Level, NoticeLook, WheelLatch, LEVEL_H, LEVEL_W, NOTICE_H,
        NOTICE_H_BUTTONS,
    },
};
use crate::{
    copy,
    island::clock::{ClockTick, TICK_MARGIN},
    island::{
        state::SILHOUETTE_STEP,
        view::{centred, text_style, BUTTON_H},
        BTN_MIDDLE, BTN_RIGHT,
    },
    providers::notifications::NotificationsCommand,
};

/// The banners' layer-shell namespace.
pub const NAMESPACE: &str = "celestina-toasts";
/// The most banners stacked at once.
pub const MAX_BANNERS: usize = 3;
/// A notice's width, logical px.
pub const BANNER_W: f32 = 420.0;
/// The surface's height: the notice, a burst of sheets fanned under it and
/// the room a level display and the page's card need.
pub const SURFACE_H: u32 = 440;
/// How far a blurred face's layer reaches past what it paints: three
/// sigmas of the strongest blur, and the offset.
const LAYER_REACH: f32 = 24.0;
/// A blur this weak is drawn plain.
const BLUR_MIN: f32 = 0.05;
/// A banner's icon.
pub const ICON_SIZE: f32 = 32.0;
/// The dismiss button's square and its glyph.
pub const DISMISS_SIZE: f32 = 24.0;
const DISMISS_GLYPH: f32 = 16.0;
/// The action key a click on the banner's body runs when offered.
pub const DEFAULT_ACTION: &str = "default";
/// The icon of a notification without a themed icon name (a path is
/// never read: it only selects this fallback).
const FALLBACK_ICON: &str = "bell";
const DISMISS_ICON: &str = "x";

/// The banner of notification `id`; a click on it runs its default.
#[must_use]
pub fn banner_key(id: u32) -> Key {
    Key::indexed("banner", u64::from(id))
}

/// The dismiss button of notification `id`.
#[must_use]
pub fn dismiss_key(id: u32) -> Key {
    Key::indexed("banner-dismiss", u64::from(id))
}

/// The `index`th action button of notification `id`.
#[must_use]
pub fn action_key(id: u32, index: usize) -> Key {
    Key::indexed(
        "banner-action",
        (u64::from(id) << 8) | (index as u64 & 0xff),
    )
}

/// What a person's gesture on the stack asks the app to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BannerAction {
    /// Hand this command to the notification server.
    Send(NotificationsCommand),
    /// Open the Notifications page on the Island of the banners' output.
    OpenPage,
    /// Open the Island's clock page on the banners' output: a click on the
    /// clock face of the lent Island.
    OpenClock,
}

/// The banners `toasts` (newest first) put on screen: the newest
/// [`MAX_BANNERS`].
#[must_use]
pub fn shown(toasts: &[Notification]) -> &[Notification] {
    &toasts[..toasts.len().min(MAX_BANNERS)]
}

/// What a click on node `key` of the stack showing `shown` asks for: the
/// body runs the default action when offered, else opens the page and
/// closes the banner; an action button invokes its key; the dismiss button
/// closes. Anything else asks for nothing.
#[must_use]
pub fn actions_for(shown: &[Notification], key: Key) -> Vec<BannerAction> {
    for n in shown {
        if key == banner_key(n.id) {
            if n.offers(DEFAULT_ACTION) {
                return vec![BannerAction::Send(NotificationsCommand::Invoke(
                    n.id,
                    DEFAULT_ACTION.to_owned(),
                ))];
            }
            return vec![
                BannerAction::OpenPage,
                BannerAction::Send(NotificationsCommand::Close(n.id)),
            ];
        }
        if key == dismiss_key(n.id) {
            return vec![BannerAction::Send(NotificationsCommand::Close(n.id))];
        }
        if let Some((_, k)) = buttons(n).find(|(i, _)| key == action_key(n.id, *i)) {
            return vec![BannerAction::Send(NotificationsCommand::Invoke(
                n.id,
                k.to_owned(),
            ))];
        }
    }
    Vec::new()
}

/// The action buttons of `n`, numbered as drawn: every offered action but
/// the default, which is the body's.
pub(crate) fn buttons(n: &Notification) -> impl Iterator<Item = (usize, &str)> {
    n.actions
        .iter()
        .filter(|a| a.key != DEFAULT_ACTION)
        .map(|a| a.key.as_str())
        .enumerate()
}

/// The hover command when the pointer goes from over the stack (`was`) to
/// `now`: the server's clock stops while a person reads.
#[must_use]
pub fn hover_command(was: bool, now: bool) -> Option<NotificationsCommand> {
    (was != now).then_some(NotificationsCommand::Hover(now))
}

/// The height of one text line of `size` px, rounded up so a clip of whole
/// lines never shows a sliver of the next.
pub(crate) fn line(size: f32) -> f32 {
    (size * LINE_HEIGHT).ceil()
}

/// A notice's height: the large row (two rows) with 12 px above and below,
/// plus a row of buttons when it has any (the model's 88 and 120).
#[must_use]
pub fn banner_height(with_buttons: bool) -> f32 {
    if with_buttons {
        NOTICE_H_BUTTONS
    } else {
        NOTICE_H
    }
}

/// The wait to arm for the clock face's next minute, or `None` when the
/// surface does not hold the capsule (the panel's own face rolls over then).
/// A margin later than the panel's tick, so the panel's frame has read the
/// new minute when this one asks for the clock.
pub fn clock_wait(
    holds: bool,
    tick: &mut ClockTick,
    now: Instant,
    secs: u64,
    nanos: u32,
) -> Option<Duration> {
    holds.then(|| tick.wait(now, secs, nanos) + TICK_MARGIN)
}

/// The surface's layer-shell request on `output`: the output's width (top,
/// left and right anchors), [`SURFACE_H`] tall, on the Overlay layer so a
/// fullscreen window does not hide it, with no keyboard. Its exclusive zone
/// is -1: it ignores the panel's zone, so its y 0 is the output's top edge
/// and the Island it paints sits exactly over the panel's (a zone of 0 would
/// place it under the bar's 48 px). Its blur and input regions are what it
/// paints.
#[must_use]
pub fn layer_spec(output: OutputId) -> LayerSpec {
    LayerSpec {
        output,
        namespace: NAMESPACE,
        layer: Layer::Overlay,
        anchor: Anchoring::TopWide,
        size: (0, SURFACE_H),
        exclusive_zone: -1,
        keyboard: KeyboardInteractivity::None,
    }
}

/// The blur and input regions of a surface painting `look`: the Island and
/// the sheets with any glass, as rounded shapes in bands, so a click beside
/// them reaches what is underneath.
#[must_use]
pub fn regions(look: &NoticeLook) -> Vec<Rect> {
    let (island, radius) = look.island;
    let mut out = silhouette_slices(island, radius, SILHOUETTE_STEP);
    for sheet in look.sheets.iter().filter(|s| s.glass > 0.0) {
        out.extend(silhouette_slices(sheet.rect, sheet.radius, SILHOUETTE_STEP));
    }
    out
}

/// The input region of a surface painting `look`: the same shapes as
/// [`regions`], and none while the level display is the face, which takes
/// no input (a click goes through to what is underneath).
#[must_use]
pub fn input_regions(look: &NoticeLook) -> Vec<Rect> {
    if look
        .faces
        .first()
        .is_some_and(|f| f.kind == FaceKind::Level)
    {
        Vec::new()
    } else {
        regions(look)
    }
}

/// The icon a banner draws: the themed icon the producer named, else the
/// semantic bell tinted like secondary text. A path selects the bell too:
/// nothing here reads a file a producer pointed at.
pub(crate) fn icon_of(image: Option<&Image>) -> (IconKey, Option<Role>) {
    match image {
        Some(Image::Name(name)) => (IconKey::Theme(name.clone()), None),
        Some(Image::Path(_)) | None => (IconKey::Semantic(FALLBACK_ICON), Some(Role::TextLo)),
    }
}

/// The cards of `shown`, one under the other at their own places (the
/// painter moves each to where its face or sheet is).
fn cards_view(theme: &Theme, shown: &[&Notification]) -> View {
    view::col(Key::named("notice-cards"))
        .absolute(0.0, 0.0)
        .width(BANNER_W)
        .gap(theme.spacing.sm)
        .children(shown.iter().map(|n| {
            card(
                theme,
                n,
                Shape {
                    width: BANNER_W,
                    pad: (theme.spacing.lg, theme.spacing.md),
                    height: banner_height(buttons(n).next().is_some()),
                },
            )
        }))
}

/// The page card's height, and with buttons.
pub const PAGE_CARD_H: f32 = 80.0;
pub const PAGE_CARD_H_BUTTONS: f32 = 112.0;
/// Where it sits in the page: from the sides and from the top.
pub const PAGE_CARD_X: f32 = 12.0;
pub const PAGE_CARD_Y: f32 = 8.0;
/// The page card's padding, across and down.
const PAGE_CARD_PAD: (f32, f32) = (12.0, 8.0);

/// The card's width in a page `page_w` wide: the page's width less the
/// card's distance from each side.
#[must_use]
pub fn page_card_width(page_w: f32) -> f32 {
    (page_w - 2.0 * PAGE_CARD_X).max(0.0)
}

/// The height the card of `n` has in a page.
#[must_use]
pub fn page_card_height(n: &Notification) -> f32 {
    if buttons(n).next().is_some() {
        PAGE_CARD_H_BUTTONS
    } else {
        PAGE_CARD_H
    }
}

/// The room a card takes at the top of a page: its height and the gap under
/// it, which is also what the page grows by.
#[must_use]
pub fn page_card_room(n: &Notification) -> f32 {
    page_card_height(n) + PAGE_CARD_Y
}

/// The card of `n` in an open page (§5.5): the notice's content on the
/// control fill, placed at its corner of the page. The same keys as the
/// Island's notice, so a click means what it means there.
#[must_use]
pub fn page_card(theme: &Theme, n: &Notification, page_w: f32) -> View {
    card(
        theme,
        n,
        Shape {
            width: page_card_width(page_w),
            pad: PAGE_CARD_PAD,
            height: page_card_height(n),
        },
    )
    .fill(Role::ControlFill)
    .radius(theme.radius_md)
    .absolute(PAGE_CARD_X, PAGE_CARD_Y)
}

/// How a notice's content is laid out: the notice on the Island and the
/// card in a page differ only in this.
#[derive(Clone, Copy)]
struct Shape {
    width: f32,
    /// Padding across and down.
    pad: (f32, f32),
    height: f32,
}

/// One notice's content: the icon, the summary on one line beside the
/// dismiss button, the body clipped to two lines and the buttons. No glass of
/// its own: the Island or a sheet is the glass behind it.
fn card(theme: &Theme, n: &Notification, shape: Shape) -> View {
    let id = u64::from(n.id);
    let text_w = shape.width - 2.0 * shape.pad.0 - ICON_SIZE - theme.spacing.md;
    let summary_w = text_w - DISMISS_SIZE - theme.spacing.sm;
    let (icon_key, tint) = icon_of(n.image.as_ref());
    let icon = View::new(
        Key::indexed("banner-icon", id),
        ViewKind::Icon {
            key: icon_key,
            tint,
            size: ICON_SIZE,
        },
    )
    .size(ICON_SIZE, ICON_SIZE);
    let summary_size = theme.type_scale.row_title;
    let summary = view::col(Key::indexed("banner-summary", id))
        .height(line(summary_size))
        .clip()
        .grow()
        .child(
            view::text(
                Key::indexed("banner-summary-text", id),
                &n.summary,
                text_style(summary_size, Weight::Semibold, false, Role::TextHi),
            )
            .max_width(summary_w),
        );
    let mut dismiss = centred(view::row(dismiss_key(n.id)))
        .size(DISMISS_SIZE, DISMISS_SIZE)
        .radius(theme.radius_sm)
        .interactive()
        .child(
            view::icon(
                Key::indexed("banner-dismiss-icon", id),
                DISMISS_ICON,
                DISMISS_GLYPH,
            )
            .tint(Role::TextLo),
        );
    dismiss.style.justify_content = JustifyContent::CENTER;
    let header = centred(view::row(Key::indexed("banner-header", id)))
        .gap(theme.spacing.sm)
        .child(summary)
        .child(dismiss);
    let body_size = theme.type_scale.body;
    let body = (!n.body.is_empty()).then(|| {
        view::col(Key::indexed("banner-body", id))
            .height(2.0 * line(body_size))
            .clip()
            .child(
                view::text(
                    Key::indexed("banner-body-text", id),
                    excerpt(&n.body),
                    text_style(body_size, Weight::Regular, false, Role::TextLo),
                )
                .max_width(text_w),
            )
    });
    let buttons: Vec<View> = buttons(n)
        .map(|(i, key)| {
            let label = n
                .actions
                .iter()
                .find(|a| a.key == key)
                .map_or(key, |a| a.label.as_str());
            centred(view::row(action_key(n.id, i)))
                .height(BUTTON_H)
                .padding_xy(theme.spacing.md, 0.0)
                .radius(theme.radius_button)
                .fill(Role::ControlFill)
                .interactive()
                .child(view::text(
                    Key::indexed("banner-action-label", (id << 8) | (i as u64 & 0xff)),
                    label,
                    text_style(body_size, Weight::Regular, false, Role::TextHi),
                ))
        })
        .collect();
    let with_buttons = !buttons.is_empty();
    let row = with_buttons.then(|| {
        view::row(Key::indexed("banner-buttons", id))
            .gap(theme.spacing.sm)
            .clip()
            .children(buttons)
    });
    let mut content = view::col(Key::indexed("banner-content", id))
        .grow()
        .gap(theme.spacing.xs)
        .child(header)
        .children(body)
        .children(row);
    content.style.justify_content = JustifyContent::CENTER;
    centred(view::row(banner_key(n.id)))
        .size(shape.width, shape.height)
        .padding_xy(shape.pad.0, shape.pad.1)
        .gap(theme.spacing.md)
        .interactive()
        .child(icon)
        .child(content)
}

/// The level face's icon, as the bottom display draws it.
const LEVEL_ICON: f32 = 18.0;
/// The level face's track, as long as the bottom display's bar.
const LEVEL_TRACK_W: f32 = 160.0;
/// The room kept for `100 %`, so the level never moves the track.
const LEVEL_LABEL_W: f32 = 40.0;

/// The level display's face (§5.4): the kind's icon, the track filled to the
/// level and `NN %`, padding and gap from the theme. It sits at the tree's
/// origin and is [`LEVEL_W`] by [`LEVEL_H`]; the Island supplies the glass.
#[must_use]
pub fn level_view(theme: &Theme, level: Level) -> View {
    let percent = level.percent.min(100);
    // A muted device keeps its level, drawn faint.
    let fill = if level.muted {
        Role::TextFaint
    } else {
        Role::Accent
    };
    let track_h = theme.linear_track_height;
    let filled = LEVEL_TRACK_W * f32::from(percent) / 100.0;
    let track = view::row(Key::named("level-track"))
        .size(LEVEL_TRACK_W, track_h)
        .radius(track_h / 2.0)
        .fill(Role::ControlFill)
        .clip()
        .child(
            view::row(Key::named("level-fill"))
                .size(filled, track_h)
                .radius(track_h / 2.0)
                .fill(fill),
        );
    let mut label = centred(view::row(Key::named("level-label"))).width(LEVEL_LABEL_W);
    label.style.justify_content = JustifyContent::FLEX_END;
    let label = label.child(view::text(
        Key::named("level-label-text"),
        &copy::osd_percent(percent),
        text_style(theme.type_scale.body, Weight::Semibold, true, Role::TextHi),
    ));
    let icon = icon_of_level(level);
    let row = centred(view::row(Key::named("level-row")))
        .size(LEVEL_W, LEVEL_H)
        .padding_xy(theme.spacing.lg, 0.0)
        .gap(theme.spacing.md)
        .child(view::icon(Key::named("level-icon"), icon, LEVEL_ICON).tint(Role::TextHi))
        .child(track)
        .child(label);
    view::col(Key::named("level-root"))
        .size(LEVEL_W, LEVEL_H)
        .child(row)
}

/// The semantic icon of `level`: the display's own, muted or not.
fn icon_of_level(level: Level) -> &'static str {
    use celestina_shell::osd::{Reading, Shown};
    crate::osd::view::icon_of(Shown {
        kind: level.kind,
        reading: Reading {
            percent: level.percent,
            muted: level.muted,
        },
    })
}

/// What one input event on the surface asks for, and whether a frame is
/// due.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NoticeInput {
    pub actions: Vec<BannerAction>,
    pub redraw: bool,
}

/// Where a pointer is on a notice or a sheet.
#[derive(Clone, Copy, Debug)]
struct Spot {
    /// The notification it shows.
    id: u32,
    /// Where its card's top-left corner is painted.
    anchor: (f32, f32),
    /// The card is visible (a sheet collapsed has none to click).
    content: bool,
}

/// The surface's content: an [`IslandNotice`] and what it paints, the
/// cards, and what the pointer does on them. The painting is the only part
/// that needs a GPU painter.
#[derive(Debug)]
pub struct NoticePainter {
    theme: Theme,
    model: IslandNotice,
    /// The panel's rest rect the Island grows from and shrinks to.
    rest: Rect,
    /// The cards of the notifications on screen or still fading, laid out
    /// one under the other.
    cards: Tree,
    /// The rest Island's face: the panel's own capsule view.
    clock: Tree,
    /// The level face's tree, built for `level_applied`.
    level: Tree,
    level_applied: Option<Level>,
    input: Interaction,
    /// Every notification the model gave that can still be seen.
    known: Vec<Notification>,
    /// The ids the cards tree holds, in its order.
    shown: Vec<u32>,
    stale: bool,
    size: (f32, f32),
    dpr: f32,
    /// The server's clock is held (the last hover command sent).
    held: bool,
    /// The press of a left click that began on the clock face.
    clock_down: bool,
    /// The scroll gesture that last closed a notice.
    wheel: WheelLatch,
    /// The notification whose collapsed sheet a left press began on.
    sheet_down: Option<u32>,
}

impl NoticePainter {
    /// An Island at rest on `rest`, in `theme`, under `reduced` motion.
    #[must_use]
    pub fn new(theme: &Theme, rest: Rect, reduced: bool) -> NoticePainter {
        let mut model = IslandNotice::new(theme, reduced);
        model.set_rest(rest, Instant::now());
        NoticePainter {
            theme: theme.clone(),
            model,
            rest,
            // The cards never slide on their own: the model owns every
            // motion, and the painter puts each card where its face is.
            cards: Tree::new(true, theme),
            clock: Tree::new(true, theme),
            level: Tree::new(true, theme),
            level_applied: None,
            input: Interaction::new(),
            known: Vec::new(),
            shown: Vec::new(),
            stale: true,
            size: (0.0, SURFACE_H as f32),
            dpr: 1.0,
            held: false,
            clock_down: false,
            wheel: WheelLatch::default(),
            sheet_down: None,
        }
    }

    /// The model, for what the app asks of it (rest, animation).
    #[must_use]
    pub fn model(&self) -> &IslandNotice {
        &self.model
    }

    /// Whether the surface has something to show at `now`: a notice, a
    /// level or a motion still running. The surface exists while it does.
    #[must_use]
    pub fn wants_surface(&self, now: Instant) -> bool {
        !self.model.at_rest(now)
    }

    /// The notifications on screen (`toasts`, newest first); whether
    /// anything changed. The pointer cannot be over notices that are gone.
    pub fn set_toasts(&mut self, toasts: &[Notification], now: Instant) -> bool {
        for n in shown(toasts) {
            match self.known.iter_mut().find(|k| k.id == n.id) {
                Some(k) => *k = n.clone(),
                None => self.known.push(n.clone()),
            }
        }
        let changed = self.model.set_toasts(toasts, now);
        if self.model.toasts().is_empty() {
            self.model.set_hovered(false, now);
        }
        self.stale |= changed;
        changed
    }

    /// Whether a change `changed` of the toasts is worth a frame at `now`:
    /// only while the surface has something to show or a motion to run, so
    /// a surface at rest never draws a frame that animates nothing.
    #[must_use]
    pub fn needs_frame(&self, changed: bool, now: Instant) -> bool {
        changed && self.wants_surface(now)
    }

    /// The level display shows `level`, or leaves with `None`.
    pub fn set_level(&mut self, level: Option<Level>, now: Instant) {
        self.model.set_level(level, now);
    }

    /// The panel's rest rect, which the Island follows.
    pub fn set_rest(&mut self, rest: Rect, now: Instant) {
        if rest != self.rest {
            self.rest = rest;
            self.model.set_rest(rest, now);
        }
    }

    /// The hover command when the server's clock must start or stop being
    /// held (the pointer is over a notice, or a level shows), once per
    /// change.
    pub fn take_hold(&mut self) -> Option<NotificationsCommand> {
        let holds = self.model.holds_timeouts();
        let command = hover_command(self.held, holds);
        self.held = holds;
        command
    }

    /// The surface is going away: the hold ends with it, so the server's
    /// clock runs again.
    pub fn release_hover(&mut self) -> Option<NotificationsCommand> {
        let command = hover_command(self.held, false);
        self.held = false;
        command
    }

    /// Where a card is painted for the Island's notice: its final rect,
    /// centred on the rest Island's centre at the Island's top.
    fn island_anchor(&self) -> (f32, f32) {
        (
            self.rest.x + self.rest.w / 2.0 - BANNER_W / 2.0,
            self.rest.y,
        )
    }

    /// Builds the cards' view when the set of cards changed, then lays
    /// both trees out with `lay`.
    fn prepare_with(
        &mut self,
        look: &NoticeLook,
        now: Instant,
        lay: &mut dyn FnMut(&mut Tree, (f32, f32)),
    ) {
        let mut ids: Vec<u32> = self.model.toasts().iter().map(|n| n.id).collect();
        let fading = look
            .faces
            .iter()
            .filter_map(|f| match f.kind {
                FaceKind::Notice(id) => Some(id),
                _ => None,
            })
            .chain(look.sheets.iter().map(|s| s.id));
        for id in fading {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        self.known.retain(|n| ids.contains(&n.id));
        ids.retain(|id| self.known.iter().any(|n| n.id == *id));
        if ids != self.shown {
            self.shown = ids;
            self.stale = true;
        }
        if self.stale {
            let cards: Vec<&Notification> = self
                .shown
                .iter()
                .filter_map(|id| self.known.iter().find(|n| n.id == *id))
                .collect();
            self.cards.apply(cards_view(&self.theme, &cards), now);
            self.stale = false;
        }
        if let Some(level) = self.model.level() {
            if self.level_applied != Some(level) {
                self.level.apply(level_view(&self.theme, level), now);
                self.level_applied = Some(level);
            }
            lay(&mut self.level, self.size);
        }
        lay(&mut self.cards, self.size);
        lay(&mut self.clock, self.size);
        for (name, tree) in [
            ("cards", &mut self.cards),
            ("clock", &mut self.clock),
            ("level", &mut self.level),
        ] {
            if let Some(fault) = tree.take_fault() {
                tracing::warn!(%fault, name, "notices: layout fault");
            }
        }
    }

    /// Builds and lays out the cards so what the input hits is what the
    /// next frame paints.
    pub fn prepare(&mut self, painter: &mut Painter, now: Instant) {
        let look = self.model.look(now);
        let dpr = self.dpr;
        self.prepare_with(&look, now, &mut |tree: &mut Tree, room: (f32, f32)| {
            tree.layout(room, painter.text(), dpr);
        });
    }

    /// The notice or sheet under (`x`, `y`), by the look at `now`.
    fn spot_at(&self, look: &NoticeLook, x: f32, y: f32) -> Option<Spot> {
        // The level display takes no input (§5.4): the notice it hides gets
        // no click, and no sheet shows under it.
        if self.model.shows_level() {
            return None;
        }
        if let Some(newest) = self.model.toasts().first() {
            if look.island.0.contains(x, y) {
                return Some(Spot {
                    id: newest.id,
                    anchor: self.island_anchor(),
                    content: true,
                });
            }
        }
        look.sheets
            .iter()
            .find(|s| s.glass > 0.0 && s.rect.contains(x, y))
            .map(|s| Spot {
                id: s.id,
                anchor: (s.rect.x, s.rect.y),
                content: s.content >= 0.5,
            })
    }

    /// `ev` moved into the cards tree's own space, where the card of `spot`
    /// is laid out.
    fn moved_to_cards(&self, ev: &InputEvent, spot: Spot) -> InputEvent {
        let Some(card) = self.cards.node_by_key(banner_key(spot.id)) else {
            return ev.clone();
        };
        let laid = self.cards.rect(card);
        let (dx, dy) = (laid.x - spot.anchor.0, laid.y - spot.anchor.1);
        match ev.clone() {
            InputEvent::PointerMotion { x, y } => InputEvent::PointerMotion {
                x: x + dx,
                y: y + dy,
            },
            InputEvent::PointerPress {
                x,
                y,
                button,
                serial,
            } => InputEvent::PointerPress {
                x: x + dx,
                y: y + dy,
                button,
                serial,
            },
            InputEvent::PointerRelease {
                x,
                y,
                button,
                serial,
            } => InputEvent::PointerRelease {
                x: x + dx,
                y: y + dy,
                button,
                serial,
            },
            other => other,
        }
    }

    /// Pointer events: over a notice or a sheet holds the server's clock
    /// and fans the burst out; a left click runs what its card asks for;
    /// right click and the wheel towards the top close the notice; middle
    /// click closes every one. Acts on the release, as the bar does. The
    /// surface never takes the keyboard.
    pub fn pointer(&mut self, ev: &InputEvent, now: Instant) -> NoticeInput {
        let look = self.model.look(now);
        let before = self.cards.highlight();
        let point = match *ev {
            InputEvent::PointerMotion { x, y }
            | InputEvent::PointerPress { x, y, .. }
            | InputEvent::PointerRelease { x, y, .. }
            | InputEvent::Axis { x, y, .. } => Some((x, y)),
            _ => None,
        };
        let spot = point.and_then(|(x, y)| self.spot_at(&look, x, y));
        let leave = matches!(ev, InputEvent::PointerLeave);
        let was = self.model.holds_timeouts();
        if point.is_some() || leave {
            self.model.set_hovered(spot.is_some(), now);
        }
        let mut actions = Vec::new();
        // The clock face of the Island (no notice on it): a left click
        // opens the clock page, as the panel's capsule does.
        if spot.is_none() && self.model.toasts().is_empty() && !self.model.shows_level() {
            let inside = point.is_some_and(|(x, y)| look.island.0.contains(x, y));
            match *ev {
                InputEvent::PointerPress { button, .. } if button == BTN_LEFT => {
                    self.clock_down = inside;
                }
                InputEvent::PointerRelease { button, .. } if button == BTN_LEFT => {
                    let down = std::mem::take(&mut self.clock_down);
                    if down && inside {
                        actions.push(BannerAction::OpenClock);
                    }
                }
                _ => {}
            }
        }
        let events = match (spot, point.is_some() || leave) {
            (Some(s), true) if s.content => {
                let moved = self.moved_to_cards(ev, s);
                self.input.pointer(&mut self.cards, &moved, now)
            }
            (_, true) => self
                .input
                .pointer(&mut self.cards, &InputEvent::PointerLeave, now),
            _ => Vec::new(),
        };
        if let Some(command) = self.take_hold() {
            actions.push(BannerAction::Send(command));
        }
        // The wheel's gesture ends with time only (300 ms without an axis
        // event on the surface): the layout moving under a still pointer
        // must not end it, or one swipe would close the next notice too. So
        // every axis event feeds the latch, over a notice or not.
        let wheel_up = match *ev {
            InputEvent::Axis { vertical, .. } => self.wheel.up(vertical, now),
            _ => false,
        };
        // A collapsed sheet has no card to click yet (the fan comes after
        // the hover): a left click on it is a click on its notice's body.
        let collapsed = spot.filter(|s| !s.content);
        match *ev {
            InputEvent::PointerPress { button, .. } if button == BTN_LEFT => {
                self.sheet_down = collapsed.map(|s| s.id);
            }
            InputEvent::PointerRelease { button, .. } if button == BTN_LEFT => {
                // The release counts on the same notice even if the fan gave
                // its sheet content between the press and the release.
                let down = self.sheet_down.take();
                if let Some(s) = spot.filter(|s| down == Some(s.id)) {
                    actions.extend(self.model.gesture(s.id, Gesture::Left(banner_key(s.id))));
                }
            }
            _ => {}
        }
        if let Some(s) = spot {
            for event in events {
                if let UiEvent::Click(key) = event {
                    actions.extend(self.model.gesture(s.id, Gesture::Left(key)));
                }
            }
            let gesture = match *ev {
                InputEvent::PointerRelease { button, .. } if button == BTN_RIGHT => {
                    Some(Gesture::Right)
                }
                InputEvent::PointerRelease { button, .. } if button == BTN_MIDDLE => {
                    Some(Gesture::Middle)
                }
                InputEvent::Axis { .. } if wheel_up => Some(Gesture::WheelUp),
                _ => None,
            };
            if let Some(gesture) = gesture {
                actions.extend(self.model.gesture(s.id, gesture));
            }
        }
        NoticeInput {
            redraw: self.cards.highlight() != before
                || was != self.model.holds_timeouts()
                || self.model.is_animating(now),
            actions,
        }
    }

    /// Draws one frame: the sheets, the Island's glass, then the faces
    /// (through a blur while theirs is above a hair), then the regions of
    /// what was painted. A frame follows only while something moves.
    /// Whether the surface still has something to show.
    pub fn frame(
        &mut self,
        cx: &mut FrameCx<'_>,
        painter: &mut Painter,
        rest: Rect,
        capsule: View,
    ) -> bool {
        let now = cx.now;
        if painter.theme() != &self.theme {
            self.theme = painter.theme().clone();
            self.cards.set_theme(&self.theme);
            self.clock.set_theme(&self.theme);
            self.level.set_theme(&self.theme);
            self.level_applied = None;
            self.stale = true;
        }
        self.size = cx.size;
        self.dpr = cx.scale as f32;
        self.set_rest(rest, now);
        self.clock.apply(capsule, now);
        self.prepare(painter, now);
        let look = self.model.look(now);
        painter.begin(self.size, self.dpr, cx.blur_available);
        self.paint(painter, &look, now);
        painter.end();
        let region = regions(&look);
        cx.ctl.set_blur_region(&region);
        cx.ctl.set_input_region(&input_regions(&look));
        if self.model.is_animating(now) || self.cards.is_animating(now) {
            cx.ctl.request_redraw();
        }
        self.wants_surface(now)
    }

    fn paint(&mut self, painter: &mut Painter, look: &NoticeLook, now: Instant) {
        let island_anchor = self.island_anchor();
        let Self {
            cards,
            clock,
            level,
            ..
        } = self;
        for sheet in look.sheets.iter().rev() {
            painter.push_state();
            painter.opacity(sheet.glass);
            painter.haze(sheet.rect, sheet.radius);
            painter.pop_state();
            if sheet.content > 0.0 {
                painter.push_clip(sheet.rect, sheet.radius);
                paint_card(
                    painter,
                    cards,
                    sheet.id,
                    (sheet.rect.x, sheet.rect.y),
                    (sheet.rect, sheet.content, 0.0, 0.0),
                    now,
                );
                painter.pop_clip();
            }
        }
        let (rect, radius) = look.island;
        painter.haze(rect, radius);
        painter.push_clip(rect, radius);
        // The wanted face is first in the look and goes on top.
        for face in look.faces.iter().rev() {
            let shape = (rect, face.opacity, face.blur, face.dy);
            match face.kind {
                FaceKind::Clock => {
                    layered(painter, rect, face.opacity, face.blur, &mut |p| {
                        p.push_state();
                        p.translate(0.0, face.dy);
                        clock.paint(p, now);
                        p.pop_state();
                    });
                }
                FaceKind::Notice(id) => {
                    paint_card(painter, cards, id, island_anchor, shape, now);
                }
                FaceKind::Level => {
                    let (x, y) = (
                        rect.x + rect.w / 2.0 - LEVEL_W / 2.0,
                        rect.y + rect.h / 2.0 - LEVEL_H / 2.0 + face.dy,
                    );
                    layered(painter, rect, face.opacity, face.blur, &mut |p| {
                        p.push_state();
                        p.translate(x, y);
                        level.paint(p, now);
                        p.pop_state();
                    });
                }
            }
        }
        painter.pop_clip();
    }
}

/// `draw` at `opacity`, through a blur of `blur` px when that is above a
/// hair; `layer` is what the drawing covers.
fn layered(
    painter: &mut Painter,
    layer: Rect,
    opacity: f32,
    blur: f32,
    draw: &mut dyn FnMut(&mut Painter),
) {
    let reach = Rect {
        x: layer.x - LAYER_REACH,
        y: layer.y - LAYER_REACH,
        w: layer.w + 2.0 * LAYER_REACH,
        h: layer.h + 2.0 * LAYER_REACH,
    };
    painter.push_state();
    painter.opacity(opacity);
    if blur > BLUR_MIN {
        painter.blurred(reach, blur, draw);
    } else {
        draw(painter);
    }
    painter.pop_state();
}

/// The card of notification `id` with its corner at `anchor`: `look` is the
/// rect it sits in, its opacity, blur and offset.
fn paint_card(
    painter: &mut Painter,
    cards: &mut Tree,
    id: u32,
    anchor: (f32, f32),
    look: (Rect, f32, f32, f32),
    now: Instant,
) {
    let Some(node) = cards.node_by_key(banner_key(id)) else {
        return;
    };
    let laid = cards.rect(node);
    let (inside, opacity, blur, dy) = look;
    let card = Rect {
        x: anchor.0,
        y: anchor.1,
        w: laid.w,
        h: laid.h,
    };
    let covers = Rect {
        x: inside.x.min(card.x),
        y: inside.y.min(card.y),
        w: (inside.x + inside.w).max(card.x + card.w) - inside.x.min(card.x),
        h: (inside.y + inside.h).max(card.y + card.h) - inside.y.min(card.y),
    };
    layered(painter, covers, opacity, blur, &mut |p| {
        p.push_state();
        p.translate(anchor.0 - laid.x, anchor.1 - laid.y + dy);
        cards.paint_inside(p, now, node);
        p.pop_state();
    });
}

#[cfg(test)]
mod tests {
    use celestina_paint::TextStyle;
    use celestina_shell::notifications::{CloseReason, Incoming, Notifications, Urgency};
    use std::time::Duration;

    use super::*;
    use crate::providers::notifications::snapshot;

    /// 7 px per char, 16 px tall.
    fn fake_measure(s: &str, _: &TextStyle, _: Option<f32>) -> (f32, f32) {
        (7.0 * s.chars().count() as f32, 16.0)
    }

    fn incoming(summary: &str, urgency: Urgency, actions: &[&str]) -> Incoming {
        Incoming {
            app_name: "app".to_owned(),
            replaces_id: 0,
            app_icon: String::new(),
            summary: summary.to_owned(),
            body: "body".to_owned(),
            actions: actions.iter().map(|s| (*s).to_owned()).collect(),
            urgency,
            image: None,
            expire_timeout: -1,
        }
    }

    fn machine_with(n: usize) -> Notifications {
        let mut machine = Notifications::new();
        for i in 0..n {
            machine.post(&incoming(&format!("n{i}"), Urgency::Normal, &[]), i as u64);
        }
        machine
    }

    /// The panel's Island on a 1920 px output.
    fn rest() -> Rect {
        Rect {
            x: 870.0,
            y: 8.0,
            w: 180.0,
            h: 40.0,
        }
    }

    fn painter(reduced: bool) -> NoticePainter {
        let mut p = NoticePainter::new(&Theme::dark(), rest(), reduced);
        p.size = (1920.0, SURFACE_H as f32);
        p
    }

    /// Builds and lays out the cards at `now` with the fake measure.
    fn prepare(p: &mut NoticePainter, now: Instant) {
        let look = p.model.look(now);
        p.prepare_with(&look, now, &mut |tree: &mut Tree, room: (f32, f32)| {
            tree.layout_measured(room, fake_measure);
        });
    }

    fn ms(t0: Instant, n: u64) -> Instant {
        t0 + Duration::from_millis(n)
    }

    #[test]
    fn the_surface_spans_the_output_and_is_440_high() {
        let spec = layer_spec(OutputId(3));
        assert_eq!(spec.output, OutputId(3));
        assert_eq!(spec.namespace, "celestina-toasts");
        assert_eq!(spec.layer, Layer::Overlay);
        assert_eq!(spec.anchor, Anchoring::TopWide, "top, left and right");
        assert_eq!(spec.size.1, 440);
        assert_eq!(
            spec.exclusive_zone, -1,
            "over the panel's zone, not under it"
        );
        assert_eq!(spec.keyboard, KeyboardInteractivity::None);
    }

    #[test]
    fn regions_are_the_painted_island_and_sheets_only() {
        let t0 = Instant::now();
        let mut p = painter(false);
        let snap = snapshot(&machine_with(3));
        p.set_toasts(&snap.toasts, t0);
        let look = p.model.look(ms(t0, 2000));
        let want_island = silhouette_slices(look.island.0, look.island.1, SILHOUETTE_STEP);
        let region = regions(&look);
        assert!(region.starts_with(&want_island));
        let mut shapes = vec![look.island];
        shapes.extend(look.sheets.iter().map(|s| (s.rect, s.radius)));
        assert_eq!(look.sheets.len(), 2);
        let want: Vec<Rect> = shapes
            .iter()
            .flat_map(|(r, radius)| silhouette_slices(*r, *radius, SILHOUETTE_STEP))
            .collect();
        assert_eq!(region, want);
        // At rest the only shape is the rest Island.
        let calm = painter(false).model.look(t0);
        assert_eq!(
            regions(&calm),
            silhouette_slices(calm.island.0, calm.island.1, SILHOUETTE_STEP)
        );
        // Nothing outside them: every slice is inside the Island or a sheet.
        for slice in &region {
            assert!(
                shapes.iter().any(|(r, _)| slice.y >= r.y.floor()
                    && slice.y + slice.h <= (r.y + r.h).ceil()
                    && slice.x >= r.x.floor()
                    && slice.x + slice.w <= (r.x + r.w).ceil()),
                "{slice:?}"
            );
        }
    }

    #[test]
    fn a_card_is_as_tall_as_its_notice_and_the_height_is_the_models() {
        let theme = Theme::dark();
        assert_eq!(banner_height(false), 88.0);
        assert_eq!(banner_height(true), 120.0);
        let t0 = Instant::now();
        let mut p = painter(false);
        let mut machine = Notifications::new();
        machine.post(&incoming("plain", Urgency::Normal, &[]), 0);
        machine.post(&incoming("with", Urgency::Normal, &["reply", "Reply"]), 1);
        let snap = snapshot(&machine);
        p.set_toasts(&snap.toasts, t0);
        prepare(&mut p, ms(t0, 2000));
        for n in shown(&snap.toasts) {
            let Some(node) = p.cards.node_by_key(banner_key(n.id)) else {
                panic!("no card for {}", n.id);
            };
            let r = p.cards.rect(node);
            assert_eq!(
                (r.w, r.h),
                (BANNER_W, banner_height(buttons(n).count() > 0))
            );
        }
        // The model's Island reaches the same height.
        let look = p.model.look(ms(t0, 2000));
        assert_eq!(look.island.0.h, banner_height(true));
        assert_eq!(theme.radius_lg, look.island.1);
    }

    fn at(p: &mut NoticePainter, t0: Instant) -> Instant {
        // Past the spring, the faces and every settle.
        let now = ms(t0, 2000);
        prepare(p, now);
        now
    }

    fn click_at(
        p: &mut NoticePainter,
        (x, y): (f32, f32),
        button: u32,
        now: Instant,
    ) -> Vec<BannerAction> {
        let press = InputEvent::PointerPress {
            x,
            y,
            button,
            serial: 1,
        };
        let release = InputEvent::PointerRelease {
            x,
            y,
            button,
            serial: 2,
        };
        let mut actions = p.pointer(&press, now).actions;
        actions.extend(p.pointer(&release, now).actions);
        actions
    }

    fn hover(on: bool) -> BannerAction {
        BannerAction::Send(NotificationsCommand::Hover(on))
    }

    /// A painter showing `n` notices, past every motion; the Island's
    /// notice centre and a point on its body.
    fn showing(n: usize) -> (NoticePainter, Instant, Instant, (f32, f32)) {
        let t0 = Instant::now();
        let mut p = painter(false);
        p.set_toasts(&snapshot(&machine_with(n)).toasts, t0);
        let now = at(&mut p, t0);
        let island = p.model.look(now).island.0;
        let body = (island.x + island.w / 2.0, island.y + island.h - 8.0);
        (p, t0, now, body)
    }

    #[test]
    fn right_click_wheel_up_and_middle_click_send_their_commands() {
        let (mut p, _, now, at) = showing(1);
        let hold = hover(true);
        // Right click closes the notice (after the hover the pointer
        // brought).
        let got = click_at(&mut p, at, BTN_RIGHT, now);
        assert_eq!(
            got,
            vec![
                hold.clone(),
                BannerAction::Send(NotificationsCommand::Close(1))
            ]
        );
        // Middle click closes every one.
        assert_eq!(
            click_at(&mut p, at, BTN_MIDDLE, now),
            vec![BannerAction::Send(NotificationsCommand::DismissAll)]
        );
        // The wheel towards the top closes; towards the bottom does not.
        let wheel = |vertical| InputEvent::Axis {
            x: at.0,
            y: at.1,
            horizontal: 0.0,
            vertical,
        };
        assert_eq!(
            p.pointer(&wheel(-10.0), now).actions,
            vec![BannerAction::Send(NotificationsCommand::Close(1))]
        );
        assert!(p.pointer(&wheel(10.0), now).actions.is_empty());
        // Beside the Island none of it reaches a notice.
        let away = (at.0 + 400.0, at.1);
        assert!(click_at(&mut p, away, BTN_RIGHT, now)
            .iter()
            .all(|a| matches!(a, BannerAction::Send(NotificationsCommand::Hover(_)))));
        assert!(p
            .pointer(
                &InputEvent::Axis {
                    x: away.0,
                    y: away.1,
                    horizontal: 0.0,
                    vertical: -10.0
                },
                now
            )
            .actions
            .is_empty());
    }

    #[test]
    fn a_sheet_takes_the_gestures_of_its_notice() {
        let (mut p, _, now, _) = showing(3);
        let look = p.model.look(now);
        let sheet = look.sheets[0];
        // The strip of it that shows under the Island.
        let (x, y) = (
            sheet.rect.x + sheet.rect.w / 2.0,
            look.island.0.y + look.island.0.h + 3.0,
        );
        // Over a collapsed sheet: its own id, not the Island's.
        assert_eq!(sheet.id, 2);
        p.pointer(&InputEvent::PointerLeave, now);
        let got = click_at(&mut p, (x, y), BTN_RIGHT, now);
        assert!(got.contains(&BannerAction::Send(NotificationsCommand::Close(2))));
        // A left click on a collapsed sheet is a click on its notice's body
        // (§5.6), without waiting for the fan.
        let left = click_at(&mut p, (x, y), celestina_ui::BTN_LEFT, now);
        assert!(left.contains(&BannerAction::OpenPage), "{left:?}");
        assert!(left.contains(&BannerAction::Send(NotificationsCommand::Close(2))));
        // A press that began elsewhere does not click it.
        let away = (x + 600.0, y);
        p.pointer(
            &InputEvent::PointerPress {
                x: away.0,
                y: away.1,
                button: celestina_ui::BTN_LEFT,
                serial: 3,
            },
            now,
        );
        let release = InputEvent::PointerRelease {
            x,
            y,
            button: celestina_ui::BTN_LEFT,
            serial: 4,
        };
        assert!(!p
            .pointer(&release, now)
            .actions
            .contains(&BannerAction::OpenPage));
    }

    #[test]
    fn a_press_on_a_sheet_that_fans_out_before_the_release_still_clicks() {
        let (mut p, _, now, _) = showing(3);
        let look = p.model.look(now);
        let sheet = look.sheets[0];
        let press_at = (
            sheet.rect.x + sheet.rect.w / 2.0,
            look.island.0.y + look.island.0.h + 3.0,
        );
        p.pointer(&InputEvent::PointerLeave, now);
        p.pointer(
            &InputEvent::PointerMotion {
                x: press_at.0,
                y: press_at.1,
            },
            now,
        );
        p.pointer(
            &InputEvent::PointerPress {
                x: press_at.0,
                y: press_at.1,
                button: celestina_ui::BTN_LEFT,
                serial: 1,
            },
            now,
        );
        // The hover fans the sheets out; the release comes once sheet 2 is a
        // card with its content, over that card.
        let later = ms(now, 600);
        prepare(&mut p, later);
        let fanned = p.model.look(later);
        let card = fanned
            .sheets
            .iter()
            .find(|s| s.id == 2)
            .expect("sheet 2 fanned");
        assert!(card.content > 0.5, "fanned: {card:?}");
        let release = InputEvent::PointerRelease {
            x: card.rect.x + card.rect.w / 2.0,
            y: card.rect.y + card.rect.h / 2.0,
            button: celestina_ui::BTN_LEFT,
            serial: 2,
        };
        let got = p.pointer(&release, later).actions;
        assert!(got.contains(&BannerAction::OpenPage), "{got:?}");
        assert!(got.contains(&BannerAction::Send(NotificationsCommand::Close(2))));
    }

    #[test]
    fn a_left_click_on_the_notice_body_reaches_the_app() {
        let (mut p, _, now, at) = showing(1);
        let got = click_at(&mut p, at, celestina_ui::BTN_LEFT, now);
        assert!(got.contains(&BannerAction::OpenPage), "{got:?}");
        assert!(got.contains(&BannerAction::Send(NotificationsCommand::Close(1))));
    }

    #[test]
    fn the_close_button_closes_it() {
        let (mut p, _, now, _) = showing(1);
        let Some(node) = p.cards.node_by_key(dismiss_key(1)) else {
            panic!("no close button");
        };
        let r = p.cards.rect(node);
        let anchor = p.island_anchor();
        let Some(card) = p.cards.node_by_key(banner_key(1)) else {
            panic!("no card");
        };
        let laid = p.cards.rect(card);
        // The button's place on screen: its place in the card moved to
        // where the Island paints the card.
        let at = (
            r.x - laid.x + anchor.0 + r.w / 2.0,
            r.y - laid.y + anchor.1 + r.h / 2.0,
        );
        let got = click_at(&mut p, at, celestina_ui::BTN_LEFT, now);
        assert!(got.contains(&BannerAction::Send(NotificationsCommand::Close(1))));
    }

    #[test]
    fn hover_and_leave_send_hover_commands_and_release_on_destroy() {
        let (mut p, _, now, at) = showing(2);
        let motion = |x: f32, y: f32| InputEvent::PointerMotion { x, y };
        assert_eq!(
            p.pointer(&motion(at.0, at.1), now).actions,
            vec![hover(true)]
        );
        assert!(p.model().holds_timeouts());
        // Moving within the notice says nothing new.
        assert!(p
            .pointer(&motion(at.0 + 3.0, at.1 - 2.0), now)
            .actions
            .is_empty());
        // Beside it is leaving.
        assert_eq!(
            p.pointer(&motion(at.0 + 500.0, at.1), now).actions,
            vec![hover(false)]
        );
        p.pointer(&motion(at.0, at.1), now);
        assert_eq!(
            p.pointer(&InputEvent::PointerLeave, now).actions,
            vec![hover(false)]
        );
        // A surface that goes away while hovered lets the clock run again.
        p.pointer(&motion(at.0, at.1), now);
        assert_eq!(p.release_hover(), Some(NotificationsCommand::Hover(false)));
        assert_eq!(p.release_hover(), None, "once");
        // The notices all went while hovered: the hover goes with them.
        let (mut p, t0, now, at) = showing(1);
        p.pointer(&motion(at.0, at.1), now);
        p.set_toasts(&[], ms(t0, 2100));
        assert!(!p.model().holds_timeouts());
        assert_eq!(p.take_hold(), Some(NotificationsCommand::Hover(false)));
    }

    fn vol(percent: u8, muted: bool) -> Level {
        Level {
            kind: celestina_shell::osd::Kind::Volume,
            percent,
            muted,
        }
    }

    fn laid_level(level: Level) -> Tree {
        let mut tree = Tree::new(true, &Theme::dark());
        tree.apply(level_view(&Theme::dark(), level), Instant::now());
        tree.layout_measured((1920.0, 440.0), fake_measure);
        tree
    }

    fn rect_by(tree: &Tree, name: &'static str) -> Rect {
        let node = tree.node_by_key(Key::named(name)).expect("node");
        tree.rect(node)
    }

    #[test]
    fn the_level_face_is_274_by_40_with_icon_track_and_label() {
        let tree = laid_level(vol(50, false));
        let row = rect_by(&tree, "level-row");
        assert_eq!((row.w, row.h), (274.0, 40.0));
        let icon = rect_by(&tree, "level-icon");
        assert_eq!((icon.w, icon.h), (18.0, 18.0));
        assert_eq!(icon.x - row.x, 16.0, "padding");
        let track = rect_by(&tree, "level-track");
        assert_eq!(
            (track.w, track.h),
            (160.0, Theme::dark().linear_track_height)
        );
        assert_eq!(track.x - (icon.x + icon.w), 12.0, "gap");
        assert_eq!(rect_by(&tree, "level-fill").w, 80.0);
        let label = rect_by(&tree, "level-label");
        assert_eq!(label.w, 40.0);
        assert_eq!(label.x - (track.x + track.w), 12.0, "gap");
        assert_eq!(row.x + row.w - (label.x + label.w), 16.0, "padding");
        // 100 % fills the track; over 100 is clamped.
        assert_eq!(rect_by(&laid_level(vol(100, false)), "level-fill").w, 160.0);
        assert_eq!(rect_by(&laid_level(vol(250, false)), "level-fill").w, 160.0);
    }

    #[test]
    fn the_level_face_uses_the_displays_icons_muted_or_not() {
        assert_eq!(icon_of_level(vol(7, false)), "media-volume");
        assert_eq!(icon_of_level(vol(7, true)), "media-volume-muted");
        let mic = Level {
            kind: celestina_shell::osd::Kind::Microphone,
            percent: 7,
            muted: true,
        };
        assert_eq!(icon_of_level(mic), "mic-off");
    }

    #[test]
    fn a_level_holds_the_notice_timeout_once_and_lets_go_once() {
        let t0 = Instant::now();
        let mut p = painter(false);
        p.set_toasts(&snapshot(&machine_with(1)).toasts, t0);
        assert_eq!(p.take_hold(), None);
        p.set_level(Some(vol(40, false)), ms(t0, 100));
        assert_eq!(p.take_hold(), Some(NotificationsCommand::Hover(true)));
        // Another reading changes nothing for the server.
        p.set_level(Some(vol(45, false)), ms(t0, 200));
        assert_eq!(p.take_hold(), None);
        // The notice closed by its program while held: still held.
        p.set_toasts(&[], ms(t0, 300));
        assert_eq!(p.take_hold(), None);
        p.set_level(None, ms(t0, 400));
        assert_eq!(p.take_hold(), Some(NotificationsCommand::Hover(false)));
        assert_eq!(p.take_hold(), None);
        assert!(!p.wants_surface(ms(t0, 5000)));
    }

    #[test]
    fn a_level_alone_keeps_the_surface_until_it_leaves_and_the_island_is_back() {
        let t0 = Instant::now();
        let mut p = painter(false);
        assert!(!p.wants_surface(t0));
        p.set_level(Some(vol(40, false)), t0);
        assert!(p.wants_surface(ms(t0, 5000)));
        p.set_level(None, ms(t0, 5000));
        assert!(p.wants_surface(ms(t0, 5100)), "still shrinking");
        assert!(!p.wants_surface(ms(t0, 6000)));
        // A level that dies with its surface lets the clock run.
        p.set_level(Some(vol(40, false)), ms(t0, 7000));
        assert_eq!(p.take_hold(), Some(NotificationsCommand::Hover(true)));
        assert_eq!(p.release_hover(), Some(NotificationsCommand::Hover(false)));
    }

    #[test]
    fn the_level_face_is_laid_out_when_it_shows() {
        let t0 = Instant::now();
        let mut p = painter(false);
        p.set_level(Some(vol(30, false)), t0);
        prepare(&mut p, ms(t0, 400));
        assert_eq!(p.level_applied, Some(vol(30, false)));
        assert!(p.level.node_by_key(Key::named("level-row")).is_some());
        p.set_level(Some(vol(35, true)), ms(t0, 500));
        prepare(&mut p, ms(t0, 600));
        assert_eq!(p.level_applied, Some(vol(35, true)));
    }

    #[test]
    fn the_clock_face_rolls_over_at_the_minute_only_while_it_holds_the_capsule() {
        let t0 = Instant::now();
        let mut tick = ClockTick::default();
        assert_eq!(clock_wait(false, &mut tick, t0, 30, 0), None);
        let wait = clock_wait(true, &mut tick, t0, 30, 0);
        // 30 s to the boundary, then the tick's margin and ours.
        assert_eq!(
            wait,
            Some(Duration::from_secs(30) + TICK_MARGIN + TICK_MARGIN)
        );
        // A frame inside the minute asks for the same deadline again.
        let again = clock_wait(true, &mut tick, ms(t0, 10_000), 40, 0);
        assert_eq!(
            again,
            Some(Duration::from_secs(20) + TICK_MARGIN + TICK_MARGIN),
            "the armed deadline, less the time passed"
        );
    }

    #[test]
    fn a_change_that_shows_nothing_asks_for_no_frame() {
        let t0 = Instant::now();
        let mut p = painter(false);
        let toasts = snapshot(&machine_with(1)).toasts;
        let changed = p.set_toasts(&toasts, t0);
        assert!(p.needs_frame(changed, t0), "a notice arriving animates");
        let same = p.set_toasts(&toasts, ms(t0, 3000));
        assert!(!p.needs_frame(same, ms(t0, 3000)), "nothing changed");
        // Settled at rest, a change that leaves nothing to show is no frame.
        let calm = painter(false);
        assert!(!calm.needs_frame(true, t0));
    }

    #[test]
    fn a_left_click_on_the_clock_face_opens_the_clock_page() {
        let t0 = Instant::now();
        let mut p = painter(false);
        let r = rest();
        let at = (r.x + r.w / 2.0, r.y + r.h / 2.0);
        // At rest, and in the middle of the shrink, the face is the clock.
        assert_eq!(
            click_at(&mut p, at, celestina_ui::BTN_LEFT, t0),
            vec![BannerAction::OpenClock]
        );
        let (mut p, t1, _, _) = showing(1);
        let gone = ms(t1, 2100);
        p.set_toasts(&[], gone);
        let mid = ms(gone, 150);
        let island = p.model.look(mid).island.0;
        let inside = (island.x + island.w / 2.0, island.y + island.h / 2.0);
        assert_eq!(
            click_at(&mut p, inside, celestina_ui::BTN_LEFT, mid),
            vec![BannerAction::OpenClock]
        );
        // Beside it, or with a press that began elsewhere: nothing.
        let away = (island.x + island.w + 50.0, inside.1);
        assert!(click_at(&mut p, away, celestina_ui::BTN_LEFT, mid).is_empty());
        let press = |x, y| InputEvent::PointerPress {
            x,
            y,
            button: celestina_ui::BTN_LEFT,
            serial: 1,
        };
        let release = |x, y| InputEvent::PointerRelease {
            x,
            y,
            button: celestina_ui::BTN_LEFT,
            serial: 2,
        };
        p.pointer(&press(away.0, away.1), mid);
        assert!(p
            .pointer(&release(inside.0, inside.1), mid)
            .actions
            .is_empty());
    }

    /// The level display is on `showing(n)`'s Island, past its motions; the
    /// level's centre.
    fn with_level(n: usize) -> (NoticePainter, Instant, (f32, f32)) {
        let (mut p, t0, _, _) = showing(n);
        let on = ms(t0, 2100);
        p.set_level(Some(vol(40, false)), on);
        let now = ms(on, 2000);
        prepare(&mut p, now);
        p.take_hold();
        let island = p.model.look(now).island.0;
        (
            p,
            now,
            (island.x + island.w / 2.0, island.y + island.h / 2.0),
        )
    }

    #[test]
    fn the_level_display_takes_no_input() {
        for toasts in [0, 1, 3] {
            let (mut p, now, at) = with_level(toasts);
            assert_eq!(p.model.look(now).faces[0].kind, FaceKind::Level);
            for button in [BTN_LEFT, BTN_RIGHT, BTN_MIDDLE] {
                assert_eq!(
                    click_at(&mut p, at, button, now),
                    vec![],
                    "{toasts} {button}"
                );
            }
            let wheel = InputEvent::Axis {
                x: at.0,
                y: at.1,
                horizontal: 0.0,
                vertical: -10.0,
            };
            assert!(p.pointer(&wheel, now).actions.is_empty(), "{toasts}");
            // Its glass lets every click through.
            assert!(input_regions(&p.model.look(now)).is_empty());
        }
    }

    #[test]
    fn the_island_takes_input_again_when_the_level_leaves() {
        let (mut p, now, _) = with_level(1);
        p.set_level(None, now);
        let back = ms(now, 2000);
        prepare(&mut p, back);
        let look = p.model.look(back);
        assert!(!input_regions(&look).is_empty());
        let at = (
            look.island.0.x + look.island.0.w / 2.0,
            look.island.0.y + 30.0,
        );
        let got = click_at(&mut p, at, BTN_RIGHT, back);
        assert!(got.contains(&BannerAction::Send(NotificationsCommand::Close(1))));
    }

    #[test]
    fn one_scroll_closes_one_notice() {
        let (mut p, _, now, at) = showing(3);
        let wheel = |vertical| InputEvent::Axis {
            x: at.0,
            y: at.1,
            horizontal: 0.0,
            vertical,
        };
        let close = BannerAction::Send(NotificationsCommand::Close(3));
        // A touchpad swipe: events every 16 ms, a second long.
        let mut closes = 0;
        for i in 0..60 {
            let got = p.pointer(&wheel(-3.0), ms(now, i * 16)).actions;
            closes += got
                .iter()
                .filter(|a| matches!(a, BannerAction::Send(NotificationsCommand::Close(_))))
                .count();
        }
        assert_eq!(closes, 1);
        // A burst of three events closes one too, and so does the swipe's
        // tail that turns downwards.
        let t = ms(now, 5000);
        let mut closes = 0;
        for (i, v) in [-15.0, -15.0, -15.0, 4.0, -2.0].into_iter().enumerate() {
            let got = p.pointer(&wheel(v), ms(t, i as u64 * 40)).actions;
            closes += got.iter().filter(|a| **a == close).count();
        }
        assert_eq!(closes, 1);
        // 300 ms without an axis event ends the gesture: the next closes.
        let later = ms(t, 160 + 300);
        assert!(p.pointer(&wheel(-15.0), later).actions.contains(&close));
        // The gesture ends only with time: a swipe that passes over no
        // notice (the layout moved under a still pointer, or a gap) and
        // comes back within 300 ms closes nothing more.
        assert!(!p
            .pointer(&wheel(-15.0), ms(later, 20))
            .actions
            .contains(&close));
        let gap = InputEvent::Axis {
            x: at.0 + 500.0,
            y: at.1,
            horizontal: 0.0,
            vertical: -15.0,
        };
        assert!(!p.pointer(&gap, ms(later, 36)).actions.contains(&close));
        assert!(!p
            .pointer(&wheel(-15.0), ms(later, 52))
            .actions
            .contains(&close));
        // 300 ms after the last event, the next scroll closes again.
        assert!(p
            .pointer(&wheel(-15.0), ms(later, 52 + 300))
            .actions
            .contains(&close));
    }

    #[test]
    fn the_cards_of_a_notice_that_left_stay_while_it_fades() {
        let (mut p, t0, now, _) = showing(2);
        assert_eq!(p.shown.len(), 2);
        // The newest closes: its face fades, so its card stays in the tree
        // until the fade is done; the next notice's card is there too.
        let rest = snapshot(&machine_with(2)).toasts[1..].to_vec();
        let gone = ms(t0, 2100);
        p.set_toasts(&rest, gone);
        prepare(&mut p, ms(gone, 20));
        assert!(p.shown.contains(&2), "fading");
        assert!(p.shown.contains(&1));
        prepare(&mut p, ms(gone, 3000));
        assert_eq!(p.shown, vec![1], "gone");
        assert!(now < gone);
    }

    #[test]
    fn banners_stack_at_most_three_newest_first() {
        let snap = snapshot(&machine_with(5));
        let ids: Vec<u32> = shown(&snap.toasts).iter().map(|n| n.id).collect();
        assert_eq!(ids, vec![5, 4, 3]);
        let (p, ..) = showing(5);
        assert_eq!(p.model().toasts().len(), 3);
        assert!(p.cards.node_by_key(banner_key(2)).is_none());
        assert!(p.cards.node_by_key(banner_key(5)).is_some());
    }

    #[test]
    fn reduced_motion_makes_the_geometry_jump() {
        let t0 = Instant::now();
        let mut p = painter(true);
        p.set_toasts(&snapshot(&machine_with(1)).toasts, t0);
        let look = p.model.look(t0);
        assert_eq!(look.island.0.w, BANNER_W);
        assert_eq!(look.island.0.h, banner_height(false));
    }

    fn one(actions: &[&str]) -> Notification {
        let mut machine = Notifications::new();
        machine.post(&incoming("s", Urgency::Normal, actions), 0);
        machine.live()[0].clone()
    }

    #[test]
    fn click_runs_the_default_action_or_opens_the_page() {
        let with = one(&["default", "Open", "reply", "Reply"]);
        assert_eq!(
            actions_for(std::slice::from_ref(&with), banner_key(with.id)),
            vec![BannerAction::Send(NotificationsCommand::Invoke(
                with.id,
                DEFAULT_ACTION.to_owned()
            ))]
        );
        let without = one(&["reply", "Reply"]);
        assert_eq!(
            actions_for(std::slice::from_ref(&without), banner_key(without.id)),
            vec![
                BannerAction::OpenPage,
                BannerAction::Send(NotificationsCommand::Close(without.id))
            ]
        );
        // The default is the body's: its button is not drawn, so the
        // first button is the reply.
        assert_eq!(
            actions_for(std::slice::from_ref(&with), action_key(with.id, 0)),
            vec![BannerAction::Send(NotificationsCommand::Invoke(
                with.id,
                "reply".to_owned()
            ))]
        );
        assert_eq!(
            actions_for(std::slice::from_ref(&with), dismiss_key(with.id)),
            vec![BannerAction::Send(NotificationsCommand::Close(with.id))]
        );
        // Another banner's keys, an unknown button: nothing.
        assert!(actions_for(std::slice::from_ref(&with), banner_key(99)).is_empty());
        assert!(actions_for(std::slice::from_ref(&with), action_key(with.id, 3)).is_empty());
        assert!(actions_for(std::slice::from_ref(&with), Key::named("banners")).is_empty());
    }

    #[test]
    fn dismiss_all_closes_every_live_notification_with_reason_dismissed() {
        let mut machine = machine_with(4);
        machine.post(&incoming("c", Urgency::Critical, &[]), 9);
        let closed = machine.dismiss_all();
        assert_eq!(closed.len(), 5);
        assert!(closed.iter().all(|c| c.reason == CloseReason::Dismissed));
        let snap = snapshot(&machine);
        assert!(snap.toasts.is_empty() && snap.live.is_empty());
        assert_eq!(snap.history.len(), 5);
        assert_eq!(snap.unread, 0, "a dismissal is the person having seen it");
        // Each banner's dismiss names its own notification.
        let machine = machine_with(3);
        let snap = snapshot(&machine);
        for n in shown(&snap.toasts) {
            assert_eq!(
                actions_for(&snap.toasts, dismiss_key(n.id)),
                vec![BannerAction::Send(NotificationsCommand::Close(n.id))]
            );
        }
    }

    #[test]
    fn do_not_disturb_hides_banners_but_keeps_history() {
        let mut machine = machine_with(2);
        machine.set_quiet(true);
        machine.post(&incoming("c", Urgency::Critical, &[]), 5);
        let snap = snapshot(&machine);
        let ids: Vec<u32> = shown(&snap.toasts).iter().map(|n| n.id).collect();
        assert_eq!(ids, vec![3], "only the critical one interrupts");
        assert_eq!(snap.live.len(), 3, "the others wait for the page");
        let rows = super::super::history::history_rows(&snap, 0, 0);
        assert_eq!(rows.len(), 3);
        // Nothing but critical: no banner at all, and the page keeps them.
        let quiet = snapshot(&{
            let mut m = machine_with(2);
            m.set_quiet(true);
            m
        });
        assert!(shown(&quiet.toasts).is_empty());
        let mut painter = painter(false);
        let t0 = Instant::now();
        assert!(!painter.set_toasts(&quiet.toasts, t0));
        assert!(painter.model().at_rest(t0), "no notice, no surface");
        assert_eq!(super::super::history::history_rows(&quiet, 0, 0).len(), 2);
    }

    #[test]
    fn the_icon_is_a_theme_name_or_the_fallback_and_a_path_is_never_read() {
        assert_eq!(
            icon_of(Some(&Image::Name("firefox".to_owned()))),
            (IconKey::Theme("firefox".to_owned()), None)
        );
        for image in [Some(Image::Path("/etc/passwd".to_owned())), None] {
            assert_eq!(
                icon_of(image.as_ref()),
                (IconKey::Semantic(FALLBACK_ICON), Some(Role::TextLo))
            );
        }
    }
}
