//! A notice while the Island page is open (CEL-D-011, design §5.5): a card at
//! the top of the page, drawn on the panel, with the notice gestures.

use celestina_shell::notifications::{Action as NoticeAction, Notification, Urgency};

use super::*;
use crate::notifications::banners;

fn notice(id: u32, buttons: &[&str]) -> Notification {
    Notification {
        id,
        app_name: "app".to_owned(),
        summary: format!("summary {id}"),
        body: "body".to_owned(),
        image: None,
        actions: buttons
            .iter()
            .map(|k| NoticeAction {
                key: (*k).to_owned(),
                label: (*k).to_owned(),
            })
            .collect(),
        urgency: Urgency::Normal,
        posted_ms: 0,
        expires_at_ms: None,
        read: false,
    }
}

/// An Island with `page` open and settled, a second after the click.
fn open_page(page: PageId) -> (Island, Instant) {
    let (mut island, t0) = bar_island(1920.0);
    island.open(page, t0);
    let t = t0 + ms(1000);
    step(&mut island, t);
    lay_fake(&mut island, t);
    (island, t)
}

fn settle(island: &mut Island, t: Instant) {
    step(island, t);
    lay_fake(island, t);
}

fn rect_of(island: &Island, key: Key) -> Option<Rect> {
    island.page.node_by_key(key).map(|id| island.page.rect(id))
}

fn card_of(island: &Island, id: u32) -> Option<Rect> {
    rect_of(island, banners::banner_key(id))
}

#[test]
fn an_open_page_takes_the_notice_as_a_card_at_its_top() {
    let (mut island, t) = open_page(PageId::CLOCK);
    let before = island.state.page_size();
    let root = rect_of(&island, Key::named("page-root")).expect("page root");
    let title = rect_of(&island, Key::named("title-row")).expect("title row");

    island.set_notice(Some(notice(7, &[])), t);
    settle(&mut island, t);

    // The page's width less 12 px each side (396 in a 420 page), 80 tall,
    // 12 px from the page's side and 8 from its top.
    let card = card_of(&island, 7).expect("the card is in the page");
    assert_eq!(root.w, 420.0);
    assert_eq!(
        (card.x, card.y, card.w, card.h),
        (root.x + 12.0, root.y + 8.0, 396.0, 80.0)
    );
    // The page grows by the card and 8 px, on its resize motion.
    assert_eq!(island.state.page_size(), (before.0, before.1 + 88.0));
    let from = island.state.rect(t, 1920.0).h;
    let later = t + ms(100);
    let mid = island.state.rect(later, 1920.0).h;
    assert_eq!(from, before.1, "it starts from the size it had");
    assert!(mid > from && mid < before.1 + 88.0, "{mid} is on its way");
    let end = island.state.rect(t + ms(1000), 1920.0).h;
    assert_eq!(end, before.1 + 88.0);
    // The content moved down by the same amount.
    let moved = rect_of(&island, Key::named("title-row")).expect("title row");
    assert_eq!(moved.y, title.y + 88.0);

    // With buttons it is 112 tall and the page grows by 120.
    island.set_notice(Some(notice(8, &["yes"])), t + ms(2000));
    settle(&mut island, t + ms(2000));
    let card = card_of(&island, 8).expect("card with buttons");
    assert_eq!(card.h, 112.0);
    assert_eq!(island.state.page_size(), (before.0, before.1 + 120.0));

    // The notice goes: the page shrinks back and the content moves up.
    island.set_notice(None, t + ms(3000));
    settle(&mut island, t + ms(3000));
    assert!(island.card().is_none());
    assert_eq!(island.state.page_size(), before);
    let back = rect_of(&island, Key::named("title-row")).expect("title row");
    assert_eq!(back.y, title.y);
}

#[test]
fn a_closed_island_takes_no_card() {
    let (mut island, t) = bar_island(1920.0);
    island.set_notice(Some(notice(7, &[])), t);
    settle(&mut island, t);
    assert!(island.card().is_none());
    assert!(!island.state.is_open());
}

#[test]
fn the_notifications_page_shows_no_card() {
    let (mut island, t) = open_page(PageId::NOTIFICATIONS);
    let before = island.state.page_size();
    island.set_notice(Some(notice(7, &[])), t);
    settle(&mut island, t);
    assert!(island.card().is_none());
    assert_eq!(island.state.page_size(), before);

    // Switching to it takes the card away again; switching off brings it back.
    let (mut island, t) = open_page(PageId::CLOCK);
    let plain = island.state.page_size();
    island.set_notice(Some(notice(7, &[])), t);
    settle(&mut island, t);
    assert!(card_of(&island, 7).is_some());
    island.open(PageId::NOTIFICATIONS, t + ms(10));
    settle(&mut island, t + ms(10));
    assert!(island.card().is_none());
    assert_eq!(island.state.shown_page(), PageId::NOTIFICATIONS);
    assert_ne!(island.state.page_size().1, plain.1 + 88.0);
}

