//! A throwaway client for the nest only (CEL-2-H Task 5, step 3, R-10
//! precursor): it locks the session through the toolkit's lock profile, draws
//! synthetic scenes with the lock's own CPU painter, and records how long each
//! frame took, so the choice of H-15 can be revisited with numbers.
//!
//! ```text
//! paint_probe --marks FILE
//! ```
//!
//! It appends one word to FILE at each stage (`empty`, `typed`, `status`,
//! `checking`, `faded`, `again`, `done`) so a script can capture the screen at
//! the right moment, prints the percentiles at the end and unlocks. It types
//! nothing anywhere, takes no password and starts no verifier: the unlock is
//! the toolkit's capability, used here only to end the rehearsal. It must run
//! against the nest's socket, never the live session. It is not shipped.

use std::{collections::BTreeMap, io::Write, thread, time::Duration};

use celestina_lock::machine::{Phase, Scene, SceneOutput, Status};
use celestina_lock::present::Presenter;
use celestina_lock::view::CpuPresenter;
use celestina_wayland::{
    lock::{run_lock, ConnectPolicy, LockCtl, LockEvent, LockShell, UnlockCapability},
    InputEvent, OutputId,
};

/// Frames per second of the synthetic typist.
const TICK: Duration = Duration::from_millis(40);

struct Probe {
    presenter: CpuPresenter,
    capability: Option<UnlockCapability>,
    outputs: BTreeMap<OutputId, SceneOutput>,
    marks: Option<std::fs::File>,
    tick: u32,
    ready_at: Option<u32>,
    /// The tick at which an output's size last changed: the nest window is
    /// moved and resized by the host after the lock is up.
    resized_at: u32,
    scene: Scene,
    samples: Vec<Duration>,
    frame_samples: Vec<Duration>,
}

impl Probe {
    fn mark(&mut self, word: &str) {
        if let Some(file) = self.marks.as_mut() {
            // A failing marks file only costs the script its cue.
            let _ = writeln!(file, "{word}");
            let _ = file.flush();
        }
        println!("{word}");
    }

    fn rebuild(&mut self) {
        let mut outputs: Vec<SceneOutput> = self.outputs.values().cloned().collect();
        outputs.sort_by(|a, b| a.name.cmp(&b.name));
        self.scene.prompt = outputs.first().map(|o| o.id);
        self.scene.outputs = outputs;
    }

    fn draw(&mut self, ctl: &mut LockCtl<'_>, timed: bool) {
        self.rebuild();
        self.presenter.render(&self.scene, ctl);
        // Only frames that really painted the prompt count; a frame the
        // compositor made us skip (both buffers held) costs nothing.
        if let Some(took) = self.presenter.take_paint_time() {
            if timed {
                self.samples.push(took);
            }
            self.frame_samples.push(took);
        }
    }

    fn step(&mut self, ctl: &mut LockCtl<'_>) {
        if self.capability.is_none() || self.outputs.values().all(|o| o.size.is_none()) {
            return;
        }
        if self.ready_at.is_none() && self.tick < self.resized_at + 75 {
            // Wait until the size has been steady for three seconds.
            self.tick += 1;
            return;
        }
        let ready = *self.ready_at.get_or_insert(self.tick);
        let t = self.tick - ready;
        self.tick += 1;
        let secs = |s: f32| (s * 25.0) as u32; // 25 ticks a second
        match t {
            0 => {
                self.draw(ctl, false);
                for output in self.outputs.values() {
                    println!(
                        "output {} logical {:?} buffer scale {}",
                        output.name,
                        output.size,
                        ctl.scale_of(output.id)
                    );
                }
                self.mark("empty");
            }
            n if n >= secs(2.5) && n < secs(2.5) + 14 => {
                self.scene.field_chars = (n - secs(2.5) + 1) as usize;
                self.draw(ctl, true);
            }
            n if n == secs(2.5) + 14 => self.mark("typed"),
            n if n == secs(5.0) => {
                self.scene.status = Status::Refused;
                self.scene.wait_secs = Some(4);
                self.scene.faillock_hint = true;
                self.scene.caps_lock = true;
                self.scene.field_chars = 0;
                self.draw(ctl, false);
                self.mark("status");
            }
            n if n == secs(7.5) => {
                self.scene.phase = Phase::Checking;
                self.scene.status = Status::Checking;
                self.scene.wait_secs = None;
                self.draw(ctl, false);
                self.mark("checking");
            }
            n if n == secs(10.0) => {
                self.scene.faded = true;
                self.draw(ctl, false);
                self.mark("faded");
            }
            n if n == secs(12.0) => {
                self.scene.faded = false;
                self.scene.phase = Phase::Prompt;
                self.scene.status = Status::Idle;
                self.scene.faillock_hint = false;
                self.scene.caps_lock = false;
                self.scene.field_chars = 5;
                self.draw(ctl, false);
                self.mark("again");
            }
            n if n > secs(14.0) && n <= secs(14.0) + 200 => {
                // 200 steady key presses: the count walks 1..=24 and wraps.
                self.scene.field_chars = ((n - secs(14.0)) % 24) as usize + 1;
                self.draw(ctl, true);
            }
            n if n == secs(14.0) + 201 => {
                self.report();
                self.mark("done");
                if let Some(capability) = self.capability.take() {
                    if let Err(err) = capability.unlock() {
                        eprintln!("paint_probe: unlock not confirmed: {err}");
                    }
                }
                ctl.stop();
            }
            _ => {}
        }
    }

