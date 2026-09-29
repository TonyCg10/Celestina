//! The phone's storage on the own wire: a client that asks the phone for
//! listings, entries, ranges and changes, and a FUSE file system over it so
//! the phone is a directory under `$XDG_RUNTIME_DIR/magnetita/<device-id>/`,
//! the path Siderita already browses. One owner for the phone's files: the
//! session that holds the link.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use fuser::{
    FileAttr, FileType, Filesystem, FopenFlags, Generation, INodeNo, MountOption, ReplyAttr,
    ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyWrite, Request,
};
use magnetita_proto::capability;
use magnetita_proto::storage::{
    Data, Delete, Done, Entry, List, Listing, Mkdir, Read, Rename, Stat, StatReply, Write,
};
use magnetita_proto::Envelope;
use tokio::sync::oneshot;

use super::writer::Outbox;
use crate::lock::LockOk;

/// How long one request may take before the file system gives up on it.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the kernel, and this side, may trust an attribute, a lookup or
/// a listing. The phone is one person's, changed from the desktop through
/// this very mount or from the phone's own hand; a quarter of a minute of
/// staleness is the price of a browse that does not pay a round trip per
/// entry.
const TTL: Duration = Duration::from_secs(15);
/// Bytes per read message: the wire's bound, so a file arrives in as few
/// round trips as the wire allows and the kernel's 128 KiB reads are served
/// from the window already fetched.
const READ_CHUNK: u32 = magnetita_proto::storage::MAX_RANGE as u32;
/// Why a request failed without reaching the phone.
const CLOSED: &str = "link closed";
/// How long a fetched read window stays good for the next kernel read.
const WINDOW_TTL: Duration = Duration::from_secs(5);
/// The most entries one directory listing gathers, across its pages. A
/// phone that keeps answering `more` stops here instead of growing the
/// daemon without end; a larger directory shows its first entries.
const MAX_LISTING: usize = 65_536;
/// The most inode numbers kept at once. The kernel's own references are
/// counted and released by `forget`; past this, numbers the kernel never
/// looked up (handed out by listings) are dropped.
const MAX_INODES: usize = 65_536;
/// The most cached attributes, listings and read windows. A window is up to
/// a wire range (about 1 MiB), so few are kept.
const MAX_ATTRS: usize = 16_384;
const MAX_DIRS: usize = 64;
const MAX_WINDOWS: usize = 8;

/// The clients by device id, for the file systems and the tests.
static CLIENTS: LazyLock<Mutex<HashMap<String, Arc<StorageClient>>>> =
    LazyLock::new(Default::default);

pub(crate) fn clients() -> &'static Mutex<HashMap<String, Arc<StorageClient>>> {
    &CLIENTS
}

/// Publishes `client` as `device_id`'s until the returned guard drops; the
/// drop closes it, so no file-system thread keeps waiting on a session that
/// is gone, however that session ended.
pub(crate) fn register(device_id: &str, client: &Arc<StorageClient>) -> Registered {
    clients()
        .lock_ok()
        .insert(device_id.to_owned(), Arc::clone(client));
    Registered {
        device_id: device_id.to_owned(),
        client: Arc::clone(client),
    }
}

/// Takes `client` out of the registry, unless a newer session of the same
/// device has already put its own there.
pub(crate) fn unregister(device_id: &str, client: &Arc<StorageClient>) {
    let mut clients = clients().lock_ok();
    if clients
        .get(device_id)
        .is_some_and(|current| Arc::ptr_eq(current, client))
    {
        clients.remove(device_id);
    }
}

pub(crate) struct Registered {
    device_id: String,
    client: Arc<StorageClient>,
}

impl Drop for Registered {
    fn drop(&mut self) {
        self.client.close();
        unregister(&self.device_id, &self.client);
    }
}

/// One session's requests to the phone, answered by request id. Closed when
/// its session ends: the requests in flight fail at once, and a later one
/// fails before it starts a timer, so no file-system thread is left waiting
/// inside the link's runtime while that runtime stops.
pub(crate) struct StorageClient {
    outbox: Outbox,
    /// `None` once closed.
    pending: Mutex<Option<HashMap<u32, oneshot::Sender<Envelope>>>>,
    next: AtomicU32,
}

impl StorageClient {
    pub(crate) fn new(outbox: Outbox) -> Arc<Self> {
        Arc::new(Self {
            outbox,
            pending: Mutex::new(Some(HashMap::new())),
            next: AtomicU32::new(1),
        })
    }

    /// Fails every request in flight and every later one.
    pub(crate) fn close(&self) {
        self.pending.lock_ok().take();
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.pending.lock_ok().is_none()
    }

