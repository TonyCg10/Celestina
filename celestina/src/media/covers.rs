//! The [`CoverSource`] the media worker runs: players' `mpris:artUrl`
//! values in, [`MediaEvent::Cover`] events out (CEL-2-D plan, decision 4).
//!
//! It runs on the worker's thread and decodes at most the **shown** player's
//! cover: the page and the piece draw no other, so every other player's url
//! is answered "no cover" without being opened. A player that floods new
//! urls gets at most one decode per [`DECODE_INTERVAL`]: a url that arrives
//! sooner waits, and only the last one waiting is decoded once the interval
//! has passed (the worker's [`CoverSource::poll`] is what brings it due).
//! [`super::cover::load`] holds every bound of one decode. A [`CoverCache`]
//! makes a repeated url free (a cache hit is no decode and waits for
//! nothing), and a refused or broken file is logged once (the cache returns
//! its error only the first time) and then stays "no cover".

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::cover::{load, Cover, CoverCache, CoverError, CoverKey};
use super::{CoverSource, MediaEvent, PlayerView};

/// The shortest time between two decodes for one bus name.
pub const DECODE_INTERVAL: Duration = Duration::from_secs(2);

/// What the source last answered for one bus name.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Answer {
    art_url: String,
    /// Whether that url was looked at (decoded, refused or cached), rather
    /// than answered "no cover" unopened because the player was not shown
    /// or its decode had to wait.
    looked: bool,
}

type Clock = Box<dyn Fn() -> Instant + Send>;
type Loader = fn(&str) -> Result<Cover, CoverError>;

/// The media worker's cover source: decodes what the shown player names,
/// at a bounded rate, and forgets who left.
pub struct Covers {
    cache: CoverCache,
    /// The non-empty art url last answered for each bus name.
    answered: BTreeMap<String, Answer>,
    /// When each bus name's last decode started.
    decoded_at: BTreeMap<String, Instant>,
    /// The players and the shown one, as last handed over.
    players: Vec<PlayerView>,
    shown: Option<String>,
    clock: Clock,
    loader: Loader,
}

impl fmt::Debug for Covers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Covers")
            .field("cache", &self.cache)
            .field("answered", &self.answered)
            .field("decoded_at", &self.decoded_at)
            .field("shown", &self.shown)
            .finish_non_exhaustive()
    }
}

impl Default for Covers {
    fn default() -> Self {
        Self::new()
    }
}

impl Covers {
    /// The production source: the monotonic clock and [`load`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_parts(Box::new(Instant::now), load)
    }

    fn with_parts(clock: Clock, loader: Loader) -> Self {
        Self {
            cache: CoverCache::new(),
            answered: BTreeMap::new(),
            decoded_at: BTreeMap::new(),
            players: Vec::new(),
            shown: None,
            clock,
            loader,
        }
    }

    /// One event per player whose answer changed, as of now.
    fn step(&mut self) -> Vec<MediaEvent> {
        let now = (self.clock)();
        let mut events = Vec::new();
        let players = std::mem::take(&mut self.players);
        for player in players.iter().filter(|p| !p.art_url.is_empty()) {
            let shown = self.shown.as_deref() == Some(player.bus_name.as_str());
            if let Some(event) = self.answer(player, shown, now) {
                events.push(event);
            }
        }
        self.players = players;
        events
    }

    /// The event `player` needs, if any.
    fn answer(&mut self, player: &PlayerView, shown: bool, now: Instant) -> Option<MediaEvent> {
        let last = self.answered.get(&player.bus_name);
        let same_url = last.is_some_and(|a| a.art_url == player.art_url);
        if same_url && (!shown || last.is_some_and(|a| a.looked)) {
            return None;
        }
        let key = CoverKey::new(&player.bus_name, &player.art_url);
        let cover = if shown {
            if let Some(cover) = self.cache.cached(&key) {
                Some(cover)
            } else if self.decode_due(&player.bus_name, now) {
                self.decoded_at.insert(player.bus_name.clone(), now);
                Some(self.decode(&key))
            } else {
                None
            }
        } else {
            None
        };
        let looked = cover.is_some();
        let cover = cover.flatten();
        self.answered.insert(
            player.bus_name.clone(),
            Answer {
                art_url: player.art_url.clone(),
                looked,
            },
        );
        if same_url && cover.is_none() {
            // Already answered "no cover" for this url: still waiting for
            // its decode, or the decode found none.
            return None;
        }
        Some(MediaEvent::Cover {
            bus_name: player.bus_name.clone(),
            art_url: player.art_url.clone(),
            cover,
        })
    }

    fn decode_due(&self, bus_name: &str, now: Instant) -> bool {
        self.decoded_at
            .get(bus_name)
            .is_none_or(|at| now.saturating_duration_since(*at) >= DECODE_INTERVAL)
    }

    /// Loads `key` through the cache, logging a fresh failure once.
    fn decode(&mut self, key: &CoverKey) -> Option<Arc<Cover>> {
        match self.cache.get_or_load(key, self.loader) {
            Ok(cover) => {
                if cover.is_some() {
                    tracing::info!(bus_name = %key.bus_name, "media: cover decoded");
                }
                cover
            }
            Err(err) => {
                tracing::warn!(
                    bus_name = %key.bus_name,
                    error = %err.describe(),
                    "media: cover refused"
                );
                None
            }
        }
    }
}

