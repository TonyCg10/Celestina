//! The use record: how often and how lately each application was launched,
//! `$XDG_CONFIG_HOME/celestina/usage.json`. Shape:
//! `{"version":1,"apps":{"<id>":{"count":N,"last":S}}}` with `last` in Unix
//! seconds. The score of an application is its count times a weight that
//! falls with the age of its last launch; the ranking uses it (see
//! `super::state`).
//!
//! The record lives in memory; every [`Usage::record`] updates it first and
//! then writes the whole file atomically
//! (`celestina_core::atomic_file::replace_private`), so two launches in the
//! same second both count and a reader never sees half a file. At most
//! [`MAX_APPS`] applications are kept, the lowest scores dropped first.
//!
//! The first time, when `usage.json` does not exist yet, the record is seeded
//! from the old `recent.json` (read only, never written again, left in
//! place): its i-th id (0 is the newest) gets one launch `60 * i` seconds
//! before now. The seeded record is written once.
//!
//! The file is bounded when read and anything in it that is not a valid
//! entry is dropped; an unreadable or malformed file is logged once and read
//! as empty.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use celestina_core::atomic_file;
use serde_json::{json, Value};

use super::recent::{acceptable, RecentLaunches};

/// The most applications the record keeps.
pub const MAX_APPS: usize = 200;
/// The largest file read: the record at its longest with room to spare.
const MAX_FILE_BYTES: u64 = 64 * 1024;
const DAY: u64 = 24 * 60 * 60;

/// One application's launches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Launch {
    /// How many times it was launched (saturating).
    pub count: u32,
    /// When it was launched last, in Unix seconds.
    pub last: u64,
}

/// The record and the file it lives in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    path: Option<PathBuf>,
    apps: BTreeMap<String, Launch>,
}

/// The clock in Unix seconds; 0 when it reads before the epoch.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The weight of a last launch `age` seconds ago.
fn weight(age: u64) -> u64 {
    match age {
        a if a < 4 * DAY => 100,
        a if a < 14 * DAY => 70,
        a if a < 31 * DAY => 50,
        a if a < 90 * DAY => 30,
        _ => 10,
    }
}

impl Usage {
    /// Reads `$XDG_CONFIG_HOME/celestina/usage.json`, seeded from `recent.json`
    /// beside it when `usage.json` does not exist.
    #[must_use]
    pub fn load() -> Self {
        let dir = celestina_core::xdg::config_home().map(|home| home.join("celestina"));
        Self::load_from(
            dir.as_ref().map(|d| d.join("usage.json")),
            dir.map(|d| d.join("recent.json")),
        )
    }

    /// Reads the record at `path` (`None` keeps it in memory only); when no
    /// file is there and `seed` reads, seeds from it and writes once.
    #[must_use]
    pub fn load_from(path: Option<PathBuf>, seed: Option<PathBuf>) -> Self {
        let now = unix_now();
        let mut usage = Self {
            path,
            apps: BTreeMap::new(),
        };
        match usage
            .path
            .as_deref()
            .map(|p| atomic_file::read_bounded(p, MAX_FILE_BYTES))
        {
            None => {}
            Some(Ok(Some(bytes))) => usage.apps = parse(&bytes),
            Some(Ok(None)) => {
                if seed.is_some() {
                    let recent = RecentLaunches::load_from(seed);
                    for (i, id) in (0u64..).zip(recent.ids()) {
                        usage.apps.insert(
                            id.clone(),
                            Launch {
                                count: 1,
                                last: now.saturating_sub(60 * i),
                            },
                        );
                    }
                    if !usage.apps.is_empty() {
                        usage.trim(now, None);
                        usage.write();
                    }
                }
            }
            Some(Err(err)) => {
                tracing::warn!("the launcher's use record: {err}; starting empty");
            }
        }
        usage.trim(now, None);
        usage
    }

    /// Counts a launch of `id` at `now` and writes the file. A bad id is
    /// ignored; a failed write is logged and the launch stands.
    pub fn record(&mut self, id: &str, now: u64) {
        if !acceptable(id) {
            return;
        }
        let launch = self.apps.entry(id.to_owned()).or_insert(Launch {
            count: 0,
            last: now,
        });
        launch.count = launch.count.saturating_add(1);
        launch.last = launch.last.max(now);
        self.trim(now, Some(id));
        self.write();
    }

