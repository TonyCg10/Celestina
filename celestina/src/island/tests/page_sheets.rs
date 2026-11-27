//! The page-backed capsule sheets (CEL-D-003): every bar piece that opened an
//! Island page opens its page as a sheet, and the Island keeps its own pages.

use celestina_ui::ViewKind;
use celestina_wayland::{AnchorEdge, KeyboardInteractivity};

use super::*;
use crate::{
    connectivity::state::sample as connectivity_sample,
    island::{
        pages::{page_view, tabs},
        view::{page_sheet_size, sheet_more_key, PAGE_SHEET_MIN_W, PAGE_SHEET_W},
    },
    pieces::SheetKind,
};

const TAB_KEY: u32 = 0xff09;
const ENTER_KEY: u32 = 0xff0d;

/// The bell's key: a piece every default bar shows, the anchor of the sheets
/// these tests open.
fn bell() -> Key {
    Key::named("bell")
}

/// An Island whose bar is laid out, with every connectivity service ready
/// and the phone duty on.
fn new_island() -> (Island, Instant) {
    let (mut island, t0) = bar_island(1920.0);
    island.set_connectivity(&connectivity_sample::ready());
    island.phone_duty = true;
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    (island, t0)
}

/// Asks the bell's sheet of `page`, lets it settle and returns the time.
fn open_page_sheet(island: &mut Island, page: PageId, t: Instant) -> Instant {
    assert_eq!(
        island.sheet_request(bell(), SheetKind::Page(page), t, &mut fake_measure),
        None
    );
    let t = t + ms(400);
    step(island, t);
    t
}

/// The centre of the open sheet's body node `key`, in surface coordinates.
fn node_at(island: &Island, key: Key, now: Instant) -> (f32, f32) {
    let sheet = shown_rect(island, now);
    let Some(r) = island.sheets.body_node_rect(key) else {
        panic!("no sheet node {key:?}");
    };
    (sheet.x + r.x + r.w / 2.0, sheet.y + r.y + r.h / 2.0)
}

fn press_key(keysym: u32) -> InputEvent {
    key_event(keysym)
}

/// Tabs through the open sheet until `want` has the focus.
fn tab_to(island: &mut Island, want: Key, t: Instant) {
    for _ in 0..40 {
        feed(island, press_key(TAB_KEY), t);
        let focused = island
            .shown_page_input()
            .and_then(|(tree, _)| tree.highlight().focused.map(|id| tree.key_of(id)));
        if focused == Some(want) {
            return;
        }
    }
    panic!("the focus never reached {want:?}");
}

#[test]
fn a_bar_piece_with_a_page_opens_its_page_as_a_sheet() {
    let pages = [
        PageId::NETWORK,
        PageId::BLUETOOTH,
        PageId::NOTIFICATIONS,
        PageId::CLIPBOARD,
        PageId::POWER,
        PageId::SYSMON,
        PageId::MEDIA,
        PageId::PHONE,
        PageId::CLOCK,
    ];
    let mut t = None;
    for page in pages {
        let (mut island, t0) = new_island();
        let now = open_page_sheet(&mut island, page, t.unwrap_or(t0));
        t = None;
        assert_eq!(island.sheets.page(), Some(page), "{page:?}");
        assert!(
            !island.state.is_open(),
            "{page:?}: the Island's page stays shut"
        );
        let natural = island.page_content_size(page, t0, &mut fake_measure);
        let (w, _) = page_sheet_size(&island.theme, natural);
        assert!(
            (PAGE_SHEET_MIN_W..=PAGE_SHEET_W).contains(&w),
            "{page:?}: {w}"
        );
        assert_eq!(island.sheets.body_size().map(|s| s.0), Some(w), "{page:?}");
        assert_eq!(shown_rect(&island, now).w, w, "{page:?}");
        // The subtitle with the overflow button, and none of the Island
        // page's chrome.
        for key in [
            crate::sheet::subtitle::text_key(),
            crate::sheet::subtitle::icon_key(),
            sheet_more_key(),
        ] {
            assert!(
                island.sheets.body_node_rect(key).is_some(),
                "{page:?} {key:?}"
            );
        }
        for key in [
            Key::named("title"),
            Key::named("page-clock"),
            Key::named("tabs"),
            view::close_key(),
            view::more_key(),
            tabs::tab_key(PageId::ACTIVITY),
        ] {
            assert!(
                island.sheets.body_node_rect(key).is_none(),
                "{page:?} {key:?}"
            );
        }
        // The title is the page's own.
        let natural = island.page_content_size(page, t0, &mut fake_measure);
        let (body, _) = island.page_sheet_view(bell(), page, natural);
        let ViewKind::Text { text, .. } = &body.children[0].children[1].kind else {
            panic!("{page:?}: the subtitle's second node is not its text");
        };
        // The bell shows no text, so the subtitle names the page.
        assert_eq!(text, view::page_title(page), "{page:?}");
    }
}

#[test]
fn the_page_sheets_body_is_the_pages_content_at_its_natural_size() {
    for page in [
        PageId::NETWORK,
        PageId::BLUETOOTH,
        PageId::POWER,
        PageId::CLIPBOARD,
    ] {
        let (mut island, t0) = new_island();
        island.set_reduced_motion(true);
        let t = open_page_sheet(&mut island, page, t0);
        let natural = island.page_content_size(page, t0, &mut fake_measure);
        let (body, size) = island.page_sheet_view(bell(), page, natural);
        assert_eq!(island.sheets.body_size(), Some(size), "{page:?}");
        assert_eq!(size, page_sheet_size(&island.theme, natural), "{page:?}");
        // The content box is as tall as the content, and holds the same view.
        let content = match island.sheets.body_node_rect(Key::named("sheet-content")) {
            Some(r) => r,
            None => panic!("{page:?}: no sheet content"),
        };
        assert_eq!(content.h, natural.1, "{page:?}");
        assert_eq!(
            body.children[1].children[0],
            page_view(page).view(&island.page_cx())
        );
        let _ = t;
    }
}

