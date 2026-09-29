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

/// One tar member spelled byte for byte, as a hostile writer spells it: the
/// `tar` crate's own builder refuses `..` and absolute names, which is exactly
/// what these archives need to carry.
enum Raw<'a> {
    Dir(&'a str),
    File(&'a str, &'a [u8]),
    Symlink(&'a str, &'a str),
    Hardlink(&'a str, &'a str),
}

/// Writes `members` into a plain `.tar` at `path`, in the order given.
fn hostile_tar(path: &Path, members: &[Raw<'_>]) {
    let file = fs::File::create(path).expect("create tar");
    let mut builder = tar::Builder::new(file);
    for member in members {
        let (name, kind, data, link): (&str, tar::EntryType, &[u8], &str) = match member {
            Raw::Dir(name) => (name, tar::EntryType::Directory, b"", ""),
            Raw::File(name, data) => (name, tar::EntryType::Regular, data, ""),
            Raw::Symlink(name, target) => (name, tar::EntryType::Symlink, b"", target),
            Raw::Hardlink(name, target) => (name, tar::EntryType::Link, b"", target),
        };
        let mut header = tar::Header::new_ustar();
        {
            let raw = header.as_old_mut();
            assert!(name.len() < raw.name.len() && link.len() < raw.linkname.len());
            raw.name[..name.len()].copy_from_slice(name.as_bytes());
            raw.linkname[..link.len()].copy_from_slice(link.as_bytes());
        }
        header.set_entry_type(kind);
        header.set_size(data.len() as u64);
        header.set_mode(if kind.is_dir() { 0o755 } else { 0o644 });
        header.set_mtime(1_600_000_000);
        header.set_cksum();
        builder.append(&header, data).expect("append member");
    }
    builder.into_inner().expect("finish tar");
}

/// Extracts `archive` into a fresh `destination` folder beside it and returns that
/// folder with the outcome.
fn extract_beside(
    dir: &Path,
    archive: &Path,
) -> (PathBuf, Result<siderita_archive::Extracted, ArchiveError>) {
    let into = dir.join("destination");
    fs::create_dir(&into).expect("mk destination");
    let outcome = extract(
        archive,
        &into,
        &ExtractOptions::new(&Utc, "extracted"),
        &live(),
        &mut ignore,
    );
    (into, outcome)
}

/// Asserts the refusal every hostile archive must earn: a typed
/// [`ArchiveError::UnsafeMember`] and a destination left exactly as it was.
fn assert_refused(into: &Path, outcome: Result<siderita_archive::Extracted, ArchiveError>) {
    match outcome {
        Err(ArchiveError::UnsafeMember { .. }) => {}
        other => panic!("expected an unsafe-member refusal, got {other:?}"),
    }
    assert_eq!(
        fs::read_dir(into).expect("read destination").count(),
        0,
        "a refused extraction must leave nothing in the destination"
    );
}

/// The auditor's proof of concept for SID-1, byte for byte: `d/up -> ..` looks
/// inside, `esc -> d/up/..` looks inside, and a file written through `esc`
/// landed beside the extraction folder while `extract` answered `Ok`.
#[test]
fn a_chain_of_links_that_each_look_inside_cannot_carry_a_write_out() {
    let dir = TestDir::new("chain-poc");
    let archive = dir.path().join("evil.tar");
    hostile_tar(
        &archive,
        &[
            Raw::Dir("d/"),
            Raw::Symlink("d/up", ".."),
            Raw::Symlink("esc", "d/up/.."),
            Raw::File("esc/pwned.txt", b"pwned"),
        ],
    );

    let (into, outcome) = extract_beside(dir.path(), &archive);

    assert!(
        !into.join("pwned.txt").exists(),
        "the write escaped the extraction root"
    );
    assert!(!dir.path().join("pwned.txt").exists());
    assert_refused(&into, outcome);
}

/// The same chain with no file behind it, in both orders: a link that resolves
/// out of the root is refused even when nothing is written through it, and even
/// when the link that makes it escape arrives after it.
#[test]
fn a_chain_of_links_that_resolves_outside_is_refused_in_either_order() {
    let forward: &[Raw<'_>] = &[
        Raw::Dir("d/"),
        Raw::Symlink("d/up", ".."),
        Raw::Symlink("esc", "d/up/.."),
    ];
    let backward: &[Raw<'_>] = &[
        Raw::Symlink("esc", "d/up/.."),
        Raw::Dir("d/"),
        Raw::Symlink("d/up", ".."),
    ];
    for (label, members) in [("forward", forward), ("backward", backward)] {
        let dir = TestDir::new(&format!("chain-{label}"));
        let archive = dir.path().join("evil.tar");
        hostile_tar(&archive, members);

        let (into, outcome) = extract_beside(dir.path(), &archive);

        assert_refused(&into, outcome);
    }
}

#[test]
fn an_absolute_member_is_refused() {
    let dir = TestDir::new("absolute");
    let outside = dir.path().join("outside-absolute.txt");
    let archive = dir.path().join("evil.tar");
    let name = outside.to_str().expect("utf-8 temp path");
    hostile_tar(&archive, &[Raw::File(name, b"escaped")]);

    let (into, outcome) = extract_beside(dir.path(), &archive);

    assert!(!outside.exists());
    assert_refused(&into, outcome);
}

#[test]
fn a_parent_member_in_a_tar_is_refused() {
    let dir = TestDir::new("dotdot");
    let archive = dir.path().join("evil.tar");
    hostile_tar(
        &archive,
        &[Raw::Dir("a/"), Raw::File("a/../../outside.txt", b"escaped")],
    );

    let (into, outcome) = extract_beside(dir.path(), &archive);

    assert!(!dir.path().join("outside.txt").exists());
    assert_refused(&into, outcome);
}

/// A hard link names its target from the archive's root. One that names a file
/// outside it — relatively or absolutely — would make the extracted entry *be*
/// that file, so a later write to it rewrites the original.
#[test]
fn a_hard_link_to_a_file_outside_is_refused() {
    let dir = TestDir::new("hardlink-out");
    let secret = dir.path().join("secret.txt");
    fs::write(&secret, b"do not touch").expect("write secret");
    let absolute = secret.to_str().expect("utf-8 temp path").to_owned();
    for (label, target) in [
        ("relative", "../secret.txt"),
        ("absolute", absolute.as_str()),
    ] {
        let archive = dir.path().join(format!("evil-{label}.tar"));
        hostile_tar(
            &archive,
            &[
                Raw::Hardlink("hard", target),
                Raw::File("hard", b"overwritten"),
            ],
        );

        let into = dir.path().join(format!("destination-{label}"));
        fs::create_dir(&into).expect("mk destination");
        let outcome = extract(
            &archive,
            &into,
            &ExtractOptions::new(&Utc, "extracted"),
            &live(),
            &mut ignore,
        );

        assert_refused(&into, outcome);
        assert_eq!(fs::read(&secret).expect("secret"), b"do not touch");
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(fs::metadata(&secret).expect("stat").nlink(), 1, "{label}");
        }
    }
}

