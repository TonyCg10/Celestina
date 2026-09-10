//! The `storage` capability on the phone's side: the desktop's requests as
//! one enum the application answers, the replies as envelopes, and a
//! server over a plain directory that the peer and the tests use in place
//! of Android's document tree.

use std::io::{Read as _, Seek, SeekFrom, Write as _};
use std::path::{Component, Path, PathBuf};

use magnetita_proto::bound::MAX_LIST;
use magnetita_proto::capability;
use magnetita_proto::storage::{
    Data, Delete, Done, Entry, List, Listing, Mkdir, Read, Rename, Stat, StatReply, StorageState,
    Write,
};
use magnetita_proto::Envelope;

/// What the desktop asks of the shared root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageRequest {
    List(List),
    Stat(Stat),
    Read(Read),
    Write(Write),
    Mkdir(Mkdir),
    Rename(Rename),
    Delete(Delete),
}

impl StorageRequest {
    /// The request an envelope carries, if it is one.
    pub fn decode(env: &Envelope) -> Option<Self> {
        if env.capability != capability::STORAGE {
            return None;
        }
        let body = &env.body;
        Some(match env.kind {
            List::KIND => Self::List(List::decode(body).ok()?),
            Stat::KIND => Self::Stat(Stat::decode(body).ok()?),
            Read::KIND => Self::Read(Read::decode(body).ok()?),
            Write::KIND => Self::Write(Write::decode(body).ok()?),
            Mkdir::KIND => Self::Mkdir(Mkdir::decode(body).ok()?),
            Rename::KIND => Self::Rename(Rename::decode(body).ok()?),
            Delete::KIND => Self::Delete(Delete::decode(body).ok()?),
            _ => return None,
        })
    }

    pub fn request(&self) -> u32 {
        match self {
            Self::List(m) => m.request,
            Self::Stat(m) => m.request,
            Self::Read(m) => m.request,
            Self::Write(m) => m.request,
            Self::Mkdir(m) => m.request,
            Self::Rename(m) => m.request,
            Self::Delete(m) => m.request,
        }
    }
}

fn envelope(kind: u16, body: Vec<u8>) -> Envelope {
    Envelope {
        capability: capability::STORAGE,
        kind,
        id: 0,
        body,
    }
}

pub fn state(available: bool) -> Envelope {
    envelope(StorageState::KIND, StorageState { available }.encode())
}

pub fn listing(request: u32, entries: Vec<Entry>, more: bool, error: &str) -> Envelope {
    envelope(
        Listing::KIND,
        Listing {
            request,
            entries,
            more,
            error: error.to_owned(),
        }
        .encode(),
    )
}

pub fn stat_reply(request: u32, entry: Option<Entry>) -> Envelope {
    envelope(StatReply::KIND, StatReply { request, entry }.encode())
}

pub fn data(request: u32, bytes: Vec<u8>, error: &str) -> Envelope {
    envelope(
        Data::KIND,
        Data {
            request,
            bytes,
            error: error.to_owned(),
        }
        .encode(),
    )
}

pub fn done(request: u32, ok: bool, error: &str) -> Envelope {
    envelope(
        Done::KIND,
        Done {
            request,
            ok,
            error: error.to_owned(),
        }
        .encode(),
    )
}

/// A request answered from a directory on this side's file system, the
/// way the peer stands in for the phone. Paths were validated at the
/// wire, so joining them cannot leave `root`.
pub fn serve(root: &Path, request: &StorageRequest) -> Envelope {
    match request {
        StorageRequest::List(m) => match list_dir(&root.join(&m.path), m.offset as usize) {
            Ok((entries, more)) => listing(m.request, entries, more, ""),
            Err(e) => listing(m.request, Vec::new(), false, &e.to_string()),
        },
        StorageRequest::Stat(m) => stat_reply(m.request, stat_path(&root.join(&m.path))),
        StorageRequest::Read(m) => match read_range(&root.join(&m.path), m.offset, m.len) {
            Ok(bytes) => data(m.request, bytes, ""),
            Err(e) => data(m.request, Vec::new(), &e.to_string()),
        },
        StorageRequest::Write(m) => outcome(
            m.request,
            write_range(&root.join(&m.path), m.offset, &m.bytes, m.truncate),
        ),
        StorageRequest::Mkdir(m) => outcome(m.request, std::fs::create_dir(root.join(&m.path))),
        StorageRequest::Rename(m) => outcome(
            m.request,
            std::fs::rename(root.join(&m.from), root.join(&m.to)),
        ),
        StorageRequest::Delete(m) => {
            let path = root.join(&m.path);
            let result = if path.is_dir() {
                std::fs::remove_dir(&path)
            } else {
                std::fs::remove_file(&path)
            };
            outcome(m.request, result)
        }
    }
}