#[test]
fn a_row_of_a_page_sheet_asks_for_its_typed_action() {
    let (mut island, t0) = new_island();
    let t = open_page_sheet(&mut island, PageId::NETWORK, t0);
    let cafe = network_page::row_key("cafe");
    let join = IslandAction::Connectivity(Action::JoinSaved(connectivity_sample::id("cafe")));
    // The pointer.
    let at = node_at(&island, cafe, t);
    assert_eq!(
        click_at(&mut island, at, BTN_LEFT, t),
        std::slice::from_ref(&join)
    );
    assert!(island.sheets.is_open(), "a row leaves the sheet open");
    // The keyboard: the row takes the focus, Enter acts on it.
    tab_to(&mut island, cafe, t);
    assert_eq!(feed(&mut island, press_key(ENTER_KEY), t), [join]);

    let (mut island, t0) = new_island();
    let t = open_page_sheet(&mut island, PageId::BLUETOOTH, t0);
    let headset = bluetooth_page::row_key("AA:BB:CC:DD:EE:01");
    let leave = IslandAction::Connectivity(Action::DisconnectDevice(connectivity_sample::address(
        "AA:BB:CC:DD:EE:01",
    )));
    let at = node_at(&island, headset, t);
    assert_eq!(
        click_at(&mut island, at, BTN_LEFT, t),
        std::slice::from_ref(&leave)
    );
    tab_to(&mut island, headset, t);
    assert_eq!(feed(&mut island, press_key(ENTER_KEY), t), [leave]);
}

#[test]
fn a_page_sheet_answers_only_for_its_own_page() {
    let (mut island, t0) = new_island();
    let t = open_page_sheet(&mut island, PageId::BLUETOOTH, t0);
    // A network row's key on the Bluetooth sheet asks for nothing.
    assert_eq!(
        island.page_action(PageId::NETWORK, network_page::row_key("cafe"), t),
        None
    );
    assert_eq!(
        island.connectivity_action(PageId::BLUETOOTH, network_page::row_key("cafe")),
        None
    );
}

#[test]
fn the_more_button_of_a_sheet_asks_for_the_menu_below_itself_in_surface_coordinates() {
    let (mut island, t0) = new_island();
    let t = open_page_sheet(&mut island, PageId::NETWORK, t0);
    let sheet = shown_rect(&island, t);
    let Some(button) = island.sheets.body_node_rect(sheet_more_key()) else {
        panic!("no overflow button");
    };
    let more = Rect {
        x: sheet.x + button.x,
        y: sheet.y + button.y,
        ..button
    };
    let at = node_at(&island, sheet_more_key(), t);
    let asked = click_at(&mut island, at, BTN_LEFT, t);
    let [IslandAction::OpenMenu {
        more: asked,
        keyboard,
    }] = asked.as_slice()
    else {
        panic!("the button did not ask for the menu");
    };
    assert!(!keyboard);
    assert!(near(*asked, more), "{asked:?} is not {more:?}");
    assert!(
        asked.y >= sheet.y,
        "in the surface's coordinates, not the body's"
    );
    // Hung below the button, as the Island page's menu is.
    let placement = menu::placement(*asked);
    assert_eq!(placement.anchor, *asked);
    assert_eq!(placement.anchor_edge, AnchorEdge::BottomLeft);
    assert!(island.sheets.is_open(), "the sheet stays under its menu");
    // The keyboard's way asks the same, with the keyboard noted.
    tab_to(&mut island, sheet_more_key(), t);
    let asked = feed(&mut island, press_key(ENTER_KEY), t);
    let [IslandAction::OpenMenu {
        more: asked,
        keyboard,
    }] = asked.as_slice()
    else {
        panic!("Enter did not ask for the menu");
    };
    assert!(*keyboard);
    assert!(near(*asked, more));
}

#[test]
fn a_dismissed_menu_closes_the_sheet_and_reopen_grows_it_again() {
    let (mut island, t0) = new_island();
    let t = open_page_sheet(&mut island, PageId::NETWORK, t0);
    // A choice or Escape closes only the menu.
    island.menu = Some(PopupId(1));
    assert!(island.menu_closed(PopupId(1), CloseReason::Shell, t));
    assert!(island.sheets.is_open());
    // The compositor's dismissal is a click outside: the sheet goes too.
    island.menu = Some(PopupId(2));
    assert!(island.menu_closed(PopupId(2), CloseReason::Dismissed, t));
    assert!(!island.sheets.is_open());
    assert!(island.page_shown().is_none(), "the page's menus go with it");

    // The reopen row: the sheet closes at once and grows again.
    let (mut island, t0) = new_island();
    let t = open_page_sheet(&mut island, PageId::NETWORK, t0);
    island.reopen_at(t);
    assert_eq!(island.sheets.page(), Some(PageId::NETWORK));
    assert_eq!(island.sheets.piece(), Some(bell()));
    assert!(!island.sheets.is_leaving());
    let capsule = zone(island.sheet_anchor(t).map(|a| a.capsule));
    let start = shown_rect(&island, t);
    assert!(
        start.w <= capsule.w + 1.0 && start.h <= capsule.h + 1.0,
        "{start:?} grows from {capsule:?}"
    );
    let later = t + ms(400);
    step(&mut island, later);
    let natural = island.page_content_size(PageId::NETWORK, t0, &mut fake_measure);
    assert_eq!(
        shown_rect(&island, later).w,
        page_sheet_size(&island.theme, natural).0
    );
}

#[test]
fn escape_closes_the_menu_first_and_then_the_sheet() {
    let (mut island, t0) = new_island();
    let t = open_page_sheet(&mut island, PageId::NETWORK, t0);
    assert!(!island.menu_takes_escape(&escape()), "no menu: the sheet's");
    island.menu = Some(PopupId(1));
    assert!(island.menu_takes_escape(&escape()), "the menu closes first");
    assert!(!island.menu_takes_escape(&key_release(0xff1b)));
    island.menu = None;
    assert!(feed(&mut island, escape(), t).is_empty());
    assert!(!island.sheets.is_open(), "Escape closes the sheet");
    // The Island's own page keeps its Escape.
    let (mut island, t0) = new_island();
    island.set_reduced_motion(true);
    island.open(PageId::NETWORK, t0);
    island.menu = Some(PopupId(1));
    assert!(!island.menu_takes_escape(&escape()));
}