    /// A reply from the phone: matched to its request, else dropped.
    pub(crate) fn reply(&self, env: Envelope) {
        let request = match env.kind {
            Listing::KIND => Listing::decode(&env.body).ok().map(|m| m.request),
            StatReply::KIND => StatReply::decode(&env.body).ok().map(|m| m.request),
            Data::KIND => Data::decode(&env.body).ok().map(|m| m.request),
            Done::KIND => Done::decode(&env.body).ok().map(|m| m.request),
            _ => None,
        };
        if let Some(request) = request {
            let waiter = self
                .pending
                .lock_ok()
                .as_mut()
                .and_then(|pending| pending.remove(&request));
            if let Some(waiter) = waiter {
                let _ = waiter.send(env);
            }
        }
    }

    fn forget_request(&self, request: u32) {
        if let Some(pending) = self.pending.lock_ok().as_mut() {
            pending.remove(&request);
        }
    }

    fn next_request(&self) -> u32 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }

    async fn call(&self, kind: u16, body: Vec<u8>, request: u32) -> Result<Envelope, String> {
        let (tx, rx) = oneshot::channel();
        match self.pending.lock_ok().as_mut() {
            Some(pending) => pending.insert(request, tx),
            None => return Err(CLOSED.into()),
        };
        let env = Envelope {
            capability: capability::STORAGE,
            kind,
            id: 0,
            body,
        };
        if let Err(refused) = self.outbox.send(env) {
            self.forget_request(request);
            return Err(refused.to_string());
        }
        match tokio::time::timeout(REQUEST_TIMEOUT, rx).await {
            Ok(Ok(env)) => Ok(env),
            Ok(Err(_)) => Err(CLOSED.into()),
            Err(_) => {
                self.forget_request(request);
                Err("the phone did not answer".into())
            }
        }
    }

    /// Every entry of `path`, page by page, up to [`MAX_LISTING`].
    pub(crate) async fn list(&self, path: &str) -> Result<Vec<Entry>, String> {
        let mut all = Vec::new();
        loop {
            let request = self.next_request();
            let m = List {
                request,
                path: path.to_owned(),
                offset: all.len() as u32,
            };
            let env = self.call(List::KIND, m.encode(), request).await?;
            let listing = Listing::decode(&env.body).map_err(|e| e.to_string())?;
            if !listing.error.is_empty() {
                return Err(listing.error);
            }
            let more = listing.more && !listing.entries.is_empty();
            all.extend(listing.entries);
            if all.len() >= MAX_LISTING {
                all.truncate(MAX_LISTING);
                crate::runtime::log(
                    "storage",
                    &format!("{path:?}: listing stopped at {MAX_LISTING} entries"),
                );
                return Ok(all);
            }
            if !more {
                return Ok(all);
            }
        }
    }

    pub(crate) async fn stat(&self, path: &str) -> Result<Option<Entry>, String> {
        let request = self.next_request();
        let m = Stat {
            request,
            path: path.to_owned(),
        };
        let env = self.call(Stat::KIND, m.encode(), request).await?;
        StatReply::decode(&env.body)
            .map(|r| r.entry)
            .map_err(|e| e.to_string())
    }

    pub(crate) async fn read(&self, path: &str, offset: u64, len: u32) -> Result<Vec<u8>, String> {
        let request = self.next_request();
        let m = Read {
            request,
            path: path.to_owned(),
            offset,
            len: len.min(READ_CHUNK),
        };
        let env = self.call(Read::KIND, m.encode(), request).await?;
        let data = Data::decode(&env.body).map_err(|e| e.to_string())?;
        if !data.error.is_empty() {
            return Err(data.error);
        }
        Ok(data.bytes)
    }

    async fn done(&self, kind: u16, body: Vec<u8>, request: u32) -> Result<(), String> {
        let env = self.call(kind, body, request).await?;
        let done = Done::decode(&env.body).map_err(|e| e.to_string())?;
        if done.ok {
            Ok(())
        } else {
            Err(done.error)
        }
    }

    pub(crate) async fn write(
        &self,
        path: &str,
        offset: u64,
        bytes: &[u8],
        truncate: bool,
    ) -> Result<(), String> {
        let request = self.next_request();
        let m = Write {
            request,
            path: path.to_owned(),
            offset,
            bytes: bytes.to_vec(),
            truncate,
        };
        self.done(Write::KIND, m.encode(), request).await
    }

    pub(crate) async fn mkdir(&self, path: &str) -> Result<(), String> {
        let request = self.next_request();
        let m = Mkdir {
            request,
            path: path.to_owned(),
        };
        self.done(Mkdir::KIND, m.encode(), request).await
    }

    pub(crate) async fn rename(&self, from: &str, to: &str) -> Result<(), String> {
        let request = self.next_request();
        let m = Rename {
            request,
            from: from.to_owned(),
            to: to.to_owned(),
        };
        self.done(Rename::KIND, m.encode(), request).await
    }

    pub(crate) async fn delete(&self, path: &str) -> Result<(), String> {
        let request = self.next_request();
        let m = Delete {
            request,
            path: path.to_owned(),
        };
        self.done(Delete::KIND, m.encode(), request).await
    }
}

