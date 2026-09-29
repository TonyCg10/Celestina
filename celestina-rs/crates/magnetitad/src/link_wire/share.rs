//! Files on the own wire: the offer, the answer and the end travel on the
//! control stream; the bytes travel on their own unidirectional stream
//! named by the transfer id, and a transfer that breaks resumes from the
//! bytes the receiver already holds.
//!
//! Receiving keeps the daemon's rules for the KDE Connect wire: a partial
//! file hidden in the downloads directory until every byte arrived, the
//! payload limiter's permit for the whole transfer, and publication only
//! while the device is still paired. Partials survive a session so the
//! next offer of the same name and size resumes.

use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use magnetita_link::{RecvStream, Transfers};
use magnetita_net::PayloadPermit;
use magnetita_proto::capability;
use magnetita_proto::daily::share::{ShareAccept, ShareDone, ShareOffer, ShareReject};
use magnetita_proto::Envelope;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc::UnboundedSender;

use crate::incoming_file;
use crate::lock::LockOk;
use crate::runtime::log;
use crate::{ui_log, Daemon};

/// One read or write on a bulk stream.
const CHUNK: usize = 64 * 1024;
/// The largest file either side may offer: the payload limit the link
/// shares. An offer past it is refused before a partial or a permit exists.
const MAX_OFFER: u64 = magnetita_net::MAX_PAYLOAD_SIZE.unsigned_abs();
/// How long an offer may wait for the other side: the phone to answer the
/// desktop's offer, or to open the stream of one the desktop accepted. Past
/// it the offer is dropped with its payload permit, so an unanswered offer
/// cannot hold one of the few transfer slots for the rest of the session.
const OFFER_TTL: Duration = Duration::from_secs(5 * 60);

/// Partials that outlive a session: (device id, name, size) → the hidden
/// file holding the bytes received so far. Bounded by the limiter while
/// receiving; between sessions a few entries per device at most.
#[derive(Default)]
pub(crate) struct ShareStore {
    partials: Mutex<HashMap<(String, String, u64), PathBuf>>,
}

/// A file the desktop offered and the phone has not answered yet.
struct Outgoing {
    path: PathBuf,
    name: String,
    size: u64,
    _permit: PayloadPermit,
}

/// A file the phone offered and the desktop accepted, awaiting its stream.
struct Incoming {
    name: String,
    size: u64,
    partial: PathBuf,
    offset: u64,
    permit: PayloadPermit,
}

/// Offers waiting on the other side, by transfer id, with when each began
/// to wait.
struct Waiting<T> {
    entries: HashMap<u32, (Instant, T)>,
}

impl<T> Default for Waiting<T> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}

impl<T> Waiting<T> {
    fn insert(&mut self, transfer: u32, value: T, now: Instant) {
        self.entries.insert(transfer, (now, value));
    }

    fn take(&mut self, transfer: u32) -> Option<T> {
        self.entries.remove(&transfer).map(|(_, value)| value)
    }

    /// Removes and returns every offer that has waited [`OFFER_TTL`] by `now`.
    fn expire(&mut self, now: Instant) -> Vec<(u32, T)> {
        let stale: Vec<u32> = self
            .entries
            .iter()
            .filter(|(_, (since, _))| now.saturating_duration_since(*since) >= OFFER_TTL)
            .map(|(transfer, _)| *transfer)
            .collect();
        stale
            .into_iter()
            .filter_map(|transfer| self.take(transfer).map(|value| (transfer, value)))
            .collect()
    }
}

/// The share state of one session: what is offered, what is accepted, and
/// the outbox the transfer tasks answer through.
pub(crate) struct SessionShare {
    daemon: Arc<Daemon>,
    device_id: String,
    device_name: String,
    transfers: Transfers,
    store: Arc<ShareStore>,
    download_dir: PathBuf,
    outbox: UnboundedSender<Envelope>,
    next_transfer: AtomicU32,
    outgoing: Mutex<Waiting<Outgoing>>,
    incoming: Mutex<Waiting<Incoming>>,
}