/// A sheet of the Clipboard page over `texts`, settled.
fn clip_sheet(texts: &[&str]) -> (Island, Instant) {
    use crate::clipboard::worker::{ClipboardEvent, Preview, Row};
    let (mut island, t0) = new_island();
    let rows: Vec<Row> = texts
        .iter()
        .enumerate()
        .map(|(index, text)| Row {
            index,
            preview: Preview::Text((*text).to_owned()),
        })
        .collect();
    let total = rows.len();
    island.clipboard.apply(ClipboardEvent::Changed {
        rows,
        total,
        snapshot: 1,
    });
    island.clipboard_on = true;
    island.rebuild_clipboard(&mut clipboard_page::FakePixmaps::default());
    let t = open_page_sheet(&mut island, PageId::CLIPBOARD, t0);
    (island, t)
}

#[test]
fn the_clipboard_sheet_takes_delete_and_the_clear_confirmation_like_the_page() {
    let (mut island, t) = clip_sheet(&["first", "second"]);
    assert!(island.shows_clipboard_page());
    // Delete removes the focused entry, once per press.
    tab_to(&mut island, clipboard_page::row_key(0), t);
    assert_eq!(feed(&mut island, press_key(XKB_KEY_DELETE), t).len(), 1);
    for _ in 0..3 {
        assert!(feed(&mut island, press_key(XKB_KEY_DELETE), t).is_empty());
    }
    assert!(feed(&mut island, press_key(XKB_KEY_KP_DELETE), t).is_empty());
    feed(&mut island, key_release(XKB_KEY_DELETE), t);
    assert_eq!(feed(&mut island, press_key(XKB_KEY_DELETE), t).len(), 1);
    feed(&mut island, key_release(XKB_KEY_DELETE), t);
    // Clear asks first; the safe button takes the keyboard.
    tab_to(&mut island, clipboard_page::clear_key(), t);
    assert!(feed(&mut island, press_key(ENTER_KEY), t).is_empty());
    assert!(island.confirming_clear && island.clip_panel.confirming_clear);
    assert!(island.stale);
    island.apply_views(t);
    lay_fake(&mut island, t);
    let focused = island
        .shown_page_input()
        .and_then(|(tree, _)| tree.highlight().focused.map(|id| tree.key_of(id)));
    assert_eq!(focused, Some(clipboard_page::cancel_key()));
    // Confirming sends the verb and ends the question.
    tab_to(&mut island, clipboard_page::confirm_key(), t);
    assert_eq!(
        feed(&mut island, press_key(ENTER_KEY), t),
        [IslandAction::Clipboard(ClipboardVerb::Clear)]
    );
    assert!(!island.confirming_clear);
    // A question left open is given up with the sheet.
    tab_to(&mut island, clipboard_page::clear_key(), t);
    feed(&mut island, press_key(ENTER_KEY), t);
    assert!(island.confirming_clear);
    island.close_sheet(t);
    assert!(!island.confirming_clear && !island.clip_panel.confirming_clear);
}

#[test]
fn delete_does_nothing_on_another_page_sheet() {
    let (mut island, t0) = new_island();
    let t = open_page_sheet(&mut island, PageId::NETWORK, t0);
    assert!(!island.shows_clipboard_page());
    assert!(feed(&mut island, press_key(XKB_KEY_DELETE), t).is_empty());
}

#[test]
fn the_notifications_sheet_counts_as_viewed() {
    let (mut island, t0) = new_island();
    assert!(!island.shows_notifications());
    let t = open_page_sheet(&mut island, PageId::NOTIFICATIONS, t0);
    assert!(island.shows_notifications());
    // The Island's own page still does not exist: the toggle verb's read.
    assert_eq!(island.page(), None);
    // Closing it ends the viewing.
    island.close_sheet(t);
    assert!(!island.shows_notifications());
}

#[test]
fn the_system_sheet_takes_every_sample() {
    let (mut island, t0) = new_island();
    let t = open_page_sheet(&mut island, PageId::CLOCK, t0);
    // The bar has taken the first reading; the next is held back for it.
    island.mirror_sysmon(Some(&sample(1)), t);
    assert_ne!(island.sysmon_wake(Some(&sample(2)), t), Wake::Now);
    island.stale = false;
    island.mirror_sysmon(Some(&sample(2)), t);
    assert!(!island.stale, "the Clock sheet does not read it");

    let (mut island, t0) = new_island();
    let t = open_page_sheet(&mut island, PageId::SYSMON, t0);
    island.mirror_sysmon(Some(&sample(1)), t);
    assert_eq!(island.sysmon_wake(Some(&sample(2)), t), Wake::Now);
    island.stale = false;
    island.mirror_sysmon(Some(&sample(2)), t);
    assert!(island.stale, "the System sheet is built again for it");
    island.apply_views(t);
    assert_eq!(island.page_sysmon(), Some(&sample(2)));
    // Closed, it stops taking them.
    island.close_sheet(t);
    assert_ne!(island.sysmon_wake(Some(&sample(3)), t), Wake::Now);
}

#[test]
fn island_open_still_opens_the_island_page_with_its_tabs() {
    let (mut island, t0) = new_island();
    island.set_reduced_motion(true);
    assert!(island.command_at(&Verb::IslandOpen(PageId::NETWORK), t0));
    assert_eq!(island.page(), Some(PageId::NETWORK));
    assert_eq!(island.sheets.page(), None);
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    for key in [
        tabs::tab_key(PageId::ACTIVITY),
        view::close_key(),
        view::more_key(),
    ] {
        assert!(island.page.node_by_key(key).is_some(), "{key:?}");
    }
    assert!(island.page.node_by_key(sheet_more_key()).is_none());
    // With a sheet open, the verb closes it and the page opens after.
    island.close_everything(t0);
    island.state.close(t0, true);
    let t = open_page_sheet(&mut island, PageId::CLOCK, t0 + ms(400));
    assert!(island.command_at(&Verb::IslandOpen(PageId::NETWORK), t));
    assert!(!island.sheets.is_open());
    assert!(island.sheets.has_queued());
    let later = t + ms(600);
    step(&mut island, later);
    assert_eq!(island.page(), Some(PageId::NETWORK));
    assert_eq!(island.sheets.page(), None);
}

