//! The Island's pages (design §5.5): what each one shows, as a pure view of
//! what the shell knows at the instant. A page names itself, builds its
//! content from a [`PageCx`] and asks for a size; the Island lays it out
//! inside [`super::state::IslandState::page_size`], which never lets the
//! request past the surface.

pub mod activity;
pub mod notifications;
pub mod tabs;
pub mod tray;

use celestina_paint::{Role, Theme, Weight};
use celestina_ui::{view, Key, View};

use crate::{
    command::Page,
    island::{
        state::PAGE_SIZE,
        view::{centred, text_style},
    },
    providers::niri::NiriModel,
};

/// The widest an explanation line gets before it wraps, logical px.
const NOTE_MAX_W: f32 = PAGE_SIZE.0 - 64.0;

/// Who holds the well-known bus names the shell would serve, so a page can
/// say why its feature is off instead of showing nothing. `None` while the
/// shell holds the name or nobody does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameOwner {
    /// The holder of `org.freedesktop.Notifications`, described for a person.
    pub notifications: Option<String>,
    /// The holder of `org.kde.StatusNotifierWatcher`, described for a person.
    pub tray: Option<String>,
}

impl NameOwner {
    /// Becomes `other`; whether that changed anything.
    pub fn replace(&mut self, other: NameOwner) -> bool {
        if *self == other {
            return false;
        }
        *self = other;
        true
    }
}

/// One notification of the history the Notifications page lists. A
/// placeholder: its fields are filled by CEL-1-F (the notification server).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryRow {}

/// One status notifier item the Tray page lists. A placeholder: its fields
/// are filled by CEL-1-G (the tray host).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayItem {}

/// Everything a page may read while it builds its view.
#[derive(Clone, Copy, Debug)]
pub struct PageCx<'a> {
    /// The Island's theme: every colour, size and radius a page draws with.
    pub theme: &'a Theme,
    /// The largest page the surface allows, logical px.
    pub max_size: (f32, f32),
    /// The last niri model, if the compositor has sent one.
    pub model: Option<&'a NiriModel>,
    /// The output whose Island shows the page.
    pub output: &'a str,
    /// Who holds the bus names the pages depend on.
    pub owners: &'a NameOwner,
    /// The do-not-disturb setting in force (the shell's settings file).
    pub do_not_disturb: bool,
    /// The notification history, newest first (CEL-1-F).
    pub history: &'a [HistoryRow],
    /// The tray's items (CEL-1-G).
    pub items: &'a [TrayItem],
}

/// One of the Island's pages.
pub trait PageView {
    /// The page's title, as the title row shows it.
    fn title(&self) -> &str;
    /// The page's content for `cx`.
    fn view(&self, cx: &PageCx<'_>) -> View;
    /// The size the page asks for with `cx`, logical px; the Island clamps
    /// it to the surface.
    fn wants(&self, cx: &PageCx<'_>) -> (f32, f32);
}

/// A list with one non-interactive row that says `text`: what the
/// Notifications and Tray pages show until their content exists.
pub(super) fn note_list(theme: &Theme, key: Key, text: &str) -> View {
    let row = centred(view::row(Key::named("note")))
        .height(theme.row_height)
        .padding_xy(theme.spacing.md, 0.0)
        .child(
            view::text(
                Key::named("note-text"),
                text,
                text_style(theme.type_scale.body, Weight::Regular, false, Role::TextLo),
            )
            .max_width(NOTE_MAX_W)
            .grow(),
        );
    view::col(key).child(row)
}

static ACTIVITY: activity::Activity = activity::Activity;
static NOTIFICATIONS: notifications::Notifications = notifications::Notifications;
static TRAY: tray::Tray = tray::Tray;

/// The view of `page`.
#[must_use]
pub fn page_view(page: Page) -> &'static dyn PageView {
    match page {
        Page::Activity => &ACTIVITY,
        Page::Notifications => &NOTIFICATIONS,
        Page::Tray => &TRAY,
    }
}

/// What `page` asks for with `model` on a surface allowing `max_size`.
#[cfg(test)]
pub(crate) fn wants_of(
    page: &dyn PageView,
    model: Option<&NiriModel>,
    max_size: (f32, f32),
) -> (f32, f32) {
    page.wants(&PageCx {
        theme: &Theme::dark(),
        max_size,
        model,
        output: "DP-1",
        owners: &NameOwner::default(),
        do_not_disturb: false,
        history: &[],
        items: &[],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page that lists how many of each thing its context holds.
    struct Counts;

    impl PageView for Counts {
        fn title(&self) -> &str {
            "counts"
        }

        fn view(&self, cx: &PageCx<'_>) -> View {
            let owned = usize::from(cx.owners.notifications.is_some())
                + usize::from(cx.owners.tray.is_some());
            let windows = cx.model.map_or(0, |m| m.windows.len());
            let counts = [cx.history.len(), cx.items.len(), owned, windows];
            view::col(Key::named(cx.output)).children(
                counts
                    .iter()
                    .map(|n| view::col(Key::indexed("n", *n as u64))),
            )
        }

        fn wants(&self, _: &PageCx<'_>) -> (f32, f32) {
            PAGE_SIZE
        }
    }

    #[test]
    fn a_page_reads_everything_through_its_context() {
        let owners = NameOwner {
            notifications: Some("noctalia".to_owned()),
            tray: None,
        };
        let history = [HistoryRow::default()];
        let items = [TrayItem::default(), TrayItem::default()];
        let cx = PageCx {
            theme: &celestina_paint::Theme::dark(),
            max_size: (760.0, 560.0),
            model: None,
            output: "DP-1",
            owners: &owners,
            do_not_disturb: false,
            history: &history,
            items: &items,
        };
        let page = Counts;
        assert_eq!(page.title(), "counts");
        assert_eq!(wants_of(&page, None, (760.0, 560.0)), PAGE_SIZE);
        let v = page.view(&cx);
        assert_eq!(v.key, Key::named("DP-1"));
        let counts: Vec<Key> = v.children.iter().map(|c| c.key).collect();
        let want: Vec<Key> = [1, 2, 1, 0].map(|n| Key::indexed("n", n)).to_vec();
        assert_eq!(counts, want);
    }

    #[test]
    fn replacing_the_owners_reports_only_a_real_change() {
        let mut owners = NameOwner::default();
        assert!(!owners.replace(NameOwner::default()));
        let held = NameOwner {
            notifications: Some("noctalia".to_owned()),
            tray: None,
        };
        assert!(owners.replace(held.clone()));
        assert_eq!(owners, held);
        assert!(!owners.replace(held));
        assert!(owners.replace(NameOwner::default()));
    }

    #[test]
    fn every_page_has_a_view_named_like_its_tab() {
        for page in Page::ALL {
            let pv = page_view(page);
            assert_eq!(pv.title(), crate::island::view::page_title(page));
            assert_eq!(wants_of(pv, None, (760.0, 560.0)), PAGE_SIZE);
        }
    }
}
