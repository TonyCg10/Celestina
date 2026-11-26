//! What a capsule sheet shows under its row: a body, laid out in its own
//! coordinates ((0, 0) is the body box's top-left) and painted translated to
//! where the sheet puts it: the level cards and the session menu.

use std::time::{Duration, Instant};

use celestina_paint::{Painter, Rect, Role, Theme, Weight};
use celestina_ui::{fit_middle, Interaction, Key, Tree, UiEvent, View};
use celestina_wayland::InputEvent;

use super::{workspaces, Measure};
use crate::{
    island::view::{sheet_more_key, text_style},
    osd::popup::{LevelCore, Outcome},
    pieces::PageId,
    session_menu::{self, SessionAction, SessionCore},
};

/// An Island page shown as a capsule sheet (CEL-D-003): the page's tree and
/// its pointer and keyboard state, under a header the Island builds.
pub struct PageBody {
    page: PageId,
    theme: Theme,
    size: (f32, f32),
    tree: Tree,
    input: Interaction,
}

impl std::fmt::Debug for PageBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PageBody")
            .field("page", &self.page)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl PageBody {
    /// The body of `page` showing `view`, `size` big.
    #[must_use]
    pub fn new(
        page: PageId,
        theme: &Theme,
        reduced: bool,
        view: View,
        size: (f32, f32),
        now: Instant,
    ) -> PageBody {
        let mut tree = Tree::new(reduced, theme);
        tree.apply(view, now);
        PageBody {
            page,
            theme: theme.clone(),
            size,
            tree,
            input: Interaction::new(),
        }
    }

    /// The page this body shows.
    #[must_use]
    pub fn page(&self) -> PageId {
        self.page
    }

    /// Shows `view` at `size`: what the pointer and keyboard held stays.
    pub fn apply(&mut self, view: View, size: (f32, f32), now: Instant) {
        self.size = size;
        self.tree.apply(view, now);
    }

    /// The tree and its input state, for what the Island does with the
    /// focused node.
    pub fn parts(&mut self) -> (&mut Tree, &mut Interaction) {
        (&mut self.tree, &mut self.input)
    }

    fn set_theme(&mut self, theme: &Theme) -> bool {
        if *theme == self.theme {
            return false;
        }
        self.theme = theme.clone();
        self.tree.set_theme(theme);
        true
    }

    /// The node of `key`, as a body outcome: the overflow button asks for its
    /// menu, anything else is the page's to read.
    fn outcome_of(&self, key: Key, keyboard: bool) -> BodyOutcome {
        if key == sheet_more_key() {
            if let Some(id) = self.tree.node_by_key(key) {
                return BodyOutcome::More {
                    rect: self.tree.rect(id),
                    keyboard,
                };
            }
        }
        BodyOutcome::Activate { key, keyboard }
    }

    fn input(&mut self, ev: &InputEvent, now: Instant) -> BodyOutcome {
        let before = self.tree.highlight();
        let events = match ev {
            InputEvent::Key { .. } | InputEvent::FocusLost => self.input.key(&mut self.tree, ev),
            InputEvent::PopupClosed { .. } | InputEvent::FocusEnter { .. } => Vec::new(),
            _ => self.input.pointer(&mut self.tree, ev, now),
        };
        let changed = !events.is_empty() || self.tree.highlight() != before;
        for event in events {
            match event {
                UiEvent::Escape => return BodyOutcome::Close,
                UiEvent::Click(key) => return self.outcome_of(key, false),
                UiEvent::Activate(key) => return self.outcome_of(key, true),
                UiEvent::HoverChanged(..)
                | UiEvent::Scroll(..)
                | UiEvent::FieldChanged(_)
                | UiEvent::SliderChanged(..) => {}
            }
        }
        BodyOutcome::Keep { changed }
    }
}

/// The workspaces list shown as a capsule sheet (CEL-D-005): the list's tree
/// and its pointer and keyboard state. A row chosen is a workspace to focus.
pub struct WorkspacesBody {
    theme: Theme,
    size: (f32, f32),
    /// The workspaces the rows show, by id.
    ids: Vec<u64>,
    tree: Tree,
    input: Interaction,
}