#[test]
fn the_phone_sheet_closes_when_the_duty_goes_off() {
    let (mut island, t0) = new_island();
    let t = open_page_sheet(&mut island, PageId::PHONE, t0);
    assert_eq!(island.sheets.page(), Some(PageId::PHONE));
    island.phone_duty = false;
    island.phone_duty_off(t);
    assert!(!island.sheets.is_open());
    // The page does not exist without its duty: no sheet opens either.
    let later = t + ms(400);
    step(&mut island, later);
    assert_eq!(
        island.sheet_request(
            bell(),
            SheetKind::Page(PageId::PHONE),
            later,
            &mut fake_measure
        ),
        None
    );
    assert!(!island.sheets.is_open());
    // Another page's sheet is left alone.
    let (mut island, t0) = new_island();
    let t = open_page_sheet(&mut island, PageId::NETWORK, t0);
    island.phone_duty_off(t);
    assert!(island.sheets.is_open());
}

#[test]
fn a_piece_asking_for_its_page_again_closes_the_sheet_and_another_switches_it() {
    let (mut island, t0) = new_island();
    let network = Key::named("network");
    let bluetooth = Key::named("bluetooth");
    // The pieces that just appeared fade in; a click needs them visible.
    let t0 = t0 + Duration::from_secs(2);
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let t0 = t0 + Duration::from_secs(2);
    lay_fake(&mut island, t0);
    // A click on the network piece opens its page as a sheet.
    let at = centre(bar_rect(&island, network));
    assert!(click_at(&mut island, at, BTN_LEFT, t0).is_empty());
    let t = t0 + ms(400);
    step(&mut island, t);
    assert_eq!(island.sheets.page(), Some(PageId::NETWORK));
    assert_eq!(island.sheets.piece(), Some(network));
    assert!(!island.state.is_open());
    // The bluetooth piece, on the same capsule, switches the open sheet.
    let at = centre(bar_rect(&island, bluetooth));
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    assert_eq!(island.sheets.page(), Some(PageId::BLUETOOTH));
    assert!(
        island.sheets.is_swapping(),
        "one window, the old body fading"
    );
    assert!(!island.sheets.is_leaving());
    // The same piece asking again closes it (it stays in its row while its
    // sheet is open).
    let t = t + ms(400);
    step(&mut island, t);
    assert_eq!(
        island.sheet_request(
            bluetooth,
            SheetKind::Page(PageId::BLUETOOTH),
            t,
            &mut fake_measure
        ),
        None
    );
    assert!(!island.sheets.is_open());
}

#[test]
fn a_page_sheet_is_rebuilt_when_what_the_page_reads_changes() {
    let (mut island, t0) = new_island();
    island.set_reduced_motion(true);
    let t = open_page_sheet(&mut island, PageId::NETWORK, t0);
    let cafe = network_page::row_key("cafe");
    assert!(island.sheets.body_node_rect(cafe).is_some());
    assert!(island
        .sheets
        .body_node_rect(Key::named("note-text"))
        .is_none());
    // The service goes away: the views are stale, and the sheet shows the
    // page's own explanation instead of the rows.
    assert!(!island.stale);
    assert!(island.set_connectivity(&ConnectivityState::default()));
    assert!(island.stale);
    step(&mut island, t);
    assert!(!island.stale);
    assert_eq!(island.sheets.page(), Some(PageId::NETWORK));
    assert!(
        island
            .sheets
            .body_node_rect(Key::named("note-text"))
            .is_some(),
        "the page's explanation is shown"
    );
}

#[test]
fn a_page_sheet_state_is_given_up_when_another_page_opens() {
    // The Clock page's month goes back to this one with the sheet.
    let (mut island, t0) = new_island();
    island.set_today(1_792_281_480, 0);
    let t = open_page_sheet(&mut island, PageId::CLOCK, t0);
    let next = island.page_action(PageId::CLOCK, clock_page::next_key(), t);
    assert_eq!(next, None);
    assert_eq!(island.month_offset, 1);
    island.close_sheet(t);
    assert_eq!(island.month_offset, 0);
    let t = open_page_sheet(&mut island, PageId::CLOCK, t + ms(400));
    assert_eq!(island.month_offset, 0);
    let _ = t;
}

#[test]
fn a_page_sheet_that_waited_for_the_island_page_is_built_again_when_it_opens() {
    let (mut island, t0) = new_island();
    island.set_reduced_motion(true);
    island.open(PageId::ACTIVITY, t0);
    let t = t0 + ms(400);
    assert_eq!(
        island.sheet_request(
            bell(),
            SheetKind::Page(PageId::NOTIFICATIONS),
            t,
            &mut fake_measure
        ),
        None
    );
    assert!(island.sheets.has_queued());
    assert!(!island.state.is_open(), "the page closes first");
    // What the page reads changes while it waits, unwatched.
    island.stale = false;
    let later = t + ms(600);
    island.sheets_before_layout(later, &mut fake_measure);
    assert_eq!(island.sheets.page(), Some(PageId::NOTIFICATIONS));
    assert!(island.stale, "the body is made again from what is now");
}

#[test]
fn the_notifications_toggle_closes_a_notifications_sheet() {
    let (mut island, t0) = new_island();
    // Nothing shown: the toggle opens the Notifications page.
    assert_eq!(
        island.notifications_toggle_verb(),
        Verb::IslandOpen(PageId::NOTIFICATIONS)
    );
    // A Notifications sheet shown: the toggle closes it.
    let t = open_page_sheet(&mut island, PageId::NOTIFICATIONS, t0);
    assert_eq!(island.page(), None);
    let verb = island.notifications_toggle_verb();
    assert_eq!(verb, Verb::IslandClose);
    assert!(island.command_at(&verb, t));
    step(&mut island, t + ms(400));
    assert_eq!(island.page_shown(), None);
    // Another page's sheet shown: the toggle opens Notifications.
    let t = open_page_sheet(&mut island, PageId::NETWORK, t + ms(400));
    assert_eq!(
        island.notifications_toggle_verb(),
        Verb::IslandOpen(PageId::NOTIFICATIONS)
    );
    let _ = t;
}

