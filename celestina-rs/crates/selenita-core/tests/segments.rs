use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use selenita_core::segments::{
    concat_list, join_argv, join_deadline, list_file, part_name, segment_file, SegmentError,
    JOINER, JOIN_BASE, JOIN_MAX, JOIN_PER_GIB,
};

#[test]
fn a_segment_sits_beside_the_recordings_hidden_file() {
    let base = Path::new("/videos/Grabaciones/.selenita-12-1.mp4");
    assert_eq!(
        segment_file(base, 1),
        PathBuf::from("/videos/Grabaciones/.selenita-12-1-part1.mp4")
    );
    assert_eq!(
        segment_file(base, 3),
        PathBuf::from("/videos/Grabaciones/.selenita-12-1-part3.mp4")
    );
    assert_ne!(segment_file(base, 1), base);
    assert_eq!(
        segment_file(Path::new("/tmp/clip"), 2),
        PathBuf::from("/tmp/clip-part2")
    );
}

#[test]
fn a_part_is_numbered_from_one() {
    let name = "Recording 2026-10-10 14.32.05.mp4";
    assert_eq!(part_name(name, 1), "Recording 2026-10-10 14.32.05 (1).mp4");
    assert_eq!(part_name(name, 2), "Recording 2026-10-10 14.32.05 (2).mp4");
    assert_eq!(part_name("clip", 3), "clip (3)");
    assert_eq!(part_name(".mp4", 1), ".mp4 (1)");
}

#[test]
fn the_list_names_every_segment_in_order_quoted_as_ffmpeg_reads_it() {
    let segments = [
        PathBuf::from("/videos/Grabaciones/.selenita-1-1-part1.mp4"),
        PathBuf::from("/videos/My clips/it's here/.selenita-1-1-part2.mp4"),
    ];
    let list = concat_list(&segments).expect("list");
    assert_eq!(
        String::from_utf8(list).expect("text"),
        "file '/videos/Grabaciones/.selenita-1-1-part1.mp4'\n\
         file '/videos/My clips/it'\\''s here/.selenita-1-1-part2.mp4'\n"
    );
    // A name that is not UTF-8 keeps its bytes.
    let odd = PathBuf::from(OsString::from_vec(b"/videos/\xffclip.mp4".to_vec()));
    let list = concat_list(&[odd]).expect("list");
    assert_eq!(list, b"file '/videos/\xffclip.mp4'\n".to_vec());
}

#[test]
fn a_list_it_cannot_write_is_refused() {
    assert_eq!(concat_list(&[]), Err(SegmentError::Empty));
    let relative = PathBuf::from("Grabaciones/a.mp4");
    assert_eq!(
        concat_list(std::slice::from_ref(&relative)),
        Err(SegmentError::NotAbsolute(relative))
    );
    for broken in ["/videos/a\nb.mp4", "/videos/a\rb.mp4"] {
        let path = PathBuf::from(broken);
        assert_eq!(
            concat_list(&[PathBuf::from("/videos/ok.mp4"), path.clone()]),
            Err(SegmentError::LineBreak(path))
        );
    }
}

#[test]
fn the_join_copies_the_streams() {
    let argv: Vec<String> = join_argv(
        Path::new("/run/user/1000/selenita/join-7.txt"),
        Path::new("/videos/Grabaciones/.selenita-1-1.mp4"),
    )
    .iter()
    .map(|word| word.to_string_lossy().into_owned())
    .collect();
    assert_eq!(
        argv,
        [
            JOINER,
            "-hide_banner",
            "-nostdin",
            "-y",
            "-f",
            "concat",
            "-safe",
            "0",
            "-i",
            "/run/user/1000/selenita/join-7.txt",
            "-c",
            "copy",
            "/videos/Grabaciones/.selenita-1-1.mp4",
        ]
    );
    assert_eq!(JOINER, "ffmpeg");
}

#[test]
fn the_joins_deadline_grows_with_the_segments_and_is_bounded() {
    const GIB: u64 = 1 << 30;
    assert_eq!(join_deadline(0), JOIN_BASE);
    assert_eq!(join_deadline(1), JOIN_BASE + JOIN_PER_GIB);
    assert_eq!(join_deadline(GIB), JOIN_BASE + JOIN_PER_GIB);
    assert_eq!(join_deadline(GIB + 1), JOIN_BASE + JOIN_PER_GIB * 2);
    assert_eq!(join_deadline(10 * GIB), std::time::Duration::from_secs(160));
    assert_eq!(join_deadline(24 * GIB), JOIN_MAX);
    assert_eq!(join_deadline(u64::MAX), JOIN_MAX);
}

#[test]
fn the_list_lives_in_the_private_runtime_folder() {
    assert_eq!(
        list_file(Path::new("/run/user/1000"), "12-1"),
        PathBuf::from("/run/user/1000/selenita/join-12-1.txt")
    );
}

/// The list and the argv against the real `ffmpeg` (installed with libmpv on
/// the author's host): two one-second clips it makes itself, in a folder
/// whose name has a space and a quote, join into one of two seconds. Skipped
/// where `ffmpeg` is missing. No screen, portal or recording is involved.
#[test]
fn the_join_runs_on_real_segments_when_ffmpeg_is_installed() {
    use selenita_core::runner;
    use std::time::Duration;

    let has = |program: &str| {
        std::process::Command::new(program)
            .arg("-version")
            .output()
            .is_ok_and(|output| output.status.success())
    };
    if !has(JOINER) || !has("ffprobe") {
        eprintln!("ffmpeg or ffprobe is missing: the real join is not exercised");
        return;
    }
    let dir = std::env::temp_dir().join(format!("selenita-join-{}", std::process::id()));
    let folder = dir.join("it's a clip");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&folder).expect("scratch");
    let base = folder.join(".selenita-1-1.mp4");
    let segments: Vec<PathBuf> = (1..=2).map(|part| segment_file(&base, part)).collect();
    for segment in &segments {
        let argv: Vec<OsString> = [
            "ffmpeg",
            "-hide_banner",
            "-nostdin",
            "-y",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=1:size=64x48:rate=10",
            "-c:v",
            "mpeg4",
        ]
        .into_iter()
        .map(OsString::from)
        .chain([segment.as_os_str().to_os_string()])
        .collect();
        runner::run(&argv, None, Duration::from_secs(30)).expect("a segment");
    }
    let list = dir.join("join.txt");
    std::fs::write(&list, concat_list(&segments).expect("list")).expect("list file");
    runner::run(&join_argv(&list, &base), None, Duration::from_secs(30)).expect("joined");
    let probe: Vec<OsString> = [
        "ffprobe",
        "-v",
        "error",
        "-show_entries",
        "format=duration",
        "-of",
        "default=noprint_wrappers=1:nokey=1",
    ]
    .into_iter()
    .map(OsString::from)
    .chain([base.as_os_str().to_os_string()])
    .collect();
    let output = runner::run(&probe, None, Duration::from_secs(30)).expect("probed");
    let seconds: f64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("a duration");
    assert!((seconds - 2.0).abs() < 0.3, "{seconds}");
    let _ = std::fs::remove_dir_all(&dir);
}
