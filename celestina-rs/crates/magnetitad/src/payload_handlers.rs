//! Completion policy for payloads received off the live-link thread.
//!
//! Network transfer can outlive the packet that started it. Publication is
//! therefore serialized against `Forget` and requires the originating device
//! to remain present and paired after the bytes have been verified.

use std::fs;
use std::sync::Arc;

use celestina_core::{CancellationToken, Generation};
use magnetita_core::{IncomingAlbumArt, IncomingFile};
use magnetita_net::PayloadPermit;

use crate::artwork;
use crate::devices::{DeviceEntry, Registry};
use crate::incoming_file::{create_partial, download_dir, publish, safe_filename};
use crate::link_commands::ArtworkOutcome;
use crate::lock::LockOk;
use crate::revocation::Revocations;
use crate::runtime::log;
use crate::{notify, ui_log, Daemon};

/// Run a publication only while `Forget` has no tombstone and the live registry
/// still says this device is paired. `if_pairing_allowed` holds the revocation
/// mutex through the callback, so a concurrent Forget is ordered entirely
/// before (deny) or after (publish) this operation.
fn with_live_paired_device<T>(
    revocations: &Revocations,
    registry: &Registry,
    device_id: &str,
    pair_generation: Generation,
    publish: impl FnOnce(&mut DeviceEntry) -> T,
) -> Option<T> {
    revocations
        .if_pairing_allowed(device_id, || {
            let mut registry = registry.lock_ok();
            let entry = registry.get_mut(device_id)?;
            (entry.connected && entry.paired && entry.pair_generation == pair_generation)
                .then(|| publish(entry))
        })
        .flatten()
}

pub(super) struct PayloadPeer<'a> {
    pub(super) device_id: &'a str,
    pub(super) device_name: &'a str,
    pub(super) host: &'a str,
    pub(super) fingerprint: &'a str,
    pub(super) pair_generation: Generation,
    pub(super) cancellation: &'a CancellationToken,
}

/// Cancels every payload that belongs to one pairing generation. Re-pairing
/// creates a fresh scope; unpair, Forget and link teardown revoke the old one.
pub(super) struct PayloadScope {
    cancellation: CancellationToken,
}

impl PayloadScope {
    pub(super) fn new() -> Self {
        Self {
            cancellation: CancellationToken::new(),
        }
    }

    pub(super) fn token(&self) -> &CancellationToken {
        &self.cancellation
    }

    pub(super) fn renew(&mut self) {
        self.cancellation.cancel();
        self.cancellation = CancellationToken::new();
    }

    pub(super) fn cancel(&self) {
        self.cancellation.cancel();
    }
}

impl Drop for PayloadScope {
    fn drop(&mut self) {
        self.cancel();
    }
}

struct ArtworkJob {
    device_id: String,
    host: String,
    fingerprint: String,
    permit: PayloadPermit,
    incoming: IncomingAlbumArt,
    pair_generation: Generation,
    completion_sequence: Generation,
    cancellation: CancellationToken,
}

struct FileJob {
    device_id: String,
    device_name: String,
    host: String,
    fingerprint: String,
    permit: PayloadPermit,
    file: IncomingFile,
    pair_generation: Generation,
    cancellation: CancellationToken,
}

impl Daemon {
    pub(super) fn spawn_file_receive(self: &Arc<Self>, peer: PayloadPeer<'_>, file: IncomingFile) {
        let Some(permit) = self.payloads.try_acquire() else {
            ui_log(
                self,
                peer.device_name,
                "archivo rechazado: demasiadas transferencias activas",
                true,
            );
            return;
        };
        let job = FileJob {
            device_id: peer.device_id.to_owned(),
            device_name: peer.device_name.to_owned(),
            host: peer.host.to_owned(),
            fingerprint: peer.fingerprint.to_owned(),
            permit,
            file,
            pair_generation: peer.pair_generation,
            cancellation: peer.cancellation.clone(),
        };
        let daemon = Arc::clone(self);
        if let Err(error) = std::thread::Builder::new()
            .name("magnetita-file-receive".to_owned())
            .spawn(move || daemon.receive_file(job))
        {
            ui_log(
                self,
                peer.device_name,
                &format!("no se pudo iniciar la recepción: {error}"),
                true,
            );
        }
    }

