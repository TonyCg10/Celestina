use std::ffi::OsString;
use std::path::{Path, PathBuf};

use selenita_core::names::user_dir_from;
use selenita_core::names::{recordings_dir_in, RECORDINGS_FOLDER};
use selenita_core::record::{
    inspect_argv, quoted, request_stop, stop_file, take_stop_request, Event, Pipeline, Source,
    State, VideoEncoder, AUDIO_BED, AUDIO_ENCODER, AUDIO_MIXER, INSPECTOR, KEY_INTERVAL, LAUNCHER,
    MUXER,
};

fn pipeline(encoder: VideoEncoder, audio: bool) -> Pipeline {
    Pipeline {
        source: Source {
            node: 42,
            fd: Some(0),
        },
        encoder,
        audio,
        out: PathBuf::from("/videos/Recording 2026-10-09 14.32.05.mp4"),
    }
}

fn joined(pipeline: &Pipeline) -> String {
    pipeline.description().join(" ")
}

#[test]
fn the_four_combinations_build_their_branches() {
    for (encoder, element) in [
        (VideoEncoder::X264, "x264enc"),
        (VideoEncoder::VaH264, "vah264enc"),
    ] {
        for audio in [false, true] {
            let text = joined(&pipeline(encoder, audio));
            assert!(text.contains(&format!(" ! {element} ")), "{text}");
            assert!(
                text.starts_with(&format!("{MUXER} name=mux ! filesink location=")),
                "{text}"
            );
            assert!(text.contains("pipewiresrc fd=0 path=42 "), "{text}");
            assert!(text.contains("! videoconvert ! "), "{text}");
            assert!(text.contains("! h264parse ! queue ! mux."), "{text}");
            let audio_branch =
                format!("! audioconvert ! audioresample ! {AUDIO_ENCODER} ! queue ! mux.");
            assert_eq!(text.contains(&audio_branch), audio, "{text}");
            assert_eq!(text.contains("stream.capture.sink=true"), audio, "{text}");
            assert_eq!(
                text.matches("pipewiresrc").count(),
                if audio { 2 } else { 1 }
            );
        }
    }
}

/// The encoders run at a constant quality, never at `x264enc`'s 2048 kbit/s
/// or `vah264enc`'s CBR default, with one keyframe a second.
#[test]
fn the_encoders_run_at_a_constant_quality() {
    let x264 = joined(&pipeline(VideoEncoder::X264, false));
    assert!(
        x264.contains("! x264enc tune=zerolatency speed-preset=veryfast pass=qual quantizer=21 key-int-max=60 !"),
        "{x264}"
    );
    assert!(!x264.contains("bitrate="), "{x264}");
    let va = joined(&pipeline(VideoEncoder::VaH264, false));
    assert!(
        va.contains("! vah264enc rate-control=cqp qpi=20 qpp=22 key-int-max=60 !"),
        "{va}"
    );
    assert!(va.contains(&format!("key-int-max={KEY_INTERVAL} ")), "{va}");
}

/// The sound branch mixes the monitor over a silent live bed, so the muxer's
/// audio pad always has data and the EOS at the stop goes through even when
/// the monitor hands over nothing; the bed fixes the format (stereo, 48 kHz)
/// and the monitor is converted into the mixer.
#[test]
fn the_sound_branch_flows_over_a_silent_bed() {
    let text = joined(&pipeline(VideoEncoder::VaH264, true));
    let bed = format!(
        "{AUDIO_BED} wave=silence is-live=true ! audio/x-raw,format=S16LE,rate=48000,channels=2 \
         ! {AUDIO_MIXER} name=mix ignore-inactive-pads=true ! audioconvert ! audioresample \
         ! {AUDIO_ENCODER} ! queue ! mux."
    );
    assert!(text.contains(&bed), "{text}");
    let monitor = "pipewiresrc stream-properties=\"props,stream.capture.sink=true\" \
                   do-timestamp=true ! audio/x-raw ! audioconvert ! audioresample ! mix.";
    assert!(text.ends_with(monitor), "{text}");
    let bed_at = text.find(AUDIO_BED).expect("the bed");
    let monitor_at = text.find("stream.capture.sink").expect("the monitor");
    assert!(bed_at < monitor_at, "the bed sets the mixer's format first");
    let silent = joined(&pipeline(VideoEncoder::VaH264, false));
    assert!(!silent.contains(AUDIO_MIXER) && !silent.contains(AUDIO_BED));
}

#[test]
fn the_output_path_is_one_quoted_token() {
    let tokens = pipeline(VideoEncoder::X264, false).description();
    let location = tokens
        .iter()
        .find(|token| token.starts_with("location="))
        .expect("a location");
    assert_eq!(
        location,
        "location=\"/videos/Recording 2026-10-09 14.32.05.mp4\""
    );
    assert_eq!(quoted(Path::new("/a \"b\"\\c")), "\"/a \\\"b\\\"\\\\c\"");
}

#[test]
fn a_source_without_fd_connects_on_its_own() {
    let mut without = pipeline(VideoEncoder::X264, false);
    without.source.fd = None;
    let text = joined(&without);
    assert!(text.contains("pipewiresrc path=42 "), "{text}");
    assert!(!text.contains("fd="), "{text}");
}

