//! The pairing agent: an `org.bluez.Agent1` object Cuprita exports and
//! registers as BlueZ's default agent, so the codes a pairing needs are asked
//! in Cuprita's window. Each BlueZ call becomes an `AgentRequest` sent to the
//! window; the call waits for the window's `AgentAnswer` (BlueZ itself gives
//! up after about a minute and calls `Cancel`). A code goes straight back to
//! BlueZ in the reply; nothing keeps it.

use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll, Waker};

use zbus::blocking::Connection;
use zbus::proxy::CacheProperties;
use zbus::zvariant::{ObjectPath, OwnedObjectPath};
use zbus::{interface, DBusError};

use super::{
    is_pairing, map_error, proxy, AGENT_MANAGER_PATH, BLUEZ, IFACE_AGENT_MANAGER, IFACE_DEVICE,
};
use crate::agent::{AgentAnswer, AgentRequest};
use crate::error::BluetoothError;

/// Where the agent object lives on Cuprita's connection.
pub const AGENT_PATH: &str = "/org/celestina/cuprita/agent";
/// Cuprita can show a passkey and take one typed: every pairing method works.
const CAPABILITY: &str = "KeyboardDisplay";

/// The replies BlueZ understands from an agent.
#[derive(Debug, DBusError)]
#[zbus(prefix = "org.bluez.Error")]
enum AgentError {
    #[zbus(error)]
    ZBus(zbus::Error),
    Rejected(String),
    Canceled(String),
}

fn rejected() -> AgentError {
    AgentError::Rejected("the person refused".to_owned())
}

fn canceled() -> AgentError {
    AgentError::Canceled("the request was cancelled".to_owned())
}

/// Where the window's answer meets the BlueZ call waiting for it. Only one
/// call waits at a time: a second blocking request while one waits (another
/// device asking during a pairing) is refused at once, so a request is never
/// both waiting and unshown, and an answer always reaches the request the
/// dialog shows.
#[derive(Default)]
struct Slot {
    /// The object path of the device whose request waits.
    waiting: Option<String>,
    answer: Option<AgentAnswer>,
    waker: Option<Waker>,
}

type Shared = Arc<Mutex<Slot>>;