/// A symlink stored before a file whose path runs through it: even when the
/// link points inside, the file must not be written through it.
#[test]
fn a_file_is_never_written_through_a_link_the_archive_made() {
    let dir = TestDir::new("through-link");
    let archive = dir.path().join("evil.tar");
    hostile_tar(
        &archive,
        &[
            Raw::Dir("sub/"),
            Raw::Symlink("link", "sub"),
            Raw::File("link/note.txt", b"through the link"),
        ],
    );

    let (into, outcome) = extract_beside(dir.path(), &archive);

    assert_refused(&into, outcome);
}

/// The honest counterpart: links that stay inside are still extracted, the
/// symlink as a symlink and the hard link as a second name for the same file.
#[test]
fn links_that_stay_inside_are_extracted() {
    let dir = TestDir::new("honest-links");
    let archive = dir.path().join("links.tar");
    hostile_tar(
        &archive,
        &[
            Raw::Dir("data/"),
            Raw::File("data/one.txt", b"one"),
            Raw::Symlink("data/soft", "one.txt"),
            Raw::Hardlink("data/hard", "data/one.txt"),
            Raw::Dir("data/inside/"),
            Raw::Symlink("data/inside/upward", "../one.txt"),
        ],
    );

    let (_into, outcome) = extract_beside(dir.path(), &archive);
    let extracted = outcome.expect("an honest archive extracts");

    assert!(extracted.skipped.is_empty(), "{:?}", extracted.skipped);
    let root = extracted.root;
    assert_eq!(
        fs::read_link(root.join("soft")).expect("symlink"),
        PathBuf::from("one.txt")
    );
    assert_eq!(
        fs::read(root.join("inside/upward")).expect("through link"),
        b"one"
    );
    assert_eq!(fs::read(root.join("hard")).expect("hard link"), b"one");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let original = fs::metadata(root.join("one.txt")).expect("stat");
        let second = fs::symlink_metadata(root.join("hard")).expect("stat");
        assert_eq!(original.ino(), second.ino());
        assert!(second.file_type().is_file());
    }
}