impl CoverSource for Covers {
    /// One [`MediaEvent::Cover`] per player whose answer changed: the shown
    /// player's decoded cover (or `None` when it is refused, fails, or must
    /// wait for [`DECODE_INTERVAL`]), `None` for any other player's new url,
    /// and one with an empty `art_url` and no cover per player that left or
    /// whose url became empty. Unchanged players produce nothing.
    fn update(&mut self, players: &[PlayerView], shown: Option<&str>) -> Vec<MediaEvent> {
        let mut events = Vec::new();

        let gone: Vec<String> = self
            .answered
            .keys()
            .filter(|bus| {
                !players
                    .iter()
                    .any(|p| p.bus_name == **bus && !p.art_url.is_empty())
            })
            .cloned()
            .collect();
        for bus_name in gone {
            self.answered.remove(&bus_name);
            events.push(MediaEvent::Cover {
                bus_name,
                art_url: String::new(),
                cover: None,
            });
        }
        // Whoever left is forgotten whatever it was last answered: a player
        // that lost its url before leaving is no longer in `answered`. Every
        // cache entry was made by a decode, and every decode is recorded in
        // `decoded_at`, so its keys name every bus the cache may hold.
        let left: Vec<String> = self
            .decoded_at
            .keys()
            .filter(|bus| !players.iter().any(|p| p.bus_name == **bus))
            .cloned()
            .collect();
        for bus_name in left {
            self.cache.forget_bus(&bus_name);
            self.decoded_at.remove(&bus_name);
        }

        self.players = players.to_vec();
        self.shown = shown.map(str::to_owned);
        events.extend(self.step());
        events
    }