fn outcome(request: u32, result: std::io::Result<()>) -> Envelope {
    match result {
        Ok(()) => done(request, true, ""),
        Err(e) => done(request, false, &e.to_string()),
    }
}

/// A relative wire path as a local path; never absolute, never climbing.
pub fn local(root: &Path, path: &str) -> PathBuf {
    let mut out = root.to_path_buf();
    for c in Path::new(path).components() {
        if let Component::Normal(part) = c {
            out.push(part);
        }
    }
    out
}

fn entry_of(name: String, meta: &std::fs::Metadata) -> Entry {
    Entry {
        name,
        dir: meta.is_dir(),
        size: if meta.is_dir() { 0 } else { meta.len() },
        mtime_ms: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    }
}

fn list_dir(dir: &Path, offset: usize) -> std::io::Result<(Vec<Entry>, bool)> {
    let mut names: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    let page: Vec<Entry> = names
        .iter()
        .skip(offset)
        .take(MAX_LIST)
        .filter_map(|name| {
            let meta = std::fs::metadata(dir.join(name)).ok()?;
            Some(entry_of(name.clone(), &meta))
        })
        .collect();
    let more = names.len() > offset + MAX_LIST;
    Ok((page, more))
}

fn stat_path(path: &Path) -> Option<Entry> {
    let meta = std::fs::metadata(path).ok()?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_owned();
    Some(entry_of(name, &meta))
}

fn read_range(path: &Path, offset: u64, len: u32) -> std::io::Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; len as usize];
    let mut filled = 0;
    while filled < buf.len() {
        let n = file.read(&mut buf[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    buf.truncate(filled);
    Ok(buf)
}

fn write_range(path: &Path, offset: u64, bytes: &[u8], truncate: bool) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    file.write_all(bytes)?;
    if truncate {
        file.set_len(offset + bytes.len() as u64)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("magnetita-storage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("photos")).unwrap();
        std::fs::write(dir.join("photos/a.jpg"), b"hello world").unwrap();
        dir
    }

    fn request(env: &Envelope) -> StorageRequest {
        StorageRequest::decode(env).unwrap()
    }

    #[test]
    fn the_directory_server_lists_reads_writes_and_removes() {
        let root = temp();
        let list = List {
            request: 1,
            path: String::new(),
            offset: 0,
        };
        let reply = serve(&root, &request(&envelope(List::KIND, list.encode())));
        let listing = Listing::decode(&reply.body).unwrap();
        assert_eq!(listing.entries.len(), 1);
        assert!(listing.entries[0].dir);
        assert_eq!(listing.entries[0].name, "photos");

        let read = Read {
            request: 2,
            path: "photos/a.jpg".into(),
            offset: 6,
            len: 100,
        };
        let reply = serve(&root, &request(&envelope(Read::KIND, read.encode())));
        assert_eq!(Data::decode(&reply.body).unwrap().bytes, b"world");

        let write = Write {
            request: 3,
            path: "photos/b.txt".into(),
            offset: 0,
            bytes: b"new".to_vec(),
            truncate: true,
        };
        let reply = serve(&root, &request(&envelope(Write::KIND, write.encode())));
        assert!(Done::decode(&reply.body).unwrap().ok);
        assert_eq!(std::fs::read(root.join("photos/b.txt")).unwrap(), b"new");

        let stat = Stat {
            request: 4,
            path: "photos/b.txt".into(),
        };
        let reply = serve(&root, &request(&envelope(Stat::KIND, stat.encode())));
        assert_eq!(
            StatReply::decode(&reply.body).unwrap().entry.unwrap().size,
            3
        );

        let rename = Rename {
            request: 5,
            from: "photos/b.txt".into(),
            to: "c.txt".into(),
        };
        serve(&root, &request(&envelope(Rename::KIND, rename.encode())));
        let delete = Delete {
            request: 6,
            path: "c.txt".into(),
        };
        let reply = serve(&root, &request(&envelope(Delete::KIND, delete.encode())));
        assert!(Done::decode(&reply.body).unwrap().ok);
        assert!(!root.join("c.txt").exists());

        let missing = Stat {
            request: 7,
            path: "nope".into(),
        };
        let reply = serve(&root, &request(&envelope(Stat::KIND, missing.encode())));
        assert!(StatReply::decode(&reply.body).unwrap().entry.is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn local_paths_never_climb() {
        assert_eq!(local(Path::new("/r"), "a/b"), PathBuf::from("/r/a/b"));
        assert_eq!(local(Path::new("/r"), "../x"), PathBuf::from("/r/x"));
    }
}