/// Inode numbers for wire paths: the root is 1, the rest are handed out on
/// first sight, moved on rename, and dropped once the kernel forgets them.
/// Only numbers handed to the kernel in an entry reply are counted; numbers
/// a listing handed out are dropped when the table is full, since the
/// kernel never holds them.
#[derive(Default)]
struct Inodes {
    by_path: HashMap<String, u64>,
    by_ino: HashMap<u64, String>,
    /// The kernel's lookup count per inode.
    lookups: HashMap<u64, u64>,
    next: u64,
    /// The table size that triggers the next prune. When the kernel holds
    /// most numbers a prune frees few, so the next one waits for a quarter
    /// of the bound more, instead of every insert paying a full scan.
    prune_at: usize,
}

impl Inodes {
    fn new() -> Self {
        let mut s = Self {
            next: 2,
            prune_at: MAX_INODES,
            ..Default::default()
        };
        s.by_path.insert(String::new(), 1);
        s.by_ino.insert(1, String::new());
        s
    }

    fn get_or_insert(&mut self, path: &str) -> u64 {
        if let Some(&ino) = self.by_path.get(path) {
            return ino;
        }
        if self.by_ino.len() >= self.prune_at {
            self.prune();
            self.prune_at = (self.by_ino.len() + MAX_INODES / 4).max(MAX_INODES);
        }
        let ino = self.next;
        self.next += 1;
        self.by_path.insert(path.to_owned(), ino);
        self.by_ino.insert(ino, path.to_owned());
        ino
    }

    fn path(&self, ino: u64) -> Option<String> {
        self.by_ino.get(&ino).cloned()
    }

    /// The kernel was handed `ino` in an entry reply.
    fn looked_up(&mut self, ino: u64) {
        if ino != 1 {
            *self.lookups.entry(ino).or_default() += 1;
        }
    }

    /// The kernel dropped `n` references; true when `ino` is gone.
    fn forget(&mut self, ino: u64, n: u64) -> bool {
        if ino == 1 {
            return false;
        }
        let left = match self.lookups.get_mut(&ino) {
            Some(count) => {
                *count = count.saturating_sub(n);
                *count
            }
            None => 0,
        };
        if left > 0 {
            return false;
        }
        self.lookups.remove(&ino);
        if let Some(path) = self.by_ino.remove(&ino) {
            self.by_path.remove(&path);
        }
        true
    }

    /// Drops every number the kernel does not hold.
    fn prune(&mut self) {
        let lookups = &self.lookups;
        self.by_ino
            .retain(|ino, _| *ino == 1 || lookups.contains_key(ino));
        let by_ino = &self.by_ino;
        self.by_path.retain(|_, ino| by_ino.contains_key(ino));
    }

    fn moved(&mut self, from: &str, to: &str) {
        let prefix = format!("{from}/");
        let affected: Vec<(String, u64)> = self
            .by_path
            .iter()
            .filter(|(p, _)| p.as_str() == from || p.starts_with(&prefix))
            .map(|(p, i)| (p.clone(), *i))
            .collect();
        for (old, ino) in affected {
            let new = format!("{to}{}", &old[from.len()..]);
            self.by_path.remove(&old);
            self.by_path.insert(new.clone(), ino);
            self.by_ino.insert(ino, new);
        }
    }
}

/// Makes room for one more entry in a cache of at most `cap`: the expired
/// go first, then the oldest.
fn make_room<V>(map: &mut HashMap<u64, V>, cap: usize, ttl: Duration, at: impl Fn(&V) -> Instant) {
    if map.len() < cap {
        return;
    }
    map.retain(|_, v| at(v).elapsed() < ttl);
    while map.len() >= cap {
        let Some(oldest) = map.iter().min_by_key(|(_, v)| at(v)).map(|(k, _)| *k) else {
            return;
        };
        map.remove(&oldest);
    }
}

fn join(parent: &str, name: &OsStr) -> Option<String> {
    let name = name.to_str()?;
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return None;
    }
    Some(if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    })
}

/// One fetched read window of a file: where it starts, its bytes, when.
struct Window {
    start: u64,
    bytes: Vec<u8>,
    at: Instant,
}

/// The phone as a file system. Every operation is one round trip on the
/// link, blocking the FUSE thread and nothing else.
pub(crate) struct PhoneFs {
    client: Arc<StorageClient>,
    handle: tokio::runtime::Handle,
    inodes: Mutex<Inodes>,
    attrs: Mutex<HashMap<u64, (Entry, Instant)>>,
    /// Listings by directory inode, so a browse asks the phone once and the
    /// lookups the kernel makes for every entry are answered here.
    dirs: Mutex<HashMap<u64, (Vec<Entry>, Instant)>>,
    /// The last window read per file inode: the kernel asks in 128 KiB
    /// pieces, the phone answers in 1 MiB ones.
    windows: Mutex<HashMap<u64, Window>>,
    uid: u32,
    gid: u32,
}