#[test]
fn the_card_takes_the_notice_gestures() {
    let (mut island, t) = open_page(PageId::CLOCK);
    island.set_notice(Some(notice(7, &["yes"])), t);
    settle(&mut island, t);
    // The page has grown to take it.
    let t = t + ms(1000);
    settle(&mut island, t);
    let card = card_of(&island, 7).expect("card");
    let inside = (card.x + card.w - 100.0, card.y + 4.0);

    // Right click closes it; the middle click closes every live one.
    feed(&mut island, press(inside.0, inside.1, BTN_RIGHT), t);
    let right = feed(&mut island, release(inside.0, inside.1, BTN_RIGHT), t);
    assert_eq!(
        right,
        vec![IslandAction::Notification(NotificationsCommand::Close(7))]
    );
    feed(&mut island, press(inside.0, inside.1, BTN_MIDDLE), t);
    let middle = feed(&mut island, release(inside.0, inside.1, BTN_MIDDLE), t);
    assert_eq!(
        middle,
        vec![IslandAction::Notification(NotificationsCommand::DismissAll)]
    );

    // The wheel towards the top over the card closes it; down does not.
    let wheel = |v: f32| InputEvent::Axis {
        x: inside.0,
        y: inside.1,
        horizontal: 0.0,
        vertical: v,
    };
    assert_eq!(
        feed(&mut island, wheel(-15.0), t),
        vec![IslandAction::Notification(NotificationsCommand::Close(7))]
    );
    assert!(feed(&mut island, wheel(15.0), t).is_empty());
    // A wheel below the card is the page's, as today.
    let below = (card.x + 10.0, card.y + card.h + 120.0);
    let page_wheel = InputEvent::Axis {
        x: below.0,
        y: below.1,
        horizontal: 0.0,
        vertical: -15.0,
    };
    assert!(feed(&mut island, page_wheel, t).is_empty());

    // Left: a button runs its action, the close button closes it.
    let button = rect_of(&island, banners::action_key(7, 0)).expect("button");
    let ran = click_at(&mut island, centre(button), BTN_LEFT, t);
    assert_eq!(
        ran,
        vec![IslandAction::Notification(NotificationsCommand::Invoke(
            7,
            "yes".to_owned()
        ))]
    );
    let close = rect_of(&island, banners::dismiss_key(7)).expect("close");
    let closed = click_at(&mut island, centre(close), BTN_LEFT, t);
    assert_eq!(
        closed,
        vec![IslandAction::Notification(NotificationsCommand::Close(7))]
    );
    // The body opens the Notifications page and closes the notice.
    let body = (card.x + card.w - 6.0, card.y + card.h / 2.0);
    let opened = click_at(&mut island, body, BTN_LEFT, t);
    assert_eq!(
        opened,
        vec![IslandAction::Notification(NotificationsCommand::Close(7))]
    );
    assert_eq!(island.state.shown_page(), PageId::NOTIFICATIONS);
}

#[test]
fn one_scroll_closes_one_notice_on_the_card() {
    let (mut island, t) = open_page(PageId::CLOCK);
    island.set_notice(Some(notice(7, &[])), t);
    settle(&mut island, t);
    let t = t + ms(1000);
    settle(&mut island, t);
    let card = card_of(&island, 7).expect("card");
    let inside = (card.x + card.w - 100.0, card.y + 4.0);
    let wheel = |v: f32| InputEvent::Axis {
        x: inside.0,
        y: inside.1,
        horizontal: 0.0,
        vertical: v,
    };
    let close = IslandAction::Notification(NotificationsCommand::Close(7));
    // A two-finger swipe: an event every 16 ms for a second.
    let mut closes = 0;
    for i in 0..60 {
        let got = feed(&mut island, wheel(-3.0), t + ms(i * 16));
        closes += got.iter().filter(|a| **a == close).count();
    }
    assert_eq!(closes, 1);
    // 300 ms without an event ends the gesture.
    let next = t + ms(59 * 16 + 300);
    assert_eq!(feed(&mut island, wheel(-3.0), next), vec![close.clone()]);
    assert!(feed(&mut island, wheel(-3.0), next + ms(16)).is_empty());
    // The pointer leaving the card ends it at once.
    feed(
        &mut island,
        InputEvent::PointerMotion {
            x: card.x + 20.0,
            y: card.y + card.h + 150.0,
        },
        next + ms(20),
    );
    assert_eq!(feed(&mut island, wheel(-3.0), next + ms(30)), vec![close]);
}

