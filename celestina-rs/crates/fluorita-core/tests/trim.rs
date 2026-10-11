//! The video trim's pure half: the span a person chose, the encoder the
//! suite's rule picks, the exact `ffmpeg` argument list, and what the child's
//! `-progress` and `-encoders` output say.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use fluorita_core::trim::{
    probe_argv, trim_argv, Encoders, Progress, SourceFacts, Span, TrimError, VideoEncoder,
    MIN_SPAN, PROBE_PROGRAM, PROGRAM,
};

fn secs(value: f64) -> Duration {
    Duration::from_secs_f64(value)
}

fn tokens(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

#[test]
fn a_span_must_run_forward_inside_the_film_and_hold_a_frame() {
    assert_eq!(
        Span::new(secs(1.0), secs(1.0), secs(3.0)),
        Err(TrimError::Reversed)
    );
    assert_eq!(
        Span::new(secs(2.0), secs(1.0), secs(3.0)),
        Err(TrimError::Reversed)
    );
    assert_eq!(
        Span::new(secs(1.0), secs(3.5), secs(3.0)),
        Err(TrimError::PastTheEnd)
    );
    assert_eq!(
        Span::new(secs(1.0), secs(1.0) + MIN_SPAN / 2, secs(3.0)),
        Err(TrimError::TooShort)
    );
    let one_frame = Span::new(secs(1.0), secs(1.0) + MIN_SPAN, secs(3.0)).expect("one frame");
    assert_eq!(one_frame.length(), MIN_SPAN);
    let middle = Span::new(secs(1.0), secs(2.0), secs(3.0)).expect("a second");
    assert_eq!((middle.start, middle.end), (secs(1.0), secs(2.0)));
    assert!(!middle.is_whole(secs(3.0)));
}

#[test]
fn a_span_from_end_to_end_is_whole_and_saves_nothing() {
    let whole = Span::new(Duration::ZERO, secs(3.0), secs(3.0)).expect("the whole film");
    assert!(whole.is_whole(secs(3.0)));
    assert!(!Span::new(Duration::ZERO, secs(2.9), secs(3.0))
        .expect("all but the end")
        .is_whole(secs(3.0)));
    assert!(!Span::new(secs(0.1), secs(3.0), secs(3.0))
        .expect("all but the start")
        .is_whole(secs(3.0)));
}

#[test]
fn the_encoder_follows_the_suite_rule() {
    let node = PathBuf::from("/dev/dri/renderD128");
    assert_eq!(
        VideoEncoder::choose(Some(node.clone()), true),
        VideoEncoder::Vaapi {
            device: node.clone()
        }
    );
    assert_eq!(VideoEncoder::choose(Some(node), false), VideoEncoder::X264);
    assert_eq!(VideoEncoder::choose(None, true), VideoEncoder::X264);
    assert_eq!(VideoEncoder::choose(None, false), VideoEncoder::X264);
}

#[test]
fn the_x264_argv_is_exactly_the_documented_tokens() {
    let span = Span::new(secs(1.0), secs(2.0), secs(3.0)).expect("a span");
    let argv = trim_argv(
        Path::new("/v/a b.mp4"),
        Path::new("/v/.a b.mp4.trim-7.mp4"),
        span,
        &VideoEncoder::X264,
    );
    assert_eq!(
        argv,
        tokens(&[
            PROGRAM,
            "-hide_banner",
            "-nostdin",
            "-nostats",
            "-y",
            "-ss",
            "1.000",
            "-to",
            "2.000",
            "-i",
            "/v/a b.mp4",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-crf",
            "21",
            "-preset",
            "veryfast",
            "-c:a",
            "aac",
            "-b:a",
            "160k",
            "-sn",
            "-dn",
            "-movflags",
            "+faststart",
            "-progress",
            "pipe:1",
            "/v/.a b.mp4.trim-7.mp4",
        ])
    );
}

#[test]
fn the_vaapi_argv_names_the_render_node_and_uploads_to_it() {
    let span = Span::new(secs(1.0), secs(2.0), secs(3.0)).expect("a span");
    let argv = trim_argv(
        Path::new("/v/a.mp4"),
        Path::new("/v/.a.mp4.trim-7.mp4"),
        span,
        &VideoEncoder::Vaapi {
            device: PathBuf::from("/dev/dri/renderD128"),
        },
    );
    let video = tokens(&[
        "-vaapi_device",
        "/dev/dri/renderD128",
        "-vf",
        "format=nv12,hwupload",
        "-c:v",
        "h264_vaapi",
        "-qp",
        "20",
    ]);
    assert_eq!(argv[11..19], video[..]);
    assert_eq!(argv.len(), 11 + video.len() + 11);
    assert!(
        !argv.contains(&OsString::from("-pix_fmt")),
        "VA-API uploads NV12 itself"
    );
}

#[test]
fn a_time_is_written_in_milliseconds_and_never_rounded_up() {
    // Frame 31 of a 30 fps film starts at 1.0333… s: rounding up to 1.034
    // would drop the very frame the handle sits on.
    let span = Span::new(
        Duration::from_nanos(1_033_333_333),
        Duration::from_nanos(2_066_666_666),
        secs(3.0),
    )
    .expect("a span");
    let argv = trim_argv(
        Path::new("/a.mp4"),
        Path::new("/b.mp4"),
        span,
        &VideoEncoder::X264,
    );
    assert_eq!(argv[6], OsString::from("1.033"));
    assert_eq!(argv[8], OsString::from("2.066"));
}

#[test]
fn a_path_that_is_not_utf8_reaches_the_child_byte_for_byte() {
    use std::os::unix::ffi::OsStrExt;

    let name = std::ffi::OsStr::from_bytes(b"/v/caf\xe9.mp4");
    let span = Span::new(secs(1.0), secs(2.0), secs(3.0)).expect("a span");
    let argv = trim_argv(
        Path::new(name),
        Path::new("/v/out.mp4"),
        span,
        &VideoEncoder::X264,
    );
    assert_eq!(argv[10], name.to_os_string());
}

#[test]
fn progress_is_the_share_of_the_span_written_so_far() {
    let span = Span::new(secs(1.0), secs(3.0), secs(3.0)).expect("two seconds");
    let mut progress = Progress::new(span);
    assert_eq!(progress.read("frame=0"), None);
    assert_eq!(progress.read("out_time_us=N/A"), None);
    assert_eq!(progress.read("out_time_us=500000"), Some(0.25));
    assert_eq!(
        progress.read("out_time_us=4000000"),
        Some(1.0),
        "never past the end"
    );
    assert_eq!(
        progress.read("out_time_us=-12"),
        Some(0.0),
        "never before the start"
    );
    assert_eq!(progress.read("frame=60"), None);
    assert_eq!(progress.frames(), 60);
    assert!(!progress.ended());
    assert_eq!(progress.read("progress=end"), None);
    assert!(progress.ended());
    assert_eq!(progress.read("garbage without an equals sign"), None);
}

#[test]
fn the_encoder_list_says_which_h264_encoders_exist() {
    let listing = "Encoders:\n V..... = Video\n ------\n \
        V....D libx264              libx264 H.264 / AVC\n \
        V....D h264_vaapi           H.264/AVC (VAAPI) (codec h264)\n \
        A....D aac                  AAC (Advanced Audio Coding)\n";
    assert_eq!(
        Encoders::parse(listing),
        Encoders {
            h264_vaapi: true,
            libx264: true
        }
    );
    assert_eq!(
        Encoders::parse(" V....D libx264rgb  libx264 RGB\n V....D h264_vaapi_x  other\n"),
        Encoders::default(),
        "a name only counts whole"
    );
}

#[test]
fn every_error_has_spanish_words_for_the_window() {
    let errors = [
        TrimError::Reversed,
        TrimError::PastTheEnd,
        TrimError::TooShort,
        TrimError::Unchanged,
        TrimError::NotAVideo,
        TrimError::ToolMissing,
        TrimError::EncoderMissing,
        TrimError::Failed {
            code: Some(1),
            detail: "Conversion failed!".to_owned(),
        },
        TrimError::NoFrames,
        TrimError::Cancelled,
        TrimError::Unstarted {
            detail: "fork".to_owned(),
        },
    ];
    for error in errors {
        let message = error.message_es();
        assert!(!message.is_empty(), "{error:?} says nothing");
        assert!(!message.contains("ffmpeg -"), "{error:?} leaks an argv");
        assert!(!format!("{error}").is_empty());
    }
    let failed = TrimError::Failed {
        code: Some(1),
        detail: "Conversion failed!".to_owned(),
    };
    assert!(failed.to_string().contains("Conversion failed!"));
}

#[test]
fn a_span_holds_at_least_one_frame_of_the_film_s_own_rate() {
    let sixty = Duration::from_nanos(16_666_667);
    let one_frame =
        Span::framed(secs(1.0), secs(1.0) + sixty, secs(3.0), sixty).expect("one frame at 60 fps");
    assert_eq!(one_frame.length(), sixty);
    assert_eq!(
        Span::framed(secs(1.0), secs(1.0) + sixty / 2, secs(3.0), sixty),
        Err(TrimError::TooShort)
    );
    let film_frame = Duration::from_nanos(41_708_334);
    assert_eq!(
        Span::framed(secs(1.0), secs(1.0) + MIN_SPAN, secs(3.0), film_frame),
        Err(TrimError::TooShort),
        "a 24 fps frame is longer than the fallback"
    );
}

#[test]
fn the_probe_asks_ffprobe_for_streams_rate_and_container() {
    let argv = probe_argv(Path::new("/v/a b.mkv"));
    assert_eq!(argv[0], OsString::from(PROBE_PROGRAM));
    assert_eq!(argv.last(), Some(&OsString::from("/v/a b.mkv")));
    assert!(argv.contains(&OsString::from("compact=p=0")));
}

#[test]
fn a_film_s_facts_give_its_frame_and_what_a_trim_leaves_out() {
    let mkv =
        "codec_type=video|r_frame_rate=30/1|avg_frame_rate=30000/1001|disposition:attached_pic=0\n\
               codec_type=audio|r_frame_rate=0/0|avg_frame_rate=0/0|disposition:attached_pic=0\n\
               codec_type=audio|r_frame_rate=0/0|avg_frame_rate=0/0|disposition:attached_pic=0\n\
               codec_type=subtitle|r_frame_rate=0/0|avg_frame_rate=0/0|disposition:attached_pic=0\n\
               format_name=matroska,webm\n";
    let facts = SourceFacts::parse(mkv);
    assert_eq!(facts.frame(), Some(Duration::from_nanos(33_366_667)));
    assert_eq!(facts.left_out(), 2, "the second audio and the subtitles");

    let mp4 = "codec_type=video|r_frame_rate=60/1|avg_frame_rate=0/0|disposition:attached_pic=0\n\
               codec_type=video|r_frame_rate=90000/1|avg_frame_rate=0/0|disposition:attached_pic=1\n\
               format_name=mov,mp4,m4a,3gp,3g2,mj2\n";
    let facts = SourceFacts::parse(mp4);
    assert_eq!(
        facts.frame(),
        Some(Duration::from_nanos(16_666_667)),
        "the real video's rate, and r_frame_rate when avg is unknown"
    );
    assert_eq!(facts.left_out(), 1, "a cover picture is not kept");

    assert_eq!(SourceFacts::parse("").frame(), None);
    assert_eq!(SourceFacts::parse("").left_out(), 0);
    let absurd = "codec_type=video|r_frame_rate=100000/1|avg_frame_rate=1/1000\n";
    assert_eq!(
        SourceFacts::parse(absurd).frame(),
        None,
        "an implausible rate is no rate"
    );
}
