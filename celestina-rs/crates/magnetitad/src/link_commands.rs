//! Local app commands executed by the thread that owns one live device link.

use std::collections::HashMap;
use std::error::Error;
use std::sync::{mpsc, Mutex};

use celestina_core::CancellationToken;
use celestina_core::Generation;
use magnetita_core::ConnectionEvent;
use magnetita_net::Device;

use crate::devices::Command;
use crate::lock::LockOk;
use crate::remote_media::RemoteMedia;
use crate::runtime::millis;
use crate::settings::Settings;
use crate::{ui_log, Daemon};

/// One coalescing clipboard slot per live device. The watcher may outpace a
/// link, but memory stays bounded and the next send always carries the newest
/// value instead of an arbitrary earlier queue entry.
#[derive(Default)]
pub(super) struct PendingClipboards {
    values: Mutex<HashMap<String, String>>,
}

impl PendingClipboards {
    pub(super) fn replace_for(&self, device_ids: impl IntoIterator<Item = String>, text: String) {
        let mut values = self.values.lock_ok();
        for device_id in device_ids {
            values.insert(device_id, text.clone());
        }
    }

    fn take(&self, device_id: &str) -> Option<String> {
        self.values.lock_ok().remove(device_id)
    }

    pub(super) fn clear(&self, device_id: &str) {
        self.values.lock_ok().remove(device_id);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ArtworkOutcome {
    Failed,
    Installed,
}

#[derive(Debug)]
struct ArtworkCompletion {
    sequence: Generation,
    pair_generation: Generation,
    player: String,
    source_url: String,
    outcome: ArtworkOutcome,
}

/// One newest artwork completion per live device. Payload workers never block
/// on the command queue, and an older transfer finishing late cannot overwrite
/// the completion of a newer request.
#[derive(Default)]
pub(super) struct PendingArtworkCompletions {
    values: Mutex<HashMap<String, ArtworkCompletion>>,
}

impl PendingArtworkCompletions {
    pub(super) fn replace(
        &self,
        device_id: &str,
        sequence: Generation,
        pair_generation: Generation,
        player: String,
        source_url: String,
        outcome: ArtworkOutcome,
    ) {
        let mut values = self.values.lock_ok();
        if values
            .get(device_id)
            .is_some_and(|current| current.sequence > sequence)
        {
            return;
        }
        values.insert(
            device_id.to_owned(),
            ArtworkCompletion {
                sequence,
                pair_generation,
                player,
                source_url,
                outcome,
            },
        );
    }

    fn take(&self, device_id: &str, pair_generation: Generation) -> Option<ArtworkCompletion> {
        self.values
            .lock_ok()
            .remove(device_id)
            .filter(|completion| completion.pair_generation == pair_generation)
    }

    pub(super) fn clear(&self, device_id: &str) {
        self.values.lock_ok().remove(device_id);
    }
}

pub(super) struct PeerContext<'a> {
    pub(super) id: &'a str,
    pub(super) name: &'a str,
    pub(super) fingerprint: &'a str,
    pub(super) pair_generation: Generation,
    pub(super) cancellation: &'a CancellationToken,
}

impl<'a> PeerContext<'a> {
    pub(super) fn new(
        id: &'a str,
        name: &'a str,
        fingerprint: &'a str,
        pair_generation: Generation,
        cancellation: &'a CancellationToken,
    ) -> Self {
        Self {
            id,
            name,
            fingerprint,
            pair_generation,
            cancellation,
        }
    }
}

impl Daemon {
    pub(super) fn drain_link_commands(
        &self,
        commands: &mpsc::Receiver<Command>,
        device: &mut Device,
        remote_media: &mut RemoteMedia,
        settings: Settings,
        peer: PeerContext<'_>,
    ) -> Result<Vec<ConnectionEvent>, Box<dyn Error>> {
        let mut events = Vec::new();
        if let Some(completion) = self.artwork_completions.take(peer.id, peer.pair_generation) {
            if settings.media && self.revocations.current(peer.id).is_none() {
                match completion.outcome {
                    ArtworkOutcome::Failed => remote_media.artwork_failed(
                        &completion.player,
                        &completion.source_url,
                        millis(),
                    ),
                    ArtworkOutcome::Installed => {
                        remote_media.artwork_succeeded(&completion.player, &completion.source_url)
                    }
                }
            }
        }
        while let Ok(command) = commands.try_recv() {
            match command {
                Command::RequestPair { observed }
                    if self.revocations.authorize_pair(peer.id, observed) =>
                {
                    events.extend(device.request_pairing()?)
                }
                Command::RequestPair { .. } => {}
                Command::Ring if device.is_paired() && settings.findmyphone => {
                    device.send(magnetita_core::findmyphone::request)?;
                    ui_log(self, peer.name, "sonando el móvil", false);
                }
                Command::Ring => {}
                Command::Media(action) if device.is_paired() && settings.media => {
                    remote_media.send_action(device, action)?;
                }
                Command::Media(_) => {}
                Command::SendFile(_) if !device.is_paired() || !settings.share => {}
                Command::SendFile(path) => {
                    let name = path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("archivo")
                        .to_owned();
                    let Some(permit) = self.payloads.try_acquire() else {
                        ui_log(
                            self,
                            peer.name,
                            "demasiadas transferencias activas; inténtalo de nuevo",
                            true,
                        );
                        continue;
                    };
                    match magnetita_net::serve_file(
                        &self.tls,
                        &path,
                        peer.fingerprint,
                        peer.cancellation,
                        permit,
                    ) {
                        Ok(payload) => {
                            device.send(|id| {
                                magnetita_core::share_request_packet(
                                    id,
                                    &name,
                                    payload.size,
                                    payload.port,
                                )
                            })?;
                            ui_log(self, peer.name, &format!("enviando {name}…"), false);
                        }
                        Err(error) => {
                            let message = if error.kind() == std::io::ErrorKind::InvalidInput {
                                format!("no se pudo enviar {name}: {error}")
                            } else {
                                format!("no se pudo leer {}: {error}", path.display())
                            };
                            ui_log(self, peer.name, &message, true);
                        }
                    }
                }
            }
        }
        if !settings.clipboard {
            self.pending_clipboards.clear(peer.id);
        } else if device.is_paired() && self.revocations.current(peer.id).is_none() {
            if let Some(text) = self.pending_clipboards.take(peer.id) {
                device.send(|id| magnetita_core::clipboard::clipboard_packet(id, &text))?;
            }
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use celestina_core::GenerationClock;

    use super::{ArtworkOutcome, PendingArtworkCompletions, PendingClipboards};

    #[test]
    fn clipboard_slot_keeps_only_the_newest_value_per_device() {
        let pending = PendingClipboards::default();
        pending.replace_for(["phone".to_owned()], "old".to_owned());
        pending.replace_for(["phone".to_owned()], "new".to_owned());

        assert_eq!(pending.take("phone").as_deref(), Some("new"));
        assert!(pending.take("phone").is_none());
    }

    #[test]
    fn clearing_one_device_preserves_another_slot() {
        let pending = PendingClipboards::default();
        pending.replace_for(
            ["phone".to_owned(), "tablet".to_owned()],
            "value".to_owned(),
        );

        pending.clear("phone");
        assert!(pending.take("phone").is_none());
        assert_eq!(pending.take("tablet").as_deref(), Some("value"));
    }

    #[test]
    fn newer_artwork_completion_cannot_be_overwritten_by_a_late_old_one() {
        let pending = PendingArtworkCompletions::default();
        let mut clock = GenerationClock::default();
        let pair = clock.issue().unwrap();
        let old = clock.issue().unwrap();
        let new = clock.issue().unwrap();
        pending.replace(
            "phone",
            new,
            pair,
            "new-player".to_owned(),
            "new-source".to_owned(),
            ArtworkOutcome::Installed,
        );
        pending.replace(
            "phone",
            old,
            pair,
            "old-player".to_owned(),
            "old-source".to_owned(),
            ArtworkOutcome::Failed,
        );

        let completion = pending.take("phone", pair).unwrap();
        assert_eq!(completion.player, "new-player");
        assert_eq!(completion.outcome, ArtworkOutcome::Installed);
    }

    #[test]
    fn artwork_completion_from_an_old_pairing_is_discarded() {
        let pending = PendingArtworkCompletions::default();
        let mut clock = GenerationClock::default();
        let old_pair = clock.issue().unwrap();
        let sequence = clock.issue().unwrap();
        let new_pair = clock.issue().unwrap();
        pending.replace(
            "phone",
            sequence,
            old_pair,
            "player".to_owned(),
            "source".to_owned(),
            ArtworkOutcome::Installed,
        );

        assert!(pending.take("phone", new_pair).is_none());
        assert!(pending.take("phone", old_pair).is_none());
    }
}
