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
use tokio::sync::{mpsc::UnboundedSender, oneshot};

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
/// How long a fetched read window stays good for the next kernel read.
const WINDOW_TTL: Duration = Duration::from_secs(5);

/// The clients by device id, for the file systems and the tests.
static CLIENTS: LazyLock<Mutex<HashMap<String, Arc<StorageClient>>>> =
    LazyLock::new(Default::default);

pub(crate) fn clients() -> &'static Mutex<HashMap<String, Arc<StorageClient>>> {
    &CLIENTS
}

/// One session's requests to the phone, answered by request id.
pub(crate) struct StorageClient {
    outbox: UnboundedSender<Envelope>,
    pending: Mutex<HashMap<u32, oneshot::Sender<Envelope>>>,
    next: AtomicU32,
}

impl StorageClient {
    pub(crate) fn new(outbox: UnboundedSender<Envelope>) -> Arc<Self> {
        Arc::new(Self {
            outbox,
            pending: Mutex::new(HashMap::new()),
            next: AtomicU32::new(1),
        })
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
            if let Some(waiter) = self.pending.lock_ok().remove(&request) {
                let _ = waiter.send(env);
            }
        }
    }

    fn next_request(&self) -> u32 {
        self.next.fetch_add(1, Ordering::Relaxed)
    }

    async fn call(&self, kind: u16, body: Vec<u8>, request: u32) -> Result<Envelope, String> {
        let (tx, rx) = oneshot::channel();
        self.pending.lock_ok().insert(request, tx);
        let env = Envelope {
            capability: capability::STORAGE,
            kind,
            id: 0,
            body,
        };
        if self.outbox.send(env).is_err() {
            self.pending.lock_ok().remove(&request);
            return Err("link closed".into());
        }
        match tokio::time::timeout(REQUEST_TIMEOUT, rx).await {
            Ok(Ok(env)) => Ok(env),
            Ok(Err(_)) => Err("link closed".into()),
            Err(_) => {
                self.pending.lock_ok().remove(&request);
                Err("the phone did not answer".into())
            }
        }
    }

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
/// first sight and kept for the mount's life, moved on rename.
#[derive(Default)]
struct Inodes {
    by_path: HashMap<String, u64>,
    by_ino: HashMap<u64, String>,
    next: u64,
}

impl Inodes {
    fn new() -> Self {
        let mut s = Self {
            next: 2,
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
        let ino = self.next;
        self.next += 1;
        self.by_path.insert(path.to_owned(), ino);
        self.by_ino.insert(ino, path.to_owned());
        ino
    }

    fn path(&self, ino: u64) -> Option<String> {
        self.by_ino.get(&ino).cloned()
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
        self.attrs.lock_ok().insert(ino, (entry, Instant::now()));
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
        let entries = self.handle.block_on(self.client.list(path))?;
        for entry in &entries {
            if let Some(child) = join(path, OsStr::new(&entry.name)) {
                let child_ino = self.inodes.lock_ok().get_or_insert(&child);
                self.remember(child_ino, entry.clone());
            }
        }
        self.dirs
            .lock_ok()
            .insert(ino, (entries.clone(), Instant::now()));
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
        let bytes = self
            .handle
            .block_on(self.client.read(path, offset, READ_CHUNK))?;
        let out = bytes[..wanted.min(bytes.len())].to_vec();
        self.windows.lock_ok().insert(
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
        let entry = self.handle.block_on(self.client.stat(&path))?;
        if let Some(e) = &entry {
            self.remember(ino, e.clone());
        }
        Ok(entry)
    }

    fn entry_reply(&self, path: &str, reply: ReplyEntry) {
        match self.handle.block_on(self.client.stat(path)) {
            Ok(Some(entry)) => {
                let ino = self.inodes.lock_ok().get_or_insert(path);
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
                let ino = self.inodes.lock_ok().get_or_insert(&path);
                let attr = self.attr(ino, &entry);
                self.remember(ino, entry);
                reply.entry(&TTL, &attr, Generation(0));
            }
            Some(None) => reply.error(fuser::Errno::ENOENT),
            None => self.entry_reply(&path, reply),
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
                .handle
                .block_on(self.client.write(&path, size, &[], true))
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
        if self.handle.block_on(self.client.mkdir(&path)).is_err() {
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
        match self.handle.block_on(self.client.delete(&path)) {
            Ok(()) => {
                let ino = self.inodes.lock_ok().by_path.get(&path).copied();
                if let Some(ino) = ino {
                    self.forget_attr(ino);
                }
                self.forget_dir(parent.0);
                reply.ok();
            }
            Err(_) => reply.error(fuser::Errno::EIO),
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
        match self.handle.block_on(self.client.rename(&from, &to)) {
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
                .handle
                .block_on(
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
        _flags: i32,
        reply: ReplyCreate,
    ) {
        let Some(path) = self.path_of(parent.0).and_then(|p| join(&p, name)) else {
            return reply.error(fuser::Errno::ENOENT);
        };
        if self
            .handle
            .block_on(self.client.write(&path, 0, &[], true))
            .is_err()
        {
            return reply.error(fuser::Errno::EIO);
        }
        self.forget_dir(parent.0);
        let entry = Entry {
            name: name.to_string_lossy().into_owned(),
            dir: false,
            size: 0,
            mtime_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        };
        let ino = self.inodes.lock_ok().get_or_insert(&path);
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn names_that_climb_are_not_joined() {
        assert_eq!(join("", OsStr::new("x")).as_deref(), Some("x"));
        assert_eq!(join("a", OsStr::new("x")).as_deref(), Some("a/x"));
        assert!(join("a", OsStr::new("..")).is_none());
        assert!(join("a", OsStr::new("b/c")).is_none());
    }
}
