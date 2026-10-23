//! The overview's open/close state machine, without the surface (CEL-2-L,
//! design §4 and §6).
//!
//! The app owns the surface, the workers and the painter; this owns only the
//! question "what happens now?": a verb, a capture reply, a timeout or a
//! keybinds reply goes in and the [`Step`]s the app must run come out. That
//! keeps the toggle, switch and fallback rules testable without a compositor.
//!
//! The rules: a Desktop open asks for a capture first and shows nothing until
//! the reply (or a timeout) arrives, so the capture never contains the panel;
//! a Shortcuts open shows the surface at once and asks for the bindings. The
//! same mode's verb while open closes (toggle) or does nothing (open); the
//! other mode's verb switches the open panel. While a capture is in flight no
//! surface is made, whatever the verb: a Shortcuts verb only changes the mode
//! the panel will open in. Anything that makes the capture impossible opens
//! the panel at once with geometry only; a timeout gives the capture up
//! (cancels it) before the panel appears, so a late frame can never hold it.
//!
//! Every opening has its own number ([`Flow::generation`]). The capture, its
//! timer and the bindings reader carry it, and an answer with another number
//! is ignored: it belongs to an opening that is gone.

use celestina_wayland::OutputInfo;

use super::Mode;
use crate::capture::{CaptureRequest, OutputRequest};
use crate::command::Presence;
use crate::providers::niri::NiriModel;

/// What the app must do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Resolve the output the panel will open on and ask the capture worker
    /// for a frame of that output only; on refusal call
    /// [`Flow::capture_failed`], with no output close the flow.
    RequestCapture,
    /// Give up the capture in flight: signal its worker and let it go.
    CancelCapture,
    /// Create the surface now, in this mode.
    OpenSurface(Mode),
    /// The open panel switches to this mode.
    SetMode(Mode),
    /// Read the niri config's bindings off the loop's thread.
    LoadKeybinds,
    /// Hand the capture's previews to the open panel.
    ApplyPreviews,
    /// Hand the bindings to the open panel.
    ApplyKeybinds,
    /// Close the panel, or give up waiting for the capture.
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Closed,
    /// A capture was asked for and no surface exists yet.
    Waiting,
    Open,
}

/// Where the overview is in its life.
#[derive(Clone, Debug)]
pub struct Flow {
    phase: Phase,
    mode: Mode,
    /// The bindings were asked for during this opening.
    asked: bool,
    /// The current opening's number; every opening gets a new one.
    generation: u64,
}

impl Default for Flow {
    fn default() -> Self {
        Self::new()
    }
}

impl Flow {
    #[must_use]
    pub fn new() -> Flow {
        Flow {
            phase: Phase::Closed,
            mode: Mode::Desktop,
            asked: false,
            generation: 0,
        }
    }