    /// `count * weight(now - last)`; 0 for an unknown id.
    #[must_use]
    pub fn score(&self, id: &str, now: u64) -> u64 {
        self.apps
            .get(id)
            .map_or(0, |launch| launch_score(*launch, now))
    }

    /// Every known id's score.
    #[must_use]
    pub fn scores(&self, now: u64) -> HashMap<String, u64> {
        self.apps
            .iter()
            .map(|(id, launch)| (id.clone(), launch_score(*launch, now)))
            .collect()
    }

    /// Drops the lowest scores (then the oldest, then the last by id) past
    /// [`MAX_APPS`], never `keep`: a launch just made always stays.
    fn trim(&mut self, now: u64, keep: Option<&str>) {
        while self.apps.len() > MAX_APPS {
            let Some(worst) = self
                .apps
                .iter()
                .filter(|(id, _)| Some(id.as_str()) != keep)
                .min_by(|(ia, a), (ib, b)| {
                    launch_score(**a, now)
                        .cmp(&launch_score(**b, now))
                        .then(a.last.cmp(&b.last))
                        .then(ib.cmp(ia))
                })
                .map(|(id, _)| id.clone())
            else {
                break;
            };
            self.apps.remove(&worst);
        }
    }

    /// Writes the whole record atomically; a failure is logged.
    fn write(&self) {
        let Some(path) = self.path.as_deref() else {
            return;
        };
        let apps: serde_json::Map<String, Value> = self
            .apps
            .iter()
            .map(|(id, l)| (id.clone(), json!({"count": l.count, "last": l.last})))
            .collect();
        let bytes = match serde_json::to_vec(&json!({"version": 1, "apps": apps})) {
            Ok(bytes) => bytes,
            Err(err) => {
                tracing::warn!(%err, "the launcher's use record could not be encoded");
                return;
            }
        };
        if let Err(err) = atomic_file::replace_private(path, &bytes) {
            tracing::warn!(
                "{} not written: {}",
                path.display(),
                crate::error_chain(&err)
            );
        }
    }
}

fn launch_score(launch: Launch, now: u64) -> u64 {
    u64::from(launch.count) * weight(now.saturating_sub(launch.last))
}