impl PhoneFs {
    pub(crate) fn new(client: Arc<StorageClient>, handle: tokio::runtime::Handle) -> Self {
        Self {
            client,
            handle,
            inodes: Mutex::new(Inodes::new()),
            attrs: Mutex::new(HashMap::new()),
            dirs: Mutex::new(HashMap::new()),
            windows: Mutex::new(HashMap::new()),
            uid: rustix::process::getuid().as_raw(),
            gid: rustix::process::getgid().as_raw(),
        }
    }

    /// Runs one request to the phone on the link's runtime, from this file
    /// system's thread; a closed client answers at once, off the runtime.
    fn wait<T>(
        &self,
        request: impl std::future::Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        if self.client.is_closed() {
            return Err(CLOSED.into());
        }
        self.handle.block_on(request)
    }

    fn attr(&self, ino: u64, entry: &Entry) -> FileAttr {
        let mtime = UNIX_EPOCH + Duration::from_millis(entry.mtime_ms);
        let (kind, perm, nlink) = if entry.dir {
            (FileType::Directory, 0o755, 2)
        } else {
            (FileType::RegularFile, 0o644, 1)
        };
        FileAttr {
            ino: INodeNo(ino),
            size: entry.size,
            blocks: entry.size.div_ceil(512),
            atime: mtime,
            mtime,
            ctime: mtime,
            crtime: mtime,
            kind,
            perm,
            nlink,
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        }
    }

    fn remember(&self, ino: u64, entry: Entry) {
        let mut attrs = self.attrs.lock_ok();
        if !attrs.contains_key(&ino) {
            make_room(&mut attrs, MAX_ATTRS, TTL, |(_, at)| *at);
        }
        attrs.insert(ino, (entry, Instant::now()));
    }

    /// The inode for `path`, counted as handed to the kernel.
    fn looked_up(&self, path: &str) -> u64 {
        let mut inodes = self.inodes.lock_ok();
        let ino = inodes.get_or_insert(path);
        inodes.looked_up(ino);
        ino
    }

    fn forget_attr(&self, ino: u64) {
        self.attrs.lock_ok().remove(&ino);
        self.windows.lock_ok().remove(&ino);
    }

    /// A directory changed under this side's hand: its listing is stale.
    fn forget_dir(&self, ino: u64) {
        self.dirs.lock_ok().remove(&ino);
        self.attrs.lock_ok().remove(&ino);
    }

    /// The listing of a directory: the cache when fresh, else the phone.
    fn listing_of(&self, ino: u64, path: &str) -> Result<Vec<Entry>, String> {
        if let Some((entries, at)) = self.dirs.lock_ok().get(&ino) {
            if at.elapsed() < TTL {
                return Ok(entries.clone());
            }
        }
        let entries = self.wait(self.client.list(path))?;
        for entry in &entries {
            if let Some(child) = join(path, OsStr::new(&entry.name)) {
                let child_ino = self.inodes.lock_ok().get_or_insert(&child);
                self.remember(child_ino, entry.clone());
            }
        }
        let mut dirs = self.dirs.lock_ok();
        if !dirs.contains_key(&ino) {
            make_room(&mut dirs, MAX_DIRS, TTL, |(_, at)| *at);
        }
        dirs.insert(ino, (entries.clone(), Instant::now()));
        Ok(entries)
    }

    /// `size` bytes at `offset`: from the last window when it covers them,
    /// else one wire-sized fetch from `offset` that becomes the window.
    fn read_window(&self, ino: u64, path: &str, offset: u64, size: u32) -> Result<Vec<u8>, String> {
        let wanted = size as usize;
        if let Some(window) = self.windows.lock_ok().get(&ino) {
            let end = window.start + window.bytes.len() as u64;
            let short = window.bytes.len() < READ_CHUNK as usize;
            if window.at.elapsed() < WINDOW_TTL
                && offset >= window.start
                && (offset + wanted as u64 <= end || (short && offset <= end))
            {
                let from = (offset - window.start) as usize;
                let to = (from + wanted).min(window.bytes.len());
                return Ok(window.bytes[from..to].to_vec());
            }
        }
        let bytes = self.wait(self.client.read(path, offset, READ_CHUNK))?;
        let out = bytes[..wanted.min(bytes.len())].to_vec();
        let mut windows = self.windows.lock_ok();
        if !windows.contains_key(&ino) {
            make_room(&mut windows, MAX_WINDOWS, WINDOW_TTL, |w| w.at);
        }
        windows.insert(
            ino,
            Window {
                start: offset,
                bytes,
                at: Instant::now(),
            },
        );
        Ok(out)
    }

    fn path_of(&self, ino: u64) -> Option<String> {
        self.inodes.lock_ok().path(ino)
    }