    /// Whether anything of the overview exists: a wait or a surface.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.phase != Phase::Closed
    }

    /// Whether a capture was asked for and the surface is not yet made.
    #[must_use]
    pub fn is_waiting(&self) -> bool {
        self.phase == Phase::Waiting
    }

    /// The current opening's number: what its capture, timer and reader carry.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The mode shown (or about to be).
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// A verb of the overview (`Mode::Desktop`) or of the shortcuts
    /// (`Mode::Shortcuts`).
    pub fn verb(&mut self, wanted: Mode, presence: Presence) -> Vec<Step> {
        if self.phase == Phase::Closed {
            return match presence {
                Presence::Close => Vec::new(),
                Presence::Toggle | Presence::Open => self.begin(wanted),
            };
        }
        match presence {
            Presence::Close => self.close(),
            Presence::Toggle if self.mode == wanted => self.close(),
            Presence::Open if self.mode == wanted => Vec::new(),
            Presence::Toggle | Presence::Open => self.switch(wanted),
        }
    }

    fn begin(&mut self, wanted: Mode) -> Vec<Step> {
        self.mode = wanted;
        self.asked = false;
        self.generation += 1;
        match wanted {
            Mode::Desktop => {
                self.phase = Phase::Waiting;
                vec![Step::RequestCapture]
            }
            Mode::Shortcuts => self.open_now(false),
        }
    }

    fn switch(&mut self, wanted: Mode) -> Vec<Step> {
        self.mode = wanted;
        if self.phase == Phase::Waiting {
            // A capture is in flight: no surface may appear before its frame
            // is taken. The panel opens in this mode once it is.
            return Vec::new();
        }
        let mut steps = vec![Step::SetMode(wanted)];
        if wanted == Mode::Shortcuts && !self.asked {
            self.asked = true;
            steps.push(Step::LoadKeybinds);
        }
        steps
    }

    /// The surface appears now in the current mode; `previews` when a capture
    /// reply brought pictures to hand over.
    fn open_now(&mut self, previews: bool) -> Vec<Step> {
        self.phase = Phase::Open;
        let mut steps = vec![Step::OpenSurface(self.mode)];
        if previews {
            steps.push(Step::ApplyPreviews);
        }
        if self.mode == Mode::Shortcuts && !self.asked {
            self.asked = true;
            steps.push(Step::LoadKeybinds);
        }
        steps
    }

    fn close(&mut self) -> Vec<Step> {
        self.closed();
        vec![Step::Close]
    }

    /// The panel is gone by itself (an action, its output) or never could
    /// open: back to rest, with nothing for the app to do.
    pub fn closed(&mut self) {
        self.phase = Phase::Closed;
        self.mode = Mode::Desktop;
        self.asked = false;
    }

    /// The panel's input switched the mode (a tab, `1`, `2`, Tab).
    pub fn mode_switched(&mut self, mode: Mode) -> Vec<Step> {
        if self.phase != Phase::Open {
            return Vec::new();
        }
        self.mode = mode;
        if mode == Mode::Shortcuts && !self.asked {
            self.asked = true;
            return vec![Step::LoadKeybinds];
        }
        Vec::new()
    }

    /// The capture could not be asked for (busy, gone, nothing to capture):
    /// nothing is in flight for this opening, and the panel opens now with
    /// geometry only.
    pub fn capture_failed(&mut self) -> Vec<Step> {
        if self.phase != Phase::Waiting {
            return Vec::new();
        }
        self.open_now(false)
    }

    /// The timer of opening `generation` fired: a capture still in flight is
    /// given up first, then the panel opens with geometry only.
    pub fn timeout(&mut self, generation: u64) -> Vec<Step> {
        if generation != self.generation || self.phase != Phase::Waiting {
            return Vec::new();
        }
        let mut steps = vec![Step::CancelCapture];
        steps.extend(self.open_now(false));
        steps
    }

    /// The capture of opening `generation` replied (with previews or
    /// without). Only the wait it belongs to uses it: after a timeout, a
    /// close or in another opening the reply is dropped.
    pub fn captured(&mut self, generation: u64) -> Vec<Step> {
        if generation != self.generation || self.phase != Phase::Waiting {
            return Vec::new();
        }
        self.open_now(true)
    }

    /// The bindings' reader could not even be started: the panel shows them
    /// unavailable, and the next switch to the Shortcuts mode asks again
    /// instead of finding the question already asked.
    pub fn reader_failed(&mut self) {
        self.asked = false;
    }

    /// The bindings of opening `generation` arrived.
    pub fn keybinds_loaded(&mut self, generation: u64) -> Vec<Step> {
        if generation == self.generation && self.phase == Phase::Open {
            vec![Step::ApplyKeybinds]
        } else {
            Vec::new()
        }
    }
}