    pub(super) fn spawn_artwork_receive(
        self: &Arc<Self>,
        peer: PayloadPeer<'_>,
        incoming: IncomingAlbumArt,
    ) -> Option<(String, String)> {
        let failed = (incoming.player.clone(), incoming.source_url.clone());
        let Some(permit) = self.payloads.try_acquire() else {
            return Some(failed);
        };
        let completion_sequence = match self.next_generation() {
            Ok(generation) => generation,
            Err(error) => {
                log("artwork", &format!("{}: {error}", peer.device_id));
                return Some(failed);
            }
        };
        let job = ArtworkJob {
            device_id: peer.device_id.to_owned(),
            host: peer.host.to_owned(),
            fingerprint: peer.fingerprint.to_owned(),
            permit,
            incoming,
            pair_generation: peer.pair_generation,
            completion_sequence,
            cancellation: peer.cancellation.clone(),
        };
        let daemon = Arc::clone(self);
        match std::thread::Builder::new()
            .name("magnetita-artwork-receive".to_owned())
            .spawn(move || daemon.receive_artwork(job))
        {
            Ok(_) => None,
            Err(error) => {
                log("artwork", &format!("{}: {error}", peer.device_id));
                Some(failed)
            }
        }
    }

    fn acknowledge_artwork(
        &self,
        device_id: &str,
        pair_generation: Generation,
        sequence: Generation,
        player: String,
        source_url: String,
        outcome: ArtworkOutcome,
    ) {
        // Keep registry -> completion-slot order through the decision. Teardown
        // removes the registry entry before clearing the slot, so an old worker
        // can neither resurrect a disconnected id nor target a new pairing.
        let registry = self.devices.lock_ok();
        let current = registry.get(device_id).is_some_and(|entry| {
            entry.connected && entry.paired && entry.pair_generation == pair_generation
        });
        if current {
            self.artwork_completions.replace(
                device_id,
                sequence,
                pair_generation,
                player,
                source_url,
                outcome,
            );
        }
    }

    /// Receive and publish a cover off the pump thread. A response for a track
    /// that has already changed is discarded instead of replacing current art.
    fn receive_artwork(self: Arc<Self>, job: ArtworkJob) {
        let ArtworkJob {
            device_id,
            host,
            fingerprint,
            permit,
            incoming,
            pair_generation,
            completion_sequence,
            cancellation,
        } = job;
        let path = match artwork::receive(
            &device_id,
            &host,
            &self.tls,
            &fingerprint,
            permit,
            &incoming,
            cancellation,
        ) {
            Ok(path) => path,
            Err(error) => {
                log("artwork", &format!("{device_id}: {error}"));
                self.acknowledge_artwork(
                    &device_id,
                    pair_generation,
                    completion_sequence,
                    incoming.player,
                    incoming.source_url,
                    ArtworkOutcome::Failed,
                );
                return;
            }
        };

        match with_live_paired_device(
            &self.revocations,
            &self.devices,
            &device_id,
            pair_generation,
            |entry| artwork::publish_received(entry, &path, &incoming),
        ) {
            Some(true) => {
                self.notify_change();
                self.acknowledge_artwork(
                    &device_id,
                    pair_generation,
                    completion_sequence,
                    incoming.player,
                    incoming.source_url,
                    ArtworkOutcome::Installed,
                );
            }
            Some(false) => {}
            None => {
                artwork::discard(&path);
                log(
                    "artwork",
                    &format!("{device_id}: discarded after pairing was revoked"),
                );
            }
        }
    }