/// The press and release of a click at `(x, y)`, the pointer already there.
fn press_release(island: &mut Island, (x, y): (f32, f32), t: Instant) {
    feed(island, InputEvent::PointerMotion { x, y }, t);
    feed(island, press(x, y, BTN_LEFT), t);
}

#[test]
fn the_keyboard_is_exclusive_while_anything_is_shown_and_none_at_rest() {
    let (mut island, t0) = new_island();
    assert_eq!(
        island.keyboard_interactivity(t0),
        KeyboardInteractivity::None
    );
    // A sheet open from rest takes the keyboard at once.
    let t = open_page_sheet(&mut island, PageId::NETWORK, t0);
    assert_eq!(
        island.keyboard_interactivity(t),
        KeyboardInteractivity::Exclusive
    );
    // Closing counts as shown; gone, it is `None` again.
    island.close_sheet(t);
    assert_eq!(
        island.keyboard_interactivity(t),
        KeyboardInteractivity::Exclusive
    );
    let later = t + ms(600);
    step(&mut island, later);
    assert_eq!(
        island.keyboard_interactivity(later),
        KeyboardInteractivity::None
    );
    // The Island's own page too, and what waits behind it.
    island.open(PageId::ACTIVITY, later);
    assert_eq!(
        island.keyboard_interactivity(later),
        KeyboardInteractivity::Exclusive
    );
}

#[test]
fn the_catcher_exists_exactly_while_something_is_shown() {
    let (mut island, t0) = new_island();
    assert!(!island.wants_catcher(t0), "nothing shown");
    let t = open_page_sheet(&mut island, PageId::NETWORK, t0);
    assert!(island.wants_catcher(t), "a sheet is open");
    // A press on the catcher is the close verb: the sheet closes on its exit
    // motion, which still counts as shown.
    assert!(island.command_at(&Verb::IslandClose, t));
    assert!(!island.sheets.is_open());
    assert!(island.wants_catcher(t), "closing is still shown");
    let later = t + ms(600);
    step(&mut island, later);
    assert!(!island.wants_catcher(later), "gone after the close");
    // The Island's page, open and closing.
    island.open(PageId::ACTIVITY, later);
    assert!(island.wants_catcher(later));
    assert!(island.command_at(&Verb::IslandClose, later));
    // A request that waits keeps it.
    let (mut island, t0) = new_island();
    island.open(PageId::ACTIVITY, t0);
    let t = t0 + ms(400);
    assert_eq!(
        island.sheet_request(
            bell(),
            SheetKind::Page(PageId::NETWORK),
            t,
            &mut fake_measure
        ),
        None
    );
    assert!(island.sheets.has_queued());
    assert!(island.wants_catcher(t), "what waits is shown to come");
    // A press on the catcher gives the queue up and closes the page.
    assert!(island.command_at(&Verb::IslandClose, t));
    assert!(!island.sheets.has_queued());
}

#[test]
fn a_notifications_sheet_that_opens_from_the_queue_is_seen_when_it_opens() {
    let (mut island, t0) = new_island();
    island.set_reduced_motion(true);
    island.open(PageId::ACTIVITY, t0);
    let t = t0 + ms(400);
    assert_eq!(
        island.sheet_request(
            bell(),
            SheetKind::Page(PageId::NOTIFICATIONS),
            t,
            &mut fake_measure
        ),
        None
    );
    assert!(
        !island.shows_notifications(),
        "the input path sees nothing shown yet"
    );
    // The frame opens it: what the app's frame path then asks is true, so it
    // marks the unread notifications read.
    step(&mut island, t + ms(600));
    assert!(island.shows_notifications());
}

#[test]
fn a_phone_sheet_that_waits_is_given_up_when_the_duty_goes_off() {
    let (mut island, t0) = new_island();
    island.open(PageId::ACTIVITY, t0);
    let t = t0 + ms(400);
    assert_eq!(
        island.sheet_request(bell(), SheetKind::Page(PageId::PHONE), t, &mut fake_measure),
        None
    );
    assert!(island.sheets.has_queued());
    island.phone_duty = false;
    island.phone_duty_off(t);
    assert!(!island.sheets.has_queued(), "the waiting request went");
    // A request that slipped through with the duty off is refused too.
    island.phone_duty = true;
    island.open(PageId::ACTIVITY, t + ms(600));
    let t = t + ms(1000);
    assert_eq!(
        island.sheet_request(bell(), SheetKind::Page(PageId::PHONE), t, &mut fake_measure),
        None
    );
    assert!(island.sheets.has_queued());
    island.phone_duty = false;
    step(&mut island, t + ms(600));
    assert!(!island.sheets.is_open(), "no phone sheet without its duty");
}

#[test]
fn a_click_that_changes_nothing_still_redraws_the_released_button() {
    let (mut island, t0) = new_island();
    island.set_today(1_792_281_480, 0);
    let t = open_page_sheet(&mut island, PageId::CLOCK, t0);
    // At the limit, next-month does nothing.
    island.month_offset = clock_page::MAX_OFFSET;
    island.stale = true;
    step(&mut island, t);
    let at = node_at(&island, clock_page::next_key(), t);
    press_release(&mut island, at, t);
    let pressed = island.sheets.revision();
    feed(&mut island, release(at.0, at.1, BTN_LEFT), t);
    assert_ne!(
        island.sheets.revision(),
        pressed,
        "the release redraws the button"
    );
    assert_eq!(island.month_offset, clock_page::MAX_OFFSET);
}

