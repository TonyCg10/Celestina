//! `storage` (capability 13): the phone's files as a tree the desktop
//! browses. Every request carries a `request` id the reply echoes; paths are
//! relative to the root the phone chose to share, `/`-separated, with no
//! empty, `.` or `..` component. Bytes travel inside the messages (at most
//! [`MAX_BYTES`] per read or write), so a browse needs no extra stream.

use crate::bound::{self, MAX_BYTES, MAX_LIST, MAX_TEXT};
use crate::codec::{self, required, Map};
use crate::error::DecodeError;

/// The most bytes one read or write carries: the envelope's body bound
/// less room for the request id, the path and the map itself.
pub const MAX_RANGE: usize = MAX_BYTES - 8192;

/// Phone → desktop: whether a root is shared at all. Kind 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorageState {
    pub available: bool,
}

/// Desktop → phone: the entries of a directory from `offset`. Kind 2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct List {
    pub request: u32,
    pub path: String,
    pub offset: u32,
}

/// One entry of a listing or the answer to a stat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub dir: bool,
    pub size: u64,
    pub mtime_ms: u64,
}

/// Phone → desktop: a page of entries. Kind 3.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    pub request: u32,
    /// At most [`MAX_LIST`]; `more` says another page follows.
    pub entries: Vec<Entry>,
    pub more: bool,
    /// Empty when the listing succeeded.
    pub error: String,
}

/// Desktop → phone: one path's entry. Kind 4.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stat {
    pub request: u32,
    pub path: String,
}

/// Phone → desktop: the entry, or that there is none. Kind 5.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatReply {
    pub request: u32,
    pub entry: Option<Entry>,
}

/// Desktop → phone: `len` bytes from `offset`. Kind 6.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    pub request: u32,
    pub path: String,
    pub offset: u64,
    /// At most [`MAX_RANGE`].
    pub len: u32,
}

/// Phone → desktop: the bytes; fewer than asked at the end. Kind 7.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Data {
    pub request: u32,
    pub bytes: Vec<u8>,
    pub error: String,
}

/// Desktop → phone: write `bytes` at `offset`, creating the file; with
/// `truncate` the file ends where these bytes end. Kind 8.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Write {
    pub request: u32,
    pub path: String,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub truncate: bool,
}

/// Phone → desktop: how a write, mkdir, rename or delete ended. Kind 9.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Done {
    pub request: u32,
    pub ok: bool,
    pub error: String,
}

/// Desktop → phone: make a directory. Kind 10.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mkdir {
    pub request: u32,
    pub path: String,
}

/// Desktop → phone: move `from` to `to`. Kind 11.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rename {
    pub request: u32,
    pub from: String,
    pub to: String,
}

/// Desktop → phone: remove a file or an empty directory. Kind 12.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delete {
    pub request: u32,
    pub path: String,
}

/// A relative path is a list of plain components; the root is empty.
pub fn check_path(path: &str) -> Result<(), DecodeError> {
    if path.is_empty() {
        return Ok(());
    }
    if path.len() > MAX_TEXT || path.contains('\0') {
        return Err(DecodeError::Malformed("path"));
    }
    for component in path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(DecodeError::Malformed("path"));
        }
    }
    Ok(())
}

fn path_field(d: &mut minicbor::Decoder<'_>, what: &'static str) -> Result<String, DecodeError> {
    let path = bound::text(d, what, MAX_TEXT)?;
    check_path(&path)?;
    Ok(path)
}

