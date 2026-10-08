//! The Bluetooth pairing agent as a pure state machine: one request at a time,
//! answered once. The D-Bus `Agent1` object in the adapter drives it.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentRequest {
    /// The device asks for a PIN the person types.
    Pin { device: String },
    /// Both sides show a passkey; the person confirms they match.
    Confirm { device: String, passkey: u32 },
    /// The person types the shown passkey on the device.
    DisplayPasskey { device: String, passkey: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentAnswer {
    Pin(String),
    Confirmed,
    Rejected,
    Cancelled,
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

    /// Accepts a request unless one is already waiting.
    pub fn request(&mut self, r: AgentRequest) -> Result<(), AgentBusy> {
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
