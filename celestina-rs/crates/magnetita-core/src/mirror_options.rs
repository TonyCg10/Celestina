//! What the mirror should look like — the handful of scrcpy choices worth a
//! control, and the only place they become command-line arguments.
//!
//! scrcpy has well over a hundred flags. Exposing them all would be a worse
//! product than exposing none: the ones that matter to someone mirroring a
//! phone at a desk are how sharp it is, how smooth it is, whether the phone's
//! own screen stays lit, and where the sound comes out. Everything else is
//! either a default that is already right or a niche the author can reach by
//! running scrcpy directly.
//!
//! The values are constrained by *type*, not by validation-after-the-fact: a
//! resolution is one of a few named caps, not an arbitrary integer, so a
//! nonsense setting cannot be stored and then fail at spawn time. This is also
//! what makes the arguments safe — every one is generated here from a closed
//! set, never assembled from text a caller supplied.

use serde::{Deserialize, Serialize};

/// How sharp the mirror is: the longest edge scrcpy will send.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MirrorResolution {
    /// 1080 px — the cheapest that still reads well on a desktop.
    Modest,
    /// 1440 px.
    #[default]
    Balanced,
    /// 1920 px.
    Sharp,
    /// The phone's own resolution, uncapped.
    Native,
}

impl MirrorResolution {
    /// The `--max-size` value, or `None` for the phone's own resolution.
    pub fn max_size(self) -> Option<u32> {
        match self {
            MirrorResolution::Modest => Some(1080),
            MirrorResolution::Balanced => Some(1440),
            MirrorResolution::Sharp => Some(1920),
            MirrorResolution::Native => None,
        }
    }

    /// The contract name, for D-Bus and the settings file.
    pub fn name(self) -> &'static str {
        match self {
            MirrorResolution::Modest => "modest",
            MirrorResolution::Balanced => "balanced",
            MirrorResolution::Sharp => "sharp",
            MirrorResolution::Native => "native",
        }
    }

    /// The resolution a contract name means, or `None` if it names none.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "modest" => Some(MirrorResolution::Modest),
            "balanced" => Some(MirrorResolution::Balanced),
            "sharp" => Some(MirrorResolution::Sharp),
            "native" => Some(MirrorResolution::Native),
            _ => None,
        }
    }
}

/// How smooth the mirror is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MirrorRate {
    /// 30 fps — enough to work in, and the kindest to a weak link.
    Calm,
    /// 60 fps.
    #[default]
    Smooth,
    /// 120 fps, for a phone and a link that can carry it.
    Fluid,
}

impl MirrorRate {
    pub fn max_fps(self) -> u32 {
        match self {
            MirrorRate::Calm => 30,
            MirrorRate::Smooth => 60,
            MirrorRate::Fluid => 120,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            MirrorRate::Calm => "calm",
            MirrorRate::Smooth => "smooth",
            MirrorRate::Fluid => "fluid",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "calm" => Some(MirrorRate::Calm),
            "smooth" => Some(MirrorRate::Smooth),
            "fluid" => Some(MirrorRate::Fluid),
            _ => None,
        }
    }
}

/// How much bandwidth the video may use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MirrorQuality {
    /// 4 Mbps — for a congested or distant Wi-Fi.
    Thrifty,
    /// 6 Mbps.
    #[default]
    Everyday,
    /// 16 Mbps, when the link is good and text must stay crisp.
    Generous,
}

impl MirrorQuality {
    /// The `--video-bit-rate` value, in scrcpy's own `M` notation.
    pub fn bit_rate(self) -> &'static str {
        match self {
            MirrorQuality::Thrifty => "4M",
            MirrorQuality::Everyday => "6M",
            MirrorQuality::Generous => "16M",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            MirrorQuality::Thrifty => "thrifty",
            MirrorQuality::Everyday => "everyday",
            MirrorQuality::Generous => "generous",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "thrifty" => Some(MirrorQuality::Thrifty),
            "everyday" => Some(MirrorQuality::Everyday),
            "generous" => Some(MirrorQuality::Generous),
            _ => None,
        }
    }
}