#[test]
fn the_launcher_and_the_inspector_argv() {
    let argv = pipeline(VideoEncoder::VaH264, true).launch_argv();
    let words: Vec<&str> = argv.iter().filter_map(|word| word.to_str()).collect();
    assert_eq!(&words[..3], [LAUNCHER, "-e", "-q"]);
    assert_eq!(words.len(), argv.len());
    assert!(words.contains(&"vah264enc"));
    assert_eq!(
        inspect_argv(MUXER),
        vec![
            OsString::from(INSPECTOR),
            OsString::from("--exists"),
            OsString::from(MUXER)
        ]
    );
}

#[test]
fn the_encoder_choice_needs_a_render_node_and_the_element() {
    assert_eq!(VideoEncoder::choose(true, true), VideoEncoder::VaH264);
    assert_eq!(VideoEncoder::choose(false, true), VideoEncoder::X264);
    assert_eq!(VideoEncoder::choose(true, false), VideoEncoder::X264);
    assert_eq!(VideoEncoder::X264.element(), "x264enc");
    assert_eq!(VideoEncoder::VaH264.element(), "vah264enc");
}

#[test]
fn the_state_machine_walks_a_recording() {
    let state = State::Idle;
    let state = state.next(Event::Start).expect("preparing");
    assert_eq!(state, State::Preparing);
    let state = state.next(Event::Prepared).expect("recording");
    assert_eq!(state, State::Recording);
    assert!(state.is_busy());
    let state = state.next(Event::Stop).expect("stopping");
    assert_eq!(state, State::Stopping);
    let state = state.next(Event::Stopped).expect("idle");
    assert_eq!(state, State::Idle);
    assert!(!state.is_busy());
}

#[test]
fn a_failure_lands_in_failed_and_leaves_it() {
    for busy in [State::Preparing, State::Recording, State::Stopping] {
        assert_eq!(busy.next(Event::Failure), Ok(State::Failed), "{busy:?}");
    }
    assert_eq!(State::Failed.next(Event::Start), Ok(State::Preparing));
    assert_eq!(State::Failed.next(Event::Acknowledged), Ok(State::Idle));
    assert!(!State::Failed.is_busy());
}

#[test]
fn the_other_transitions_are_refused() {
    let refused = [
        (State::Idle, Event::Stop),
        (State::Idle, Event::Prepared),
        (State::Idle, Event::Stopped),
        (State::Idle, Event::Failure),
        (State::Preparing, Event::Start),
        (State::Preparing, Event::Stop),
        (State::Preparing, Event::Stopped),
        (State::Recording, Event::Start),
        (State::Recording, Event::Prepared),
        (State::Recording, Event::Stopped),
        (State::Stopping, Event::Start),
        (State::Stopping, Event::Stop),
        (State::Stopping, Event::Prepared),
        (State::Failed, Event::Stop),
        (State::Failed, Event::Prepared),
    ];
    for (state, event) in refused {
        let error = state.next(event).expect_err("refused");
        assert_eq!((error.state, error.event), (state, event));
        assert!(error.to_string().contains(state.as_str()));
    }
    assert_eq!(State::Recording.as_str(), "recording");
    assert_eq!(State::Idle.as_str(), "idle");
}

#[test]
fn the_stop_file_is_touched_and_taken_once() {
    let runtime = std::env::temp_dir().join(format!("selenita-record-stop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&runtime);
    std::fs::create_dir_all(&runtime).expect("runtime");
    let file = stop_file(&runtime);
    assert_eq!(file, runtime.join("selenita").join("stop"));
    assert!(!take_stop_request(&file));
    request_stop(&file).expect("touched");
    assert!(file.is_file());
    assert!(take_stop_request(&file));
    assert!(!file.exists());
    assert!(!take_stop_request(&file));
    let _ = std::fs::remove_dir_all(&runtime);
}

#[test]
fn the_videos_folder_reads_like_the_pictures_one() {
    let home = Path::new("/home/ana");
    let file = "XDG_PICTURES_DIR=\"$HOME/Pictures\"\nXDG_VIDEOS_DIR=\"$HOME/Videos\"\n".as_bytes();
    assert_eq!(
        user_dir_from(file, "XDG_VIDEOS_DIR", home),
        Some(PathBuf::from("/home/ana/Videos"))
    );
}

/// A recording lands in the videos folder's `Recordings`, as a capture lands
/// in the pictures folder's «Capturas», never in the videos root.
#[test]
fn a_recording_lands_in_the_recordings_folder_of_the_videos_folder() {
    assert_eq!(RECORDINGS_FOLDER, "Recordings");
    assert_eq!(
        recordings_dir_in(Path::new("/home/ana/Videos")),
        PathBuf::from("/home/ana/Videos/Recordings")
    );
    assert_eq!(
        recordings_dir_in(Path::new("/home/ana/Videos"))
            .parent()
            .map(Path::to_path_buf),
        Some(PathBuf::from("/home/ana/Videos"))
    );
}
