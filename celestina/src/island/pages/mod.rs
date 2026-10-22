//! The Island's pages (design §5.5): what each one shows, as a pure view of
//! what the shell knows at the instant. A page names itself, builds its
//! content from a [`PageCx`] and asks for a size; the Island lays it out
//! inside [`super::state::IslandState::page_size`], which never lets the
//! request past the surface.

pub mod activity;
pub mod bluetooth;
pub mod clipboard;
pub mod clock;
pub mod media;
pub mod network;
pub mod notifications;
pub mod phone;
pub mod power;
pub mod rows;
pub mod sysmon;
pub mod tabs;
pub mod tray;

use celestina_paint::{Role, Theme, Weight};
use celestina_shell::glance::Today;
use celestina_ui::{view, Key, View};

use crate::{
    connectivity::state::ConnectivityState,
    island::{
        state::PAGE_SIZE,
        view::{centred, text_style},
    },
    pieces::{self, PageId},
    providers::{niri::NiriModel, sysmon::SysReading, weather::WeatherReading},
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
    /// The holder of `org.kde.StatusNotifierWatcher`; the Tray page's copy
    /// describes it for a person.
    pub tray: Option<crate::tray::NameHolder>,
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

/// One notification the Notifications page lists, live or ended
/// (`crate::notifications::history::history_rows` builds them).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryRow {
    /// The server's id: a live row's dismiss names it.
    pub id: u32,
    /// The producer's name, bounded by the server.
    pub app_name: String,
    pub summary: String,
    /// The body's excerpt, never the whole body.
    pub body: String,
    /// The themed icon or the path the producer named; never read here.
    pub image: Option<celestina_shell::notifications::Image>,
    /// Still on screen (live), or ended (history).
    pub live: bool,
    /// The person has seen it.
    pub read: bool,
    /// When it arrived, as the row shows it: the clock time for today, else
    /// the day and month before it; empty when the arrival is unknown.
    pub when: String,
    /// The buttons a live row offers (the producer's actions but the
    /// default, which belongs to the banner's body), at most the machine's
    /// bound; ended rows offer none.
    pub actions: Vec<RowAction>,
}

/// One action button of a live notification's row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RowAction {
    /// The key the producer offered; what `ActionInvoked` carries.
    pub key: String,
    /// The label the producer gave it.
    pub label: String,
}

/// One status notifier item as the Tray page lists it.
pub use tray::TrayRow;

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
    pub items: &'a [TrayRow],
    /// The system monitor's newest reading (not the bar's held-back one).
    pub sysmon: Option<&'a SysReading>,
    /// The weather's reading; `None` while there is no fresh one.
    pub weather: Option<&'a WeatherReading>,
    /// Today's local date, as of the last minute the Island read.
    pub today: Option<Today>,
    /// The month the Clock page shows, in months from the current one.
    pub month_offset: i32,
    /// The local UTC offset in seconds, for the times a page prints.
    pub utc_offset: i64,
    /// The clipboard history's previews (CEL-2-G); empty while the duty is
    /// off.
    pub clipboard: &'a clipboard::ClipboardPanel,
    /// The network, Bluetooth and power state (CEL-2-F), read-only.
    pub connectivity: &'a ConnectivityState,
    /// The phone as Magnetita lists it (CEL-2-J); `Absent` while the duty
    /// is off.
    pub phone: &'a celestina_shell::phone::PhoneState,
    /// The players and the one shown (CEL-2-D).
    pub media: &'a media::MediaState,
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

/// What the Island would draw for a page the piece table does not hold:
/// nothing, at the default size. Only the pieces make a [`PageId`], so this
/// keeps [`page_view`] total and is never drawn.
struct Absent;

impl PageView for Absent {
    fn title(&self) -> &str {
        ""
    }

    fn view(&self, _: &PageCx<'_>) -> View {
        view::col(Key::named("absent"))
    }

    fn wants(&self, _: &PageCx<'_>) -> (f32, f32) {
        PAGE_SIZE
    }
}

/// The view of `page`, from the piece table.
#[must_use]
pub fn page_view(page: PageId) -> &'static dyn PageView {
    let absent: &'static dyn PageView = &Absent;
    pieces::page_entry(page).map_or(absent, |entry| entry.view)
}

