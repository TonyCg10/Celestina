//! The `polkit` duty's lifecycle on the loop's side (CEL-2-I; design 4.5 and
//! 6.4 F-9, F-24, F-25).
//!
//! The duty is `off` by default and follows `shell.json` live, like the other
//! duties: **off** means no worker, no bus connection and nothing registered
//! with polkitd; **on** starts the agent ([`crate::providers::polkit`]) and
//! the prompt state over it; switched **off** again, [`PolkitDuty::stop`]
//! tears the state down on purpose: the active request is answered
//! `Cancelled`, its helper conversation is cancelled (the socket closes and an
//! answer on its way is dropped), the typed text is wiped, the surfaces are
//! destroyed, and the agent is handed to a reaper thread, which answers every
//! waiting request `Cancelled`, unregisters from polkitd and joins the agent's
//! threads. The loop waits for none of that (N-22); the next agent waits for
//! the reaper before it registers. The chain is transitive: an agent's reaper
//! also waits for the teardown that agent was started behind, so the duty
//! keeps only the latest reaper and a quick off, on, off, on never lets the
//! third agent register while the first still holds the session.
//!
//! Starting is behind [`AgentLauncher`], the same seam the energy duties use
//! for their workers, so the rules are tested without a compositor.
//!
//! **Where the duty reports its state.** Like the other duties it reports
//! through the journal: the agent publishes its registration state
//! (`registered`, `refused`, `unavailable`, `waiting`) as events, logged once
//! each on change by [`crate::providers::polkit::PolkitEvent::log`]. A
//! refused or unavailable registration is final until the person acts: the
//! duty does not retry on its own, and switching it off and on again starts a
//! new agent that tries once.
//!
//! **Stale news is dropped.** The events of an agent that was switched off
//! must not reach the next one, so every agent is wrapped in a [`Gated`] that
//! stops publishing the moment it is dropped or retired. Request ids are also
//! never reused between agents (`providers::polkit` hands them out from one
//! counter), so a late event cannot name a request of the next agent.

use std::cell::OnceCell;
use std::io;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use celestina_paint::Theme;
use celestina_shell::polkit::{Ended, Ending, Promotion, RegistrationState, RequestId};
use celestina_shell::settings::Duty;
use celestina_wayland::{OutputId, OutputInfo, Sender};

use super::dumpable::DumpableWindow;
use super::state::PolkitState;
use super::{AgentPort, HelperSpawner, PromptHost, ThreadHelpers};
use crate::message::Message;
use crate::providers::polkit::helper::HelperInput;
use crate::providers::polkit::{self, PolkitAgent, PolkitConfig, PolkitEvent, Prompt};
use crate::providers::Reaper;

/// What a launch produced: the agent behind its port, the way helper
/// conversations are started, and the non-dumpable window of the process.
pub struct Launched {
    /// The agent.
    pub agent: Box<dyn AgentPort>,
    /// Starts the helper conversation of a request.
    pub helpers: Box<dyn HelperSpawner>,
    /// The window opened while a request is active. The production launcher
    /// gives the process's real flag; a test gives an inert one.
    pub dumpable: DumpableWindow,
}

/// How an agent comes to exist: the seam between the rules and the system.
pub trait AgentLauncher {
    /// Starts an agent, which registers with polkitd on its own thread, once
    /// `after` (the teardown of the agent it replaces, possibly empty) is
    /// over.
    ///
    /// # Errors
    ///
    /// The failure to start a thread.
    fn launch(&self, after: Reaper) -> io::Result<Launched>;
}

/// The slot the launcher is put in once the loop runs (as for the energy
/// handle): the sender and the shutdown flag exist only inside `setup`.
pub type LauncherSlot = Rc<OnceCell<Box<dyn AgentLauncher>>>;

/// The gate in front of an agent's events: open while it lives, closed the
/// moment it is dropped.
struct Gate(Arc<AtomicBool>);