/// An archive with one top folder is extracted *as* that folder, which moves
/// the tree one level up. A link that climbed to the wrapper would then point
/// at the folder the person was looking at, so such a tree keeps its wrapper.
#[test]
fn lifting_the_archives_own_folder_never_lets_a_link_reach_past_it() {
    let dir = TestDir::new("lift-link");
    let archive = dir.path().join("wrapped.tar");
    hostile_tar(
        &archive,
        &[
            Raw::Dir("top/"),
            Raw::File("top/one.txt", b"one"),
            Raw::Symlink("top/upward", ".."),
        ],
    );

    let (into, outcome) = extract_beside(dir.path(), &archive);
    let extracted = outcome.expect("the link stays inside the extraction");

    let root = fs::canonicalize(&extracted.root).expect("root");
    // Wherever the tree landed — lifted or still wrapped — the link is in it.
    let link = [root.join("upward"), root.join("top/upward")]
        .into_iter()
        .find(|path| fs::symlink_metadata(path).is_ok())
        .expect("the link was extracted");
    let reached = fs::canonicalize(link).expect("resolve link");
    assert!(
        reached.starts_with(&root),
        "{} resolves outside {}",
        reached.display(),
        root.display()
    );
    assert_ne!(reached, fs::canonicalize(&into).expect("destination"));
}

/// A process running beside the extraction swaps a folder it is filling for a
/// link to somewhere else, over and over. Whatever the timing, no write may
/// follow the link: each member is created through the folder that was
/// checked, never through its name looked up again.
#[cfg(unix)]
#[test]
fn a_folder_swapped_for_a_link_during_extraction_never_carries_a_write_out() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    let dir = TestDir::new("race");
    let outside = dir.path().join("outside");
    fs::create_dir(&outside).expect("mk outside");
    let archive = dir.path().join("race.tar");
    let names: Vec<String> = (0..2000).map(|i| format!("sub/f{i:04}.txt")).collect();
    let mut members = vec![Raw::Dir("sub/")];
    members.extend(names.iter().map(|name| Raw::File(name, b"x")));
    hostile_tar(&archive, &members);

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut trials = 0;
    while trials < 500 && Instant::now() < deadline {
        trials += 1;
        let into = dir.path().join(format!("destination-{trials}"));
        fs::create_dir(&into).expect("mk destination");
        let stop = Arc::new(AtomicBool::new(false));
        let racer = {
            let stop = Arc::clone(&stop);
            let sub = into.join("race/sub");
            let aside = into.join("race/sub-aside");
            let outside = outside.clone();
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    if fs::rename(&sub, &aside).is_ok() {
                        let _ = std::os::unix::fs::symlink(&outside, &sub);
                        let _ = fs::remove_file(&sub);
                        let _ = fs::rename(&aside, &sub);
                    } else {
                        std::thread::yield_now();
                    }
                }
            })
        };
        let _ = extract(
            &archive,
            &into,
            &ExtractOptions::new(&Utc, "extracted"),
            &live(),
            &mut ignore,
        );
        stop.store(true, Ordering::Relaxed);
        racer.join().expect("racer thread");

        let landed = fs::read_dir(&outside).expect("read outside").count();
        assert_eq!(
            landed, 0,
            "trial {trials}: {landed} member(s) written outside"
        );
        let _ = fs::remove_dir_all(&into);
    }
}

