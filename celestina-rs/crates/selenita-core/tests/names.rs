use chrono::NaiveDate;
use selenita_core::names::{file_name_at, numbered, user_dir_from};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

#[test]
fn the_name_carries_the_stem_and_the_local_stamp() {
    let at = NaiveDate::from_ymd_opt(2026, 10, 9)
        .and_then(|day| day.and_hms_opt(14, 32, 5))
        .expect("a fixed time");
    assert_eq!(
        file_name_at("Captura", at, "png"),
        "Captura 2026-10-09 14.32.05.png"
    );
}

#[test]
fn the_public_rule_formats_the_same_shape() {
    let name = selenita_core::capture_file_name("Captura", std::time::SystemTime::now(), "png");
    let stamp = name
        .strip_prefix("Captura ")
        .and_then(|rest| rest.strip_suffix(".png"))
        .expect("stem and extension");
    assert_eq!(stamp.len(), "2026-10-09 14.32.05".len());
}

#[test]
fn a_taken_name_gets_the_next_number_before_the_extension() {
    assert_eq!(numbered("Captura 1.png", 1), "Captura 1.png");
    assert_eq!(numbered("Captura 1.png", 2), "Captura 1 (2).png");
    assert_eq!(numbered("sin-extension", 3), "sin-extension (3)");
}

#[test]
fn the_pictures_folder_reads_from_user_dirs() {
    let home = Path::new("/home/someone");
    let text =
        "# comment\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\nXDG_PICTURES_DIR=\"$HOME/Pictures\"\n";
    assert_eq!(
        user_dir_from(text.as_bytes(), "XDG_PICTURES_DIR", home),
        Some(PathBuf::from("/home/someone/Pictures"))
    );
    assert_eq!(
        user_dir_from(
            b"XDG_PICTURES_DIR=\"/srv/fotos\"\n",
            "XDG_PICTURES_DIR",
            home
        ),
        Some(PathBuf::from("/srv/fotos"))
    );
    // The home itself means unset, as xdg-user-dirs writes it.
    assert_eq!(
        user_dir_from(b"XDG_PICTURES_DIR=\"$HOME/\"\n", "XDG_PICTURES_DIR", home)
            .filter(|p| p != Path::new("/home/someone/")),
        None
    );
    assert_eq!(
        user_dir_from(b"XDG_PICTURES_DIR=\"$HOME\"\n", "XDG_PICTURES_DIR", home),
        None
    );
    assert_eq!(user_dir_from(b"", "XDG_PICTURES_DIR", home), None);
    assert_eq!(
        user_dir_from(b"XDG_PICTURES_DIR=relative\n", "XDG_PICTURES_DIR", home),
        None
    );
}

#[test]
fn a_pictures_folder_that_is_not_utf8_comes_back_exact() {
    let home = Path::new("/home/someone");
    let mut bytes = b"XDG_PICTURES_DIR=\"$HOME/Im".to_vec();
    bytes.push(0xe1); // a Latin-1 accent, not UTF-8
    bytes.extend_from_slice(b"genes\"\n");
    let mut expected = b"/home/someone/Im".to_vec();
    expected.push(0xe1);
    expected.extend_from_slice(b"genes");
    assert_eq!(
        user_dir_from(&bytes, "XDG_PICTURES_DIR", home),
        Some(PathBuf::from(OsStr::from_bytes(&expected)))
    );
}