    fn root_entry() -> Entry {
        Entry {
            name: String::new(),
            dir: true,
            size: 0,
            mtime_ms: 0,
        }
    }

    /// The entry of an inode: the cache when fresh, else the phone.
    fn entry_of(&self, ino: u64) -> Result<Option<Entry>, String> {
        if ino == 1 {
            return Ok(Some(Self::root_entry()));
        }
        if let Some((entry, at)) = self.attrs.lock_ok().get(&ino) {
            if at.elapsed() < TTL {
                return Ok(Some(entry.clone()));
            }
        }
        let path = self.path_of(ino).ok_or("unknown inode")?;
        let entry = self.wait(self.client.stat(&path))?;
        if let Some(e) = &entry {
            self.remember(ino, e.clone());
        }
        Ok(entry)
    }

    fn entry_reply(&self, path: &str, reply: ReplyEntry) {
        match self.wait(self.client.stat(path)) {
            Ok(Some(entry)) => {
                let ino = self.looked_up(path);
                let attr = self.attr(ino, &entry);
                self.remember(ino, entry);
                reply.entry(&TTL, &attr, Generation(0));
            }
            Ok(None) => reply.error(fuser::Errno::ENOENT),
            Err(_) => reply.error(fuser::Errno::EIO),
        }
    }
}

