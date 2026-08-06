//! RAII ownership for one device entry and all session-scoped side state.

use std::sync::Arc;

use crate::artwork;
use crate::lock::LockOk;
use crate::runtime::log;
use crate::{ui_log, Daemon};

/// Keeps a live session published until its thread returns or unwinds. Cleanup
/// holds the registry exclusion token until command slots, completions and
/// artwork are gone, so a reconnect cannot be damaged by an older teardown.
pub(super) struct SessionRegistration {
    daemon: Arc<Daemon>,
    device_id: String,
    device_name: String,
}

impl SessionRegistration {
    pub(super) fn new(daemon: Arc<Daemon>, device_id: String, device_name: String) -> Self {
        Self {
            daemon,
            device_id,
            device_name,
        }
    }
}

impl Drop for SessionRegistration {
    fn drop(&mut self) {
        {
            // This lock is also the one-per-device admission token. Marking the
            // entry unpaired first makes every waiting payload gate fail, while
            // holding it prevents a new session from appearing mid-cleanup.
            let mut devices = self.daemon.devices.lock_ok();
            if let Some(entry) = devices.get_mut(&self.device_id) {
                entry.paired = false;
            }
            self.daemon.commands.lock_ok().remove(&self.device_id);
            self.daemon.pending_clipboards.clear(&self.device_id);
            self.daemon.artwork_completions.clear(&self.device_id);
            artwork::clear_device(&self.device_id);
            devices.remove(&self.device_id);
        }
        self.daemon.notifications.forget_device(&self.device_id);
        {}
        self.daemon.revocations.clear_if(&self.device_id, || {
            !self.daemon.devices.lock_ok().contains_key(&self.device_id)
        });
        self.daemon.notify_change();
        log("closed", &format!("{} disconnected", self.device_name));
        ui_log(&self.daemon, &self.device_name, "desconectado", false);
    }
}