/// The record of a file's bytes: version 1 with an `apps` object whose
/// entries have an acceptable id, a positive integer `count` (saturated to
/// `u32`) and an integer `last`. Anything else is dropped; a file that is
/// not that shape is an empty record, logged once.
fn parse(bytes: &[u8]) -> BTreeMap<String, Launch> {
    let mut apps = BTreeMap::new();
    let Ok(Value::Object(root)) = serde_json::from_slice::<Value>(bytes) else {
        tracing::warn!("the launcher's use record is not an object; starting empty");
        return apps;
    };
    let Some(Value::Object(entries)) = root
        .get("apps")
        .filter(|_| root.get("version") == Some(&json!(1)))
    else {
        tracing::warn!("the launcher's use record is not version 1; starting empty");
        return apps;
    };
    let mut dropped = 0usize;
    for (id, value) in entries {
        let count = value
            .get("count")
            .and_then(Value::as_u64)
            .filter(|c| *c > 0);
        let last = value.get("last").and_then(Value::as_u64);
        match (acceptable(id), count, last) {
            (true, Some(count), Some(last)) => {
                apps.insert(
                    id.clone(),
                    Launch {
                        count: u32::try_from(count).unwrap_or(u32::MAX),
                        last,
                    },
                );
            }
            _ => dropped += 1,
        }
    }
    if dropped > 0 {
        tracing::warn!(dropped, "the launcher's use record had invalid entries");
    }
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory for one test, removed when the guard drops.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let directory =
                std::env::temp_dir().join(format!("celestina-usage-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&directory);
            std::fs::create_dir_all(directory.join("celestina")).expect("a test directory");
            Self(directory)
        }

        fn usage(&self) -> PathBuf {
            self.0.join("celestina").join("usage.json")
        }

        fn recent(&self) -> PathBuf {
            self.0.join("celestina").join("recent.json")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const NOW: u64 = 1_800_000_000;

    fn load(scratch: &Scratch) -> Usage {
        Usage::load_from(Some(scratch.usage()), Some(scratch.recent()))
    }

    #[test]
    fn a_launch_counts_and_survives_a_restart() {
        let scratch = Scratch::new("round-trip");
        let mut usage = load(&scratch);
        assert!(usage.scores(NOW).is_empty());
        usage.record("a.desktop", NOW);
        usage.record("a.desktop", NOW + 5);
        usage.record("b.desktop", NOW + 6);
        let again = load(&scratch);
        assert_eq!(again, usage);
        assert_eq!(again.score("a.desktop", NOW + 6), 200);
        assert_eq!(again.score("b.desktop", NOW + 6), 100);
        assert_eq!(again.score("c.desktop", NOW + 6), 0);
    }

    #[test]
    fn two_launches_in_one_second_both_count() {
        let scratch = Scratch::new("same-second");
        let mut usage = load(&scratch);
        usage.record("a.desktop", NOW);
        usage.record("a.desktop", NOW);
        assert_eq!(usage.score("a.desktop", NOW), 200);
        assert_eq!(load(&scratch).score("a.desktop", NOW), 200);
    }

    #[test]
    fn the_weight_changes_at_each_boundary() {
        let day = 24 * 60 * 60;
        let cases = [
            (0, 100),
            (4 * day - 1, 100),
            (4 * day, 70),
            (14 * day - 1, 70),
            (14 * day, 50),
            (31 * day - 1, 50),
            (31 * day, 30),
            (90 * day - 1, 30),
            (90 * day, 10),
            (5000 * day, 10),
        ];
        for (age, expected) in cases {
            let mut usage = Usage::load_from(None, None);
            usage.record("a.desktop", NOW);
            assert_eq!(usage.score("a.desktop", NOW + age), expected, "age {age}");
        }
    }

    #[test]
    fn a_future_launch_counts_as_age_zero() {
        let mut usage = Usage::load_from(None, None);
        usage.record("a.desktop", NOW + 10_000_000);
        assert_eq!(usage.score("a.desktop", NOW), 100);
    }

    #[test]
    fn the_count_saturates() {
        let scratch = Scratch::new("saturate");
        std::fs::write(
            scratch.usage(),
            br#"{"version":1,"apps":{"a.desktop":{"count":4294967295,"last":1800000000}}}"#,
        )
        .expect("the file");
        let mut usage = load(&scratch);
        usage.record("a.desktop", NOW);
        assert_eq!(usage.score("a.desktop", NOW), u64::from(u32::MAX) * 100);
        let too_big = br#"{"version":1,"apps":{"a.desktop":{"count":99999999999,"last":1}}}"#;
        std::fs::write(scratch.usage(), too_big).expect("the file");
        assert_eq!(
            load(&scratch).score("a.desktop", 1),
            u64::from(u32::MAX) * 100
        );
    }

    #[test]
    fn the_record_keeps_the_best_two_hundred() {
        let mut usage = Usage::load_from(None, None);
        for i in 0..MAX_APPS {
            usage.record(&format!("{i}.desktop"), NOW);
            usage.record(&format!("{i}.desktop"), NOW);
        }
        assert_eq!(usage.scores(NOW).len(), MAX_APPS);
        usage.record("new.desktop", NOW);
        let scores = usage.scores(NOW);
        assert_eq!(scores.len(), MAX_APPS);
        assert!(
            scores.contains_key("new.desktop"),
            "a launch just made stays; another app goes"
        );
        assert!(!scores.contains_key("99.desktop"), "the lowest, last by id");
        usage.record("low.desktop", NOW);
        assert!(usage.score("low.desktop", NOW) > 0);
        assert_eq!(usage.scores(NOW).len(), MAX_APPS);
    }

    #[test]
    fn the_first_load_seeds_from_recent_once() {
        let scratch = Scratch::new("seed");
        std::fs::write(
            scratch.recent(),
            br#"["new.desktop","mid.desktop","old.desktop"]"#,
        )
        .expect("the file");
        let usage = load(&scratch);
        let now = unix_now();
        let scores = usage.scores(now);
        assert_eq!(scores.len(), 3);
        assert!(scratch.usage().exists(), "the seed is written once");
        assert!(usage.apps["new.desktop"].last > usage.apps["mid.desktop"].last);
        assert!(usage.apps["mid.desktop"].last > usage.apps["old.desktop"].last);
        assert_eq!(
            usage.apps["new.desktop"].last - usage.apps["mid.desktop"].last,
            60
        );
        assert!(usage.apps.values().all(|l| l.count == 1));
        // recent.json is never touched; a later change to it is ignored.
        std::fs::write(scratch.recent(), br#"["other.desktop"]"#).expect("the file");
        assert_eq!(load(&scratch).scores(now).len(), 3);
        assert_eq!(
            std::fs::read(scratch.recent()).expect("kept"),
            br#"["other.desktop"]"#
        );
    }

    #[test]
    fn no_seed_without_recent_and_none_when_usage_exists() {
        let scratch = Scratch::new("no-seed");
        assert!(load(&scratch).scores(NOW).is_empty());
        assert!(!scratch.usage().exists(), "nothing to write");
        std::fs::write(scratch.usage(), br#"{"version":1,"apps":{}}"#).expect("the file");
        std::fs::write(scratch.recent(), br#"["a.desktop"]"#).expect("the file");
        assert!(load(&scratch).scores(NOW).is_empty());
    }

    #[test]
    fn malformed_files_read_as_empty_or_trimmed() {
        let scratch = Scratch::new("malformed");
        for bad in [
            &b"not json"[..],
            b"[]",
            b"{\"version\":1}",
            b"{\"version\":1,\"apps\":[]}",
            b"{\"version\":2,\"apps\":{\"a.desktop\":{\"count\":1,\"last\":1}}}",
            b"{\"version\":1,\"apps\":{\"a.desktop\":{\"count\":-1,\"last\":1}}}",
            b"{\"version\":1,\"apps\":{\"a.desktop\":{\"count\":1.5,\"last\":1}}}",
            b"{\"version\":1,\"apps\":{\"a.desktop\":{\"count\":0,\"last\":1}}}",
            b"{\"version\":1,\"apps\":{\"a.desktop\":{\"count\":1,\"last\":-3}}}",
            b"{\"version\":1,\"apps\":{\"a.desktop\":7}}",
            b"{\"version\":1,\"apps\":{\"../x\":{\"count\":1,\"last\":1}}}",
        ] {
            std::fs::write(scratch.usage(), bad).expect("the file");
            let usage = load(&scratch);
            assert!(
                usage.scores(NOW).is_empty(),
                "{}",
                String::from_utf8_lossy(bad)
            );
        }
        // A 10 000-byte id is dropped, its neighbour kept.
        let long = "x".repeat(10_000);
        let text = format!(
            r#"{{"version":1,"apps":{{"{long}":{{"count":1,"last":1}},"ok.desktop":{{"count":2,"last":1}}}}}}"#
        );
        std::fs::write(scratch.usage(), text).expect("the file");
        let usage = load(&scratch);
        assert_eq!(usage.scores(1).len(), 1);
        assert_eq!(usage.score("ok.desktop", 1), 200);
        // 5 000 ids exceed the file bound: empty. 1 500 are trimmed to MAX_APPS.
        let mut apps = serde_json::Map::new();
        for i in 0..5000 {
            apps.insert(format!("{i}.d"), json!({"count": 1, "last": 1}));
        }
        let text = serde_json::to_vec(&json!({"version": 1, "apps": apps})).expect("json");
        assert!(text.len() > 64 * 1024);
        std::fs::write(scratch.usage(), text).expect("the file");
        assert!(load(&scratch).scores(1).is_empty());
        let mut apps = serde_json::Map::new();
        for i in 0..1500 {
            apps.insert(format!("{i}.d"), json!({"count": 1, "last": 1}));
        }
        let text = serde_json::to_vec(&json!({"version": 1, "apps": apps})).expect("json");
        assert!(text.len() < 64 * 1024);
        std::fs::write(scratch.usage(), text).expect("the file");
        assert_eq!(load(&scratch).scores(1).len(), MAX_APPS);
    }

    #[test]
    fn an_oversized_file_starts_empty() {
        let scratch = Scratch::new("large");
        std::fs::write(scratch.usage(), vec![b' '; 70 * 1024]).expect("the file");
        assert!(load(&scratch).scores(NOW).is_empty());
    }

    #[test]
    fn a_bad_id_is_not_recorded() {
        let mut usage = Usage::load_from(None, None);
        usage.record("", NOW);
        usage.record("../x", NOW);
        usage.record(&"x".repeat(300), NOW);
        assert!(usage.scores(NOW).is_empty());
    }
}