    fn report(&mut self) {
        let mut micros: Vec<u128> = self.samples.iter().map(Duration::as_micros).collect();
        micros.sort_unstable();
        let pick = |p: f64| -> u128 {
            if micros.is_empty() {
                return 0;
            }
            let at = ((micros.len() as f64 - 1.0) * p).round() as usize;
            micros[at.min(micros.len() - 1)]
        };
        println!(
            "key-press frames: n={} p50={}us p99={}us max={}us",
            micros.len(),
            pick(0.50),
            pick(0.99),
            micros.last().copied().unwrap_or(0)
        );
        let mut all: Vec<u128> = self.frame_samples.iter().map(Duration::as_micros).collect();
        all.sort_unstable();
        println!(
            "all frames: n={} first={}us max={}us",
            all.len(),
            self.frame_samples.first().map_or(0, Duration::as_micros),
            all.last().copied().unwrap_or(0)
        );
    }
}

impl LockShell for Probe {
    type Message = ();

    fn event(&mut self, ev: LockEvent, ctl: &mut LockCtl<'_>) {
        match ev {
            LockEvent::Locked(capability) => self.capability = Some(capability),
            LockEvent::OutputAdded(info) => {
                self.outputs.insert(
                    info.id,
                    SceneOutput {
                        id: info.id,
                        name: info.name,
                        size: None,
                        scale: info.scale,
                    },
                );
            }
            LockEvent::OutputRemoved(id) => {
                self.outputs.remove(&id);
            }
            LockEvent::SurfaceConfigured { output, size } => {
                if let Some(entry) = self.outputs.get_mut(&output) {
                    if entry.size != Some(size) {
                        self.resized_at = self.tick;
                    }
                    entry.size = Some(size);
                }
            }
            LockEvent::Redraw => {
                if self.ready_at.is_some() {
                    self.draw(ctl, false);
                }
            }
            LockEvent::Finished | LockEvent::Keyboard { .. } => {}
        }
    }

    fn input(&mut self, _: OutputId, _: InputEvent, _: &mut LockCtl<'_>) {}

    fn message(&mut self, _: (), ctl: &mut LockCtl<'_>) {
        self.step(ctl);
    }
}

fn main() {
    // The toolkit and the painter log through `tracing`; a refused frame is a
    // warning worth seeing here.
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(std::io::stderr)
        .init();
    let marks = std::env::args()
        .skip(1)
        .collect::<Vec<_>>()
        .windows(2)
        .find(|pair| pair[0] == "--marks")
        .map(|pair| pair[1].clone());
    let marks = marks.and_then(|path| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()
    });
    let probe = Probe {
        presenter: CpuPresenter::new(),
        capability: None,
        outputs: BTreeMap::new(),
        marks,
        tick: 0,
        ready_at: None,
        resized_at: 0,
        scene: Scene {
            outputs: Vec::new(),
            prompt: None,
            phase: Phase::Prompt,
            field_chars: 0,
            status: Status::Idle,
            wait_secs: None,
            faillock_hint: false,
            caps_lock: false,
            keyboard: true,
            faded: false,
            rehearsal: true,
        },
        samples: Vec::new(),
        frame_samples: Vec::new(),
    };
    let result = run_lock(ConnectPolicy::detect(true), probe, |sender, _handle| {
        println!("requested");
        thread::spawn(move || loop {
            thread::sleep(TICK);
            if sender.send(()).is_err() {
                break;
            }
        });
        Ok(())
    });
    match result {
        Ok(()) => println!("stopped"),
        Err(err) => {
            eprintln!("paint_probe: {err:#}");
            std::process::exit(1);
        }
    }
}