/// Where the phone's sound comes out while mirroring.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MirrorAudio {
    /// On the phone, as if nothing were mirroring. scrcpy's `--no-audio`.
    #[default]
    Phone,
    /// On this desktop, forwarded over the link — scrcpy's own default.
    Desktop,
}

impl MirrorAudio {
    pub fn name(self) -> &'static str {
        match self {
            MirrorAudio::Phone => "phone",
            MirrorAudio::Desktop => "desktop",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "phone" => Some(MirrorAudio::Phone),
            "desktop" => Some(MirrorAudio::Desktop),
            _ => None,
        }
    }
}

/// Every mirror choice, as one value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MirrorOptions {
    pub resolution: MirrorResolution,
    pub rate: MirrorRate,
    pub quality: MirrorQuality,
    pub audio: MirrorAudio,
    /// Turn the phone's own screen off while mirroring (`--turn-screen-off`).
    /// Off by default: it is the option most likely to be mistaken for the
    /// phone having died, and the author should choose it deliberately.
    pub screen_off: bool,
    /// Keep the phone awake while it is plugged in (`--stay-awake`), so a
    /// mirror does not go dark mid-use.
    pub stay_awake: bool,
}

impl MirrorOptions {
    /// The scrcpy argument vector for mirroring `serial`.
    ///
    /// Every element is either a literal or generated from a closed enum, so
    /// nothing a caller typed reaches the command line. The serial is the one
    /// value from outside, and it was validated where it became a
    /// [`MirrorEndpoint`](crate::mirror::MirrorEndpoint).
    pub fn scrcpy_args(&self, serial: &str) -> Vec<String> {
        let mut args = vec![
            "-s".to_owned(),
            serial.to_owned(),
            "--video-codec=h264".to_owned(),
            format!("--video-bit-rate={}", self.quality.bit_rate()),
            format!("--max-fps={}", self.rate.max_fps()),
            // One stable title, so a compositor rule can place the mirror and
            // so the window is recognisably this daemon's.
            "--window-title=Magnetita".to_owned(),
        ];
        if let Some(max_size) = self.resolution.max_size() {
            args.push(format!("--max-size={max_size}"));
        }
        if self.audio == MirrorAudio::Phone {
            args.push("--no-audio".to_owned());
        }
        if self.screen_off {
            args.push("--turn-screen-off".to_owned());
        }
        if self.stay_awake {
            args.push("--stay-awake".to_owned());
        }
        args
    }

    /// Applies one named setting, or `false` if the name or the value is not
    /// one this contract defines. Refusing is the point: a caller cannot store
    /// a value that would only fail later, at spawn time.
    pub fn set(&mut self, key: &str, value: &str) -> bool {
        match key {
            "resolution" => match MirrorResolution::from_name(value) {
                Some(resolution) => {
                    self.resolution = resolution;
                    true
                }
                None => false,
            },
            "rate" => match MirrorRate::from_name(value) {
                Some(rate) => {
                    self.rate = rate;
                    true
                }
                None => false,
            },
            "quality" => match MirrorQuality::from_name(value) {
                Some(quality) => {
                    self.quality = quality;
                    true
                }
                None => false,
            },
            "audio" => match MirrorAudio::from_name(value) {
                Some(audio) => {
                    self.audio = audio;
                    true
                }
                None => false,
            },
            "screenOff" => match parse_flag(value) {
                Some(flag) => {
                    self.screen_off = flag;
                    true
                }
                None => false,
            },
            "stayAwake" => match parse_flag(value) {
                Some(flag) => {
                    self.stay_awake = flag;
                    true
                }
                None => false,
            },
            _ => false,
        }
    }

    /// The settings as the contract's `(key, value)` pairs, for the app to read
    /// without knowing this crate's types.
    pub fn to_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            ("resolution", self.resolution.name().to_owned()),
            ("rate", self.rate.name().to_owned()),
            ("quality", self.quality.name().to_owned()),
            ("audio", self.audio.name().to_owned()),
            ("screenOff", flag_name(self.screen_off).to_owned()),
            ("stayAwake", flag_name(self.stay_awake).to_owned()),
        ]
    }
}

