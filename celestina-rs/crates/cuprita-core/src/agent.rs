//! The Bluetooth pairing agent as a pure state machine: one request at a time,
//! answered once. BlueZ's calls into the exported `Agent1` object (`bluez`)
//! arrive as requests; the window's dialog answers them.
//!
//! The dialog speaks in tokens: a request's `kind` is `pin`, `confirm` or
//! `display`, and an answer is that kind with a value — the PIN, `yes` or
//! `no` for a passkey comparison, or `cancel` for any of them.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentRequest {
    /// The device asks for a PIN the person types.
    Pin { device: String },
    /// Both sides show a passkey; the person confirms they match.
    Confirm { device: String, passkey: u32 },
    /// The person types the shown passkey on the device.
    DisplayPasskey { device: String, passkey: u32 },
    /// BlueZ withdrew the waiting request (the device gave up, or the
    /// pairing timed out): the dialog closes. Never pending itself.
    Cancel,
}

impl AgentRequest {
    /// The dialog's look for this request, its device and the passkey to show
    /// (0 when there is none); `None` for `Cancel`, which shows nothing.
    #[must_use]
    pub fn presentation(&self) -> Option<(&'static str, &str, u32)> {
        match self {
            Self::Pin { device } => Some(("pin", device, 0)),
            Self::Confirm { device, passkey } => Some(("confirm", device, *passkey)),
            Self::DisplayPasskey { device, passkey } => Some(("display", device, *passkey)),
            Self::Cancel => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentAnswer {
    Pin(String),
    Confirmed,
    Rejected,
    Cancelled,
}

impl AgentAnswer {
    /// The dialog's answer to a request of `kind`: `cancel` cancels any
    /// request; a `pin` request is answered with its value; a `confirm`
    /// request with `yes` (the passkeys match) or `no`. Anything else
    /// cancels, so an unexpected token never pairs a device.
    #[must_use]
    pub fn from_tokens(kind: &str, value: &str) -> Self {
        match (kind, value) {
            (_, "cancel") => Self::Cancelled,
            ("pin", pin) if !pin.is_empty() => Self::Pin(pin.to_owned()),
            ("confirm", "yes") => Self::Confirmed,
            ("confirm", "no") => Self::Rejected,
            _ => Self::Cancelled,
        }
    }
}

/// A request arrived while another one waits for its answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentBusy;

#[derive(Debug, Default)]
pub struct Agent {
    pending: Option<AgentRequest>,
}

impl Agent {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Accepts a request unless one is already waiting. `Cancel` withdraws
    /// the waiting request instead and is always accepted.
    pub fn request(&mut self, r: AgentRequest) -> Result<(), AgentBusy> {
        if r == AgentRequest::Cancel {
            self.pending = None;
            return Ok(());
        }
        if self.pending.is_some() {
            return Err(AgentBusy);
        }
        self.pending = Some(r);
        Ok(())
    }

    /// Answers the waiting request, returning it with its answer; `None` when
    /// nothing was waiting.
    pub fn answer(&mut self, a: AgentAnswer) -> Option<(AgentRequest, AgentAnswer)> {
        self.pending.take().map(|r| (r, a))
    }

    #[must_use]
    pub fn pending(&self) -> Option<&AgentRequest> {
        self.pending.as_ref()
    }
}