impl std::fmt::Debug for WorkspacesBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspacesBody")
            .field("size", &self.size)
            .field("ids", &self.ids)
            .finish_non_exhaustive()
    }
}

impl WorkspacesBody {
    /// The body showing `view`, one row for each of `ids`.
    #[must_use]
    pub fn new(theme: &Theme, reduced: bool, view: View, ids: Vec<u64>, now: Instant) -> Self {
        let mut tree = Tree::new(reduced, theme);
        tree.apply(view, now);
        WorkspacesBody {
            theme: theme.clone(),
            size: workspaces::list_size(ids.len()),
            ids,
            tree,
            input: Interaction::new(),
        }
    }

    /// Shows `view` again, one row for each of `ids`: what the pointer and
    /// keyboard held stays, the sheet follows the new height.
    pub fn apply(&mut self, view: View, ids: Vec<u64>, now: Instant) {
        self.size = workspaces::list_size(ids.len());
        self.ids = ids;
        self.tree.apply(view, now);
    }

    /// The workspace of the row `key`, when it is one.
    fn workspace_of(&self, key: Key) -> Option<u64> {
        self.ids
            .iter()
            .copied()
            .find(|&id| workspaces::row_key(id) == key)
    }

    fn set_theme(&mut self, theme: &Theme) -> bool {
        if *theme == self.theme {
            return false;
        }
        self.theme = theme.clone();
        self.tree.set_theme(theme);
        true
    }

    fn input(&mut self, ev: &InputEvent, now: Instant) -> BodyOutcome {
        let before = self.tree.highlight();
        let events = match ev {
            InputEvent::Key { .. } | InputEvent::FocusLost => self.input.key(&mut self.tree, ev),
            InputEvent::PopupClosed { .. } | InputEvent::FocusEnter { .. } => Vec::new(),
            _ => self.input.pointer(&mut self.tree, ev, now),
        };
        let changed = !events.is_empty() || self.tree.highlight() != before;
        for event in events {
            match event {
                UiEvent::Escape => return BodyOutcome::Close,
                UiEvent::Click(key) | UiEvent::Activate(key) => {
                    if let Some(id) = self.workspace_of(key) {
                        return BodyOutcome::FocusWorkspace(id);
                    }
                }
                UiEvent::HoverChanged(..)
                | UiEvent::Scroll(..)
                | UiEvent::FieldChanged(_)
                | UiEvent::SliderChanged(..) => {}
            }
        }
        BodyOutcome::Keep { changed }
    }
}

/// A sheet's body.
#[derive(Debug)]
pub enum SheetBody {
    /// The volume's (or, later, the brightness') sliders and rows.
    Level(Box<LevelCore>),
    /// The session menu's rows and its confirmation card.
    Session(Box<SessionCore>),
    /// An Island page: its content under a header with the overflow button.
    Page(Box<PageBody>),
    /// The workspaces list: one row per workspace of the output.
    Workspaces(Box<WorkspacesBody>),
}

/// What a body asks of its sheet after an event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BodyOutcome {
    /// The sheet stays; `changed` when the body looks different.
    Keep { changed: bool },
    /// The body is done (Escape): the sheet closes.
    Close,
    /// A session row was chosen: the sheet closes and the Island posts it.
    Session(SessionAction),
    /// A node of a page body was clicked or activated (`keyboard`): the
    /// Island reads it as the page does.
    Activate { key: Key, keyboard: bool },
    /// A workspace row was chosen: the sheet closes and the Island posts it.
    FocusWorkspace(u64),
    /// The overflow button was chosen: `rect` is the button's, in body
    /// coordinates until [`super::Sheets::input`] moves it to the surface's.
    More { rect: Rect, keyboard: bool },
}

/// The height of a session question wrapped at `width`, with `measure`.
fn question_h(measure: &mut Measure<'_>, text: &str, size: f32, width: f32) -> f32 {
    let style = text_style(size, Weight::Regular, false, Role::DangerFillInk);
    measure(text, &style, Some(width)).1
}