impl Filesystem for PhoneFs {
    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let parent_ino = parent.0;
        let Some(parent) = self.path_of(parent_ino) else {
            return reply.error(fuser::Errno::ENOENT);
        };
        let Some(path) = join(&parent, name) else {
            return reply.error(fuser::Errno::ENOENT);
        };
        let listed = self
            .dirs
            .lock_ok()
            .get(&parent_ino)
            .filter(|(_, at)| at.elapsed() < TTL)
            .map(|(entries, _)| {
                entries
                    .iter()
                    .find(|e| e.name.as_bytes() == name.as_encoded_bytes())
                    .cloned()
            });
        match listed {
            Some(Some(entry)) => {
                let ino = self.looked_up(&path);
                let attr = self.attr(ino, &entry);
                self.remember(ino, entry);
                reply.entry(&TTL, &attr, Generation(0));
            }
            Some(None) => reply.error(fuser::Errno::ENOENT),
            None => self.entry_reply(&path, reply),
        }
    }

    /// The kernel let go of `ino`: once it holds no reference, its number
    /// and everything cached for it go.
    fn forget(&self, _req: &Request, ino: INodeNo, nlookup: u64) {
        if self.inodes.lock_ok().forget(ino.0, nlookup) {
            self.forget_dir(ino.0);
            self.forget_attr(ino.0);
        }
    }

    fn getattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: Option<fuser::FileHandle>,
        reply: ReplyAttr,
    ) {
        match self.entry_of(ino.0) {
            Ok(Some(entry)) => reply.attr(&TTL, &self.attr(ino.0, &entry)),
            Ok(None) => reply.error(fuser::Errno::ENOENT),
            Err(_) => reply.error(fuser::Errno::EIO),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn setattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<fuser::FileHandle>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<fuser::BsdFileFlags>,
        reply: ReplyAttr,
    ) {
        if let Some(size) = size {
            let Some(path) = self.path_of(ino.0) else {
                return reply.error(fuser::Errno::ENOENT);
            };
            if self
                .wait(self.client.write(&path, size, &[], true))
                .is_err()
            {
                return reply.error(fuser::Errno::EIO);
            }
            self.forget_attr(ino.0);
        }
        match self.entry_of(ino.0) {
            Ok(Some(entry)) => reply.attr(&TTL, &self.attr(ino.0, &entry)),
            Ok(None) => reply.error(fuser::Errno::ENOENT),
            Err(_) => reply.error(fuser::Errno::EIO),
        }
    }

    fn mkdir(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let Some(path) = self.path_of(parent.0).and_then(|p| join(&p, name)) else {
            return reply.error(fuser::Errno::ENOENT);
        };
        if self.wait(self.client.mkdir(&path)).is_err() {
            return reply.error(fuser::Errno::EIO);
        }
        self.forget_dir(parent.0);
        self.entry_reply(&path, reply);
    }

    fn unlink(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        self.rmdir(_req, parent, name, reply);
    }

    fn rmdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let Some(path) = self.path_of(parent.0).and_then(|p| join(&p, name)) else {
            return reply.error(fuser::Errno::ENOENT);
        };
        match self.wait(self.client.delete(&path)) {
            Ok(()) => {
                let ino = self.inodes.lock_ok().by_path.get(&path).copied();
                if let Some(ino) = ino {
                    self.forget_attr(ino);
                }
                self.forget_dir(parent.0);
                reply.ok();
            }
            Err(error) => reply.error(delete_errno(&error)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn rename(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        newparent: INodeNo,
        newname: &OsStr,
        _flags: fuser::RenameFlags,
        reply: ReplyEmpty,
    ) {
        let from = self.path_of(parent.0).and_then(|p| join(&p, name));
        let to = self.path_of(newparent.0).and_then(|p| join(&p, newname));
        let (Some(from), Some(to)) = (from, to) else {
            return reply.error(fuser::Errno::ENOENT);
        };
        match self.wait(self.client.rename(&from, &to)) {
            Ok(()) => {
                self.inodes.lock_ok().moved(&from, &to);
                self.attrs.lock_ok().clear();
                self.dirs.lock_ok().clear();
                self.windows.lock_ok().clear();
                reply.ok();
            }
            Err(_) => reply.error(fuser::Errno::EIO),
        }
    }

    fn open(
        &self,
        _req: &Request,
        _ino: INodeNo,
        _flags: fuser::OpenFlags,
        reply: fuser::ReplyOpen,
    ) {
        reply.opened(fuser::FileHandle(0), FopenFlags::empty());
    }

    #[allow(clippy::too_many_arguments)]
    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: fuser::FileHandle,
        offset: u64,
        size: u32,
        _flags: fuser::OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        reply: ReplyData,
    ) {
        let Some(path) = self.path_of(ino.0) else {
            return reply.error(fuser::Errno::ENOENT);
        };
        match self.read_window(ino.0, &path, offset, size) {
            Ok(bytes) => reply.data(&bytes),
            Err(_) => reply.error(fuser::Errno::EIO),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn write(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: fuser::FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: fuser::WriteFlags,
        _flags: fuser::OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        reply: ReplyWrite,
    ) {
        let Some(path) = self.path_of(ino.0) else {
            return reply.error(fuser::Errno::ENOENT);
        };
        let mut written = 0usize;
        for chunk in data.chunks(READ_CHUNK as usize) {
            if self
                .wait(
                    self.client
                        .write(&path, offset + written as u64, chunk, false),
                )
                .is_err()
            {
                return reply.error(fuser::Errno::EIO);
            }
            written += chunk.len();
        }
        self.forget_attr(ino.0);
        reply.written(written as u32);
    }

    fn flush(
        &self,
        _req: &Request,
        _ino: INodeNo,
        _fh: fuser::FileHandle,
        _lock_owner: fuser::LockOwner,
        reply: ReplyEmpty,
    ) {
        reply.ok();
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: fuser::FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        let Some(path) = self.path_of(ino.0) else {
            return reply.error(fuser::Errno::ENOENT);
        };
        let entries = match self.listing_of(ino.0, &path) {
            Ok(entries) => entries,
            Err(_) => return reply.error(fuser::Errno::EIO),
        };
        let mut all: Vec<(u64, FileType, String)> = vec![
            (ino.0, FileType::Directory, ".".into()),
            (ino.0, FileType::Directory, "..".into()),
        ];
        for entry in entries {
            let Some(child) = join(&path, OsStr::new(&entry.name)) else {
                continue;
            };
            let child_ino = self.inodes.lock_ok().get_or_insert(&child);
            let kind = if entry.dir {
                FileType::Directory
            } else {
                FileType::RegularFile
            };
            all.push((child_ino, kind, entry.name));
        }
        for (i, (child_ino, kind, name)) in all.into_iter().enumerate().skip(offset as usize) {
            if reply.add(INodeNo(child_ino), (i + 1) as u64, kind, name) {
                break;
            }
        }
        reply.ok();
    }

    #[allow(clippy::too_many_arguments)]
    fn create(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        flags: i32,
        reply: ReplyCreate,
    ) {
        let Some(path) = self.path_of(parent.0).and_then(|p| join(&p, name)) else {
            return reply.error(fuser::Errno::ENOENT);
        };
        // The kernel asks to create from its own, possibly stale, view: a
        // file made on the phone meanwhile is asked about afresh, and only
        // an explicit O_TRUNC ever empties it.
        let existing = match self.wait(self.client.stat(&path)) {
            Ok(existing) => existing,
            Err(_) => return reply.error(fuser::Errno::EIO),
        };
        let (exclusive, truncate) = create_flags(flags);
        let entry = match existing {
            Some(_) if exclusive => return reply.error(fuser::Errno::EEXIST),
            Some(entry) if entry.dir => return reply.error(fuser::Errno::EISDIR),
            Some(entry) if !truncate => entry,
            _ => {
                if self
                    .wait(self.client.write(&path, 0, &[], truncate))
                    .is_err()
                {
                    return reply.error(fuser::Errno::EIO);
                }
                Entry {
                    name: name.to_string_lossy().into_owned(),
                    dir: false,
                    size: 0,
                    mtime_ms: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0),
                }
            }
        };
        self.forget_dir(parent.0);
        let ino = self.looked_up(&path);
        let attr = self.attr(ino, &entry);
        self.remember(ino, entry);
        reply.created(
            &TTL,
            &attr,
            Generation(0),
            fuser::FileHandle(0),
            FopenFlags::empty(),
        );
    }
}

/// What a create's open flags ask: (`O_EXCL`, `O_TRUNC`).
fn create_flags(flags: i32) -> (bool, bool) {
    let flags = rustix::fs::OFlags::from_bits_retain(flags as u32);
    (
        flags.contains(rustix::fs::OFlags::EXCL),
        flags.contains(rustix::fs::OFlags::TRUNC),
    )
}

/// Mounts the phone at `mountpoint`; dropping the session unmounts.
pub(crate) fn mount(
    client: Arc<StorageClient>,
    handle: tokio::runtime::Handle,
    mountpoint: &Path,
) -> std::io::Result<fuser::BackgroundSession> {
    std::fs::create_dir_all(mountpoint)?;
    let mut config = fuser::Config::default();
    config.mount_options = vec![
        MountOption::FSName("magnetita".into()),
        MountOption::Subtype("phone".into()),
        MountOption::NoAtime,
    ];
    fuser::spawn_mount(PhoneFs::new(client, handle), mountpoint, &config)
}

/// The errno a refused delete answers: `ENOTEMPTY` for a directory that
/// still has entries, so `rmdir` and file managers see what they would on
/// any other file system, and `EIO` for everything else.
fn delete_errno(error: &str) -> fuser::Errno {
    if error == magnetita_proto::storage::ERROR_NOT_EMPTY {
        fuser::Errno::ENOTEMPTY
    } else {
        fuser::Errno::EIO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_that_is_not_empty_answers_enotempty() {
        assert_eq!(delete_errno("not empty"), fuser::Errno::ENOTEMPTY);
        assert_eq!(delete_errno("not deleted"), fuser::Errno::EIO);
        assert_eq!(delete_errno("the phone did not answer"), fuser::Errno::EIO);
    }

    #[test]
    fn inodes_follow_a_rename_with_their_children() {
        let mut inodes = Inodes::new();
        let a = inodes.get_or_insert("a");
        let ab = inodes.get_or_insert("a/b");
        let other = inodes.get_or_insert("ab");
        inodes.moved("a", "c");
        assert_eq!(inodes.path(a).as_deref(), Some("c"));
        assert_eq!(inodes.path(ab).as_deref(), Some("c/b"));
        assert_eq!(inodes.path(other).as_deref(), Some("ab"));
        assert_eq!(inodes.get_or_insert("c/b"), ab);
    }

    /// MAG-17: a phone that answers `more` forever is stopped at the cap.
    #[tokio::test]
    async fn an_endless_listing_stops_at_the_cap() {
        let (outbox, mut requests) = super::super::writer::Outbox::detached(4);
        let client = StorageClient::new(outbox);
        let phone = Arc::clone(&client);
        tokio::spawn(async move {
            while let Some(item) = requests.recv().await {
                let Some(env) = item.into_envelope() else {
                    continue;
                };
                let Ok(list) = List::decode(&env.body) else {
                    continue;
                };
                let entries = (0..256)
                    .map(|n| Entry {
                        name: format!("{}-{n}", list.offset),
                        dir: false,
                        size: 0,
                        mtime_ms: 0,
                    })
                    .collect();
                let listing = Listing {
                    request: list.request,
                    entries,
                    more: true,
                    error: String::new(),
                };
                phone.reply(Envelope {
                    capability: capability::STORAGE,
                    kind: Listing::KIND,
                    id: 0,
                    body: listing.encode(),
                });
            }
        });
        let listed = tokio::time::timeout(Duration::from_secs(60), client.list("DCIM"))
            .await
            .expect("an endless listing must end")
            .unwrap();
        assert_eq!(listed.len(), MAX_LISTING);
    }

    /// MAG-27: a file-system thread waiting on the phone when the session
    /// ends returns at once, and neither it nor a later kernel request
    /// touches the link's runtime once that runtime has stopped (a timer on
    /// a stopped runtime panics the thread).
    #[test]
    fn a_closed_client_frees_the_file_system_thread_before_the_runtime_stops() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let (outbox, _requests) = super::super::writer::Outbox::detached(8);
        let client = StorageClient::new(outbox);
        let fs = Arc::new(PhoneFs::new(Arc::clone(&client), runtime.handle().clone()));
        let waiting = {
            let fs = Arc::clone(&fs);
            let client = Arc::clone(&client);
            std::thread::spawn(move || {
                let started = Instant::now();
                (fs.wait(client.list("DCIM")), started.elapsed())
            })
        };
        std::thread::sleep(Duration::from_millis(200));
        client.close();
        let (answer, waited) = waiting.join().expect("the waiting thread returns");
        assert_eq!(answer, Err(CLOSED.to_owned()));
        assert!(waited < Duration::from_secs(2), "waited {waited:?}");
        drop(runtime);
        let later = {
            let fs = Arc::clone(&fs);
            std::thread::spawn(move || fs.wait(client.stat("notes.txt")))
        };
        assert_eq!(
            later
                .join()
                .expect("a request after the runtime stopped must not panic"),
            Err(CLOSED.to_owned())
        );
    }

    /// A session cut before its own cleanup still closes its client and
    /// leaves the registry, and never removes a newer session's client.
    #[test]
    fn a_dropped_registration_closes_its_client_and_spares_a_newer_one() {
        let client = || StorageClient::new(super::super::writer::Outbox::detached(1).0);
        let old = client();
        let guard = register("guarded-phone", &old);
        drop(guard);
        assert!(old.is_closed());
        assert!(!clients().lock_ok().contains_key("guarded-phone"));

        let older = client();
        let older_guard = register("guarded-phone", &older);
        let newer = client();
        let newer_guard = register("guarded-phone", &newer);
        drop(older_guard);
        assert!(older.is_closed() && !newer.is_closed());
        assert!(clients()
            .lock_ok()
            .get("guarded-phone")
            .is_some_and(|current| Arc::ptr_eq(current, &newer)));
        drop(newer_guard);
        assert!(!clients().lock_ok().contains_key("guarded-phone"));
    }

    #[test]
    fn inodes_the_kernel_forgets_are_dropped_and_unheld_ones_pruned() {
        let mut inodes = Inodes::new();
        let held = inodes.get_or_insert("held");
        inodes.looked_up(held);
        inodes.looked_up(held);
        let listed = inodes.get_or_insert("listed");
        assert!(!inodes.forget(held, 1), "one reference is left");
        assert!(inodes.forget(held, 1));
        assert_eq!(inodes.path(held), None);
        assert!(!inodes.forget(1, 1), "the root stays");
        let again = inodes.get_or_insert("again");
        inodes.looked_up(again);
        inodes.prune();
        assert_eq!(inodes.path(listed), None, "nobody holds a listed number");
        assert_eq!(inodes.path(again).as_deref(), Some("again"));
        assert_eq!(inodes.path(1).as_deref(), Some(""));
    }

    #[test]
    fn the_inode_table_is_bounded() {
        let mut inodes = Inodes::new();
        for n in 0..(MAX_INODES * 2) {
            inodes.get_or_insert(&format!("f{n}"));
        }
        assert!(inodes.by_ino.len() <= MAX_INODES);
        assert_eq!(inodes.by_ino.len(), inodes.by_path.len());
    }

    #[test]
    fn a_table_the_kernel_holds_is_not_scanned_on_every_insert() {
        let mut inodes = Inodes::new();
        assert_eq!(inodes.prune_at, MAX_INODES);
        for n in 0..=MAX_INODES {
            let ino = inodes.get_or_insert(&format!("held{n}"));
            inodes.looked_up(ino);
        }
        let raised = inodes.prune_at;
        assert!(
            raised > MAX_INODES,
            "the full table pruned nothing and waits"
        );
        for n in 0..(MAX_INODES / 8) {
            inodes.get_or_insert(&format!("listed{n}"));
        }
        assert_eq!(inodes.prune_at, raised, "no prune until the next quarter");
    }

    #[test]
    fn a_full_cache_drops_the_expired_then_the_oldest() {
        let now = Instant::now();
        let old = now - Duration::from_secs(60);
        let mut map: HashMap<u64, Instant> = HashMap::new();
        map.insert(1, old);
        map.insert(2, now);
        map.insert(3, now + Duration::from_millis(1));
        make_room(&mut map, 3, Duration::from_secs(15), |at| *at);
        assert_eq!(map.len(), 2);
        assert!(!map.contains_key(&1));
        make_room(&mut map, 2, Duration::from_secs(15), |at| *at);
        assert_eq!(map.keys().copied().collect::<Vec<_>>(), [3]);
    }

    #[test]
    fn a_create_truncates_or_refuses_only_when_asked() {
        use rustix::fs::OFlags;
        let bits = |f: OFlags| f.bits() as i32;
        assert_eq!(
            create_flags(bits(OFlags::CREATE | OFlags::WRONLY)),
            (false, false)
        );
        assert_eq!(
            create_flags(bits(OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY)),
            (true, false)
        );
        assert_eq!(
            create_flags(bits(OFlags::CREATE | OFlags::TRUNC | OFlags::WRONLY)),
            (false, true)
        );
    }

    #[test]
    fn names_that_climb_are_not_joined() {
        assert_eq!(join("", OsStr::new("x")).as_deref(), Some("x"));
        assert_eq!(join("a", OsStr::new("x")).as_deref(), Some("a/x"));
        assert!(join("a", OsStr::new("..")).is_none());
        assert!(join("a", OsStr::new("b/c")).is_none());
    }
}