#[test]
fn leaving_with_the_pointer_on_the_card_lets_go_of_the_servers_clock_once() {
    let (mut island, t) = open_page(PageId::CLOCK);
    island.set_notice(Some(notice(7, &[])), t);
    settle(&mut island, t);
    let t = t + ms(1000);
    settle(&mut island, t);
    let card = card_of(&island, 7).expect("card");
    assert!(!island.release_card_hold(), "nothing held yet");
    feed(
        &mut island,
        InputEvent::PointerMotion {
            x: card.x + 20.0,
            y: card.y + 20.0,
        },
        t,
    );
    assert_eq!(island.take_card_hold(), Some(true));
    assert!(island.release_card_hold(), "held: the Island is going");
    assert!(!island.release_card_hold(), "told once");
    assert_eq!(island.take_card_hold(), None);
}

#[test]
fn hovering_the_card_holds_the_servers_clock_until_it_goes() {
    let (mut island, t) = open_page(PageId::CLOCK);
    island.set_notice(Some(notice(7, &[])), t);
    settle(&mut island, t);
    let t = t + ms(1000);
    settle(&mut island, t);
    let card = card_of(&island, 7).expect("card");
    assert_eq!(island.take_card_hold(), None);
    feed(
        &mut island,
        InputEvent::PointerMotion {
            x: card.x + 20.0,
            y: card.y + 20.0,
        },
        t,
    );
    assert_eq!(island.take_card_hold(), Some(true));
    assert_eq!(island.take_card_hold(), None, "told once");
    // Off the card: released.
    feed(
        &mut island,
        InputEvent::PointerMotion {
            x: card.x + 20.0,
            y: card.y + card.h + 150.0,
        },
        t,
    );
    assert_eq!(island.take_card_hold(), Some(false));
    // On it again, then the notice goes: released with it.
    feed(
        &mut island,
        InputEvent::PointerMotion {
            x: card.x + 20.0,
            y: card.y + 20.0,
        },
        t,
    );
    assert_eq!(island.take_card_hold(), Some(true));
    island.set_notice(None, t);
    assert_eq!(island.take_card_hold(), Some(false));
}

#[test]
fn closing_the_page_with_a_notice_out_hands_it_to_the_notices_surface() {
    let (mut island, t) = open_page(PageId::CLOCK);
    let plain = island.state.page_size();
    island.set_notice(Some(notice(7, &[])), t);
    settle(&mut island, t);
    assert!(island.page_open());

    // The page closes: the app sees it closed and gives the toast to the
    // surface (it asks the Island for nothing more).
    island.close(t + ms(2000));
    assert!(!island.page_open());
    settle(&mut island, t + ms(2000));
    assert!(island.card().is_none(), "no card on a closing page");
    // The page does not jump while it shrinks: the card's room stays until
    // it is closed.
    assert_eq!(island.state.page_size().1, plain.1 + 88.0);
    // The notice the app took back is cleared from the Island.
    island.set_notice(None, t + ms(2000));
    settle(&mut island, t + ms(2100));
    assert_eq!(island.state.page_size().1, plain.1 + 88.0);
    // Opened again without a notice, the page has its own size.
    island.open(PageId::CLOCK, t + ms(4000));
    settle(&mut island, t + ms(4000));
    assert_eq!(island.state.page_size(), plain);
}

#[test]
fn a_page_opening_on_a_lent_capsule_is_the_panels_again() {
    let (mut island, t) = bar_island(1920.0);
    island.lend_capsule(true);
    assert!(!island.paints_capsule(t));
    island.open(PageId::CLOCK, t);
    assert!(!island.capsule_lent(), "taken back in the turn it opens");
    assert!(island.paints_capsule(t));
    // And once closed again it is the panel's, not the notices surface's.
    island.close(t + ms(10));
    assert!(island.paints_capsule(t + ms(2000)));
}

#[test]
fn a_press_on_the_card_of_a_page_still_growing_does_not_close_it() {
    let (mut island, t0) = bar_island(1920.0);
    island.open(PageId::CLOCK, t0);
    island.set_notice(Some(notice(7, &[])), t0);
    let late = t0 + ms(1000);
    settle(&mut island, late);
    let card = card_of(&island, 7).expect("card");
    let at = (card.x + card.w - 100.0, card.y + card.h - 4.0);
    // 40 ms into the growth the painted Island is still a sliver, far from
    // the card the page will have.
    let early = t0 + ms(40);
    assert!(!island.state.rect(early, 1920.0).contains(at.0, at.1));
    feed(&mut island, press(at.0, at.1, BTN_LEFT), early);
    assert!(island.page_open(), "the press was on the page's place");
    // A press far from the page still closes it.
    feed(&mut island, press(10.0, 300.0, BTN_LEFT), late);
    assert!(!island.page_open());
}