#[test]
fn a_polkit_prompt_holds_the_keyboard_from_the_island() {
    let (mut island, t0) = new_island();
    let t = open_page_sheet(&mut island, PageId::NETWORK, t0);
    assert!(island.wants_catcher(t));
    // The prompt activates: the host claims, then closes what is shown.
    island.set_keyboard_claimed(true);
    assert!(island.command_at(&Verb::IslandClose, t));
    assert_eq!(
        island.keyboard_interactivity(t),
        KeyboardInteractivity::None,
        "no keyboard, even while the sheet closes"
    );
    assert!(!island.wants_catcher(t), "the catcher goes at once");
    let later = t + ms(600);
    step(&mut island, later);
    // A press that would open a sheet or a page is refused.
    assert_eq!(
        island.sheet_request(
            bell(),
            SheetKind::Page(PageId::NETWORK),
            later,
            &mut fake_measure
        ),
        None
    );
    assert!(!island.sheets.is_shown(later));
    island.open(PageId::ACTIVITY, later);
    assert!(!island.state.is_open());
    let session = SheetKind::Session;
    assert_eq!(
        island.sheet_request(bell(), session, later, &mut fake_measure),
        None
    );
    assert!(!island.sheets.is_shown(later));
    assert!(!island.wants_catcher(later));
    assert_eq!(
        island.keyboard_interactivity(later),
        KeyboardInteractivity::None
    );
    // The prompt ends: the Island opens again.
    island.set_keyboard_claimed(false);
    let t = open_page_sheet(&mut island, PageId::NETWORK, later);
    assert_eq!(
        island.keyboard_interactivity(t),
        KeyboardInteractivity::Exclusive
    );
}

/// An Island with a page sheet open under the network piece, the pointer
/// over it, and the bluetooth piece beside it ready to be clicked.
fn network_open() -> (Island, Instant, Key, Key) {
    let (mut island, t0) = new_island();
    let network = Key::named("network");
    let bluetooth = Key::named("bluetooth");
    let t0 = t0 + Duration::from_secs(2);
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let t0 = t0 + Duration::from_secs(2);
    lay_fake(&mut island, t0);
    let at = centre(bar_rect(&island, network));
    assert!(click_at(&mut island, at, BTN_LEFT, t0).is_empty());
    let t = t0 + ms(400);
    step(&mut island, t);
    (island, t, network, bluetooth)
}

#[test]
fn the_open_piece_stays_in_its_row_and_the_others_keep_their_hover() {
    let (mut island, t0) = new_island();
    let network = Key::named("network");
    let bluetooth = Key::named("bluetooth");
    let t0 = t0 + Duration::from_secs(2);
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let t0 = t0 + Duration::from_secs(2);
    lay_fake(&mut island, t0);
    let before = bar_rect(&island, network);
    let at = centre(before);
    assert!(click_at(&mut island, at, BTN_LEFT, t0).is_empty());
    let t = t0 + ms(400);
    step(&mut island, t);
    assert_eq!(island.sheets.piece(), Some(network));
    let bar = island.bar.as_ref();
    assert_eq!(
        bar.and_then(|b| b.open_piece()),
        Some(network),
        "the bar holds it open in its slot"
    );
    assert_eq!(bar_rect(&island, network), before, "it did not move");
    // Another piece keeps its hover, at full opacity.
    let at = centre(bar_rect(&island, bluetooth));
    feed(
        &mut island,
        InputEvent::PointerMotion { x: at.0, y: at.1 },
        t,
    );
    let bar = island.bar.as_ref();
    assert_eq!(bar.and_then(|b| b.hovered_key()), Some(bluetooth));
    assert_eq!(bar.map(|b| b.content_opacity(bluetooth, t)), Some(1.0));
    assert_eq!(
        bar.map(|b| b.box_roles(bluetooth).0),
        Some(Some(celestina_paint::Role::ControlHover))
    );
}

#[test]
fn switching_moves_the_pill_to_the_new_piece() {
    let (mut island, t, network, bluetooth) = network_open();
    let at = centre(bar_rect(&island, bluetooth));
    feed(
        &mut island,
        InputEvent::PointerMotion { x: at.0, y: at.1 },
        t,
    );
    feed(&mut island, press(at.0, at.1, BTN_LEFT), t);
    feed(&mut island, release(at.0, at.1, BTN_LEFT), t);
    assert_eq!(island.sheets.page(), Some(PageId::BLUETOOTH));
    // At once the new piece is the one the sheet is open on.
    assert_eq!(island.sheets.piece(), Some(bluetooth));
    step(&mut island, t);
    let later = t + ms(200);
    step(&mut island, later);
    let bar = island.bar.as_ref();
    assert_eq!(bar.and_then(|b| b.open_piece()), Some(bluetooth));
    // The old piece is back at full content in 200 ms, still in its row.
    assert_eq!(bar.map(|b| b.content_opacity(network, later)), Some(1.0));
    assert!(bar.is_some_and(|b| b.piece_rect(network).is_some_and(|r| r.w > 0.0)));
    // The pill has slid to the new piece by +350 ms.
    let end = t + ms(350);
    step(&mut island, end);
    let bar = island.bar.as_ref();
    let pills = bar.map(|b| b.pill(end)).unwrap_or_default();
    assert_eq!(pills.len(), 1, "{pills:?}");
    assert_eq!(pills[0].piece, bluetooth);
    assert_eq!(
        bar.and_then(|b| b.pill_of(bluetooth, end)),
        Some(pills[0].rect)
    );
}

#[test]
fn after_close_the_piece_lets_go_of_its_pill_and_shows_its_hover_again() {
    let (mut island, t, network, _bluetooth) = network_open();
    island.close_sheet(t);
    assert!(!island.sheets.is_open());
    // The restore moment is the first frame of the close: the pill fades
    // out and the content comes back.
    step(&mut island, t + ms(10));
    let bar = island.bar.as_ref();
    assert_eq!(bar.and_then(|b| b.open_piece()), None);
    assert!(bar.is_some_and(|b| b.content_opacity(network, t + ms(10)) < 1.0));
    // The pill's 150 ms run from that first frame.
    let gone = t + ms(160);
    step(&mut island, gone);
    assert!(island.bar.as_ref().is_some_and(|b| b.pill(gone).is_empty()));
    let t = t + ms(400);
    step(&mut island, t);
    let bar = island.bar.as_ref();
    assert_eq!(bar.map(|b| b.content_opacity(network, t)), Some(1.0));
    let at = centre(bar_rect(&island, network));
    feed(
        &mut island,
        InputEvent::PointerMotion { x: at.0, y: at.1 },
        t,
    );
    // The pointer's hover lifts it again.
    let bar = island.bar.as_ref();
    assert_eq!(bar.and_then(|b| b.hovered_key()), Some(network));
    assert_eq!(
        bar.map(|b| b.box_roles(network).0),
        Some(Some(celestina_paint::Role::ControlHover))
    );
}