impl StorageState {
    pub const KIND: u16 = 1;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(1).bool(0, self.available).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let mut available = None;
        codec::read(body, "storage state", |k, d| {
            match k {
                0 => available = Some(bound::bool(d, "available")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            available: required(available, "available")?,
        })
    }
}

impl List {
    pub const KIND: u16 = 2;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(3)
            .u32(0, self.request)
            .text(1, &self.path)
            .u32(2, self.offset)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut request, mut path, mut offset) = (None, None, None);
        codec::read(body, "list", |k, d| {
            match k {
                0 => request = Some(bound::u32(d, "request")?),
                1 => path = Some(path_field(d, "path")?),
                2 => offset = Some(bound::u32(d, "offset")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            request: required(request, "request")?,
            path: path.unwrap_or_default(),
            offset: offset.unwrap_or(0),
        })
    }
}

impl Entry {
    fn encode(&self) -> Vec<u8> {
        Map::new(4)
            .text(0, &self.name)
            .bool(1, self.dir)
            .u64(2, self.size)
            .u64(3, self.mtime_ms)
            .finish()
    }

    fn decode(d: &mut minicbor::Decoder<'_>) -> Result<Self, DecodeError> {
        let (mut name, mut dir, mut size, mut mtime) = (None, None, None, None);
        codec::read_map(d, "entry", |k, d| {
            match k {
                0 => name = Some(bound::text(d, "name", bound::MAX_FILENAME)?),
                1 => dir = Some(bound::bool(d, "dir")?),
                2 => size = Some(bound::u64(d, "size")?),
                3 => mtime = Some(bound::u64(d, "mtime")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        let name = required(name, "name")?;
        if name.is_empty()
            || name.contains('/')
            || name.contains('\0')
            || name == "."
            || name == ".."
        {
            return Err(DecodeError::Malformed("entry name"));
        }
        Ok(Self {
            name,
            dir: dir.unwrap_or(false),
            size: size.unwrap_or(0),
            mtime_ms: mtime.unwrap_or(0),
        })
    }
}

impl Listing {
    pub const KIND: u16 = 3;

    pub fn encode(&self) -> Vec<u8> {
        let mut m = Map::new(4).u32(0, self.request).list(1, self.entries.len());
        for e in &self.entries {
            m = m.item(&e.encode());
        }
        m.bool(2, self.more).text(3, &self.error).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut request, mut entries, mut more, mut error) = (None, None, None, None);
        codec::read(body, "listing", |k, d| {
            match k {
                0 => request = Some(bound::u32(d, "request")?),
                1 => entries = Some(codec::read_list(d, "entries", MAX_LIST, Entry::decode)?),
                2 => more = Some(bound::bool(d, "more")?),
                3 => error = Some(bound::text(d, "error", MAX_TEXT)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            request: required(request, "request")?,
            entries: entries.unwrap_or_default(),
            more: more.unwrap_or(false),
            error: error.unwrap_or_default(),
        })
    }
}

fn decode_request_path(body: &[u8], what: &'static str) -> Result<(u32, String), DecodeError> {
    let (mut request, mut path) = (None, None);
    codec::read(body, what, |k, d| {
        match k {
            0 => request = Some(bound::u32(d, "request")?),
            1 => path = Some(path_field(d, "path")?),
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok((required(request, "request")?, path.unwrap_or_default()))
}

impl Stat {
    pub const KIND: u16 = 4;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2)
            .u32(0, self.request)
            .text(1, &self.path)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (request, path) = decode_request_path(body, "stat")?;
        Ok(Self { request, path })
    }
}

impl StatReply {
    pub const KIND: u16 = 5;

    pub fn encode(&self) -> Vec<u8> {
        let m = Map::new(codec::count(1, &[self.entry.is_some()])).u32(0, self.request);
        match &self.entry {
            Some(e) => m.nested(1, &e.encode()).finish(),
            None => m.finish(),
        }
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut request, mut entry) = (None, None);
        codec::read(body, "stat reply", |k, d| {
            match k {
                0 => request = Some(bound::u32(d, "request")?),
                1 => entry = Some(Entry::decode(d)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            request: required(request, "request")?,
            entry,
        })
    }
}

impl Read {
    pub const KIND: u16 = 6;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(4)
            .u32(0, self.request)
            .text(1, &self.path)
            .u64(2, self.offset)
            .u32(3, self.len)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut request, mut path, mut offset, mut len) = (None, None, None, None);
        codec::read(body, "read", |k, d| {
            match k {
                0 => request = Some(bound::u32(d, "request")?),
                1 => path = Some(path_field(d, "path")?),
                2 => offset = Some(bound::u64(d, "offset")?),
                3 => len = Some(bound::u32(d, "len")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        let len = required(len, "len")?;
        if len as usize > MAX_RANGE {
            return Err(DecodeError::Malformed("read length"));
        }
        Ok(Self {
            request: required(request, "request")?,
            path: required(path, "path")?,
            offset: offset.unwrap_or(0),
            len,
        })
    }
}

impl Data {
    pub const KIND: u16 = 7;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(3)
            .u32(0, self.request)
            .bytes(1, &self.bytes)
            .text(2, &self.error)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut request, mut bytes, mut error) = (None, None, None);
        codec::read(body, "data", |k, d| {
            match k {
                0 => request = Some(bound::u32(d, "request")?),
                1 => bytes = Some(bound::bytes(d, "bytes", MAX_RANGE)?),
                2 => error = Some(bound::text(d, "error", MAX_TEXT)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            request: required(request, "request")?,
            bytes: bytes.unwrap_or_default(),
            error: error.unwrap_or_default(),
        })
    }
}

impl Write {
    pub const KIND: u16 = 8;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(5)
            .u32(0, self.request)
            .text(1, &self.path)
            .u64(2, self.offset)
            .bytes(3, &self.bytes)
            .bool(4, self.truncate)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut request, mut path, mut offset, mut bytes, mut truncate) =
            (None, None, None, None, None);
        codec::read(body, "write", |k, d| {
            match k {
                0 => request = Some(bound::u32(d, "request")?),
                1 => path = Some(path_field(d, "path")?),
                2 => offset = Some(bound::u64(d, "offset")?),
                3 => bytes = Some(bound::bytes(d, "bytes", MAX_RANGE)?),
                4 => truncate = Some(bound::bool(d, "truncate")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        let path = required(path, "path")?;
        if path.is_empty() {
            return Err(DecodeError::Malformed("write path"));
        }
        Ok(Self {
            request: required(request, "request")?,
            path,
            offset: offset.unwrap_or(0),
            bytes: bytes.unwrap_or_default(),
            truncate: truncate.unwrap_or(false),
        })
    }
}

impl Done {
    pub const KIND: u16 = 9;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(3)
            .u32(0, self.request)
            .bool(1, self.ok)
            .text(2, &self.error)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut request, mut ok, mut error) = (None, None, None);
        codec::read(body, "done", |k, d| {
            match k {
                0 => request = Some(bound::u32(d, "request")?),
                1 => ok = Some(bound::bool(d, "ok")?),
                2 => error = Some(bound::text(d, "error", MAX_TEXT)?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            request: required(request, "request")?,
            ok: required(ok, "ok")?,
            error: error.unwrap_or_default(),
        })
    }
}

impl Mkdir {
    pub const KIND: u16 = 10;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2)
            .u32(0, self.request)
            .text(1, &self.path)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (request, path) = decode_request_path(body, "mkdir")?;
        if path.is_empty() {
            return Err(DecodeError::Malformed("mkdir path"));
        }
        Ok(Self { request, path })
    }
}

impl Rename {
    pub const KIND: u16 = 11;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(3)
            .u32(0, self.request)
            .text(1, &self.from)
            .text(2, &self.to)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut request, mut from, mut to) = (None, None, None);
        codec::read(body, "rename", |k, d| {
            match k {
                0 => request = Some(bound::u32(d, "request")?),
                1 => from = Some(path_field(d, "from")?),
                2 => to = Some(path_field(d, "to")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        let (from, to) = (required(from, "from")?, required(to, "to")?);
        if from.is_empty() || to.is_empty() {
            return Err(DecodeError::Malformed("rename path"));
        }
        Ok(Self {
            request: required(request, "request")?,
            from,
            to,
        })
    }
}

impl Delete {
    pub const KIND: u16 = 12;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2)
            .u32(0, self.request)
            .text(1, &self.path)
            .finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (request, path) = decode_request_path(body, "delete")?;
        if path.is_empty() {
            return Err(DecodeError::Malformed("delete path"));
        }
        Ok(Self { request, path })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::testing::{hex, unhex};

    const LIST: &str = "a300070164446f63730218ff";

    #[test]
    fn golden_vectors_round_trip() {
        let l = List {
            request: 7,
            path: "Docs".into(),
            offset: 255,
        };
        assert_eq!(hex(&l.encode()), LIST);
        assert_eq!(List::decode(&unhex(LIST)).unwrap(), l);
        let listing = Listing {
            request: 7,
            entries: vec![Entry {
                name: "a.txt".into(),
                dir: false,
                size: 3,
                mtime_ms: 1_700_000_000_000,
            }],
            more: true,
            error: String::new(),
        };
        assert_eq!(Listing::decode(&listing.encode()).unwrap(), listing);
        let none = StatReply {
            request: 1,
            entry: None,
        };
        assert_eq!(StatReply::decode(&none.encode()).unwrap(), none);
        let some = StatReply {
            request: 1,
            entry: Some(listing.entries[0].clone()),
        };
        assert_eq!(StatReply::decode(&some.encode()).unwrap(), some);
        let r = Read {
            request: 2,
            path: "a/b".into(),
            offset: 10,
            len: 20,
        };
        assert_eq!(Read::decode(&r.encode()).unwrap(), r);
        let w = Write {
            request: 3,
            path: "a/b".into(),
            offset: 0,
            bytes: vec![1, 2, 3],
            truncate: true,
        };
        assert_eq!(Write::decode(&w.encode()).unwrap(), w);
        let d = Data {
            request: 2,
            bytes: vec![9],
            error: String::new(),
        };
        assert_eq!(Data::decode(&d.encode()).unwrap(), d);
        let done = Done {
            request: 3,
            ok: false,
            error: "no space".into(),
        };
        assert_eq!(Done::decode(&done.encode()).unwrap(), done);
        let rn = Rename {
            request: 4,
            from: "a".into(),
            to: "b/c".into(),
        };
        assert_eq!(Rename::decode(&rn.encode()).unwrap(), rn);
        assert_eq!(
            Mkdir::decode(
                &Mkdir {
                    request: 5,
                    path: "d".into()
                }
                .encode()
            )
            .unwrap()
            .path,
            "d"
        );
        assert_eq!(
            Delete::decode(
                &Delete {
                    request: 6,
                    path: "d".into()
                }
                .encode()
            )
            .unwrap()
            .path,
            "d"
        );
        assert!(
            StorageState::decode(&StorageState { available: true }.encode())
                .unwrap()
                .available
        );
    }

    #[test]
    fn paths_that_escape_or_hide_are_refused() {
        for bad in ["../x", "a//b", "/a", "a/", ".", "a/./b", "a\0b"] {
            let l = List {
                request: 1,
                path: bad.into(),
                offset: 0,
            };
            assert!(List::decode(&l.encode()).is_err(), "{bad}");
        }
        assert!(Delete::decode(
            &Delete {
                request: 1,
                path: String::new()
            }
            .encode()
        )
        .is_err());
        let big = Read {
            request: 1,
            path: "a".into(),
            offset: 0,
            len: MAX_RANGE as u32 + 1,
        };
        assert!(Read::decode(&big.encode()).is_err());
        // A full range answers inside the envelope's bound with room to spare.
        let full = Data {
            request: 1,
            bytes: vec![0; MAX_RANGE],
            error: String::new(),
        };
        assert!(full.encode().len() <= MAX_BYTES);
        assert_eq!(Data::decode(&full.encode()).unwrap().bytes.len(), MAX_RANGE);
    }
}