impl Drop for Gate {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// An agent that stops publishing when it is dropped or retired, so the news
/// of an agent that was switched off never reaches the next one.
pub struct Gated {
    // Declared first, so it drops (closes) before the agent stops and joins.
    live: Gate,
    agent: PolkitAgent,
}

impl Gated {
    /// Starts an agent on `config` whose events go to `publish` while this
    /// value lives. The agent's shutdown flag is its own: joining it raises
    /// it, and a duty switched off must not stop the shell.
    ///
    /// # Errors
    ///
    /// The failure to start a thread.
    pub fn start(
        config: PolkitConfig,
        publish: impl Fn(PolkitEvent) -> bool + Send + Sync + 'static,
    ) -> io::Result<Gated> {
        let open = Arc::new(AtomicBool::new(true));
        let gate = Arc::clone(&open);
        let own = Arc::new(AtomicBool::new(false));
        let agent = polkit::spawn(
            config,
            move |event| gate.load(Ordering::Acquire) && publish(event),
            &own,
        )?;
        Ok(Gated {
            live: Gate(open),
            agent,
        })
    }
}

impl AgentPort for Gated {
    fn set_locked(&self, locked: bool) {
        self.agent.set_locked(locked);
    }

    fn promote(&self, now: Instant) -> Promotion {
        self.agent.promote(now)
    }

    fn prompt(&self, id: RequestId) -> Option<Prompt> {
        self.agent.prompt(id)
    }

    fn helper_input(&self, id: RequestId) -> Option<HelperInput> {
        self.agent.helper_input(id)
    }

    fn end_active(
        &self,
        id: RequestId,
        ending: Ending,
        attempted: bool,
        now: Instant,
    ) -> Option<Ended> {
        self.agent.end_active(id, ending, attempted, now)
    }

    fn retire(self: Box<Self>) -> Reaper {
        // What the agent says while it stops is nobody's news: the gate goes
        // down first, then the agent stops on the reaper.
        let Gated { live, agent } = *self;
        drop(live);
        agent.retire()
    }
}

/// The production launcher: the system bus, publishing into the loop.
pub struct SystemLauncher {
    sender: Sender<Message>,
    shutdown: Arc<AtomicBool>,
}

impl SystemLauncher {
    /// A launcher whose agents publish through `sender`; `shutdown` is the
    /// shell's flag, which ends the helper conversations too.
    #[must_use]
    pub fn new(sender: Sender<Message>, shutdown: Arc<AtomicBool>) -> SystemLauncher {
        SystemLauncher { sender, shutdown }
    }
}

impl AgentLauncher for SystemLauncher {
    fn launch(&self, after: Reaper) -> io::Result<Launched> {
        let sender = self.sender.clone();
        let config = PolkitConfig {
            after,
            ..PolkitConfig::default()
        };
        let agent = Gated::start(config, move |event| {
            sender.send(Message::Polkit(event)).is_ok()
        })?;
        Ok(Launched {
            agent: Box::new(agent),
            helpers: Box::new(ThreadHelpers::new(
                self.sender.clone(),
                Arc::clone(&self.shutdown),
            )),
            dumpable: DumpableWindow::process(),
        })
    }
}

/// What [`PolkitDuty::apply`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// The duty is as it should be.
    Unchanged,
    /// The agent and the prompt state were started.
    Started,
    /// The prompt state was torn down and the agent dropped.
    Stopped,
    /// The duty is on and the agent did not start; nothing retries.
    Failed,
}

/// The `polkit` duty: nothing while off, the agent and the prompt state while
/// on.
pub struct PolkitDuty {
    launcher: LauncherSlot,
    state: Option<PolkitState>,
    /// Every output announced, kept while the duty is off so a prompt started
    /// later dims them all.
    outputs: Vec<OutputInfo>,
    registration: RegistrationState,
    origin: Instant,
    /// The teardown of the last agent that was switched off, which a new
    /// agent is started behind; the loop itself never waits for it (N-22).
    /// It is **kept**, not handed over: each agent's reaper also waits for
    /// the one its agent was started behind, so this one is over only when
    /// every earlier teardown is, and a launch that fails loses nothing.
    reaper: Reaper,
}

impl std::fmt::Debug for PolkitDuty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolkitDuty")
            .field("running", &self.state.is_some())
            .field("registration", &self.registration)
            .finish_non_exhaustive()
    }
}

impl PolkitDuty {
    /// The duty over the slot the launcher is put in once the loop runs.
    #[must_use]
    pub fn new(launcher: LauncherSlot) -> PolkitDuty {
        PolkitDuty {
            launcher,
            state: None,
            outputs: Vec::new(),
            registration: RegistrationState::Off,
            origin: Instant::now(),
            reaper: Reaper::default(),
        }
    }

