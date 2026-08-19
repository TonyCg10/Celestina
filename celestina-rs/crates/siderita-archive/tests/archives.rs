//! What the archive domain promises, exercised on real files: a round trip that
//! preserves the tree, the refusal to write outside the destination, the refusal
//! to overwrite, and cancellation that leaves nothing behind.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use celestina_core::CancellationToken;
use siderita_archive::{
    can_read, create, extract, list, sniff, ArchiveError, ExtractOptions, Format, Utc, Zone,
};
use siderita_ops::{OpError, Progress};

/// A throwaway directory in the system temp dir, removed on drop.
struct TestDir(PathBuf);

impl TestDir {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "siderita-archive-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn live() -> CancellationToken {
    CancellationToken::new()
}

fn ignore(_: Progress) {}

/// `notas/` with a nested file, an empty subfolder and an executable.
fn seed_tree(root: &Path) -> PathBuf {
    let tree = root.join("notas");
    fs::create_dir(&tree).expect("mk tree");
    fs::write(tree.join("uno.txt"), b"uno").expect("write uno");
    fs::create_dir(tree.join("dentro")).expect("mk nested");
    fs::write(tree.join("dentro/dos.txt"), b"dos dos").expect("write dos");
    fs::create_dir(tree.join("vacia")).expect("mk empty");
    tree
}

/// An encrypted zip asks for a password, refuses a wrong one, and opens with the
/// right one — the three answers the host's dialog is built on.
///
/// AES is the cipher a modern writer uses (7-Zip, WinZip); the archive here is
/// written with it rather than with the legacy one, so the test proves the
/// member actually decrypts and not merely that a header was read.
#[test]
fn an_encrypted_zip_answers_for_its_password() {
    let dir = TestDir::new("password");
    let archive = dir.path().join("secreto.zip");
    let file = fs::File::create(&archive).expect("create zip");
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .with_aes_encryption(zip::AesMode::Aes256, "clave 1");
    writer.start_file("datos/uno.txt", options).expect("start");
    writer.write_all(b"secreto").expect("write");
    writer.finish().expect("finish");

    let into = dir.path().join("destino");
    fs::create_dir(&into).expect("mk destino");

    let error = extract(
        &archive,
        &into,
        &ExtractOptions::new(&Utc, "extracted"),
        &live(),
        &mut ignore,
    )
    .expect_err("must ask");
    assert!(
        matches!(error, ArchiveError::PasswordRequired { .. }),
        "{error:?}"
    );
    assert!(error.needs_password());

    let error = extract(
        &archive,
        &into,
        &ExtractOptions::new(&Utc, "extracted").with_password("otra"),
        &live(),
        &mut ignore,
    )
    .expect_err("must refuse");
    assert!(
        matches!(error, ArchiveError::WrongPassword { .. }),
        "{error:?}"
    );
    assert!(error.needs_password());

    let extracted = extract(
        &archive,
        &into,
        &ExtractOptions::new(&Utc, "extracted").with_password("clave 1"),
        &live(),
        &mut ignore,
    )
    .expect("must open");
    let written = fs::read(extracted.root.join("uno.txt")).expect("read member");
    assert_eq!(written, b"secreto");
    // Nothing of the two refused attempts survived next to the real result.
    let left: Vec<_> = fs::read_dir(&into)
        .expect("read destino")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(left.len(), 1, "{left:?}");
}

