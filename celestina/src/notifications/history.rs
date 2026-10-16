//! The rows the Notifications page lists (design §5.5): the live
//! notifications, newest first, then the page's share of the ended ones
//! (the snapshot's `history`, newest first, at most
//! [`celestina_shell::notifications::PAGE_HISTORY`]). Pure: the page reads
//! the rows, the snapshot stays the worker's.

use celestina_shell::{calendar, notifications::Notification};

use super::{banners::DEFAULT_ACTION, excerpt};
use crate::{
    island::{
        clock::format_local,
        pages::{HistoryRow, RowAction},
    },
    providers::notifications::NotificationsSnapshot,
};

const DAY_SECS: i64 = 86_400;

/// The page's rows for `snapshot`: live first, newest first, then ended.
/// `now_unix_ms` and `utc_offset` (seconds east of UTC) turn each arrival
/// into the label the row shows.
#[must_use]
pub fn history_rows(
    snapshot: &NotificationsSnapshot,
    now_unix_ms: u64,
    utc_offset: i64,
) -> Vec<HistoryRow> {
    let when = |n: &Notification| {
        if snapshot.epoch_unix_ms == 0 {
            return String::new();
        }
        let posted = snapshot.epoch_unix_ms.saturating_add(n.posted_ms);
        time_label(posted, now_unix_ms, utc_offset)
    };
    let live = snapshot.live.iter().rev().map(|n| row(n, true, when(n)));
    let ended = snapshot.history.iter().map(|n| row(n, false, when(n)));
    live.chain(ended).collect()
}