    /// Receive a shared file into the downloads dir, then notify and log. Runs on
    /// its own thread. The name is reduced to its base component so the phone
    /// cannot write outside the target dir, and a half-received file is removed.
    fn receive_file(self: Arc<Self>, job: FileJob) {
        let FileJob {
            device_id,
            device_name,
            host,
            fingerprint,
            permit,
            file,
            pair_generation,
            cancellation,
        } = job;
        let name = safe_filename(&file.filename);
        let dir = download_dir();
        if fs::create_dir_all(&dir).is_err() {
            return;
        }
        let (partial, destination) = match create_partial(&dir) {
            Ok(reserved) => reserved,
            Err(error) => {
                ui_log(
                    &self,
                    &device_name,
                    &format!("no se pudo reservar {name}: {error}"),
                    true,
                );
                return;
            }
        };
        ui_log(&self, &device_name, &format!("recibiendo {name}…"), false);
        match magnetita_net::receive_to_file(
            magnetita_net::PayloadSource {
                host: &host,
                port: file.port,
                size: file.size,
                expected_peer_fingerprint: &fingerprint,
            },
            &self.tls,
            &cancellation,
            permit,
            destination,
        ) {
            Ok(_) => {
                let published = with_live_paired_device(
                    &self.revocations,
                    &self.devices,
                    &device_id,
                    pair_generation,
                    |_| publish(&partial, &dir, &name),
                );
                match published {
                    Some(Ok(path)) => {
                        let final_name = path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or(&name);
                        ui_log(
                            &self,
                            &device_name,
                            &format!("recibido: {final_name}"),
                            false,
                        );
                        if let Some(connection) = &self.dbus {
                            notify::post(
                                connection,
                                &device_name,
                                0,
                                "Archivo recibido",
                                final_name,
                            );
                        }
                    }
                    Some(Err(error)) => {
                        let _ = fs::remove_file(&partial);
                        ui_log(
                            &self,
                            &device_name,
                            &format!("fallo al publicar {name}: {error}"),
                            true,
                        );
                    }
                    None => {
                        let _ = fs::remove_file(&partial);
                        log(
                            "share",
                            &format!("{device_id}: discarded {name} after pairing was revoked"),
                        );
                    }
                }
            }
            Err(error) => {
                ui_log(
                    &self,
                    &device_name,
                    &format!("fallo al recibir {name}: {error}"),
                    true,
                );
                let _ = fs::remove_file(&partial);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use celestina_core::{Generation, GenerationClock};

    use crate::devices::{DeviceEntry, Registry};
    use crate::revocation::Revocations;

    use super::with_live_paired_device;

    fn registry(paired: bool) -> Registry {
        let mut devices = BTreeMap::new();
        let mut phone = DeviceEntry::connected(
            "phone".to_owned(),
            "Phone".to_owned(),
            "phone".to_owned(),
            "fingerprint".to_owned(),
        );
        phone.paired = paired;
        devices.insert("phone".to_owned(), phone);
        Arc::new(Mutex::new(devices))
    }

    #[test]
    fn publication_requires_a_live_paired_device_without_a_tombstone() {
        let revocations = Revocations::new();
        let paired = registry(true);
        assert_eq!(
            with_live_paired_device(&revocations, &paired, "phone", Generation::INITIAL, |_| 7),
            Some(7)
        );
        assert_eq!(
            with_live_paired_device(
                &revocations,
                &registry(false),
                "phone",
                Generation::INITIAL,
                |_| 8
            ),
            None
        );

        revocations.request("phone").unwrap();
        assert_eq!(
            with_live_paired_device(&revocations, &paired, "phone", Generation::INITIAL, |_| 9),
            None
        );
    }

    #[test]
    fn publication_rejects_a_previous_pairing_generation() {
        let revocations = Revocations::new();
        let paired = registry(true);
        let mut clock = GenerationClock::default();
        let stale = clock.issue().unwrap();

        assert_eq!(
            with_live_paired_device(&revocations, &paired, "phone", stale, |_| 7),
            None
        );
    }
}