/// A delegated container is recognised by its bytes whether or not a tool for it
/// is installed — that separation is what lets a host say "install unrar"
/// instead of "unknown file".
#[test]
fn a_delegated_container_is_recognised_by_its_signature() {
    let dir = TestDir::new("signatures");
    for (name, signature, expected) in [
        ("cuatro.rar", b"Rar!\x1a\x07\x00".as_slice(), Format::Rar),
        ("cinco.rar", b"Rar!\x1a\x07\x01\x00".as_slice(), Format::Rar),
        (
            "paquete.7z",
            b"7z\xbc\xaf\x27\x1c".as_slice(),
            Format::SevenZip,
        ),
    ] {
        let path = dir.path().join(name);
        fs::write(&path, signature).expect("write signature");
        assert_eq!(sniff(&path), Some(expected), "{name}");
        assert!(!expected.is_native());
        // Its index is not this domain's to describe: the tool extracts, it does
        // not report.
        assert!(matches!(
            list(&path),
            Err(ArchiveError::UnsupportedFormat { .. })
        ));

        // The stub is a signature and nothing else, so an installed tool must
        // call it damaged, and a machine without one must say what is missing.
        let into = dir.path().join(format!("destino-{name}"));
        fs::create_dir(&into).expect("mk destino");
        let error = extract(
            &path,
            &into,
            &ExtractOptions::new(&Utc, "extracted"),
            &live(),
            &mut ignore,
        )
        .expect_err("a signature alone is not an archive");
        if can_read(expected) {
            assert!(matches!(error, ArchiveError::Malformed { .. }), "{error:?}");
        } else {
            assert!(
                matches!(error, ArchiveError::ToolMissing { .. }),
                "{error:?}"
            );
        }
        // Either way the destination is left exactly as it was found.
        assert_eq!(fs::read_dir(&into).expect("read").count(), 0);
    }
}

#[test]
fn a_zip_round_trip_keeps_the_tree_and_the_bytes() {
    let dir = TestDir::new("zip-round-trip");
    let tree = seed_tree(dir.path());
    let archive = dir.path().join("notas.zip");

    create(
        std::slice::from_ref(&tree),
        &archive,
        Format::Zip,
        &Utc,
        &live(),
        &mut ignore,
    )
    .expect("create zip");
    assert_eq!(sniff(&archive), Some(Format::Zip));

    // The archive carries its own folder, so extracting into a fresh directory
    // reproduces `notas/` rather than spilling its content.
    let into = dir.path().join("destino");
    fs::create_dir(&into).expect("mk destino");
    let extracted = extract(
        &archive,
        &into,
        &ExtractOptions::new(&Utc, "extracted"),
        &live(),
        &mut ignore,
    )
    .expect("extract zip");

    assert_eq!(extracted.root, into.join("notas"));
    assert!(extracted.skipped.is_empty());
    assert_eq!(
        fs::read(into.join("notas/dentro/dos.txt")).expect("read dos"),
        b"dos dos"
    );
    assert!(into.join("notas/vacia").is_dir());
}

#[test]
fn a_tar_gz_round_trip_keeps_the_tree_and_the_bytes() {
    let dir = TestDir::new("targz-round-trip");
    let tree = seed_tree(dir.path());
    let archive = dir.path().join("notas.tar.gz");

    create(
        std::slice::from_ref(&tree),
        &archive,
        Format::TarGz,
        &Utc,
        &live(),
        &mut ignore,
    )
    .expect("create tar.gz");
    assert_eq!(sniff(&archive), Some(Format::TarGz));

    let members = list(&archive).expect("list tar.gz");
    assert!(members
        .iter()
        .any(|member| member.name == Path::new("notas/dentro/dos.txt")));

    let into = dir.path().join("destino");
    fs::create_dir(&into).expect("mk destino");
    let extracted = extract(
        &archive,
        &into,
        &ExtractOptions::new(&Utc, "extracted"),
        &live(),
        &mut ignore,
    )
    .expect("extract tar.gz");
    assert_eq!(extracted.root, into.join("notas"));
    assert_eq!(
        fs::read(into.join("notas/uno.txt")).expect("read uno"),
        b"uno"
    );
}

#[test]
fn several_loose_entries_get_a_folder_named_after_the_archive() {
    let dir = TestDir::new("loose");
    fs::write(dir.path().join("a.txt"), b"a").expect("write a");
    fs::write(dir.path().join("b.txt"), b"b").expect("write b");
    let archive = dir.path().join("dos-cosas.zip");

    create(
        &[dir.path().join("a.txt"), dir.path().join("b.txt")],
        &archive,
        Format::Zip,
        &Utc,
        &live(),
        &mut ignore,
    )
    .expect("create zip");

    let into = dir.path().join("destino");
    fs::create_dir(&into).expect("mk destino");
    let extracted = extract(
        &archive,
        &into,
        &ExtractOptions::new(&Utc, "extracted"),
        &live(),
        &mut ignore,
    )
    .expect("extract");

    // Never scattered over the folder the person was looking at.
    assert_eq!(extracted.root, into.join("dos-cosas"));
    assert!(into.join("dos-cosas/a.txt").is_file());
}