/// The capture request of opening `generation` for `output`, the one output
/// the panel opens on: one entry when niri knows that output and it has an
/// active workspace, carrying that workspace's id so the whole-output preview
/// is drawn as the preview of that workspace. No other output is ever named.
/// Windows play no part: their tiles stay geometry only. Empty when the
/// output has no active workspace: there is nothing to capture.
#[must_use]
pub fn capture_request(model: &NiriModel, output: &OutputInfo, generation: u64) -> CaptureRequest {
    let mut request = CaptureRequest {
        generation,
        outputs: Vec::new(),
    };
    if let Some(active) = model
        .workspaces
        .iter()
        .find(|ws| ws.is_active && ws.output.as_deref() == Some(output.name.as_str()))
    {
        request.outputs.push(OutputRequest {
            name: output.name.clone(),
            logical_width: output.logical_size.0,
            logical_height: output.logical_size.1,
            scale: output.scale,
            workspace_id: active.id,
        });
    }
    request
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::niri::{Window, Workspace};
    use celestina_wayland::OutputId;

    fn info(id: u32, name: &str) -> OutputInfo {
        OutputInfo {
            id: OutputId(id),
            name: name.to_owned(),
            logical_size: (1920, 1080),
            scale: 1.0,
            refresh_mhz: 60_000,
        }
    }

    fn window(id: u64, workspace: u64) -> Window {
        Window {
            id,
            workspace_id: Some(workspace),
            tile_size: (400, 300),
            ..Default::default()
        }
    }

    fn workspace(id: u64, output: &str, active: bool) -> Workspace {
        Workspace {
            id,
            idx: 1,
            name: None,
            output: Some(output.to_owned()),
            is_active: active,
            is_focused: active,
            columns: 0,
        }
    }

    #[test]
    fn a_desktop_open_waits_for_the_capture_and_shows_nothing_until_it_replies() {
        let mut flow = Flow::new();
        assert_eq!(
            flow.verb(Mode::Desktop, Presence::Open),
            vec![Step::RequestCapture]
        );
        assert!(flow.is_waiting() && flow.is_active());
        assert_eq!(
            flow.captured(flow.generation()),
            vec![Step::OpenSurface(Mode::Desktop), Step::ApplyPreviews]
        );
        assert!(!flow.is_waiting() && flow.is_active());
    }

    #[test]
    fn a_busy_or_unavailable_capture_opens_at_once_with_geometry_only() {
        let mut flow = Flow::new();
        flow.verb(Mode::Desktop, Presence::Toggle);
        assert_eq!(
            flow.capture_failed(),
            vec![Step::OpenSurface(Mode::Desktop)]
        );
        assert_eq!(flow.capture_failed(), Vec::new(), "only once");
    }

    #[test]
    fn the_timeout_cancels_the_capture_then_opens_and_a_stale_timer_does_nothing() {
        let mut flow = Flow::new();
        flow.verb(Mode::Desktop, Presence::Open);
        let first = flow.generation();
        assert_eq!(flow.verb(Mode::Desktop, Presence::Close), vec![Step::Close]);
        flow.verb(Mode::Desktop, Presence::Open);
        assert_ne!(first, flow.generation());
        assert_eq!(flow.timeout(first), Vec::new());
        assert!(flow.is_waiting());
        assert_eq!(
            flow.timeout(flow.generation()),
            vec![Step::CancelCapture, Step::OpenSurface(Mode::Desktop)]
        );
        assert_eq!(flow.timeout(flow.generation()), Vec::new(), "only once");
    }

    #[test]
    fn a_late_reply_after_the_panel_opened_is_dropped() {
        let mut flow = Flow::new();
        flow.verb(Mode::Desktop, Presence::Open);
        flow.timeout(flow.generation());
        assert_eq!(
            flow.captured(flow.generation()),
            Vec::new(),
            "the panel is on screen: a frame taken now could hold it"
        );
        let mut refused = Flow::new();
        refused.verb(Mode::Desktop, Presence::Open);
        refused.capture_failed();
        assert_eq!(refused.captured(refused.generation()), Vec::new());
    }

    #[test]
    fn a_reply_of_another_opening_is_dropped() {
        let mut flow = Flow::new();
        flow.verb(Mode::Desktop, Presence::Open);
        let old = flow.generation();
        flow.verb(Mode::Desktop, Presence::Close);
        flow.verb(Mode::Desktop, Presence::Open);
        assert_eq!(flow.captured(old), Vec::new());
        assert!(flow.is_waiting(), "the current wait goes on");
        assert_eq!(
            flow.captured(flow.generation()),
            vec![Step::OpenSurface(Mode::Desktop), Step::ApplyPreviews]
        );
    }

    #[test]
    fn a_reply_after_close_is_dropped() {
        let mut flow = Flow::new();
        flow.verb(Mode::Desktop, Presence::Open);
        let generation = flow.generation();
        flow.verb(Mode::Desktop, Presence::Close);
        assert!(!flow.is_active());
        assert_eq!(flow.captured(generation), Vec::new());
        assert_eq!(flow.keybinds_loaded(generation), Vec::new());
    }

    #[test]
    fn shortcuts_open_at_once_and_ask_for_the_bindings() {
        let mut flow = Flow::new();
        assert_eq!(
            flow.verb(Mode::Shortcuts, Presence::Open),
            vec![Step::OpenSurface(Mode::Shortcuts), Step::LoadKeybinds]
        );
        assert_eq!(
            flow.keybinds_loaded(flow.generation()),
            vec![Step::ApplyKeybinds]
        );
    }

    #[test]
    fn bindings_of_another_opening_are_dropped() {
        let mut flow = Flow::new();
        flow.verb(Mode::Shortcuts, Presence::Open);
        let old = flow.generation();
        flow.verb(Mode::Shortcuts, Presence::Close);
        flow.verb(Mode::Shortcuts, Presence::Open);
        assert_ne!(old, flow.generation());
        assert_eq!(flow.keybinds_loaded(old), Vec::new());
        assert_eq!(
            flow.keybinds_loaded(flow.generation()),
            vec![Step::ApplyKeybinds]
        );
    }

    #[test]
    fn the_same_verb_toggles_closed_and_open_does_nothing() {
        let mut flow = Flow::new();
        flow.verb(Mode::Shortcuts, Presence::Open);
        assert_eq!(flow.verb(Mode::Shortcuts, Presence::Open), Vec::new());
        assert_eq!(
            flow.verb(Mode::Shortcuts, Presence::Toggle),
            vec![Step::Close]
        );
        assert!(!flow.is_active());
        assert_eq!(flow.verb(Mode::Shortcuts, Presence::Close), Vec::new());
    }

    #[test]
    fn the_other_modes_verb_switches_the_open_panel_and_loads_bindings_once() {
        let mut flow = Flow::new();
        flow.verb(Mode::Desktop, Presence::Open);
        flow.captured(flow.generation());
        assert_eq!(
            flow.verb(Mode::Shortcuts, Presence::Toggle),
            vec![Step::SetMode(Mode::Shortcuts), Step::LoadKeybinds]
        );
        assert_eq!(
            flow.verb(Mode::Desktop, Presence::Open),
            vec![Step::SetMode(Mode::Desktop)]
        );
        assert_eq!(
            flow.verb(Mode::Shortcuts, Presence::Open),
            vec![Step::SetMode(Mode::Shortcuts)],
            "already asked in this opening"
        );
    }

    #[test]
    fn a_reader_that_could_not_start_is_asked_for_again() {
        let mut flow = Flow::new();
        assert_eq!(
            flow.verb(Mode::Shortcuts, Presence::Open),
            vec![Step::OpenSurface(Mode::Shortcuts), Step::LoadKeybinds]
        );
        assert_eq!(flow.mode_switched(Mode::Shortcuts), Vec::new(), "asked");
        flow.reader_failed();
        assert_eq!(
            flow.mode_switched(Mode::Shortcuts),
            vec![Step::LoadKeybinds],
            "the failed start did not use up the question"
        );
    }

    #[test]
    fn a_shortcuts_verb_while_waiting_makes_no_surface_until_the_capture_ends() {
        let mut flow = Flow::new();
        flow.verb(Mode::Desktop, Presence::Open);
        assert_eq!(
            flow.verb(Mode::Shortcuts, Presence::Open),
            Vec::new(),
            "screencopy is in flight: no surface yet"
        );
        assert!(flow.is_waiting());
        assert_eq!(flow.mode(), Mode::Shortcuts);
        assert_eq!(
            flow.captured(flow.generation()),
            vec![
                Step::OpenSurface(Mode::Shortcuts),
                Step::ApplyPreviews,
                Step::LoadKeybinds
            ]
        );
        // And on the timeout instead of a reply.
        let mut timed = Flow::new();
        timed.verb(Mode::Desktop, Presence::Open);
        timed.verb(Mode::Shortcuts, Presence::Toggle);
        assert_eq!(
            timed.timeout(timed.generation()),
            vec![
                Step::CancelCapture,
                Step::OpenSurface(Mode::Shortcuts),
                Step::LoadKeybinds
            ]
        );
    }

    #[test]
    fn closing_while_waiting_gives_up_the_wait() {
        let mut flow = Flow::new();
        flow.verb(Mode::Desktop, Presence::Open);
        assert_eq!(
            flow.verb(Mode::Desktop, Presence::Toggle),
            vec![Step::Close]
        );
        assert!(!flow.is_active());
        // A re-open asks anew, under a new number.
        let closed = flow.generation();
        assert_eq!(
            flow.verb(Mode::Desktop, Presence::Open),
            vec![Step::RequestCapture]
        );
        assert_ne!(flow.generation(), closed);
    }

    #[test]
    fn a_tab_to_shortcuts_asks_for_the_bindings_once_per_opening() {
        let mut flow = Flow::new();
        flow.verb(Mode::Desktop, Presence::Open);
        flow.captured(flow.generation());
        assert_eq!(
            flow.mode_switched(Mode::Shortcuts),
            vec![Step::LoadKeybinds]
        );
        assert_eq!(flow.mode_switched(Mode::Desktop), Vec::new());
        assert_eq!(flow.mode_switched(Mode::Shortcuts), Vec::new());
        flow.closed();
        flow.verb(Mode::Desktop, Presence::Open);
        flow.captured(flow.generation());
        assert_eq!(
            flow.mode_switched(Mode::Shortcuts),
            vec![Step::LoadKeybinds],
            "a new opening reads the config again"
        );
    }

    fn model() -> NiriModel {
        NiriModel {
            workspaces: vec![
                workspace(1, "DP-1", true),
                workspace(2, "DP-1", false),
                workspace(3, "HDMI-A-1", true),
            ],
            windows: vec![window(10, 1), window(12, 2)],
            ..Default::default()
        }
    }

    #[test]
    fn the_request_names_only_the_output_the_panel_opens_on() {
        let request = capture_request(&model(), &info(2, "HDMI-A-1"), 9);
        assert_eq!(request.generation, 9);
        assert_eq!(request.outputs.len(), 1, "never the other outputs");
        assert_eq!(request.outputs[0].name, "HDMI-A-1");
        assert_eq!(request.outputs[0].workspace_id, 3);
        let request = capture_request(&model(), &info(1, "DP-1"), 9);
        assert_eq!(request.outputs.len(), 1);
        assert_eq!(request.outputs[0].workspace_id, 1);
    }

    #[test]
    fn an_empty_active_workspace_is_still_captured() {
        let model = NiriModel {
            workspaces: vec![workspace(1, "DP-1", true)],
            windows: Vec::new(),
            ..Default::default()
        };
        let request = capture_request(&model, &info(1, "DP-1"), 1);
        assert_eq!(request.outputs.len(), 1);
        assert_eq!(request.outputs[0].workspace_id, 1);
    }

    #[test]
    fn an_output_without_an_active_workspace_is_an_empty_request() {
        assert!(capture_request(&model(), &info(3, "DP-9"), 1)
            .outputs
            .is_empty());
        let idle = NiriModel {
            workspaces: vec![workspace(1, "DP-1", false)],
            ..Default::default()
        };
        assert!(capture_request(&idle, &info(1, "DP-1"), 1)
            .outputs
            .is_empty());
    }
}