/// The sheet's content box, as the body shows it.
fn content_box(island: &Island) -> celestina_paint::Rect {
    match island.sheets.body_node_rect(Key::named("sheet-content")) {
        Some(r) => r,
        None => panic!("no sheet content"),
    }
}

#[test]
fn the_energy_window_has_no_empty_area_under_its_rows() {
    let (mut island, t0) = new_island();
    island.set_reduced_motion(true);
    let t = open_page_sheet(&mut island, PageId::POWER, t0);
    let theme = &island.theme;
    let rows = match island.sheets.body_node_rect(Key::named("power-rows")) {
        Some(r) => r,
        None => panic!("no power rows"),
    };
    let content = content_box(&island);
    // The box is exactly the rows: nothing is left under the last one.
    assert_eq!(content.h, rows.h);
    assert_eq!(rows.y, content.y);
    let (_, h) = island.sheets.body_size().unwrap_or_default();
    assert_eq!(
        h,
        theme.spacing.xs
            + crate::sheet::subtitle::SUBTITLE_H
            + theme.spacing.sm
            + rows.h
            + theme.spacing.lg
    );
    // The Island's page asks for a taller content box than that.
    let island_h = page_view(PageId::POWER).wants(&island.page_cx()).1;
    let old_content_h = island_h - view::chrome_height(theme);
    assert!(content.h < old_content_h, "{} < {old_content_h}", content.h);
    assert_eq!(shown_rect(&island, t).h, h);
}

#[test]
fn a_page_window_is_as_wide_as_its_content() {
    let theme = Theme::dark();
    let pad = 2.0 * theme.spacing.lg;
    assert_eq!(page_sheet_size(&theme, (100.0, 50.0)).0, PAGE_SHEET_MIN_W);
    assert_eq!(page_sheet_size(&theme, (300.0, 50.0)).0, 300.0 + pad);
    assert_eq!(page_sheet_size(&theme, (900.0, 50.0)).0, PAGE_SHEET_W);
    assert_eq!(
        page_sheet_size(&theme, (f32::NAN, 50.0)).0,
        PAGE_SHEET_MIN_W
    );
    // On screen: each page's window is its measured content, within 240..420.
    for page in [PageId::NETWORK, PageId::BLUETOOTH, PageId::POWER] {
        let (mut island, t0) = new_island();
        let now = open_page_sheet(&mut island, page, t0);
        let natural = island.page_content_size(page, t0, &mut fake_measure);
        let w = shown_rect(&island, now).w;
        assert_eq!(w, (natural.0 + pad).clamp(PAGE_SHEET_MIN_W, PAGE_SHEET_W));
    }
}

/// A Bluetooth section with `n` paired devices.
fn bluetooth_with(n: usize) -> ConnectivityState {
    use celestina_shell::connectivity::{BtIcon, BtRow, BtSection, Service};

    let rows = (0..n).filter_map(|i| {
        BtRow::new(
            &format!("AA:BB:CC:DD:EE:{i:02X}"),
            &format!("Device {i}"),
            BtIcon::Other,
            true,
            false,
            true,
            None,
        )
    });
    let mut state = connectivity_sample::ready();
    state.apply(&crate::connectivity::ConnectivityEvent::Bluetooth(
        Service::Ready(BtSection::new(true, false, rows)),
    ));
    state
}

#[test]
fn bluetooth_with_five_devices_measures_every_row() {
    let (mut island, t0) = new_island();
    island.set_reduced_motion(true);
    island.set_connectivity(&bluetooth_with(5));
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    open_page_sheet(&mut island, PageId::BLUETOOTH, t0);
    let content = content_box(&island);
    for i in 0..5 {
        let key = bluetooth_page::row_key(&format!("AA:BB:CC:DD:EE:{i:02X}"));
        let row = match island.sheets.body_node_rect(key) {
            Some(r) => r,
            None => panic!("no row for device {i}"),
        };
        assert!(
            row.y + row.h <= content.y + content.h,
            "device {i} is cut: {row:?} in {content:?}"
        );
    }
    // One more device makes the window one row taller, and nothing else.
    let before = island.sheets.body_size().unwrap_or_default().1;
    island.set_connectivity(&bluetooth_with(6));
    step(&mut island, t0 + ms(500));
    let after = island.sheets.body_size().unwrap_or_default().1;
    assert!(
        after > before,
        "{after} > {before}: the window follows its content"
    );
}

#[test]
fn a_tall_page_still_scrolls_at_496() {
    let theme = Theme::dark();
    let surface = (1920.0, 560.0);
    // A body taller than the room: the window stops at 496, the body keeps
    // its size and scrolls.
    let body = page_sheet_size(&theme, (300.0, 600.0));
    assert!(body.1 > 496.0, "{body:?}");
    let window = crate::sheet::geometry::window(
        celestina_paint::Rect {
            x: 800.0,
            y: 8.0,
            w: 200.0,
            h: 40.0,
        },
        crate::sheet::geometry::Edge::Centre,
        body,
        surface,
    );
    assert_eq!(window.h, 496.0);
    assert_eq!(window.h, crate::sheet::geometry::max_height(surface));
    // On screen: a window never passes that cap.
    let (mut island, t0) = new_island();
    island.set_reduced_motion(true);
    island.set_connectivity(&bluetooth_with(12));
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let t = open_page_sheet(&mut island, PageId::BLUETOOTH, t0);
    let body_h = island.sheets.body_size().unwrap_or_default().1;
    assert_eq!(
        shown_rect(&island, t).h,
        body_h.min(crate::sheet::geometry::max_height(island.size))
    );
}

