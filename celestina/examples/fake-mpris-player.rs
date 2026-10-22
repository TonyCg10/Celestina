//! A fake MPRIS player for `scripts/smoke.sh`'s media phase.
//!
//! Usage: `fake-mpris-player ADDRESS ART_URL SECONDS`. It connects to the bus
//! at ADDRESS (the smoke's private bus, never the desktop's), owns
//! `org.mpris.MediaPlayer2.fake` and serves one paused track whose
//! `mpris:artUrl` is ART_URL, then exits after SECONDS. It claims no other
//! name and acts on nothing; the shell's media worker reads it like any other
//! player.

use std::collections::HashMap;
use std::time::Duration;

use zbus::blocking::connection::Builder;
use zbus::zvariant::{OwnedValue, Value};

struct Root;

#[zbus::interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    #[zbus(property)]
    fn identity(&self) -> String {
        "Fake player".to_owned()
    }

    fn raise(&self) {}

    fn quit(&self) {}
}

struct Player {
    art_url: String,
}

#[zbus::interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    #[zbus(property)]
    fn playback_status(&self) -> String {
        "Paused".to_owned()
    }

    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        let mut map = HashMap::new();
        let mut put = |key: &str, value: Value<'_>| {
            if let Ok(value) = OwnedValue::try_from(value) {
                map.insert(key.to_owned(), value);
            }
        };
        put("xesam:title", Value::from("Smoke track"));
        put("xesam:artist", Value::from(vec!["Smoke artist"]));
        put("mpris:length", Value::I64(180_000_000));
        put("mpris:artUrl", Value::from(self.art_url.as_str()));
        map
    }

    #[zbus(property)]
    fn position(&self) -> i64 {
        30_000_000
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }

    fn next(&self) {}

    fn previous(&self) {}

    fn play(&self) {}

    fn pause(&self) {}

    fn play_pause(&self) {}
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(address), Some(art_url), Some(seconds)) = (args.next(), args.next(), args.next())
    else {
        return Err("usage: fake-mpris-player ADDRESS ART_URL SECONDS".into());
    };
    let seconds: u64 = seconds.parse()?;
    let _connection = Builder::address(address.as_str())?
        .name("org.mpris.MediaPlayer2.fake")?
        .serve_at("/org/mpris/MediaPlayer2", Root)?
        .serve_at("/org/mpris/MediaPlayer2", Player { art_url })?
        .build()?;
    println!("fake-mpris-player: serving");
    std::thread::sleep(Duration::from_secs(seconds));
    Ok(())
}