fn parse_flag(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn flag_name(flag: bool) -> &'static str {
    if flag {
        "true"
    } else {
        "false"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_flags_the_author_was_already_using() {
        // The working `~/Scripts/cpy.sh` invocation, minus `--turn-screen-off`,
        // which is now a deliberate choice rather than a permanent surprise.
        let args = MirrorOptions::default().scrcpy_args("10.0.0.190:45461");
        assert!(args.contains(&"--max-size=1440".to_owned()));
        assert!(args.contains(&"--video-bit-rate=6M".to_owned()));
        assert!(args.contains(&"--max-fps=60".to_owned()));
        assert!(args.contains(&"--no-audio".to_owned()));
        assert!(!args.iter().any(|arg| arg == "--turn-screen-off"));
    }

    #[test]
    fn the_serial_is_its_own_argument_and_never_interpolated() {
        let args = MirrorOptions::default().scrcpy_args("10.0.0.190:45461");
        let serial_at = args.iter().position(|arg| arg == "-s").unwrap() + 1;
        assert_eq!(args[serial_at], "10.0.0.190:45461");
        assert!(args.iter().all(|arg| !arg.contains(' ')));
    }

    #[test]
    fn a_native_resolution_caps_nothing() {
        let options = MirrorOptions {
            resolution: MirrorResolution::Native,
            ..Default::default()
        };
        let args = options.scrcpy_args("phone");
        assert!(!args.iter().any(|arg| arg.starts_with("--max-size")));
    }

    #[test]
    fn desktop_audio_drops_the_flag_that_keeps_it_on_the_phone() {
        let options = MirrorOptions {
            audio: MirrorAudio::Desktop,
            ..Default::default()
        };
        let args = options.scrcpy_args("phone");
        assert!(!args.contains(&"--no-audio".to_owned()));
    }

    #[test]
    fn the_two_switches_add_their_flags_only_when_chosen() {
        let options = MirrorOptions {
            screen_off: true,
            stay_awake: true,
            ..Default::default()
        };
        let args = options.scrcpy_args("phone");
        assert!(args.contains(&"--turn-screen-off".to_owned()));
        assert!(args.contains(&"--stay-awake".to_owned()));
    }

    #[test]
    fn a_value_this_contract_does_not_define_is_refused_not_stored() {
        let mut options = MirrorOptions::default();
        assert!(!options.set("resolution", "8k"));
        assert!(!options.set("rate", "240"));
        assert!(!options.set("screenOff", "yes"));
        assert!(!options.set("nonsense", "balanced"));
        // A refusal leaves the value it was asked to change untouched.
        assert_eq!(options, MirrorOptions::default());
    }

    #[test]
    fn every_setting_round_trips_through_its_contract_name() {
        let mut options = MirrorOptions::default();
        assert!(options.set("resolution", "sharp"));
        assert!(options.set("rate", "fluid"));
        assert!(options.set("quality", "generous"));
        assert!(options.set("audio", "desktop"));
        assert!(options.set("screenOff", "true"));
        assert!(options.set("stayAwake", "true"));

        let mut restored = MirrorOptions::default();
        for (key, value) in options.to_pairs() {
            assert!(restored.set(key, &value), "{key} did not round trip");
        }
        assert_eq!(restored, options);
    }

    #[test]
    fn the_settings_survive_the_file_they_persist_in() {
        let options = MirrorOptions {
            resolution: MirrorResolution::Native,
            screen_off: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&options).unwrap();
        assert_eq!(
            serde_json::from_str::<MirrorOptions>(&json).unwrap(),
            options
        );
        // A file written before an option existed still loads.
        let older: MirrorOptions = serde_json::from_str("{\"rate\":\"calm\"}").unwrap();
        assert_eq!(older.rate, MirrorRate::Calm);
        assert_eq!(older.resolution, MirrorResolution::default());
    }
}