impl SessionShare {
    pub(crate) fn new(
        daemon: Arc<Daemon>,
        device_id: &str,
        device_name: &str,
        transfers: Transfers,
        store: Arc<ShareStore>,
        download_dir: PathBuf,
        outbox: UnboundedSender<Envelope>,
    ) -> Arc<Self> {
        Arc::new(Self {
            daemon,
            device_id: device_id.to_owned(),
            device_name: device_name.to_owned(),
            transfers,
            store,
            download_dir,
            outbox,
            next_transfer: AtomicU32::new(1),
            outgoing: Mutex::new(Waiting::default()),
            incoming: Mutex::new(Waiting::default()),
        })
    }

    fn envelope(capability: u16, kind: u16, body: Vec<u8>) -> Envelope {
        Envelope {
            capability,
            kind,
            id: 0,
            body,
        }
    }

    /// The desktop offers `path`: the envelope to send, or why not.
    pub(crate) fn offer(&self, path: PathBuf) -> Result<Envelope, String> {
        let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        if !meta.is_file() {
            return Err("not a regular file".into());
        }
        if meta.len() > MAX_OFFER {
            return Err("larger than any transfer may be".into());
        }
        let name = incoming_file::safe_filename(&path.to_string_lossy());
        let Some(permit) = self.daemon.payloads.try_acquire() else {
            return Err("too many transfers active".into());
        };
        let transfer = self.next_transfer.fetch_add(1, Ordering::Relaxed);
        self.outgoing.lock_ok().insert(
            transfer,
            Outgoing {
                path,
                name: name.clone(),
                size: meta.len(),
                _permit: permit,
            },
            Instant::now(),
        );
        Ok(Self::envelope(
            capability::SHARE,
            ShareOffer::KIND,
            ShareOffer {
                transfer,
                name,
                size: meta.len(),
                mime: String::new(),
            }
            .encode(),
        ))
    }

    /// A share envelope from the phone; the reply to send, if any.
    pub(crate) fn handle(self: &Arc<Self>, env: &Envelope) -> Option<Envelope> {
        match env.kind {
            ShareOffer::KIND => {
                let offer = match ShareOffer::decode(&env.body) {
                    Ok(o) => o,
                    Err(e) => {
                        log("link", &format!("{}: share offer: {e}", self.device_name));
                        return None;
                    }
                };
                Some(self.answer_offer(offer))
            }
            ShareAccept::KIND => {
                let accept = ShareAccept::decode(&env.body).ok()?;
                let job = self.outgoing.lock_ok().take(accept.transfer)?;
                self.spawn_send(accept.transfer, job, accept.offset);
                None
            }
            ShareReject::KIND => {
                let reject = ShareReject::decode(&env.body).ok()?;
                let job = self.outgoing.lock_ok().take(reject.transfer);
                if let Some(job) = job {
                    ui_log(
                        &self.daemon,
                        &self.device_name,
                        &format!("archivo rechazado por el m\u{f3}vil: {}", job.name),
                        true,
                    );
                }
                None
            }
            ShareDone::KIND => {
                if let Ok(done) = ShareDone::decode(&env.body) {
                    log(
                        "link",
                        &format!(
                            "{}: transfer {} {}",
                            self.device_name,
                            done.transfer,
                            if done.complete {
                                "complete"
                            } else {
                                "abandoned"
                            }
                        ),
                    );
                }
                None
            }
            _ => None,
        }
    }