/// What `page` asks for with `model` on a surface allowing `max_size`.
/// The size the real font gives `text` in `style`, wrapped at `max_width`, for
/// the tests that check a string fits where it is drawn. One `TextSystem` for
/// the whole test binary: loading the fonts is slow.
#[cfg(test)]
pub(crate) fn measure_real(
    text: &str,
    style: &celestina_paint::TextStyle,
    max_width: Option<f32>,
) -> (f32, f32) {
    use std::sync::{Mutex, OnceLock};

    use celestina_paint::{default_font_dirs, TextSystem};

    static SYSTEM: OnceLock<Mutex<TextSystem>> = OnceLock::new();
    let lock = SYSTEM.get_or_init(|| {
        Mutex::new(TextSystem::new(&default_font_dirs()).expect("Inter Variable in the checkout"))
    });
    let mut system = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    system.measure(text, style, max_width, 1.0)
}

#[cfg(test)]
pub(crate) fn wants_of(
    page: &dyn PageView,
    model: Option<&NiriModel>,
    max_size: (f32, f32),
) -> (f32, f32) {
    page.wants(&PageCx {
        clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
        theme: &Theme::dark(),
        max_size,
        model,
        output: "DP-1",
        owners: &NameOwner::default(),
        do_not_disturb: false,
        history: &[],
        items: &[],
        sysmon: None,
        weather: None,
        today: None,
        month_offset: 0,
        utc_offset: 0,
        connectivity: &ConnectivityState::default(),
        phone: &celestina_shell::phone::PhoneState::Absent,
        media: &crate::island::pages::media::NO_MEDIA,
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
        let items = [TrayRow::default(), TrayRow::default()];
        let cx = PageCx {
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            theme: &celestina_paint::Theme::dark(),
            max_size: (760.0, 560.0),
            model: None,
            output: "DP-1",
            owners: &owners,
            do_not_disturb: false,
            history: &history,
            items: &items,
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            connectivity: &ConnectivityState::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
            media: &crate::island::pages::media::NO_MEDIA,
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

    // ---- Characterisation (CEL-2-A, spec 6.7): today's pages, pinned before the
    // `Page` enum is replaced by the piece table. They must pass unchanged.

    use crate::{
        providers::niri::{Window, Workspace},
        tray::{ItemId, NameHolder, Status},
    };

    /// The three context fixtures a page is snapshotted against.
    struct Fixture {
        name: &'static str,
        model: Option<NiriModel>,
        owners: NameOwner,
        history: Vec<HistoryRow>,
        items: Vec<TrayRow>,
        media: crate::island::pages::media::MediaState,
    }

    fn media_player(n: u32, playing: bool) -> crate::media::PlayerView {
        crate::media::PlayerView {
            bus_name: format!("org.mpris.MediaPlayer2.p{n}"),
            identity: format!("Player {n}"),
            playing,
            title: format!("Title {n}"),
            artist: "Artist".to_owned(),
            album: "Album".to_owned(),
            art_url: String::new(),
            length_ms: 200_000,
            position_ms: 50_000,
            pinned: false,
        }
    }

    /// Two players, the first shown: the page with its chooser.
    fn populated_media() -> crate::island::pages::media::MediaState {
        let players = vec![media_player(1, true), media_player(2, false)];
        crate::island::pages::media::MediaState {
            active: players.first().map(|p| p.bus_name.clone()),
            players,
            cover: None,
            duty_on: true,
            unavailable: false,
        }
    }

    fn workspace(id: u64, idx: u8, name: Option<&str>, active: bool, columns: u16) -> Workspace {
        Workspace {
            id,
            idx,
            name: name.map(str::to_owned),
            output: Some("DP-1".to_owned()),
            is_active: active,
            is_focused: active,
            columns,
        }
    }

    fn window(id: u64, title: &str, workspace: u64) -> Window {
        Window {
            id,
            title: Some(title.to_owned()),
            app_id: Some("kitty".to_owned()),
            workspace_id: Some(workspace),
            is_focused: false,
            icon: Some("kitty".to_owned()),
        }
    }

    fn history_row(id: u32, live: bool, read: bool, actions: &[(&str, &str)]) -> HistoryRow {
        HistoryRow {
            id,
            app_name: "Mail".to_owned(),
            summary: format!("Summary {id}"),
            body: format!("Body {id}"),
            image: None,
            live,
            read,
            when: "09:41".to_owned(),
            actions: actions
                .iter()
                .map(|(key, label)| RowAction {
                    key: (*key).to_owned(),
                    label: (*label).to_owned(),
                })
                .collect(),
        }
    }

    fn tray_row(name: &str, status: tray::RowIcon, has_menu: bool) -> TrayRow {
        TrayRow {
            id: ItemId {
                bus_name: format!(":1.{}", name.len()),
                path: format!("/{name}"),
            },
            title: name.to_owned(),
            status: Status::Active,
            icon: status,
            has_menu,
            sni_id: String::new(),
        }
    }

    fn fixtures() -> Vec<Fixture> {
        let populated_model = NiriModel {
            workspaces: vec![
                workspace(11, 1, None, true, 2),
                workspace(12, 2, Some("web"), false, 1),
            ],
            windows: vec![window(100, "main.rs", 11), window(101, "Docs", 11)],
            outputs: vec!["DP-1".to_owned()],
        };
        let populated_history = vec![
            history_row(7, true, false, &[("reply", "Reply")]),
            history_row(6, false, true, &[]),
        ];
        let populated_items = vec![
            tray_row(
                "nm-applet",
                tray::RowIcon::Theme("network".to_owned()),
                true,
            ),
            tray_row("blueman", tray::RowIcon::default(), false),
        ];
        vec![
            Fixture {
                name: "empty",
                model: None,
                owners: NameOwner::default(),
                history: Vec::new(),
                items: Vec::new(),
                media: crate::island::pages::media::MediaState::default(),
            },
            Fixture {
                name: "populated",
                model: Some(populated_model.clone()),
                owners: NameOwner::default(),
                history: populated_history.clone(),
                items: populated_items.clone(),
                media: populated_media(),
            },
            Fixture {
                name: "name-held",
                model: Some(populated_model),
                owners: NameOwner {
                    notifications: Some("noctalia".to_owned()),
                    tray: Some(NameHolder {
                        unique: ":1.42".to_owned(),
                        comm: Some("noctalia-shell".to_owned()),
                    }),
                },
                history: populated_history,
                items: populated_items,
                media: crate::island::pages::media::MediaState::default(),
            },
        ]
    }

    /// The pre-order list of `(key, kind)` of a view tree: structure only, no
    /// pixels and no layout.
    fn structure(v: &View) -> Vec<String> {
        fn kind(v: &View) -> String {
            match &v.kind {
                celestina_ui::ViewKind::Box { .. } => "box".to_owned(),
                celestina_ui::ViewKind::Text { text, .. } => format!("text {text:?}"),
                celestina_ui::ViewKind::Icon { .. } => "icon".to_owned(),
                celestina_ui::ViewKind::Image { .. } => "image".to_owned(),
                celestina_ui::ViewKind::Field { .. } => "field".to_owned(),
                celestina_ui::ViewKind::Slider(_) => "slider".to_owned(),
            }
        }
        fn walk(v: &View, depth: usize, out: &mut Vec<String>) {
            out.push(format!("{}{:?} {}", " ".repeat(depth), v.key, kind(v)));
            for child in &v.children {
                walk(child, depth + 1, out);
            }
        }
        let mut out = Vec::new();
        walk(v, 0, &mut out);
        out
    }

    fn snapshot(page: PageId, fixture: &Fixture) -> (String, (f32, f32), Vec<String>) {
        let theme = Theme::dark();
        let pv = page_view(page);
        let cx = PageCx {
            clipboard: &crate::island::pages::clipboard::NO_CLIPBOARD,
            theme: &theme,
            max_size: (760.0, 560.0),
            model: fixture.model.as_ref(),
            output: "DP-1",
            owners: &fixture.owners,
            do_not_disturb: false,
            history: &fixture.history,
            items: &fixture.items,
            sysmon: None,
            weather: None,
            today: None,
            month_offset: 0,
            utc_offset: 0,
            connectivity: &ConnectivityState::default(),
            phone: &celestina_shell::phone::PhoneState::Absent,
            media: &fixture.media,
        };
        (
            pv.title().to_owned(),
            pv.wants(&cx),
            structure(&pv.view(&cx)),
        )
    }

    /// One pinned snapshot: what a page said for a fixture at 1.0.0.
    struct Pinned {
        page: PageId,
        fixture: &'static str,
        title: &'static str,
        wants: (f32, f32),
        tree: &'static [&'static str],
    }

    const PINNED: &[Pinned] = &[
        Pinned {
            page: PageId::ACTIVITY,
            fixture: "empty",
            title: "Actividad",
            wants: (420.0, 300.0),
            tree: &[
                "Key(11973191988154937484) box",
                " Key(3549857502210335328) text \"Sin datos del compositor\"",
            ],
        },
        Pinned {
            page: PageId::ACTIVITY,
            fixture: "populated",
            title: "Actividad",
            wants: (420.0, 300.0),
            tree: &[
                "Key(11973191988154937484) box",
                " Key(18010335123842183115) box",
                "  Key(3652693545672305645) box",
                "  Key(5201864321132865653) text \"Espacio 1\"",
                "  Key(8452039938623860829) text \"2 columnas\"",
                " Key(1795906457100220908) box",
                "  Key(15402491398479089034) box",
                "  Key(16951662173939649042) text \"web\"",
                "  Key(1755093717721092602) text \"1 columna\"",
                " Key(4796775128866736326) box",
                " Key(7460383448109864406) box",
                "  Key(16315061430520391794) icon",
                "  Key(6352214508856464827) text \"main.rs\"",
                " Key(9692698855077453815) box",
                "  Key(100632763778429587) icon",
                "  Key(4119899101888875418) text \"Docs\"",
            ],
        },
        Pinned {
            page: PageId::ACTIVITY,
            fixture: "name-held",
            title: "Actividad",
            wants: (420.0, 300.0),
            tree: &[
                "Key(11973191988154937484) box",
                " Key(18010335123842183115) box",
                "  Key(3652693545672305645) box",
                "  Key(5201864321132865653) text \"Espacio 1\"",
                "  Key(8452039938623860829) text \"2 columnas\"",
                " Key(1795906457100220908) box",
                "  Key(15402491398479089034) box",
                "  Key(16951662173939649042) text \"web\"",
                "  Key(1755093717721092602) text \"1 columna\"",
                " Key(4796775128866736326) box",
                " Key(7460383448109864406) box",
                "  Key(16315061430520391794) icon",
                "  Key(6352214508856464827) text \"main.rs\"",
                " Key(9692698855077453815) box",
                "  Key(100632763778429587) icon",
                "  Key(4119899101888875418) text \"Docs\"",
            ],
        },
        Pinned {
            page: PageId::NOTIFICATIONS,
            fixture: "empty",
            title: "Notificaciones",
            wants: (420.0, 300.0),
            tree: &[
                "Key(4062871517857070543) box",
                " Key(10974953685449066664) box",
                "  Key(5425448682025521546) text \"No molestar\"",
                "  Key(14554566484198523827) box",
                "   Key(14428596922856908928) box",
                " Key(2551833107654166475) box",
                "  Key(4321480622256717021) box",
                "   Key(12128004882728413383) text \"Sin notificaciones\"",
            ],
        },
        Pinned {
            page: PageId::NOTIFICATIONS,
            fixture: "populated",
            title: "Notificaciones",
            wants: (420.0, 378.0),
            tree: &[
                "Key(4062871517857070543) box",
                " Key(10974953685449066664) box",
                "  Key(5425448682025521546) text \"No molestar\"",
                "  Key(14554566484198523827) box",
                "   Key(14428596922856908928) box",
                " Key(2551833107654166475) box",
                "  Key(15487842299666878042) box",
                "   Key(10834817680499137729) box",
                "    Key(1830734672865307682) text \"Descartar todo\"",
                "  Key(16452716187036512225) box",
                "   Key(13453905839215043437) icon",
                "   Key(888077872929100793) box",
                "    Key(14649546737545539593) box",
                "     Key(12290077761644798770) box",
                "      Key(13589382029912239730) text \"Summary 7\"",
                "     Key(8095988470496979282) text \"09:41\"",
                "     Key(4279276437269447046) box",
                "      Key(5905735950225300024) icon",
                "    Key(11220128692444802078) box",
                "     Key(14760829962931092774) text \"Body 7\"",
                "    Key(9700973095190067993) box",
                "     Key(17067164290754220862) box",
                "      Key(17085524527825065143) text \"Reply\"",
                "  Key(14220400780068922816) box",
                "   Key(11221590432247454028) icon",
                "   Key(17102506539671063000) box",
                "    Key(12417231330577950184) box",
                "     Key(14522393168612388179) box",
                "      Key(15821697436879829139) text \"Summary 6\"",
                "     Key(10328303877464568691) text \"09:41\"",
                "    Key(13452444099412391487) box",
                "     Key(16993145369898682183) text \"Body 6\"",
            ],
        },
        Pinned {
            page: PageId::NOTIFICATIONS,
            fixture: "name-held",
            title: "Notificaciones",
            wants: (420.0, 300.0),
            tree: &[
                "Key(4062871517857070543) box",
                " Key(10974953685449066664) box",
                "  Key(5425448682025521546) text \"No molestar\"",
                "  Key(14554566484198523827) box",
                "   Key(14428596922856908928) box",
                " Key(2551833107654166475) box",
                "  Key(4321480622256717021) box",
                "   Key(12128004882728413383) text \"Otro programa atiende las notificaciones: noctalia\"",
            ],
        },
        Pinned {
            page: PageId::TRAY,
            fixture: "empty",
            title: "Bandeja",
            wants: (420.0, 300.0),
            tree: &[
                "Key(6564195396842955373) box",
                " Key(12562133755713480981) box",
                "  Key(4321480622256717021) box",
                "   Key(12128004882728413383) text \"Sin elementos en la bandeja\"",
            ],
        },
        Pinned {
            page: PageId::TRAY,
            fixture: "populated",
            title: "Bandeja",
            wants: (420.0, 300.0),
            tree: &[
                "Key(6564195396842955373) box",
                " Key(12562133755713480981) box",
                "  Key(13217651681185837584) box",
                "   Key(15872869184252048698) icon",
                "   Key(4472443167801523467) text \"nm-applet\"",
                "  Key(15449967088153426993) box",
                "   Key(18105184591219638107) icon",
                "   Key(2240127760833934058) text \"blueman\"",
            ],
        },
        Pinned {
            page: PageId::TRAY,
            fixture: "name-held",
            title: "Bandeja",
            wants: (420.0, 300.0),
            tree: &[
                "Key(6564195396842955373) box",
                " Key(12562133755713480981) box",
                "  Key(4321480622256717021) box",
                "   Key(12128004882728413383) text \"Otro programa atiende la bandeja: noctalia-shell (:1.42)\"",
            ],
        },
        Pinned {
            page: PageId::MEDIA,
            fixture: "empty",
            title: "Medios",
            wants: (420.0, 300.0),
            tree: &[
                "Key(12912867724530986883) box",
                " Key(12908918722445455807) box",
                "  Key(4321480622256717021) box",
                "   Key(12128004882728413383) text {MEDIA_DUTY_OFF}",
            ],
        },
        Pinned {
            page: PageId::MEDIA,
            fixture: "populated",
            title: "Medios",
            wants: (420.0, 370.0),
            tree: &[
                "Key(12912867724530986883) box",
                " Key(7479864549746549683) box",
                "  Key(3808597450201731097) box",
                "   Key(17994177349704965389) icon",
                "  Key(10125010047677325910) box",
                "   Key(17438774065759434300) box",
                "    Key(6224763943379904600) text \"Title 1\"",
                "   Key(9043335483256446157) box",
                "    Key(11985544631978028503) text \"Artist\"",
                "   Key(15024860372699801001) box",
                "    Key(15779014933917365123) text \"Album\"",
                " Key(13295523530429524629) box",
                "  Key(11394782890341783650) text \"0:50\"",
                "  Key(4170701675842587951) box",
                "   Key(16654101708943011928) box",
                "  Key(8605989064839537614) text \"3:20\"",
                " Key(1339505620677176319) box",
                "  Key(17705621818541804623) box",
                "   Key(7089719975625110285) icon",
                "  Key(1297283240111316756) box",
                "   Key(4857404568657520876) icon",
                "  Key(2278735135178521767) box",
                "   Key(11554350789560289103) icon",
                " Key(9322524121378681680) box",
                "  Key(5295094148091143664) box",
                "   Key(5620392692241185538) text \"Player 1\"",
                "   Key(6582625169947144852) icon",
                "  Key(8394441596627289079) box",
                "   Key(15962224898935245669) text \"Player 2\"",
                "   Key(395426872861881291) icon",
            ],
        },
        Pinned {
            page: PageId::MEDIA,
            fixture: "name-held",
            title: "Medios",
            wants: (420.0, 300.0),
            tree: &[
                "Key(12912867724530986883) box",
                " Key(12908918722445455807) box",
                "  Key(4321480622256717021) box",
                "   Key(12128004882728413383) text {MEDIA_DUTY_OFF}",
            ],
        },
    ];

    /// The pages that existed when the snapshots were taken (CEL-2-A). The
    /// glance pages (CEL-2-K), the Clipboard page (CEL-2-G), the
    /// connectivity pages (CEL-2-F), the Phone page (CEL-2-J) and the Media
    /// page (CEL-2-D) were appended to the table after them; none of these
    /// three may change because of them.
    const ORIGINAL: [PageId; 3] = [PageId::ACTIVITY, PageId::NOTIFICATIONS, PageId::TRAY];

    /// The pages appended after the snapshots, in table order. Each is pinned
    /// by its own module's tests (`pages::sysmon`, `pages::clock`,
    /// `pages::clipboard`, `pages::network`, `pages::bluetooth`,
    /// `pages::power`, `pages::phone`), except Media, pinned in [`PINNED`]
    /// against the shared fixtures.
    const LATER: [PageId; 8] = [
        PageId::SYSMON,
        PageId::CLOCK,
        PageId::CLIPBOARD,
        PageId::NETWORK,
        PageId::BLUETOOTH,
        PageId::POWER,
        PageId::PHONE,
        PageId::MEDIA,
    ];

    /// The pages [`PINNED`] snapshots against every fixture.
    const PINNED_HERE: [PageId; 4] = [
        PageId::ACTIVITY,
        PageId::NOTIFICATIONS,
        PageId::TRAY,
        PageId::MEDIA,
    ];

    #[test]
    fn every_page_of_the_table_is_pinned_here_or_declared_pinned_elsewhere() {
        // A page added to the table without a decision escapes both lists
        // and fails here: either pin it above or declare where it is.
        let mut known: Vec<PageId> = ORIGINAL.to_vec();
        known.extend(LATER);
        assert_eq!(PageId::all().to_vec(), known);
        for page in PINNED_HERE {
            assert!(
                PINNED.iter().any(|p| p.page == page),
                "{page:?} has no pinned snapshot"
            );
        }
    }

    #[test]
    fn page_title_wants_and_view_structure_are_pinned() {
        let fixtures = fixtures();
        assert_eq!(PINNED.len(), PINNED_HERE.len() * fixtures.len());
        for page in PINNED_HERE {
            for fixture in &fixtures {
                let pinned = PINNED
                    .iter()
                    .find(|p| p.page == page && p.fixture == fixture.name);
                let Some(pinned) = pinned else {
                    panic!("no snapshot for {page:?} / {}", fixture.name);
                };
                let (title, wants, tree) = snapshot(page, fixture);
                let at = format!("{page:?} / {}", fixture.name);
                assert_eq!(title, pinned.title, "{at}: title");
                assert_eq!(wants, pinned.wants, "{at}: wants");
                let want: Vec<String> = pinned
                    .tree
                    .iter()
                    .map(|line| {
                        line.replace(
                            "{MEDIA_DUTY_OFF}",
                            &format!("{:?}", crate::copy::MEDIA_DUTY_OFF),
                        )
                    })
                    .collect();
                assert_eq!(tree, want, "{at}: view structure");
            }
        }
    }

    #[test]
    fn page_order_and_words_are_pinned() {
        // The original three keep their places; later pages follow them.
        assert_eq!(PageId::all()[..3], ORIGINAL);
        assert_eq!(PageId::all()[3..], LATER);
        let words: Vec<&str> = PageId::all().into_iter().map(PageId::word).collect();
        assert_eq!(
            words,
            [
                "activity",
                "notifications",
                "tray",
                "sysmon",
                "clock",
                "clipboard",
                "network",
                "bluetooth",
                "power",
                "phone",
                "media"
            ]
        );
    }

    #[test]
    fn every_page_has_a_view_named_like_its_tab() {
        for page in PageId::all() {
            let pv = page_view(page);
            assert_eq!(pv.title(), crate::island::view::page_title(page));
            assert_eq!(wants_of(pv, None, (760.0, 560.0)), PAGE_SIZE);
        }
    }
}
