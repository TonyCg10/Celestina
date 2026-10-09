//! The watcher and the follower on a temporary configuration home. The
//! environment is process-wide, so the tests take one lock while they point
//! `XDG_CONFIG_HOME` at their own directory; the author's real configuration
//! is never touched.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use celestina_settings::{follow, load, save, watch, Appearance, TextScale};

static ENVIRONMENT: Mutex<()> = Mutex::new(());

struct Home {
    root: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl Home {
    fn new(tag: &str) -> Self {
        let lock = ENVIRONMENT
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let root = std::env::temp_dir().join(format!(
            "celestina-settings-watch-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch home");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::remove_var("CELESTINA_REDUCED_MOTION");
        Self { root, _lock: lock }
    }

    fn file(&self) -> PathBuf {
        self.root.join("celestina").join("appearance.toml")
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn large() -> Appearance {
    Appearance {
        reduced_motion: true,
        text_scale: TextScale::Large,
    }
}

fn channel() -> (
    impl Fn(Appearance) + Clone + Send + 'static,
    Receiver<Appearance>,
) {
    let (sender, received) = mpsc::channel();
    let deliver = move |value| {
        let _ = sender.send(value);
    };
    (deliver, received)
}

#[test]
fn a_change_calls_back_once_and_a_same_value_rewrite_does_not() {
    let home = Home::new("change");
    let (deliver, received) = channel();
    let handle = watch(deliver).expect("the watch starts");

    save(&large()).expect("saved");
    assert_eq!(received.recv_timeout(Duration::from_secs(2)), Ok(large()));
    assert!(
        received.recv_timeout(Duration::from_millis(800)).is_err(),
        "one change, one callback"
    );

    // The same value again, through an editor-style write in place.
    std::fs::write(
        home.file(),
        "reduced_motion = true\ntext_scale = \"large\"\n",
    )
    .expect("rewritten");
    assert!(
        received.recv_timeout(Duration::from_millis(1200)).is_err(),
        "an unchanged value is silent"
    );
    drop(handle);
}

#[test]
fn deleting_the_file_calls_back_with_the_defaults() {
    let home = Home::new("delete");
    save(&large()).expect("saved");
    let (deliver, received) = channel();
    let _handle = watch(deliver).expect("the watch starts");

    std::fs::remove_file(home.file()).expect("deleted");
    assert_eq!(
        received.recv_timeout(Duration::from_secs(2)),
        Ok(Appearance::default())
    );
}

#[test]
fn two_writes_inside_the_debounce_call_back_once_with_the_last() {
    let _home = Home::new("burst");
    let (deliver, received) = channel();
    let _handle = watch(deliver).expect("the watch starts");

    save(&large()).expect("first");
    let last = Appearance {
        reduced_motion: false,
        text_scale: TextScale::Larger,
    };
    save(&last).expect("second");
    assert_eq!(received.recv_timeout(Duration::from_secs(2)), Ok(last));
    assert!(
        received.recv_timeout(Duration::from_millis(800)).is_err(),
        "one burst, one callback"
    );
}

#[test]
fn a_missing_directory_is_created_by_the_watch_and_followed() {
    let home = Home::new("absent");
    assert!(!home.root.join("celestina").exists());
    let (deliver, received) = channel();
    let _handle = watch(deliver).expect("the watch starts");
    assert!(home.root.join("celestina").is_dir());

    save(&large()).expect("saved");
    assert_eq!(received.recv_timeout(Duration::from_secs(2)), Ok(large()));
}

#[test]
fn follow_delivers_the_loaded_value_first_and_then_a_change() {
    let _home = Home::new("follow");
    save(&Appearance {
        reduced_motion: false,
        text_scale: TextScale::Compact,
    })
    .expect("saved");
    let (deliver, received) = channel();
    let follower = follow(deliver);

    assert_eq!(received.recv_timeout(Duration::from_secs(2)), Ok(load()));
    save(&large()).expect("changed");
    assert_eq!(received.recv_timeout(Duration::from_secs(2)), Ok(large()));
    drop(follower);
}