/// One row: the texts a row draws, the body cut to its excerpt so a
/// 50 000-character body is never copied into the page.
fn row(n: &Notification, live: bool, when: String) -> HistoryRow {
    let actions = if live {
        n.actions
            .iter()
            .filter(|a| a.key != DEFAULT_ACTION)
            .map(|a| RowAction {
                key: a.key.clone(),
                label: a.label.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };
    HistoryRow {
        id: n.id,
        app_name: n.app_name.clone(),
        summary: n.summary.clone(),
        body: excerpt(&n.body).to_owned(),
        image: n.image.clone(),
        live,
        read: n.read,
        when,
        actions,
    }
}

/// When Unix time `posted_unix_ms` was, seen from `now_unix_ms`, at
/// `utc_offset` seconds east of UTC: `HH:MM` the same local day, else
/// `DD/MM HH:MM`.
#[must_use]
pub fn time_label(posted_unix_ms: u64, now_unix_ms: u64, utc_offset: i64) -> String {
    let secs = posted_unix_ms / 1000;
    let day_of = |ms: u64| {
        i64::try_from(ms / 1000)
            .unwrap_or(i64::MAX)
            .saturating_add(utc_offset)
            .div_euclid(DAY_SECS)
    };
    let clock = format_local(secs, utc_offset);
    let (posted_day, today) = (day_of(posted_unix_ms), day_of(now_unix_ms));
    if posted_day == today {
        return clock;
    }
    let (_, month, day) = civil(posted_day);
    format!("{day:02}/{month:02} {clock}")
}

/// The (year, month, day) of `days` since 1970-01-01, by the calendar's
/// own month lengths.
fn civil(days: i64) -> (i32, u8, u8) {
    let mut year: i32 = 1970;
    let mut left = days;
    while left < 0 {
        year -= 1;
        left += if calendar::is_leap(year) { 366 } else { 365 };
    }
    loop {
        let length = if calendar::is_leap(year) { 366 } else { 365 };
        if left < length {
            break;
        }
        left -= length;
        year += 1;
    }
    let mut month: u8 = 1;
    loop {
        let length = i64::from(calendar::days_in_month(year, month));
        if left < length || month == 12 {
            break;
        }
        left -= length;
        month += 1;
    }
    (year, month, u8::try_from(left + 1).unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use celestina_shell::notifications::{
        CloseReason, Incoming, Notifications, Urgency, PAGE_HISTORY,
    };

    use super::*;
    use crate::{notifications::EXCERPT_CHARS, providers::notifications::snapshot};

    fn incoming(summary: &str, body: &str) -> Incoming {
        Incoming {
            app_name: "app".to_owned(),
            replaces_id: 0,
            app_icon: String::new(),
            summary: summary.to_owned(),
            body: body.to_owned(),
            actions: Vec::new(),
            urgency: Urgency::Normal,
            image: None,
            expire_timeout: -1,
        }
    }

    #[test]
    fn history_page_lists_live_then_the_last_twenty_ended() {
        let mut machine = Notifications::new();
        let ids: Vec<u32> = (0..27)
            .map(|i| machine.post(&incoming(&format!("n{i}"), "body"), i))
            .collect();
        // Five stay live; the 22 oldest went to history. Ending three more
        // leaves two live and 25 ended.
        for id in &ids[22..25] {
            assert!(machine.close(*id, CloseReason::Dismissed).is_some());
        }
        let rows = history_rows(&snapshot(&machine), 0, 0);
        assert_eq!(rows.len(), 2 + PAGE_HISTORY);
        let order: Vec<(u32, bool)> = rows.iter().map(|r| (r.id, r.live)).collect();
        assert_eq!(order[..2], [(ids[26], true), (ids[25], true)]);
        // Ended, newest first: the three closed by hand, then the pushed out.
        let ended: Vec<u32> = rows[2..].iter().map(|r| r.id).collect();
        let mut want = vec![ids[24], ids[23], ids[22]];
        want.extend(ids[..22].iter().rev().take(PAGE_HISTORY - 3));
        assert_eq!(ended, want);
        assert!(rows[2..].iter().all(|r| !r.live));
        assert_eq!(rows[0].summary, "n26");
    }

    #[test]
    fn a_row_carries_only_the_bodys_excerpt() {
        let mut machine = Notifications::new();
        let body = "\u{e9}".repeat(EXCERPT_CHARS * 3);
        machine.post(&incoming("long", &body), 0);
        let rows = history_rows(&snapshot(&machine), 0, 0);
        assert_eq!(rows[0].body.chars().count(), EXCERPT_CHARS);
        assert!(!rows[0].read);
    }

    #[test]
    fn a_live_row_offers_the_producers_buttons_but_the_default_and_an_ended_one_none() {
        let mut machine = Notifications::new();
        let with_actions = Incoming {
            actions: ["default", "Open", "reply", "Reply", "mute", "Mute"]
                .map(str::to_owned)
                .to_vec(),
            ..incoming("chat", "hi")
        };
        let id = machine.post(&with_actions, 0);
        let rows = history_rows(&snapshot(&machine), 0, 0);
        let keys: Vec<&str> = rows[0].actions.iter().map(|a| a.key.as_str()).collect();
        assert_eq!(keys, ["reply", "mute"]);
        assert_eq!(rows[0].actions[0].label, "Reply");
        assert!(machine.close(id, CloseReason::Dismissed).is_some());
        let rows = history_rows(&snapshot(&machine), 0, 0);
        assert!(!rows[0].live && rows[0].actions.is_empty());
    }

    const HOUR: u64 = 3_600_000;
    const DAY: u64 = 24 * HOUR;
    /// 2026-10-16 12:00:00 UTC.
    const NOON: u64 = 1_792_152_000_000;

    #[test]
    fn the_time_is_the_clock_today_and_the_date_before() {
        assert_eq!(time_label(NOON, NOON + HOUR, 0), "12:00");
        assert_eq!(time_label(NOON - DAY, NOON, 0), "15/10 12:00");
        // Across a month and a year boundary, at an offset.
        let new_year = 1_767_225_600_000; // 2026-01-01 00:00 UTC
        assert_eq!(time_label(new_year - HOUR, new_year, 0), "31/12 23:00");
        // 23:30 UTC is already tomorrow at +02:00, the same local day as now.
        assert_eq!(
            time_label(NOON + 11 * HOUR + HOUR / 2, NOON + 12 * HOUR, 7200),
            "01:30"
        );
        // 29 February.
        let leap = 1_709_208_000_000; // 2024-02-29 12:00 UTC
        assert_eq!(time_label(leap, leap + 2 * DAY, 0), "29/02 12:00");
    }

    #[test]
    fn a_row_without_a_known_epoch_has_no_time() {
        let mut machine = Notifications::new();
        machine.post(&incoming("n", ""), 5);
        let mut snap = snapshot(&machine);
        assert_eq!(history_rows(&snap, NOON, 0)[0].when, "");
        snap.epoch_unix_ms = NOON - 5;
        assert_eq!(history_rows(&snap, NOON, 0)[0].when, "12:00");
    }
}