    fn answer_offer(self: &Arc<Self>, offer: ShareOffer) -> Envelope {
        let reject = Self::envelope(
            capability::SHARE,
            ShareReject::KIND,
            ShareReject {
                transfer: offer.transfer,
            }
            .encode(),
        );
        if !self.daemon.settings.lock_ok().share {
            return reject;
        }
        if offer.size > MAX_OFFER {
            ui_log(
                &self.daemon,
                &self.device_name,
                "archivo rechazado: demasiado grande",
                true,
            );
            return reject;
        }
        let Some(permit) = self.daemon.payloads.try_acquire() else {
            ui_log(
                &self.daemon,
                &self.device_name,
                "archivo rechazado: demasiadas transferencias activas",
                true,
            );
            return reject;
        };
        let name = incoming_file::safe_filename(&offer.name);
        let key = (self.device_id.clone(), name.clone(), offer.size);
        let held = self
            .store
            .partials
            .lock_ok()
            .get(&key)
            .cloned()
            .filter(|p| p.is_file());
        let (partial, offset) = match held {
            Some(partial) => {
                let offset = std::fs::metadata(&partial).map(|m| m.len()).unwrap_or(0);
                (partial, offset.min(offer.size))
            }
            None => {
                if std::fs::create_dir_all(&self.download_dir).is_err() {
                    return reject;
                }
                match incoming_file::create_partial(&self.download_dir) {
                    Ok((path, _file)) => {
                        self.store.partials.lock_ok().insert(key, path.clone());
                        (path, 0)
                    }
                    Err(e) => {
                        ui_log(
                            &self.daemon,
                            &self.device_name,
                            &format!("no se pudo preparar la descarga: {e}"),
                            true,
                        );
                        return reject;
                    }
                }
            }
        };
        self.incoming.lock_ok().insert(
            offer.transfer,
            Incoming {
                name,
                size: offer.size,
                partial,
                offset,
                permit,
            },
            Instant::now(),
        );
        Self::envelope(
            capability::SHARE,
            ShareAccept::KIND,
            ShareAccept {
                transfer: offer.transfer,
                offset,
            }
            .encode(),
        )
    }

    /// A bulk stream arrived: the accepted transfer it names starts receiving.
    pub(crate) fn stream_arrived(self: &Arc<Self>, transfer: u32, stream: RecvStream) {
        let job = self.incoming.lock_ok().take(transfer);
        let Some(job) = job else {
            log(
                "link",
                &format!(
                    "{}: stream for unknown transfer {transfer}",
                    self.device_name
                ),
            );
            return;
        };
        let this = Arc::clone(self);
        tokio::spawn(async move { this.receive(transfer, job, stream).await });
    }

    /// Drops every offer that has waited [`OFFER_TTL`] by `now`, and with it
    /// its payload permit. A partial stays in the store, so the same file
    /// offered again resumes. Called from the session's tick.
    pub(crate) fn expire(&self, now: Instant) {
        let unanswered = self.outgoing.lock_ok().expire(now);
        for (transfer, job) in unanswered {
            log(
                "link",
                &format!(
                    "{}: offer {transfer} of {} unanswered; dropped",
                    self.device_name, job.name
                ),
            );
            ui_log(
                &self.daemon,
                &self.device_name,
                &format!(
                    "el m\u{f3}vil no respondi\u{f3} al env\u{ed}o: {}",
                    job.name
                ),
                true,
            );
        }
        let unstarted = self.incoming.lock_ok().expire(now);
        for (transfer, job) in unstarted {
            log(
                "link",
                &format!(
                    "{}: transfer {transfer} of {} never started; dropped",
                    self.device_name, job.name
                ),
            );
        }
    }

    async fn receive(self: Arc<Self>, transfer: u32, job: Incoming, mut stream: RecvStream) {
        let complete = match self.pull(&job, &mut stream).await {
            Ok(complete) => complete,
            Err(e) => {
                log(
                    "link",
                    &format!("{}: receive {}: {e}", self.device_name, job.name),
                );
                false
            }
        };
        let key = (self.device_id.clone(), job.name.clone(), job.size);
        if complete {
            let published = self
                .daemon
                .revocations
                .if_pairing_allowed(&self.device_id, || {
                    let paired = self
                        .daemon
                        .devices
                        .lock_ok()
                        .get(&self.device_id)
                        .is_some_and(|e| e.connected && e.paired);
                    paired.then(|| {
                        incoming_file::publish(&job.partial, &self.download_dir, &job.name)
                    })
                });
            match published.flatten() {
                Some(Ok(path)) => {
                    self.store.partials.lock_ok().remove(&key);
                    ui_log(
                        &self.daemon,
                        &self.device_name,
                        &format!("archivo recibido: {}", path.display()),
                        false,
                    );
                }
                Some(Err(e)) => {
                    ui_log(
                        &self.daemon,
                        &self.device_name,
                        &format!("no se pudo guardar {}: {e}", job.name),
                        true,
                    );
                }
                None => {
                    let _ = std::fs::remove_file(&job.partial);
                    self.store.partials.lock_ok().remove(&key);
                    log(
                        "link",
                        &format!(
                            "{}: {} dropped: no longer paired",
                            self.device_name, job.name
                        ),
                    );
                }
            }
        } else {
            ui_log(
                &self.daemon,
                &self.device_name,
                &format!(
                    "transferencia interrumpida: {}; se reanudar\u{e1}",
                    job.name
                ),
                true,
            );
        }
        drop(job.permit);
        let _ = self.outbox.send(Self::envelope(
            capability::SHARE,
            ShareDone::KIND,
            ShareDone { transfer, complete }.encode(),
        ));
    }