fn lock(slot: &Shared) -> std::sync::MutexGuard<'_, Slot> {
    slot.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Hands `answer` to the waiting call; dropped when nothing waits (a late
/// answer to a request BlueZ already withdrew).
fn deliver(slot: &Shared, answer: AgentAnswer) {
    let mut slot = lock(slot);
    if slot.waiting.is_some() {
        slot.answer = Some(answer);
        if let Some(waker) = slot.waker.take() {
            waker.wake();
        }
    }
}

/// Takes the slot for `device`'s request; `false` while another one waits.
fn claim(slot: &Shared, device: &str) -> bool {
    let mut slot = lock(slot);
    if slot.waiting.is_some() {
        return false;
    }
    slot.waiting = Some(device.to_owned());
    slot.answer = None;
    slot.waker = None;
    true
}

/// Cancels the waiting request when it is `device`'s; `None` names whatever
/// waits. BlueZ's `Cancel` carries no device: it withdraws the one request
/// it has outstanding with this agent, which is the waiting one, since every
/// other blocking request was refused at once. `true` when one was cancelled.
fn cancel_waiting(slot: &Shared, device: Option<&str>) -> bool {
    let matches = {
        let slot = lock(slot);
        match (&slot.waiting, device) {
            (Some(_), None) => true,
            (Some(waiting), Some(device)) => waiting == device,
            (None, _) => false,
        }
    };
    if matches {
        deliver(slot, AgentAnswer::Cancelled);
    }
    matches
}

/// Resolves with the answer `deliver` leaves in the slot. Dropped unanswered
/// (BlueZ gave up on the call), it frees the slot.
struct Answer(Shared);

impl Drop for Answer {
    fn drop(&mut self) {
        let mut slot = lock(&self.0);
        slot.waiting = None;
        slot.answer = None;
        slot.waker = None;
    }
}

impl Future for Answer {
    type Output = AgentAnswer;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<AgentAnswer> {
        let mut slot = lock(&self.0);
        match slot.answer.take() {
            Some(answer) => {
                slot.waiting = None;
                slot.waker = None;
                Poll::Ready(answer)
            }
            None => {
                slot.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

struct AgentObject {
    requests: Sender<AgentRequest>,
    slot: Shared,
}

impl AgentObject {
    /// Sends `device`'s `request` to the window and waits for its answer;
    /// refused at once while another request waits.
    async fn ask(&self, device: &str, request: AgentRequest) -> Result<AgentAnswer, AgentError> {
        if !claim(&self.slot, device) {
            return Err(rejected());
        }
        // From here the future frees the slot however the call ends.
        let answer = Answer(Arc::clone(&self.slot));
        if self.requests.send(request).is_err() {
            // No window to ask: refuse rather than pair blind.
            return Err(rejected());
        }
        Ok(answer.await)
    }

    fn say(&self, request: AgentRequest) {
        // No window: nothing to show the passkey on; BlueZ goes on anyway.
        let _ = self.requests.send(request);
    }
}

/// The device's name as the dialog shows it: its `Alias`, else the address
/// its path carries.
async fn device_name(connection: &zbus::Connection, device: &ObjectPath<'_>) -> String {
    let alias = async {
        zbus::proxy::Builder::<zbus::Proxy<'_>>::new(connection)
            .destination(BLUEZ)?
            .path(device.clone())?
            .interface(IFACE_DEVICE)?
            .cache_properties(CacheProperties::No)
            .build()
            .await?
            .get_property::<String>("Alias")
            .await
    };
    match alias.await {
        Ok(alias) if !alias.is_empty() => alias,
        _ => address_of(device.as_str()),
    }
}

/// `…/dev_AA_BB_CC_DD_EE_FF` as `AA:BB:CC:DD:EE:FF`.
fn address_of(path: &str) -> String {
    path.rsplit_once("/dev_")
        .map_or(path, |(_, tail)| tail)
        .replace('_', ":")
}

/// Whether BlueZ may go ahead without asking: the device is paired already,
/// or it is the one the person is pairing from Cuprita right now.
async fn authorised(connection: &zbus::Connection, device: &ObjectPath<'_>) -> bool {
    if is_pairing(device.as_str()) {
        return true;
    }
    let paired = async {
        zbus::proxy::Builder::<zbus::Proxy<'_>>::new(connection)
            .destination(BLUEZ)?
            .path(device.clone())?
            .interface(IFACE_DEVICE)?
            .cache_properties(CacheProperties::No)
            .build()
            .await?
            .get_property::<bool>("Paired")
            .await
    };
    paired.await.unwrap_or(false)
}

fn passkey_of(pin: &str) -> Option<u32> {
    pin.parse::<u32>().ok().filter(|key| *key <= 999_999)
}

#[interface(name = "org.bluez.Agent1")]
impl AgentObject {
    /// BlueZ unregistered the agent (it is shutting down): whatever was
    /// shown is withdrawn.
    async fn release(&self) {
        cancel_waiting(&self.slot, None);
        self.say(AgentRequest::Cancel);
    }

    async fn request_pin_code(
        &self,
        device: OwnedObjectPath,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> Result<String, AgentError> {
        let path = device.to_string();
        let device = device_name(connection, &device).await;
        match self.ask(&path, AgentRequest::Pin { device }).await? {
            AgentAnswer::Pin(pin) => Ok(pin),
            AgentAnswer::Rejected => Err(rejected()),
            _ => Err(canceled()),
        }
    }

    /// A legacy device shows a PIN of its own to type on it.
    async fn display_pin_code(
        &self,
        device: OwnedObjectPath,
        pincode: String,
        #[zbus(connection)] connection: &zbus::Connection,
    ) {
        let device = device_name(connection, &device).await;
        let passkey = passkey_of(&pincode).unwrap_or(0);
        self.say(AgentRequest::DisplayPasskey { device, passkey });
    }

    async fn request_passkey(
        &self,
        device: OwnedObjectPath,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> Result<u32, AgentError> {
        let path = device.to_string();
        let device = device_name(connection, &device).await;
        match self.ask(&path, AgentRequest::Pin { device }).await? {
            AgentAnswer::Pin(pin) => passkey_of(&pin).ok_or_else(rejected),
            AgentAnswer::Rejected => Err(rejected()),
            _ => Err(canceled()),
        }
    }

    /// The passkey to type on the device. BlueZ calls this again as keys are
    /// typed; the window keeps showing the first.
    async fn display_passkey(
        &self,
        device: OwnedObjectPath,
        passkey: u32,
        _entered: u16,
        #[zbus(connection)] connection: &zbus::Connection,
    ) {
        let device = device_name(connection, &device).await;
        self.say(AgentRequest::DisplayPasskey { device, passkey });
    }

    async fn request_confirmation(
        &self,
        device: OwnedObjectPath,
        passkey: u32,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> Result<(), AgentError> {
        let path = device.to_string();
        let device = device_name(connection, &device).await;
        match self
            .ask(&path, AgentRequest::Confirm { device, passkey })
            .await?
        {
            AgentAnswer::Confirmed => Ok(()),
            AgentAnswer::Rejected => Err(rejected()),
            _ => Err(canceled()),
        }
    }

    /// A device asks to pair with no code at all.
    async fn request_authorization(
        &self,
        device: OwnedObjectPath,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> Result<(), AgentError> {
        if authorised(connection, &device).await {
            Ok(())
        } else {
            Err(rejected())
        }
    }

    /// A device asks to use one of its services.
    async fn authorize_service(
        &self,
        device: OwnedObjectPath,
        _uuid: String,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> Result<(), AgentError> {
        if authorised(connection, &device).await {
            Ok(())
        } else {
            Err(rejected())
        }
    }

    /// BlueZ withdrew its request: the waiting one, or a passkey shown with
    /// nothing waiting. Either way the dialog closes.
    async fn cancel(&self) {
        cancel_waiting(&self.slot, None);
        self.say(AgentRequest::Cancel);
    }
}

/// Keeps the agent exported and registered. Dropping it unregisters the agent
/// (best effort) and closes its connection.
pub struct AgentHandle {
    connection: Connection,
}

impl Drop for AgentHandle {
    fn drop(&mut self) {
        let unregistered = proxy(&self.connection, AGENT_MANAGER_PATH, IFACE_AGENT_MANAGER)
            .and_then(|manager| {
                let path = ObjectPath::from_static_str_unchecked(AGENT_PATH);
                manager
                    .call::<_, _, ()>("UnregisterAgent", &(path,))
                    .map_err(map_error)
            });
        if let Err(error) = unregistered {
            eprintln!("Cuprita: the pairing agent could not be unregistered: {error}");
        }
    }
}

/// Exports the agent at `AGENT_PATH` on a connection of its own, registers it
/// with capability `KeyboardDisplay` and asks to be BlueZ's default agent:
/// a pairing started from Cuprita's own `Pair` call reaches the default
/// agent, since that call goes out on another connection. Each BlueZ request
/// is sent through `requests`; each answer read from `answers` replies to the
/// request waiting for it. Blocking: call it on a worker thread.
pub fn serve_agent(
    requests: Sender<AgentRequest>,
    answers: Receiver<AgentAnswer>,
) -> Result<AgentHandle, BluetoothError> {
    let slot = Shared::default();
    let object = AgentObject {
        requests,
        slot: Arc::clone(&slot),
    };
    let connection = zbus::blocking::connection::Builder::system()
        .and_then(|b| b.serve_at(AGENT_PATH, object))
        .and_then(|b| b.build())
        .map_err(map_error)?;
    // From here the handle owns the connection: a failed registration below
    // unregisters (harmlessly) and closes it.
    let handle = AgentHandle { connection };
    let manager = proxy(&handle.connection, AGENT_MANAGER_PATH, IFACE_AGENT_MANAGER)?;
    let path = ObjectPath::from_static_str_unchecked(AGENT_PATH);
    manager
        .call::<_, _, ()>("RegisterAgent", &(&path, CAPABILITY))
        .map_err(map_error)?;
    manager
        .call::<_, _, ()>("RequestDefaultAgent", &(&path,))
        .map_err(map_error)?;
    drop(manager);

    std::thread::Builder::new()
        .name("cuprita-bluez-agent".to_owned())
        .spawn(move || {
            // Ends when the window drops its sender.
            for answer in answers {
                deliver(&slot, answer);
            }
        })
        .map_err(|e| BluetoothError::Failed(e.to_string()))?;
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_paths_read_as_addresses() {
        assert_eq!(
            address_of("/org/bluez/hci0/dev_AA_BB_CC_00_00_01"),
            "AA:BB:CC:00:00:01"
        );
    }

    #[test]
    fn typed_passkeys_are_six_digits_at_most() {
        assert_eq!(passkey_of("123456"), Some(123_456));
        assert_eq!(passkey_of("0"), Some(0));
        assert_eq!(passkey_of("1000000"), None);
        assert_eq!(passkey_of("12a4"), None);
    }

    #[test]
    fn an_answer_reaches_only_a_waiting_call() {
        let slot = Shared::default();
        // Nothing waits: dropped.
        deliver(&slot, AgentAnswer::Confirmed);
        assert!(lock(&slot).answer.is_none());
        assert!(claim(&slot, "/org/bluez/hci0/dev_A"));
        deliver(&slot, AgentAnswer::Rejected);
        let answer = zbus::block_on(Answer(Arc::clone(&slot)));
        assert_eq!(answer, AgentAnswer::Rejected);
        assert!(lock(&slot).waiting.is_none());
    }

    #[test]
    fn a_second_request_while_one_waits_is_refused() {
        let slot = Shared::default();
        assert!(claim(&slot, "/org/bluez/hci0/dev_A"));
        assert!(!claim(&slot, "/org/bluez/hci0/dev_B"));
        // The waiting request is still device A's, and the answer reaches it.
        deliver(&slot, AgentAnswer::Confirmed);
        assert_eq!(
            lock(&slot).waiting.as_deref(),
            Some("/org/bluez/hci0/dev_A")
        );
        assert_eq!(
            zbus::block_on(Answer(Arc::clone(&slot))),
            AgentAnswer::Confirmed
        );
        // Free again once answered.
        assert!(claim(&slot, "/org/bluez/hci0/dev_B"));
    }

    #[test]
    fn a_cancel_for_another_device_is_ignored() {
        let slot = Shared::default();
        assert!(claim(&slot, "/org/bluez/hci0/dev_A"));
        assert!(!cancel_waiting(&slot, Some("/org/bluez/hci0/dev_B")));
        assert!(lock(&slot).answer.is_none());
        assert!(cancel_waiting(&slot, Some("/org/bluez/hci0/dev_A")));
        assert_eq!(
            zbus::block_on(Answer(Arc::clone(&slot))),
            AgentAnswer::Cancelled
        );
        // Nothing waits: nothing to cancel.
        assert!(!cancel_waiting(&slot, None));
    }

    #[test]
    fn an_abandoned_call_frees_the_slot() {
        let slot = Shared::default();
        assert!(claim(&slot, "/org/bluez/hci0/dev_A"));
        drop(Answer(Arc::clone(&slot)));
        assert!(claim(&slot, "/org/bluez/hci0/dev_B"));
    }
}