#[test]
fn the_level_session_and_workspaces_windows_keep_their_widths() {
    let (mut island, t0) = level_island();
    island.set_reduced_motion(true);
    let core = volume_core(&island.theme);
    assert!(island.open_level_sheet_at(volume(), core, t0));
    let t = t0 + ms(400);
    step(&mut island, t);
    assert_eq!(shown_rect(&island, t).w, 320.0, "volume");
    let (mut island, t0) = level_island();
    island.set_reduced_motion(true);
    let core = brightness_core(&island.theme);
    assert!(island.open_level_sheet_at(brightness_key(), core, t0));
    let t = t0 + ms(400);
    step(&mut island, t);
    assert_eq!(shown_rect(&island, t).w, 320.0, "brightness");
    for (piece, kind) in [
        ("session", SheetKind::Session),
        ("workspaces", SheetKind::Workspaces),
    ] {
        let (mut island, t0) = level_island();
        island.set_reduced_motion(true);
        assert_eq!(
            island.sheet_request(Key::named(piece), kind, t0, &mut fake_measure),
            None
        );
        let t = t0 + ms(400);
        step(&mut island, t);
        assert_eq!(shown_rect(&island, t).w, 240.0, "{piece}");
    }
}

#[test]
fn the_system_load_and_clock_windows_are_360_wide_and_their_parts_fit() {
    // The System page with a reading: 360, and its sparkline inside.
    let (mut island, t0) = new_island();
    island.set_reduced_motion(true);
    island.mirror_sysmon(Some(&sample(1)), t0);
    island.stale = true;
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let t = open_page_sheet(&mut island, PageId::SYSMON, t0);
    assert_eq!(shown_rect(&island, t).w, 360.0);
    let lg = island.theme.spacing.lg;
    let content = content_box(&island);
    assert_eq!(content.w, 360.0 - 2.0 * lg);
    let spark = match island
        .sheets
        .body_node_rect(Key::indexed("sysmon-spark", 0))
    {
        Some(r) => r,
        None => panic!("no sparkline"),
    };
    assert!(spark.x >= content.x && spark.x + spark.w <= content.x + content.w + 0.01);
    // The Clock page with a date: 360, its grid of seven cells inside.
    let (mut island, t0) = new_island();
    island.set_reduced_motion(true);
    island.set_today(1_792_281_480, 0);
    island.apply_views(t0);
    lay_fake(&mut island, t0);
    let t = open_page_sheet(&mut island, PageId::CLOCK, t0);
    assert_eq!(shown_rect(&island, t).w, 360.0);
    // Every other page is measured, none declares a width.
    for page in [
        PageId::NETWORK,
        PageId::BLUETOOTH,
        PageId::NOTIFICATIONS,
        PageId::CLIPBOARD,
        PageId::POWER,
        PageId::MEDIA,
        PageId::PHONE,
    ] {
        assert_eq!(page_view(page).sheet_width(), None, "{page:?}");
    }
    assert_eq!(page_view(PageId::SYSMON).sheet_width(), Some(360.0));
    assert_eq!(page_view(PageId::CLOCK).sheet_width(), Some(360.0));
}

#[test]
fn only_the_new_body_takes_input_during_a_travel() {
    let (mut island, t, network, bluetooth) = network_open();
    let cafe = network_page::row_key("cafe");
    let headset = bluetooth_page::row_key("AA:BB:CC:DD:EE:01");
    // Where the network body has its row, in the surface.
    let old_row = node_at(&island, cafe, t);
    assert!(island.sheets.body_node_rect(cafe).is_some());
    let at = centre(bar_rect(&island, bluetooth));
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    assert_eq!(island.sheets.piece(), Some(bluetooth));
    assert_ne!(island.sheets.piece(), Some(network));
    let soon = t + ms(30);
    step(&mut island, soon);
    assert!(island.sheets.is_swapping());
    // The network body is gone from the input: its row's place acts as the
    // bluetooth body has it, never as the network's.
    assert!(island.sheets.body_node_rect(cafe).is_none());
    let rect = shown_rect(&island, soon);
    assert!(rect.contains(old_row.0, old_row.1), "inside the window");
    let actions = click_at(&mut island, old_row, BTN_LEFT, soon);
    assert!(
        !actions.contains(&IslandAction::Connectivity(Action::JoinSaved(
            connectivity_sample::id("cafe")
        ))),
        "the old body takes no input: {actions:?}"
    );
    assert!(island.sheets.is_open(), "the window stays");
    // A row of the new body acts at once, at its target's coordinates, when
    // the travelling rect already covers it.
    let anchor = island.sheet_anchor(soon);
    let row = anchor
        .as_ref()
        .and_then(|a| island.sheets.body_rect(headset, a));
    let Some(row) = row else {
        panic!("no bluetooth row");
    };
    let (x, y) = (row.x + row.w / 2.0, row.y + row.h / 2.0);
    assert!(shown_rect(&island, soon).contains(x, y), "inside the rect");
    let leave = IslandAction::Connectivity(Action::DisconnectDevice(connectivity_sample::address(
        "AA:BB:CC:DD:EE:01",
    )));
    assert_eq!(click_at(&mut island, (x, y), BTN_LEFT, soon), [leave]);
    // A press outside the window closes it.
    assert!(click_at(&mut island, (30.0, 400.0), BTN_LEFT, soon).is_empty());
    assert!(!island.sheets.is_open());
}

#[test]
fn a_body_that_changes_size_during_a_travel_ends_on_its_size() {
    let (mut island, t, _network, bluetooth) = network_open();
    island.set_connectivity(&bluetooth_with(2));
    step(&mut island, t);
    let at = centre(bar_rect(&island, bluetooth));
    assert!(click_at(&mut island, at, BTN_LEFT, t).is_empty());
    step(&mut island, t);
    let before = island.sheets.body_size().unwrap_or_default().1;
    // Taller at +100, in the middle of the travel.
    let mid = t + ms(100);
    step(&mut island, mid);
    island.set_connectivity(&bluetooth_with(7));
    step(&mut island, mid + ms(1));
    let after = island.sheets.body_size().unwrap_or_default().1;
    assert!(after > before, "{after} > {before}");
    let end = t + ms(300);
    step(&mut island, end);
    let at_end = shown_rect(&island, end);
    let rest = t + ms(900);
    step(&mut island, rest);
    assert!(near(at_end, shown_rect(&island, rest)), "the new target");
    assert!(!island.sheets.wants_frame(t + ms(301)));
}
