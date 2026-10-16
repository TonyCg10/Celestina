//! The banners (design §5.5): up to three notifications stacked under the
//! Island on the focused output, newest first, on their own
//! `celestina-toasts` layer surface. This file is the pure part (the
//! stack's view, what a click on it means, its regions) and the stack's
//! state; the app creates and destroys the surface.

use std::time::Instant;

use celestina_paint::{
    silhouette_slices, IconKey, Painter, Rect, Role, Theme, Weight, LINE_HEIGHT,
};
use celestina_shell::notifications::{Image, Notification};
use celestina_ui::{view, Interaction, Key, Tree, UiEvent, View, ViewKind};
use celestina_wayland::{FrameCx, InputEvent};
use taffy::JustifyContent;

use super::excerpt;
use crate::{
    island::{
        state::SILHOUETTE_STEP,
        view::{centred, text_style, BUTTON_H},
    },
    providers::notifications::NotificationsCommand,
};

/// The banners' layer-shell namespace.
pub const NAMESPACE: &str = "celestina-toasts";
/// The most banners stacked at once.
pub const MAX_BANNERS: usize = 3;
/// A banner's width and the surface's, logical px.
pub const BANNER_W: f32 = 420.0;
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
fn buttons(n: &Notification) -> impl Iterator<Item = (usize, &str)> {
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

/// A banner's height: the large row (two rows), plus a row of buttons when
/// it has any.
#[must_use]
pub fn banner_height(theme: &Theme, with_buttons: bool) -> f32 {
    let base = 2.0 * theme.row_height;
    if with_buttons {
        base + BUTTON_H + theme.spacing.xs
    } else {
        base
    }
}

/// Where the stack starts in its surface: under the Island's exclusive
/// zone `island_zone` and the small gap. The surface is anchored to the
/// top edge with no margin, so the offset is painted.
#[must_use]
pub fn top_offset(theme: &Theme, island_zone: i32) -> f32 {
    island_zone.max(0) as f32 + theme.spacing.sm
}

/// The surface, logical px: as wide as a banner and as tall as three
/// banners with buttons under the Island. It never resizes; only its
/// regions follow the banners.
#[must_use]
pub fn surface_size(theme: &Theme, island_zone: i32) -> (u32, u32) {
    let stack = MAX_BANNERS as f32 * banner_height(theme, true)
        + (MAX_BANNERS - 1) as f32 * theme.spacing.sm;
    (
        BANNER_W.ceil() as u32,
        (top_offset(theme, island_zone) + stack).ceil() as u32,
    )
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

/// The stack of `shown` banners, newest first, `top` px down the surface.
#[must_use]
pub fn view(theme: &Theme, shown: &[Notification], top: f32) -> View {
    view::col(Key::named("banners"))
        .absolute(0.0, top)
        .width(BANNER_W)
        .gap(theme.spacing.sm)
        .children(shown.iter().map(|n| banner(theme, n)))
}

/// One banner: a Haze card with the icon, the summary on one line beside
/// the dismiss button, the body clipped to two lines and the buttons.
fn banner(theme: &Theme, n: &Notification) -> View {
    let id = u64::from(n.id);
    let text_w = BANNER_W - 2.0 * theme.spacing.md - ICON_SIZE - theme.spacing.md;
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
        .haze(theme.radius_md)
        .size(BANNER_W, banner_height(theme, with_buttons))
        .padding_xy(theme.spacing.md, theme.spacing.sm)
        .gap(theme.spacing.md)
        .interactive()
        .child(icon)
        .child(content)
}

/// What one input event on the stack asks for, and whether its look
/// changed (a frame is due).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StackInput {
    pub actions: Vec<BannerAction>,
    pub redraw: bool,
}

/// The banners' surface without its painter: the tree, the pointer state
/// and what it shows.
#[derive(Debug)]
pub struct BannerStack {
    theme: Theme,
    tree: Tree,
    input: Interaction,
    /// What the view shows, newest first (at most [`MAX_BANNERS`]).
    shown: Vec<Notification>,
    /// Banners that left and may still be fading: their silhouette keeps
    /// its blur until the tree prunes them.
    departed: Vec<u32>,
    /// The stack's offset from the surface's top.
    top: f32,
    size: (f32, f32),
    dpr: f32,
    /// The pointer is over a banner (the server's clock is paused).
    hovered: bool,
    reduced: bool,
    /// The view must be built again before the next layout.
    stale: bool,
}

impl BannerStack {
    /// An empty stack `top` px down its surface, in `theme`, under
    /// `reduced` motion.
    #[must_use]
    pub fn new(theme: &Theme, top: f32, reduced: bool) -> BannerStack {
        let (w, h) = surface_size(theme, (top - theme.spacing.sm).max(0.0) as i32);
        BannerStack {
            theme: theme.clone(),
            tree: Tree::new(reduced, theme),
            input: Interaction::new(),
            shown: Vec::new(),
            departed: Vec::new(),
            top,
            size: (w as f32, h as f32),
            dpr: 1.0,
            hovered: false,
            reduced,
            stale: true,
        }
    }

    /// Takes the server's `toasts` (newest first); whether what the stack
    /// shows changed.
    pub fn set_toasts(&mut self, toasts: &[Notification]) -> bool {
        let next = shown(toasts);
        if next == self.shown.as_slice() {
            return false;
        }
        for old in &self.shown {
            if !next.iter().any(|n| n.id == old.id) && !self.departed.contains(&old.id) {
                self.departed.push(old.id);
            }
        }
        self.departed.retain(|id| !next.iter().any(|n| n.id == *id));
        self.shown = next.to_vec();
        self.stale = true;
        true
    }

    /// Nothing to show.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shown.is_empty()
    }

    /// The pointer is over a banner.
    #[must_use]
    pub fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// The surface is going away: the hover ends with it, so the server's
    /// clock runs again.
    pub fn release_hover(&mut self) -> Option<NotificationsCommand> {
        let command = hover_command(self.hovered, false);
        self.hovered = false;
        command
    }

    fn set_reduced_motion(&mut self, on: bool) {
        if on != self.reduced {
            self.reduced = on;
            self.tree.set_reduced_motion(on);
        }
    }

    fn apply(&mut self, now: Instant) {
        self.tree
            .apply(view(&self.theme, &self.shown, self.top), now);
        self.stale = false;
    }

    /// Builds the view if it changed and lays it out, so what the input
    /// hits is what the next frame paints.
    pub fn layout(&mut self, painter: &mut Painter, now: Instant) {
        if self.stale {
            self.apply(now);
        }
        self.tree.layout(self.size, painter.text(), self.dpr);
        if let Some(fault) = self.tree.take_fault() {
            tracing::warn!(%fault, "banners: layout fault");
        }
    }

    /// The live banners' rects, newest first: what the input hits.
    #[must_use]
    pub fn live_rects(&self) -> Vec<Rect> {
        self.shown
            .iter()
            .filter_map(|n| self.tree.node_by_key(banner_key(n.id)))
            .map(|id| self.tree.rect(id))
            .collect()
    }

    /// The input region: the live banners' silhouettes only, so a click
    /// beside, between or above them reaches what is underneath.
    #[must_use]
    pub fn input_region(&self) -> Vec<Rect> {
        self.live_rects()
            .into_iter()
            .flat_map(|r| silhouette_slices(r, self.theme.radius_md, SILHOUETTE_STEP))
            .collect()
    }

    /// The blur region at `now`: every banner painted, the live ones where
    /// their slide puts them and the leaving ones while they fade.
    fn painted_region(&self, now: Instant) -> Vec<Rect> {
        let ids = self
            .shown
            .iter()
            .map(|n| n.id)
            .chain(self.departed.iter().copied());
        ids.filter_map(|id| self.tree.node_by_key(banner_key(id)))
            .filter(|node| self.tree.effective_opacity(*node, now) > 0.0)
            .flat_map(|node| {
                let r = self.tree.rect(node);
                let (dx, dy) = self.tree.offset(node, now);
                let moved = Rect {
                    x: r.x + dx,
                    y: r.y + dy,
                    ..r
                };
                silhouette_slices(moved, self.theme.radius_md, SILHOUETTE_STEP)
            })
            .collect()
    }

    /// Pointer events: entering or leaving the banners pauses or resumes
    /// expiry; a click on a banner, a button or a dismiss asks for its
    /// action. The surface never takes the keyboard.
    pub fn pointer(&mut self, ev: &InputEvent, now: Instant) -> StackInput {
        let before = self.tree.highlight();
        let events = self.input.pointer(&mut self.tree, ev, now);
        let over = match *ev {
            InputEvent::PointerMotion { x, y }
            | InputEvent::PointerPress { x, y, .. }
            | InputEvent::PointerRelease { x, y, .. }
            | InputEvent::Axis { x, y, .. } => {
                Some(self.live_rects().iter().any(|r| r.contains(x, y)))
            }
            InputEvent::PointerLeave => Some(false),
            _ => None,
        };
        let mut actions = Vec::new();
        if let Some(over) = over {
            if let Some(command) = hover_command(self.hovered, over) {
                self.hovered = over;
                actions.push(BannerAction::Send(command));
            }
        }
        for event in events {
            if let UiEvent::Click(key) = event {
                actions.extend(actions_for(&self.shown, key));
            }
        }
        StackInput {
            redraw: self.tree.highlight() != before,
            actions,
        }
    }

    /// Draws one frame: the banners, then the blur region of what was
    /// painted and the input region of the live banners. A frame follows
    /// only while something animates: an idle stack draws nothing.
    pub fn frame(&mut self, cx: &mut FrameCx<'_>, painter: &mut Painter, reduced: bool) {
        let now = cx.now;
        self.set_reduced_motion(reduced);
        if painter.theme() != &self.theme {
            self.theme = painter.theme().clone();
            self.tree.set_theme(&self.theme);
            self.stale = true;
        }
        self.size = cx.size;
        self.dpr = cx.scale as f32;
        self.layout(painter, now);
        self.departed
            .retain(|id| self.tree.node_by_key(banner_key(*id)).is_some());
        painter.begin(self.size, self.dpr, cx.blur_available);
        self.tree.paint(painter, now);
        painter.end();
        cx.ctl.set_blur_region(&self.painted_region(now));
        cx.ctl.set_input_region(&self.input_region());
        if self.tree.is_animating(now) {
            cx.ctl.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use celestina_paint::TextStyle;
    use celestina_shell::notifications::{CloseReason, Incoming, Notifications, Urgency};

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

    fn stack(reduced: bool) -> BannerStack {
        BannerStack::new(&Theme::dark(), 56.0, reduced)
    }

    fn laid_out(stack: &mut BannerStack, toasts: &[Notification], now: Instant) {
        stack.set_toasts(toasts);
        stack.apply(now);
        stack.tree.layout_measured(stack.size, fake_measure);
    }

    #[test]
    fn banners_stack_at_most_three_newest_first() {
        let machine = machine_with(5);
        let snap = snapshot(&machine);
        let ids: Vec<u32> = shown(&snap.toasts).iter().map(|n| n.id).collect();
        assert_eq!(ids, vec![5, 4, 3]);
        let mut stack = stack(false);
        let t0 = Instant::now();
        laid_out(&mut stack, &snap.toasts, t0);
        let rects = stack.live_rects();
        assert_eq!(rects.len(), 3);
        // Newest on top, each below the other with the small gap.
        let theme = Theme::dark();
        assert_eq!(rects[0].y, 56.0);
        for pair in rects.windows(2) {
            assert_eq!(pair[1].y, pair[0].y + pair[0].h + theme.spacing.sm);
        }
        assert!(rects.iter().all(|r| (r.x, r.w) == (0.0, BANNER_W)));
        assert!(rects.iter().all(|r| r.h == banner_height(&theme, false)));
        assert!(stack.tree.node_by_key(banner_key(2)).is_none());
        assert!(stack.tree.node_by_key(banner_key(5)).is_some());
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
        let mut stack = stack(false);
        laid_out(&mut stack, &quiet.toasts, Instant::now());
        assert!(stack.is_empty() && stack.input_region().is_empty());
        assert_eq!(super::super::history::history_rows(&quiet, 0, 0).len(), 2);
    }

    fn motion(x: f32, y: f32) -> InputEvent {
        InputEvent::PointerMotion { x, y }
    }

    #[test]
    fn hovering_the_stack_pauses_expiry_and_leaving_resumes_it() {
        assert_eq!(hover_command(false, false), None);
        assert_eq!(hover_command(true, true), None);
        assert_eq!(
            hover_command(false, true),
            Some(NotificationsCommand::Hover(true))
        );
        assert_eq!(
            hover_command(true, false),
            Some(NotificationsCommand::Hover(false))
        );
        let snap = snapshot(&machine_with(2));
        let mut stack = stack(false);
        let t0 = Instant::now();
        laid_out(&mut stack, &snap.toasts, t0);
        let r = stack.live_rects()[1];
        let hover = |on| BannerAction::Send(NotificationsCommand::Hover(on));
        assert_eq!(
            stack.pointer(&motion(r.x + 40.0, r.y + 10.0), t0).actions,
            vec![hover(true)]
        );
        // Moving within the stack says nothing new.
        assert!(stack
            .pointer(&motion(r.x + 50.0, r.y + 12.0), t0)
            .actions
            .is_empty());
        // Below the stack (outside every banner) is leaving it.
        assert_eq!(
            stack
                .pointer(&motion(r.x + 50.0, r.y + r.h + 100.0), t0)
                .actions,
            vec![hover(false)]
        );
        stack.pointer(&motion(r.x + 40.0, r.y + 10.0), t0);
        assert_eq!(
            stack.pointer(&InputEvent::PointerLeave, t0).actions,
            vec![hover(false)]
        );
        // A surface that goes away while hovered lets the clock run again.
        stack.pointer(&motion(r.x + 40.0, r.y + 10.0), t0);
        assert!(stack.is_hovered());
        assert_eq!(
            stack.release_hover(),
            Some(NotificationsCommand::Hover(false))
        );
        assert_eq!(stack.release_hover(), None);
    }

    #[test]
    fn a_click_on_a_banner_body_reaches_the_app() {
        let snap = snapshot(&machine_with(1));
        let mut stack = stack(false);
        let t0 = Instant::now();
        laid_out(&mut stack, &snap.toasts, t0);
        let r = stack.live_rects()[0];
        let (x, y) = (r.x + r.w / 2.0, r.y + r.h - 6.0);
        let press = InputEvent::PointerPress {
            x,
            y,
            button: celestina_ui::BTN_LEFT,
            serial: 1,
        };
        let release = InputEvent::PointerRelease {
            x,
            y,
            button: celestina_ui::BTN_LEFT,
            serial: 2,
        };
        stack.pointer(&press, t0);
        let got = stack.pointer(&release, t0).actions;
        assert_eq!(
            got,
            vec![
                BannerAction::OpenPage,
                BannerAction::Send(NotificationsCommand::Close(1))
            ]
        );
    }

    #[test]
    fn the_input_region_equals_the_banner_rectangles() {
        let snap = snapshot(&machine_with(3));
        let mut stack = stack(false);
        let t0 = Instant::now();
        laid_out(&mut stack, &snap.toasts, t0);
        let rects = stack.live_rects();
        let radius = Theme::dark().radius_md;
        let want: Vec<Rect> = rects
            .iter()
            .flat_map(|r| silhouette_slices(*r, radius, SILHOUETTE_STEP))
            .collect();
        let region = stack.input_region();
        assert_eq!(region, want);
        // Every slice lies inside a banner: the gaps and the space above
        // the stack let clicks through.
        for slice in &region {
            assert!(
                rects.iter().any(|r| slice.y >= r.y.floor()
                    && slice.y + slice.h <= (r.y + r.h).ceil()
                    && slice.x >= r.x.floor()
                    && slice.x + slice.w <= (r.x + r.w).ceil()),
                "{slice:?}"
            );
        }
        let top = region.iter().map(|s| s.y).fold(f32::MAX, f32::min);
        assert_eq!(top, 56.0);
        // The painted silhouette at rest is the same.
        assert_eq!(stack.painted_region(t0), want);
    }

    #[test]
    fn reduced_motion_yields_no_spatial_animation() {
        let first = snapshot(&machine_with(1));
        let second = snapshot(&machine_with(2));
        for (reduced, slides) in [(true, false), (false, true)] {
            let mut stack = stack(reduced);
            let t0 = Instant::now();
            laid_out(&mut stack, &first.toasts, t0);
            // A newer banner pushes the first one down.
            let t1 = t0 + std::time::Duration::from_secs(1);
            laid_out(&mut stack, &second.toasts, t1);
            let Some(old) = stack.tree.node_by_key(banner_key(1)) else {
                panic!("the first banner is gone");
            };
            let moving = stack.tree.offset(old, t1) != (0.0, 0.0);
            assert_eq!(moving, slides, "reduced {reduced}");
        }
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

    #[test]
    fn a_banner_with_buttons_is_taller_and_the_surface_holds_three_of_them() {
        let theme = Theme::dark();
        assert_eq!(banner_height(&theme, false), 2.0 * theme.row_height);
        assert!(banner_height(&theme, true) > banner_height(&theme, false));
        let (w, h) = surface_size(&theme, 48);
        assert_eq!(w, BANNER_W as u32);
        assert!(h as f32 >= top_offset(&theme, 48) + 3.0 * banner_height(&theme, true));
        assert_eq!(top_offset(&theme, 48), 48.0 + theme.spacing.sm);
    }
}