#[test]
fn extracting_twice_never_overwrites_the_first_result() {
    let dir = TestDir::new("twice");
    let tree = seed_tree(dir.path());
    let archive = dir.path().join("notas.zip");
    create(&[tree], &archive, Format::Zip, &Utc, &live(), &mut ignore).expect("create");

    let into = dir.path().join("destino");
    fs::create_dir(&into).expect("mk destino");
    let first = extract(
        &archive,
        &into,
        &ExtractOptions::new(&Utc, "extracted"),
        &live(),
        &mut ignore,
    )
    .expect("first");
    let second = extract(
        &archive,
        &into,
        &ExtractOptions::new(&Utc, "extracted"),
        &live(),
        &mut ignore,
    )
    .expect("second");

    assert_eq!(first.root, into.join("notas"));
    assert_ne!(second.root, first.root);
    assert!(second.root.is_dir());
    // The freed name is the domain's own "keep both" policy, not a new recipe.
    assert!(second
        .root
        .file_name()
        .expect("name")
        .to_string_lossy()
        .starts_with("notas ("));
}

#[test]
fn compressing_onto_an_existing_name_is_refused() {
    let dir = TestDir::new("exists");
    let tree = seed_tree(dir.path());
    let archive = dir.path().join("ocupado.zip");
    fs::write(&archive, b"no me pises").expect("seed archive");

    let error = create(&[tree], &archive, Format::Zip, &Utc, &live(), &mut ignore)
        .expect_err("must refuse an existing destination");

    assert!(matches!(
        error,
        ArchiveError::Op(OpError::AlreadyExists { .. })
    ));
    assert_eq!(fs::read(&archive).expect("still there"), b"no me pises");
}

#[test]
fn a_member_that_would_escape_the_destination_fails_the_extraction() {
    let dir = TestDir::new("zip-slip");
    let archive = dir.path().join("malicioso.zip");

    // Hand-built: no honest writer produces this, which is the point.
    let file = fs::File::create(&archive).expect("create archive");
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
    zip.start_file("../fuera.txt", zip::write::SimpleFileOptions::default())
        .expect("start member");
    zip.write_all(b"escapado").expect("write member");
    zip.finish().expect("finish").flush().expect("flush");

    let into = dir.path().join("destino");
    fs::create_dir(&into).expect("mk destino");
    let error = extract(
        &archive,
        &into,
        &ExtractOptions::new(&Utc, "extracted"),
        &live(),
        &mut ignore,
    )
    .expect_err("must refuse");

    assert!(matches!(error, ArchiveError::UnsafeMember { .. }));
    assert!(!dir.path().join("fuera.txt").exists());
    // Nothing half-extracted is left behind either.
    assert_eq!(
        fs::read_dir(&into).expect("read destino").count(),
        0,
        "the staging folder must be gone"
    );
}

#[test]
fn a_cancelled_extraction_leaves_the_destination_untouched() {
    let dir = TestDir::new("cancel");
    let tree = seed_tree(dir.path());
    let archive = dir.path().join("notas.tar.gz");
    create(&[tree], &archive, Format::TarGz, &Utc, &live(), &mut ignore).expect("create");

    let into = dir.path().join("destino");
    fs::create_dir(&into).expect("mk destino");
    let token = CancellationToken::new();
    token.cancel();

    let error = extract(
        &archive,
        &into,
        &ExtractOptions::new(&Utc, "extracted"),
        &token,
        &mut ignore,
    )
    .expect_err("must stop");
    assert!(error.is_cancelled());
    assert_eq!(fs::read_dir(&into).expect("read destino").count(), 0);
}