impl SheetBody {
    /// The body box's size, logical px.
    #[must_use]
    pub fn size(&self) -> (f32, f32) {
        match self {
            SheetBody::Level(core) => core.size(),
            SheetBody::Session(core) => core.size(),
            SheetBody::Page(body) => body.size,
            SheetBody::Workspaces(body) => body.size,
        }
    }

    /// Whether the wheel at (`x`, `y`) of the body belongs to the body (a
    /// slider under the pointer) rather than to the sheet's scroll.
    #[must_use]
    pub fn takes_wheel(&self, x: f32, y: f32, now: Instant) -> bool {
        match self {
            SheetBody::Level(core) => core.slider_at(x, y, now).is_some(),
            SheetBody::Session(_) | SheetBody::Page(_) | SheetBody::Workspaces(_) => false,
        }
    }

    /// Whether the pointer is held on the body (a drag): every pointer event
    /// then belongs to it, wherever it is.
    #[must_use]
    pub fn held(&self) -> bool {
        match self {
            SheetBody::Level(core) => core.held(),
            SheetBody::Session(_) | SheetBody::Page(_) | SheetBody::Workspaces(_) => false,
        }
    }

    /// Sends what the body's throttle holds, now: the last thing the person
    /// chose is never lost to a close or a switch.
    pub fn flush(&mut self, now: Instant) {
        match self {
            SheetBody::Level(core) => {
                core.flush(now, true);
            }
            SheetBody::Session(_) | SheetBody::Page(_) | SheetBody::Workspaces(_) => {}
        }
    }

    /// Lays the body out in its own size with `lay`.
    pub fn layout(&mut self, lay: &mut dyn FnMut(&mut Tree, (f32, f32))) {
        match self {
            SheetBody::Level(core) => core.layout_in(lay),
            SheetBody::Session(core) => core.layout_in(lay),
            SheetBody::Page(body) => {
                let size = body.size;
                lay(&mut body.tree, size);
                if let Some(fault) = body.tree.take_fault() {
                    tracing::warn!(page = ?body.page, %fault, "page sheet: layout fault");
                }
            }
            SheetBody::Workspaces(body) => {
                let size = body.size;
                lay(&mut body.tree, size);
                if let Some(fault) = body.tree.take_fault() {
                    tracing::warn!(%fault, "workspaces sheet: layout fault");
                }
            }
        }
    }

    /// Paints the body at its own coordinates (the caller translates).
    pub fn paint(&mut self, painter: &mut Painter, now: Instant) {
        match self {
            SheetBody::Level(core) => core.paint(painter, now),
            SheetBody::Session(core) => core.paint(painter, now),
            SheetBody::Page(body) => body.tree.paint(painter, now),
            SheetBody::Workspaces(body) => body.tree.paint(painter, now),
        }
    }

    /// Whether the body's own animations still run.
    #[must_use]
    pub fn is_animating(&self, now: Instant) -> bool {
        match self {
            SheetBody::Level(core) => core.is_animating(now),
            SheetBody::Session(core) => core.is_animating(now),
            SheetBody::Page(body) => body.tree.is_animating(now),
            SheetBody::Workspaces(body) => body.tree.is_animating(now),
        }
    }