    /// Appends the stream to the partial from `offset`; true when the
    /// announced size was reached.
    async fn pull(&self, job: &Incoming, stream: &mut RecvStream) -> std::io::Result<bool> {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(&job.partial)
            .await?;
        file.set_len(job.offset).await?;
        file.seek(SeekFrom::Start(job.offset)).await?;
        let mut written = job.offset;
        while written < job.size {
            let want = CHUNK.min((job.size - written) as usize);
            let Some(chunk) = stream
                .read_chunk(want, true)
                .await
                .map_err(|e| std::io::Error::other(e.to_string()))?
            else {
                break;
            };
            file.write_all(&chunk.bytes).await?;
            written += chunk.bytes.len() as u64;
        }
        file.sync_all().await?;
        Ok(written == job.size)
    }

    fn spawn_send(self: &Arc<Self>, transfer: u32, job: Outgoing, offset: u64) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let complete = match this.push(&job, offset, transfer).await {
                Ok(()) => true,
                Err(e) => {
                    log(
                        "link",
                        &format!("{}: send {}: {e}", this.device_name, job.name),
                    );
                    false
                }
            };
            ui_log(
                &this.daemon,
                &this.device_name,
                &if complete {
                    format!("archivo enviado: {}", job.name)
                } else {
                    format!("env\u{ed}o interrumpido: {}", job.name)
                },
                !complete,
            );
            let _ = this.outbox.send(Self::envelope(
                capability::SHARE,
                ShareDone::KIND,
                ShareDone { transfer, complete }.encode(),
            ));
        });
    }

    async fn push(&self, job: &Outgoing, offset: u64, transfer: u32) -> std::io::Result<()> {
        let mut file = tokio::fs::File::open(&job.path).await?;
        file.seek(SeekFrom::Start(offset.min(job.size))).await?;
        let mut stream = self
            .transfers
            .open(transfer)
            .await
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut buf = vec![0u8; CHUNK];
        let mut sent = offset;
        while sent < job.size {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            stream
                .write_all(&buf[..n])
                .await
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            sent += n as u64;
        }
        stream
            .finish()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        // Let the peer read the end before the task drops the stream.
        let _ = stream.stopped().await;
        if sent < job.size {
            return Err(std::io::Error::other("the file shrank while sending"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_offer_left_unanswered_expires_and_gives_its_permit_back() {
        let limiter = magnetita_net::PayloadLimiter::new();
        let mut waiting = Waiting::default();
        let start = Instant::now();
        waiting.insert(1, limiter.try_acquire().unwrap(), start);
        waiting.insert(2, limiter.try_acquire().unwrap(), start + OFFER_TTL / 2);
        assert!(waiting.expire(start + OFFER_TTL / 2).is_empty());

        let expired = waiting.expire(start + OFFER_TTL);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].0, 1);
        drop(expired);
        assert!(waiting.take(1).is_none(), "an expired offer is gone");
        assert!(waiting.take(2).is_some(), "a younger offer still waits");
        for _ in 0..4 {
            std::mem::forget(limiter.try_acquire().expect("every permit is back"));
        }
    }
}