/// A link with an absolute target, followed by a member under it: refused at
/// the link, before anything could be written through it.
#[test]
fn an_absolute_link_then_a_member_under_it_is_refused() {
    let dir = TestDir::new("absolute-link");
    let outside = dir.path().join("outside");
    fs::create_dir(&outside).expect("mk outside");
    let archive = dir.path().join("evil.tar");
    let target = outside.to_str().expect("utf-8 temp path");
    hostile_tar(
        &archive,
        &[Raw::Symlink("a", target), Raw::File("a/x", b"escaped")],
    );

    let (into, outcome) = extract_beside(dir.path(), &archive);

    assert!(!outside.join("x").exists());
    assert_refused(&into, outcome);
}

/// A folder the archive filled, then a link of the same name: placing the
/// link would make what was written under the folder reachable through it.
#[test]
fn a_folder_replaced_by_a_link_is_refused() {
    let dir = TestDir::new("folder-then-link");
    let archive = dir.path().join("evil.tar");
    hostile_tar(
        &archive,
        &[
            Raw::Dir("sub/"),
            Raw::Dir("d/"),
            Raw::File("d/f.txt", b"f"),
            Raw::Symlink("d", "sub"),
        ],
    );

    let (into, outcome) = extract_beside(dir.path(), &archive);

    assert_refused(&into, outcome);
}

/// A hard link to anything but a file this archive wrote — a symlink member, a
/// folder, a name that does not exist — is left out and reported, never made.
#[test]
fn a_hard_link_to_a_link_a_folder_or_nothing_is_skipped() {
    let dir = TestDir::new("hardlink-kinds");
    let archive = dir.path().join("links.tar");
    hostile_tar(
        &archive,
        &[
            Raw::File("one.txt", b"one"),
            Raw::Symlink("soft", "one.txt"),
            Raw::Dir("folder/"),
            Raw::Hardlink("to-link", "soft"),
            Raw::Hardlink("to-folder", "folder"),
            Raw::Hardlink("to-nothing", "nothing"),
        ],
    );

    let (_into, outcome) = extract_beside(dir.path(), &archive);
    let extracted = outcome.expect("the rest extracts");

    let mut skipped: Vec<_> = extracted
        .skipped
        .iter()
        .map(|skip| (skip.name.clone(), skip.reason))
        .collect();
    skipped.sort_by(|one, other| one.0.cmp(&other.0));
    assert_eq!(
        skipped,
        [
            (
                PathBuf::from("to-folder"),
                siderita_archive::SkipReason::UnsupportedKind
            ),
            (
                PathBuf::from("to-link"),
                siderita_archive::SkipReason::UnsupportedKind
            ),
            (
                PathBuf::from("to-nothing"),
                siderita_archive::SkipReason::UnsupportedKind
            ),
        ]
    );
    for name in ["to-link", "to-folder", "to-nothing"] {
        assert!(
            fs::symlink_metadata(extracted.root.join(name)).is_err(),
            "{name}"
        );
    }
    assert!(fs::symlink_metadata(extracted.root.join("soft"))
        .expect("symlink kept")
        .file_type()
        .is_symlink());
}

#[test]
fn a_loop_of_links_is_refused() {
    let dir = TestDir::new("loop");
    let archive = dir.path().join("evil.tar");
    hostile_tar(&archive, &[Raw::Symlink("a", "b"), Raw::Symlink("b", "a")]);

    let (into, outcome) = extract_beside(dir.path(), &archive);

    assert_refused(&into, outcome);
}

/// An archive whose only member is `top -> .` carries no folder of its own: a
/// link is never lifted out of the wrapper as if it were one.
#[test]
fn a_single_link_to_itself_is_not_lifted() {
    let dir = TestDir::new("lift-self-link");
    let archive = dir.path().join("wrapped.tar");
    hostile_tar(&archive, &[Raw::Symlink("top", ".")]);

    let (into, outcome) = extract_beside(dir.path(), &archive);
    let extracted = outcome.expect("a link to its own folder stays inside");

    assert_eq!(extracted.root, into.join("wrapped"));
    let link = extracted.root.join("top");
    assert!(fs::symlink_metadata(&link)
        .expect("link kept")
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::canonicalize(&link).expect("resolve"),
        fs::canonicalize(&extracted.root).expect("root")
    );
}
