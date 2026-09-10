//! The UniFFI face: one object per Rust object, blocking calls Kotlin runs
//! off its main thread, and records instead of tuples.
//!
//! The runtime lives inside [`MobilePhone`], so a Kotlin caller never sees
//! tokio. Every method blocks on that runtime; the application calls them
//! from its foreground service's coroutine, not from the UI thread.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use magnetita_link::trust::fingerprint_text;

use crate::phone::{clipboard_text, describe, notification_fields, Phone, PhoneSession};
use magnetita_proto::daily::notifications::{Action, NotificationPosted};

/// Why a call failed, as Kotlin sees it.
#[derive(Debug, uniffi::Error)]
#[uniffi(flat_error)]
pub enum MobileError {
    Link(String),
}

impl std::fmt::Display for MobileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Link(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for MobileError {}

impl From<magnetita_link::LinkError> for MobileError {
    fn from(e: magnetita_link::LinkError) -> Self {
        Self::Link(e.to_string())
    }
}

/// Who this phone is.
#[derive(uniffi::Record)]
pub struct Identity {
    pub device_id: String,
    pub name: String,
    /// Colon-separated hex SHA-256 of the certificate.
    pub fingerprint: String,
}

/// A desktop this phone has pinned.
#[derive(uniffi::Record)]
pub struct PinnedDesktop {
    pub device_id: String,
    pub name: String,
    pub fingerprint: String,
}

/// One envelope, described for a log and carried for a handler.
#[derive(uniffi::Record)]
pub struct Event {
    pub capability: u16,
    pub kind: u16,
    pub description: String,
    pub body: Vec<u8>,
    /// The clipboard text, or a notification reply's, decoded here so
    /// Kotlin never reads the wire.
    pub text: Option<String>,
    /// The phone's notification key a desktop message names.
    pub key: Option<String>,
    /// The button index of a notification action.
    pub action: Option<u16>,
}

/// One of the phone's notifications, as the listener sees it.
#[derive(uniffi::Record)]
pub struct MobileNotification {
    pub key: String,
    pub app_name: String,
    pub title: String,
    pub body: String,
    pub timestamp_ms: u64,
    pub replyable: bool,
    pub actions: Vec<String>,
    /// PNG bytes, absent when none or unchanged.
    pub icon: Option<Vec<u8>>,
}

/// The phone, held by the application's service for its lifetime.
#[derive(uniffi::Object)]
pub struct MobilePhone {
    runtime: tokio::runtime::Runtime,
    phone: Phone,
}

#[uniffi::export]
impl MobilePhone {
    /// Opens the identity under `dir` (the app's files directory).
    #[uniffi::constructor]
    pub fn open(dir: String, name: String) -> Result<Arc<Self>, MobileError> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| MobileError::Link(e.to_string()))?;
        let phone = runtime.block_on(async { Phone::open(&PathBuf::from(dir), &name) })?;
        Ok(Arc::new(Self { runtime, phone }))
    }

    pub fn identity(&self) -> Identity {
        Identity {
            device_id: self.phone.device_id.clone(),
            name: self.phone.name.clone(),
            fingerprint: fingerprint_text(&self.phone.fingerprint),
        }
    }

    pub fn pinned(&self) -> Vec<PinnedDesktop> {
        self.phone
            .pinned()
            .into_iter()
            .map(|p| PinnedDesktop {
                device_id: p.device_id,
                name: p.device_name,
                fingerprint: p.fingerprint,
            })
            .collect()
    }

    pub fn forget(&self, device_id: String) -> Result<(), MobileError> {
        Ok(self.phone.forget(&device_id)?)
    }

    /// Pairs with the desktop a scanned QR names; the session stays open.
    pub fn pair(&self, uri: String) -> Result<Arc<MobileSession>, MobileError> {
        let (_, session) = self.runtime.block_on(self.phone.pair(&uri))?;
        Ok(Arc::new(MobileSession {
            handle: self.runtime.handle().clone(),
            inner: session,
        }))
    }

    /// Connects to a pinned desktop at `address` (`ip:port`).
    pub fn connect(&self, address: String) -> Result<Arc<MobileSession>, MobileError> {
        let address = address
            .parse()
            .map_err(|_| MobileError::Link("bad address".into()))?;
        let session = self.runtime.block_on(self.phone.connect(address))?;
        Ok(Arc::new(MobileSession {
            handle: self.runtime.handle().clone(),
            inner: session,
        }))
    }
}

/// One open session with a desktop.
#[derive(uniffi::Object)]
pub struct MobileSession {
    handle: tokio::runtime::Handle,
    inner: PhoneSession,
}

#[uniffi::export]
impl MobileSession {
    pub fn desktop_id(&self) -> String {
        self.inner.desktop.device_id.clone()
    }

    pub fn desktop_name(&self) -> String {
        self.inner.desktop.device_name.clone()
    }

    pub fn report_battery(&self, level: u8, charging: bool) -> Result<(), MobileError> {
        Ok(self
            .handle
            .block_on(self.inner.report_battery(level, charging))?)
    }

    /// Sends the phone's clipboard text.
    pub fn send_clipboard(&self, text: String) -> Result<(), MobileError> {
        Ok(self.handle.block_on(self.inner.send_clipboard(&text))?)
    }

    /// A notification appeared or changed on the phone.
    pub fn send_notification(&self, note: MobileNotification) -> Result<(), MobileError> {
        let posted = NotificationPosted {
            key: note.key,
            app_name: note.app_name,
            title: note.title,
            body: note.body,
            timestamp_ms: note.timestamp_ms,
            replyable: note.replyable,
            actions: note
                .actions
                .into_iter()
                .map(|label| Action { label })
                .collect(),
            icon: note.icon,
        };
        Ok(self
            .handle
            .block_on(self.inner.send_notification(&posted))?)
    }

    /// A notification left the phone.
    pub fn send_notification_gone(&self, key: String) -> Result<(), MobileError> {
        Ok(self
            .handle
            .block_on(self.inner.send_notification_gone(&key))?)
    }

    /// Blocks up to `timeout_ms` for the next envelope; `None` on timeout.
    pub fn next(&self, timeout_ms: u64) -> Result<Option<Event>, MobileError> {
        let env = self
            .handle
            .block_on(self.inner.next(Duration::from_millis(timeout_ms)))?;
        Ok(env.map(|e| {
            let (key, action, reply) = notification_fields(&e);
            Event {
                capability: e.capability,
                kind: e.kind,
                description: describe(&e),
                text: clipboard_text(&e).or(reply),
                key,
                action,
                body: e.body,
            }
        }))
    }

    pub fn close(&self, reason: String) {
        self.inner.close(&reason);
    }
}
