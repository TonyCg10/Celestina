//! The appearance file through the public API, each test in its own
//! configuration home. The environment is process-wide, so the tests take one
//! lock while they point `XDG_CONFIG_HOME` somewhere.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use celestina_settings::{load, path, save, Appearance, TextScale};

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
        let root =
            std::env::temp_dir().join(format!("celestina-settings-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch home");
        std::env::set_var("XDG_CONFIG_HOME", &root);
        std::env::remove_var("CELESTINA_REDUCED_MOTION");
        Self { root, _lock: lock }
    }

    fn file(&self) -> PathBuf {
        self.root.join("celestina").join("appearance.toml")
    }

    fn write(&self, text: &str) {
        std::fs::create_dir_all(self.root.join("celestina")).expect("the directory");
        std::fs::write(self.file(), text).expect("the file");
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        std::env::remove_var("CELESTINA_REDUCED_MOTION");
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).expect("the file")
}

#[test]
fn the_path_is_under_the_config_home() {
    let home = Home::new("path");
    assert_eq!(path(), home.file());
}

#[test]
fn a_missing_file_answers_the_defaults() {
    let _home = Home::new("missing");
    assert_eq!(
        load(),
        Appearance {
            reduced_motion: false,
            text_scale: TextScale::Normal
        }
    );
}

#[test]
fn both_keys_round_trip() {
    let home = Home::new("round-trip");
    for value in [
        Appearance {
            reduced_motion: true,
            text_scale: TextScale::Larger,
        },
        Appearance {
            reduced_motion: false,
            text_scale: TextScale::Compact,
        },
        Appearance {
            reduced_motion: true,
            text_scale: TextScale::Large,
        },
    ] {
        save(&value).expect("saved");
        assert_eq!(load(), value);
    }
    assert_eq!(
        read(&home.file()),
        "reduced_motion = true\ntext_scale = \"large\"\n"
    );
}

#[test]
fn an_unknown_key_and_a_comment_survive_a_save() {
    let home = Home::new("unknown");
    home.write("# chosen in Cuprita\naccent = \"teal\"\ntext_scale = \"compact\"\n");
    save(&Appearance {
        reduced_motion: true,
        text_scale: TextScale::Large,
    })
    .expect("saved");
    assert_eq!(
        read(&home.file()),
        "# chosen in Cuprita\naccent = \"teal\"\ntext_scale = \"large\"\nreduced_motion = true\n"
    );
}

#[test]
fn a_malformed_file_answers_the_defaults_and_is_left_alone() {
    let home = Home::new("malformed");
    let text = "reduced_motion = maybe\ntext_scale = \"huge\"\n";
    home.write(text);
    assert_eq!(load(), Appearance::default());
    assert_eq!(read(&home.file()), text);
}

#[test]
fn the_environment_forces_reduced_motion() {
    let home = Home::new("environment");
    home.write("reduced_motion = false\ntext_scale = \"larger\"\n");
    std::env::set_var("CELESTINA_REDUCED_MOTION", "1");
    assert!(celestina_settings::env_forces_reduced_motion());
    assert_eq!(
        load(),
        Appearance {
            reduced_motion: true,
            text_scale: TextScale::Larger
        }
    );
}

#[test]
fn the_factor_table() {
    assert_eq!(TextScale::Compact.factor(), 0.9);
    assert_eq!(TextScale::Normal.factor(), 1.0);
    assert_eq!(TextScale::Large.factor(), 1.15);
    assert_eq!(TextScale::Larger.factor(), 1.3);
    assert_eq!(TextScale::default(), TextScale::Normal);
}