    /// The prompt state while the duty runs.
    #[must_use]
    pub fn state(&self) -> Option<&PolkitState> {
        self.state.as_ref()
    }

    /// The prompt state while the duty runs.
    pub fn state_mut(&mut self) -> Option<&mut PolkitState> {
        self.state.as_mut()
    }

    /// Whether an agent is running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.state.is_some()
    }

    /// Where the registration with polkitd stands, as last reported.
    #[must_use]
    pub fn registration(&self) -> &RegistrationState {
        &self.registration
    }

    /// The agent reported where its registration stands. Ignored when no agent
    /// runs: it is the last word of one that was switched off.
    pub fn registration_reported(&mut self, state: RegistrationState) {
        if self.state.is_some() {
            self.registration = state;
        }
    }

    /// An output appeared or changed: remembered for a prompt started later.
    pub fn note_output(&mut self, info: &OutputInfo) {
        match self.outputs.iter_mut().find(|known| known.id == info.id) {
            Some(known) => *known = info.clone(),
            None => self.outputs.push(info.clone()),
        }
    }

    /// An output went away.
    pub fn forget_output(&mut self, id: OutputId) {
        self.outputs.retain(|known| known.id != id);
    }

    /// Brings the duty to `want`: starts it when it is on and not running,
    /// tears it down when it is off and running, and does nothing otherwise.
    /// In particular a duty that is on and whose registration was refused is
    /// **not** restarted: switching it off and on is the retry.
    pub fn apply(
        &mut self,
        want: Duty,
        theme: &Theme,
        reduced: bool,
        now: Instant,
        host: &mut dyn PromptHost,
    ) -> Change {
        match (want, self.state.is_some()) {
            (Duty::On, false) => self.start(theme, reduced, host),
            (Duty::Off, true) => {
                self.stop(now, host);
                Change::Stopped
            }
            _ => Change::Unchanged,
        }
    }

    fn start(&mut self, theme: &Theme, reduced: bool, host: &mut dyn PromptHost) -> Change {
        let Some(launcher) = self.launcher.get() else {
            tracing::warn!("polkit: the shell has not finished starting; the duty waits");
            return Change::Failed;
        };
        let launched = match launcher.launch(self.reaper.clone()) {
            Ok(launched) => launched,
            Err(err) => {
                let why = crate::error_chain(&err);
                tracing::warn!("polkit: the agent did not start: {why}");
                self.registration = RegistrationState::Unavailable(why);
                return Change::Failed;
            }
        };
        let mut state = PolkitState::new(
            launched.agent,
            launched.helpers,
            self.origin,
            theme.clone(),
            reduced,
        );
        state.set_dumpable(launched.dumpable);
        for info in &self.outputs {
            state.output_added(info, host);
        }
        self.state = Some(state);
        self.registration = RegistrationState::Waiting;
        tracing::info!("polkit: the duty is on; the agent registers");
        Change::Started
    }

    /// Switches the duty off: every request is cancelled, the helper
    /// conversation closed, the typed text wiped, the surfaces destroyed, and
    /// the agent handed to a reaper, which answers every request that was
    /// only waiting `Cancelled`, unregisters, closes its bus connection and
    /// joins its threads. **The loop does not wait for any of that** (N-22):
    /// polkitd may be slow to answer the unregistration. The next agent
    /// waits for the reaper before it registers. Does nothing when it is off.
    pub fn stop(&mut self, now: Instant, host: &mut dyn PromptHost) {
        let Some(mut state) = self.state.take() else {
            return;
        };
        state.shut_down(now, host);
        // What the agent publishes while stopping is dropped. The new reaper
        // also waits for the one this agent was started behind, so it
        // replaces it.
        self.reaper = state.hand_off_agent();
        drop(state);
        self.registration = RegistrationState::Off;
        tracing::info!("polkit: the duty is off; the agent is unregistered");
    }
}

impl Drop for PolkitDuty {
    /// The shell is ending: this is the one place that waits for the reaper
    /// of the agents switched off earlier, so it is not cut short by the
    /// exit.
    /// An agent still running is dropped with its state and stops bounded.
    fn drop(&mut self) {
        self.reaper.wait();
    }
}

#[cfg(test)]
mod tests;