#[test]
fn something_that_is_not_an_archive_is_not_claimed() {
    let dir = TestDir::new("sniff");
    let text = dir.path().join("nota.zip");
    fs::write(&text, b"esto es texto, no un zip").expect("write");

    assert_eq!(sniff(&text), None);
    let into = dir.path().join("destino");
    fs::create_dir(&into).expect("mk destino");
    assert!(matches!(
        extract(
            &text,
            &into,
            &ExtractOptions::new(&Utc, "extracted"),
            &live(),
            &mut ignore
        )
        .expect_err("must refuse"),
        ArchiveError::UnsupportedFormat { .. }
    ));
}

/// A zip's date must survive being written in one zone and read in another.
///
/// The MS-DOS field alone cannot do that — it has no zone, so a reader in a
/// different one shifts it by the difference. The exact Unix instant written
/// alongside it can, and this is the test that says so: written as if the
/// machine were five hours east of UTC, read back as UTC, same instant.
#[test]
fn a_zip_date_does_not_move_when_the_reader_is_in_another_zone() {
    struct Eastern;

    impl Zone for Eastern {
        fn offset_at(&self, _time: SystemTime) -> i32 {
            5 * 3600
        }
    }

    let dir = TestDir::new("zones");
    let tree = seed_tree(dir.path());
    let stamp = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
    let file = fs::File::options()
        .write(true)
        .open(tree.join("uno.txt"))
        .expect("open uno");
    file.set_times(fs::FileTimes::new().set_modified(stamp))
        .expect("stamp uno");
    drop(file);

    let archive = dir.path().join("notas.zip");
    create(
        std::slice::from_ref(&tree),
        &archive,
        Format::Zip,
        &Eastern,
        &live(),
        &mut ignore,
    )
    .expect("create");
    let into = dir.path().join("destino");
    fs::create_dir(&into).expect("mk destino");
    let extracted = extract(
        &archive,
        &into,
        &ExtractOptions::new(&Utc, "extracted"),
        &live(),
        &mut ignore,
    )
    .expect("extract");

    let written = fs::metadata(extracted.root.join("uno.txt"))
        .expect("stat")
        .modified()
        .expect("modified");
    assert_eq!(written, stamp);
}

#[test]
fn a_round_trip_gives_back_the_modification_date() {
    let dir = TestDir::new("dates");
    let tree = seed_tree(dir.path());
    // A date the test controls, well in the past, so "today" cannot pass by luck.
    let stamp = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
    let file = fs::File::options()
        .write(true)
        .open(tree.join("uno.txt"))
        .expect("open uno");
    file.set_times(fs::FileTimes::new().set_modified(stamp))
        .expect("stamp uno");
    drop(file);

    for (format, name) in [(Format::Zip, "notas.zip"), (Format::TarGz, "notas.tar.gz")] {
        let archive = dir.path().join(name);
        create(
            std::slice::from_ref(&tree),
            &archive,
            format,
            &Utc,
            &live(),
            &mut ignore,
        )
        .expect("create");
        let into = dir.path().join(format!("destino-{name}"));
        fs::create_dir(&into).expect("mk destino");
        let extracted = extract(
            &archive,
            &into,
            &ExtractOptions::new(&Utc, "extracted"),
            &live(),
            &mut ignore,
        )
        .expect("extract");

        let written = fs::metadata(extracted.root.join("uno.txt"))
            .expect("stat")
            .modified()
            .expect("modified");
        // A zip records whole two-second steps, so the comparison allows that
        // much and not a second more.
        let drift = written
            .duration_since(stamp)
            .or_else(|_| stamp.duration_since(written))
            .expect("comparable");
        assert!(
            drift.as_secs() <= 2,
            "{name}: the date was lost ({drift:?})"
        );
    }
}

#[test]
fn progress_counts_every_member_and_every_byte() {
    let dir = TestDir::new("progress");
    let tree = seed_tree(dir.path());
    let archive = dir.path().join("notas.zip");

    let mut last = Progress::default();
    create(
        &[tree],
        &archive,
        Format::Zip,
        &Utc,
        &live(),
        &mut |progress| {
            assert!(progress.bytes >= last.bytes && progress.items >= last.items);
            last = progress;
        },
    )
    .expect("create");

    // Two files (3 + 7 bytes) and three directories.
    assert_eq!(last.bytes, 10);
    assert_eq!(last.items, 5);
}
