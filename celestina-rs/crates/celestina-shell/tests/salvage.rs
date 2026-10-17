//! The two contracts the salvage adds on top of the copied modules: the
//! shell's own settings file carries only do-not-disturb, the duties, the
//! energy switches and the wallpaper, and the notifications history keeps
//! fifty while its page shows twenty.

use celestina_shell::notifications::{
    CloseReason, Incoming, Notifications, MAX_HISTORY, PAGE_HISTORY,
};
use celestina_shell::settings::{Settings, Store, WriteOutcome, SCHEMA_VERSION};

#[test]
fn settings_round_trips_only_the_shells_own_choices() {
    let settings = Settings {
        do_not_disturb: true,
        ..Settings::default()
    };

    let bytes = settings.to_bytes().expect("the settings serialize");
    let text = String::from_utf8(bytes.clone()).expect("the settings are UTF-8");
    let value: serde_json::Value = serde_json::from_str(&text).expect("the file is JSON");
    let keys: Vec<&str> = value
        .as_object()
        .expect("the file is one object")
        .keys()
        .map(String::as_str)
        .collect();
    // CEL-2-B adds the `energy` table and CEL-2-E the `wallpaper` one; the
    // `duties` table carries both units' switches (all default-valued). The
    // `bar` table and `duties.bar` (CEL-2-A) are written only once chosen.
    assert!(value["duties"].get("bar").is_none());
    // CEL-2-K's `weather` location is likewise written only once chosen.
    assert!(value.get("weather").is_none());
    // CEL-2-G adds `duties.clipboard`, written off like the other duties.
    assert_eq!(value["duties"]["clipboard"], "off");
    assert_eq!(
        keys,
        [
            "do_not_disturb",
            "duties",
            "energy",
            "schema_version",
            "wallpaper"
        ]
    );
    assert_eq!(SCHEMA_VERSION, 1);
    assert_eq!(Settings::from_bytes(&bytes), Some(settings));

    // A file that still carries the halted shell's other choices is read for
    // the one it shares, and the next write drops the rest.
    let older = br#"{"schema_version":1,"do_not_disturb":true,"caffeine":true,"tray_items":{}}"#;
    let read = Settings::from_bytes(older).expect("a file with extra keys is ours");
    assert!(read.do_not_disturb);
    let rewritten = String::from_utf8(read.to_bytes().expect("serialize")).expect("UTF-8");
    let rewritten: serde_json::Value = serde_json::from_str(&rewritten).expect("JSON");
    assert!(rewritten.get("caffeine").is_none());
    assert!(rewritten.get("tray_items").is_none());

    // The durable-before-published rule survives the reduction.
    let mut store = Store::new(Settings::default());
    let pending = store.stage(|settings| settings.do_not_disturb = true);
    assert!(!store.apply(pending.clone(), WriteOutcome::Failed));
    assert!(!store.current().do_not_disturb);
    assert!(store.apply(pending, WriteOutcome::Durable));
    assert!(store.current().do_not_disturb);
}

#[test]
fn notifications_history_cap_is_fifty_and_the_page_shows_twenty() {
    assert_eq!(MAX_HISTORY, 50);
    assert_eq!(PAGE_HISTORY, 20);

    let mut server = Notifications::new();
    for index in 0..MAX_HISTORY + 7 {
        let id = server.post(
            &Incoming {
                summary: format!("notice {index}"),
                expire_timeout: -1,
                ..Incoming::default()
            },
            u64::try_from(index).expect("a small index"),
        );
        server.close(id, CloseReason::Dismissed);
    }

    assert_eq!(server.history().len(), MAX_HISTORY);
    let page = server.history_page();
    assert_eq!(page.len(), PAGE_HISTORY);
    // The page is the newest twenty, newest first.
    assert_eq!(page[0].summary, format!("notice {}", MAX_HISTORY + 6));
    assert_eq!(
        page[PAGE_HISTORY - 1].summary,
        server.history()[PAGE_HISTORY - 1].summary
    );

    server.clear_history();
    assert!(server.history_page().is_empty());
}