    /// How long until something the body holds is due (a throttled send, a
    /// level held while its monitor settles).
    #[must_use]
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        match self {
            SheetBody::Level(core) => [core.throttle_wait(now), core.graph_wait(now)]
                .into_iter()
                .flatten()
                .min(),
            SheetBody::Session(_) | SheetBody::Page(_) | SheetBody::Workspaces(_) => None,
        }
    }

    /// The rect of the body's node `key`, in body coordinates (tests).
    #[cfg(test)]
    #[must_use]
    pub fn node_rect(&self, key: celestina_ui::Key) -> Option<celestina_paint::Rect> {
        match self {
            SheetBody::Level(core) => core.node_rect(key),
            SheetBody::Session(core) => core.node_rect(key),
            SheetBody::Page(body) => body.tree.node_by_key(key).map(|id| body.tree.rect(id)),
            SheetBody::Workspaces(body) => body.tree.node_by_key(key).map(|id| body.tree.rect(id)),
        }
    }

    /// Whether Escape is the body's own (the session's confirmation card goes
    /// back to the list) rather than the sheet's close.
    #[must_use]
    pub fn takes_escape(&self) -> bool {
        match self {
            SheetBody::Level(_) => false,
            SheetBody::Session(core) => core.asking(),
            SheetBody::Page(_) | SheetBody::Workspaces(_) => false,
        }
    }

    /// The page the body shows, when it is an Island page.
    #[must_use]
    pub fn page(&self) -> Option<PageId> {
        match self {
            SheetBody::Page(body) => Some(body.page),
            SheetBody::Level(_) | SheetBody::Session(_) | SheetBody::Workspaces(_) => None,
        }
    }

    /// The page body, when it is one.
    pub fn page_body_mut(&mut self) -> Option<&mut PageBody> {
        match self {
            SheetBody::Page(body) => Some(body),
            SheetBody::Level(_) | SheetBody::Session(_) | SheetBody::Workspaces(_) => None,
        }
    }

    /// The workspaces body, when it is one.
    pub fn workspaces_body_mut(&mut self) -> Option<&mut WorkspacesBody> {
        match self {
            SheetBody::Workspaces(body) => Some(body),
            SheetBody::Level(_) | SheetBody::Session(_) | SheetBody::Page(_) => None,
        }
    }

    /// Whether the body is the volume's or brightness' level card.
    #[must_use]
    pub fn is_level(&self) -> bool {
        matches!(self, SheetBody::Level(_))
    }

    /// Handles `ev` (already in body coordinates). Names are cut with
    /// `measure`, in hand for this call only.
    pub fn input(
        &mut self,
        ev: &InputEvent,
        now: Instant,
        measure: &mut Measure<'_>,
    ) -> BodyOutcome {
        match self {
            SheetBody::Level(core) => {
                let style = core.name_style();
                let mut fit = |name: &str, width: f32| {
                    fit_middle(name, width, |text| measure(text, &style, None).0)
                };
                match core.input(ev, now, &mut fit) {
                    Outcome::Close => BodyOutcome::Close,
                    Outcome::Keep { changed } => BodyOutcome::Keep { changed },
                }
            }
            SheetBody::Session(core) => {
                let mut fit = |text: &str, size, width| question_h(measure, text, size, width);
                match core.input(ev, now, &mut fit) {
                    session_menu::Outcome::Close(Some(action)) => BodyOutcome::Session(action),
                    session_menu::Outcome::Close(None) => BodyOutcome::Close,
                    session_menu::Outcome::Keep { changed } => BodyOutcome::Keep { changed },
                }
            }
            SheetBody::Page(body) => body.input(ev, now),
            SheetBody::Workspaces(body) => body.input(ev, now),
        }
    }

    /// Takes in a new theme and the app's newest readings, sends what is due
    /// and says whether the look changed. Names are cut with `measure`.
    pub fn sync(&mut self, theme: &Theme, now: Instant, measure: &mut Measure<'_>) -> bool {
        match self {
            SheetBody::Level(core) => {
                core.cut_monitors(theme, now, |name, width, weight: Weight| {
                    let style = text_style(theme.type_scale.body, weight, false, Role::TextHi);
                    fit_middle(name, width, |text| measure(text, &style, None).0)
                });
                let style = text_style(theme.type_scale.body, Weight::Regular, false, Role::TextHi);
                let changed = core.sync(theme, now, |name, width| {
                    fit_middle(name, width, |text| measure(text, &style, None).0)
                });
                core.flush(now, false);
                changed
            }
            SheetBody::Session(core) => {
                let mut fit = |text: &str, size, width| question_h(measure, text, size, width);
                core.set_theme(theme, now, &mut fit)
            }
            SheetBody::Page(body) => body.set_theme(theme),
            SheetBody::Workspaces(body) => body.set_theme(theme),
        }
    }
}