    /// The shown player's waiting decode, once its interval has passed.
    fn poll(&mut self) -> Vec<MediaEvent> {
        let waiting = self.shown.as_deref().is_some_and(|shown| {
            self.players.iter().any(|p| {
                p.bus_name == shown
                    && !p.art_url.is_empty()
                    && !self
                        .answered
                        .get(shown)
                        .is_some_and(|a| a.art_url == p.art_url && a.looked)
            })
        });
        if waiting {
            self.step()
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::cover::COVER_SIZE;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "celestina-covers-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("create scratch dir");
            Self(dir)
        }

        fn png(&self, name: &str, shade: u8) -> String {
            let path = self.0.join(name);
            RgbaImage::from_pixel(4, 4, Rgba([shade, 0, 0, 255]))
                .save_with_format(&path, ImageFormat::Png)
                .expect("write png");
            format!("file://{}", path.display())
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn player(bus: &str, art_url: &str) -> PlayerView {
        PlayerView {
            bus_name: bus.to_owned(),
            identity: bus.to_owned(),
            playing: true,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            art_url: art_url.to_owned(),
            length_ms: -1,
            position_ms: -1,
            pinned: false,
        }
    }

    fn cover_of(event: &MediaEvent) -> Option<(&str, &str, bool)> {
        match event {
            MediaEvent::Cover {
                bus_name,
                art_url,
                cover,
            } => Some((bus_name, art_url, cover.is_some())),
            _ => None,
        }
    }

    /// A clock the test moves by hand, in milliseconds from its start.
    #[derive(Clone)]
    struct Hand(Arc<AtomicU64>);

    impl Hand {
        fn advance(&self, by: Duration) {
            let ms = u64::try_from(by.as_millis()).unwrap_or(u64::MAX);
            self.0.fetch_add(ms, Ordering::Relaxed);
        }
    }

    static DECODES: AtomicUsize = AtomicUsize::new(0);

    fn counted(url: &str) -> Result<Cover, CoverError> {
        DECODES.fetch_add(1, Ordering::Relaxed);
        load(url)
    }

    /// A source on a hand clock, with the real loader.
    fn on_hand() -> (Covers, Hand) {
        let hand = Hand(Arc::new(AtomicU64::new(0)));
        let start = Instant::now();
        let ticks = hand.clone();
        let clock =
            Box::new(move || start + Duration::from_millis(ticks.0.load(Ordering::Relaxed)));
        (Covers::with_parts(clock, load), hand)
    }

    #[test]
    fn a_new_url_is_decoded_once_and_then_silent() {
        let dir = Scratch::new("new");
        let url = dir.png("a.png", 200);
        let mut covers = Covers::new();
        let players = [player("a", &url)];
        let events = covers.update(&players, Some("a"));
        assert_eq!(events.len(), 1);
        assert_eq!(cover_of(&events[0]), Some(("a", url.as_str(), true)));
        if let MediaEvent::Cover {
            cover: Some(cover), ..
        } = &events[0]
        {
            assert_eq!((cover.width, cover.height), (COVER_SIZE, COVER_SIZE));
        }
        assert!(covers.update(&players, Some("a")).is_empty());
        assert!(covers.poll().is_empty());
    }

    #[test]
    fn a_changed_url_emits_the_new_cover() {
        let dir = Scratch::new("change");
        let (one, two) = (dir.png("1.png", 10), dir.png("2.png", 90));
        let (mut covers, hand) = on_hand();
        let _ = covers.update(&[player("a", &one)], Some("a"));
        hand.advance(DECODE_INTERVAL);
        let events = covers.update(&[player("a", &two)], Some("a"));
        assert_eq!(events.len(), 1);
        assert_eq!(cover_of(&events[0]), Some(("a", two.as_str(), true)));
    }

    #[test]
    fn only_the_shown_player_is_decoded() {
        let dir = Scratch::new("shown");
        let (one, two) = (dir.png("1.png", 10), dir.png("2.png", 90));
        let (mut covers, _hand) = on_hand();
        let players = [player("a", &one), player("b", &two)];
        let events = covers.update(&players, Some("b"));
        let mut answers: Vec<_> = events.iter().filter_map(cover_of).collect();
        answers.sort_unstable();
        assert_eq!(
            answers,
            [("a", one.as_str(), false), ("b", two.as_str(), true)],
            "a is answered without being opened"
        );
        // Nothing shown: nothing new to say, nothing decoded.
        assert!(covers.update(&players, None).is_empty());
        // a becomes shown: now it is decoded (it never was), b keeps its own.
        let events = covers.update(&players, Some("a"));
        assert_eq!(events.len(), 1);
        assert_eq!(cover_of(&events[0]), Some(("a", one.as_str(), true)));
    }

    #[test]
    fn a_player_that_floods_urls_gets_one_decode_per_interval_and_the_last_url() {
        let dir = Scratch::new("flood");
        let urls: Vec<String> = (0..5).map(|i| dir.png(&format!("{i}.png"), i)).collect();
        let hand = Hand(Arc::new(AtomicU64::new(0)));
        let start = Instant::now();
        let ticks = hand.clone();
        let mut covers = Covers::with_parts(
            Box::new(move || start + Duration::from_millis(ticks.0.load(Ordering::Relaxed))),
            counted,
        );
        let before = DECODES.load(Ordering::Relaxed);
        let first = covers.update(&[player("a", &urls[0])], Some("a"));
        assert_eq!(cover_of(&first[0]), Some(("a", urls[0].as_str(), true)));
        // Four more urls inside the interval: each is "no cover" for now.
        for url in &urls[1..] {
            hand.advance(Duration::from_millis(100));
            let events = covers.update(&[player("a", url)], Some("a"));
            assert_eq!(cover_of(&events[0]), Some(("a", url.as_str(), false)));
            assert!(covers.poll().is_empty(), "not due yet");
        }
        assert_eq!(DECODES.load(Ordering::Relaxed) - before, 1);
        // Once due, the trailing decode is the last url only.
        hand.advance(DECODE_INTERVAL);
        let events = covers.poll();
        assert_eq!(events.len(), 1);
        assert_eq!(cover_of(&events[0]), Some(("a", urls[4].as_str(), true)));
        assert_eq!(DECODES.load(Ordering::Relaxed) - before, 2);
        assert!(covers.poll().is_empty());
    }

    #[test]
    fn a_cached_url_is_answered_without_waiting() {
        let dir = Scratch::new("cached");
        let (one, two) = (dir.png("1.png", 10), dir.png("2.png", 90));
        let (mut covers, hand) = on_hand();
        let _ = covers.update(&[player("a", &one)], Some("a"));
        hand.advance(DECODE_INTERVAL);
        let _ = covers.update(&[player("a", &two)], Some("a"));
        // Back to the first url at once: a cache hit, no decode to wait for.
        let events = covers.update(&[player("a", &one)], Some("a"));
        assert_eq!(cover_of(&events[0]), Some(("a", one.as_str(), true)));
    }

    #[test]
    fn a_player_leaving_or_losing_its_url_clears_its_cover() {
        let dir = Scratch::new("leave");
        let (one, two) = (dir.png("1.png", 10), dir.png("2.png", 90));
        let mut covers = Covers::new();
        let _ = covers.update(&[player("a", &one), player("b", &two)], Some("b"));

        let events = covers.update(&[player("a", ""), player("b", &two)], Some("b"));
        assert_eq!(events.len(), 1);
        assert_eq!(cover_of(&events[0]), Some(("a", "", false)));

        let events = covers.update(&[player("a", "")], Some("a"));
        assert_eq!(events.len(), 1);
        assert_eq!(cover_of(&events[0]), Some(("b", "", false)));
        assert!(covers.cache.is_empty(), "b's outcomes were forgotten");

        assert!(covers.update(&[player("a", "")], Some("a")).is_empty());
    }

    #[test]
    fn a_player_that_lost_its_url_and_then_left_is_forgotten() {
        let dir = Scratch::new("lost-then-left");
        let url = dir.png("1.png", 10);
        let mut covers = Covers::new();
        let _ = covers.update(&[player("a", &url)], Some("a"));
        assert!(!covers.cache.is_empty() && covers.decoded_at.contains_key("a"));

        // The url goes first: the player is still there, its cover cleared.
        let events = covers.update(&[player("a", "")], Some("a"));
        assert_eq!(events.len(), 1);
        assert_eq!(cover_of(&events[0]), Some(("a", "", false)));

        // Then the player leaves: nothing more to say, but nothing kept.
        assert!(covers.update(&[], None).is_empty());
        assert!(covers.decoded_at.is_empty(), "{:?}", covers.decoded_at);
        assert!(covers.cache.is_empty(), "a's outcomes were forgotten");
        assert!(covers.answered.is_empty());
    }

    #[test]
    fn hostile_urls_give_no_cover_and_are_not_retried() {
        let dir = Scratch::new("hostile");
        let fifo = dir.0.join("x.png");
        let status = Command::new("mkfifo").arg(&fifo).status().expect("mkfifo");
        assert!(status.success());
        let fifo_url = format!("file://{}", fifo.display());
        let text = dir.0.join("t.png");
        fs::write(&text, b"not an image at all").expect("write");
        let text_url = format!("file://{}", text.display());

        let (mut covers, _hand) = on_hand();
        let players = [
            player("a", "http://example.com/a.png"),
            player("b", &fifo_url),
            player("c", &text_url),
            player("d", "file:///nonexistent/cover.png"),
        ];
        let start = Instant::now();
        let mut events = covers.update(&players, None);
        for shown in ["a", "b", "c", "d"] {
            events.extend(covers.update(&players, Some(shown)));
        }
        assert!(start.elapsed() < Duration::from_secs(5));
        // Four unopened answers, then one looked-at answer each.
        assert_eq!(events.len(), 4);
        assert!(events
            .iter()
            .all(|e| matches!(cover_of(e), Some((_, _, false)))));
        for shown in ["a", "b", "c", "d"] {
            assert!(covers.update(&players, Some(shown)).is_empty());
        }
        assert!(covers.poll().is_empty());
    }
}
