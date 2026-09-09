//! `contacts` (capability 11): one-way sync of the phone's contacts as
//! vCard 4.0, versioned per contact so only changes travel.

use crate::bound::{self, MAX_LIST, MAX_VCARD};
use crate::codec::{self, required, Map};
use crate::error::DecodeError;

/// Desktop → phone: send what changed since `since_version` (0 for all). Kind 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContactsRequest {
    pub since_version: u64,
}

/// One contact, as the phone's vCard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contact {
    pub id: u64,
    /// The version at which this vCard was last changed.
    pub version: u64,
    /// vCard 4.0 text, at most [`MAX_VCARD`] bytes.
    pub vcard: String,
}

/// Phone → desktop: a page of changes. Kind 2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContactsSync {
    /// The phone's current version; the next request names it.
    pub version: u64,
    /// At most [`MAX_LIST`] per message; more pages follow until `complete`.
    pub contacts: Vec<Contact>,
    /// Ids deleted since the requested version, at most [`MAX_LIST`].
    pub removed: Vec<u64>,
    pub complete: bool,
}

impl ContactsRequest {
    pub const KIND: u16 = 1;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(1).u64(0, self.since_version).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let mut since = 0;
        codec::read(body, "contacts request", |k, d| {
            match k {
                0 => since = bound::u64(d, "since_version")?,
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            since_version: since,
        })
    }
}

impl ContactsSync {
    pub const KIND: u16 = 2;

    pub fn encode(&self) -> Vec<u8> {
        let mut m = Map::new(4)
            .u64(0, self.version)
            .list(1, self.contacts.len());
        for c in &self.contacts {
            m = m.item(
                &Map::new(3)
                    .u64(0, c.id)
                    .u64(1, c.version)
                    .text(2, &c.vcard)
                    .finish(),
            );
        }
        m = m.list(2, self.removed.len());
        for r in &self.removed {
            m = m.u64_item(*r);
        }
        m.bool(3, self.complete).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (mut version, mut contacts, mut removed, mut complete) = (None, None, None, None);
        codec::read(body, "contacts", |k, d| {
            match k {
                0 => version = Some(bound::u64(d, "version")?),
                1 => {
                    contacts = Some(codec::read_list(d, "contacts", MAX_LIST, |d| {
                        let (mut id, mut ver, mut vcard) = (None, None, None);
                        codec::read_map(d, "contact", |k, d| {
                            match k {
                                0 => id = Some(bound::u64(d, "id")?),
                                1 => ver = Some(bound::u64(d, "contact version")?),
                                2 => vcard = Some(bound::text(d, "vcard", MAX_VCARD)?),
                                _ => return Ok(false),
                            }
                            Ok(true)
                        })?;
                        Ok(Contact {
                            id: required(id, "id")?,
                            version: required(ver, "contact version")?,
                            vcard: required(vcard, "vcard")?,
                        })
                    })?)
                }
                2 => {
                    removed = Some(codec::read_list(d, "removed", MAX_LIST, |d| {
                        bound::u64(d, "removed id")
                    })?)
                }
                3 => complete = Some(bound::bool(d, "complete")?),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(Self {
            version: required(version, "version")?,
            contacts: contacts.unwrap_or_default(),
            removed: removed.unwrap_or_default(),
            complete: required(complete, "complete")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::testing::{hex, unhex};

    const VECTOR: &str = "a400182a0181a3000901182902782d424547494e3a56434152440d0a56455253494f4e3a342e300d0a464e3a416e610d0a454e443a56434152440d0a02810303f5";

    #[test]
    fn golden_vector_round_trips() {
        let m = ContactsSync {
            version: 42,
            contacts: vec![Contact {
                id: 9,
                version: 41,
                vcard: "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Ana\r\nEND:VCARD\r\n".into(),
            }],
            removed: vec![3],
            complete: true,
        };
        assert_eq!(hex(&m.encode()), VECTOR);
        assert_eq!(ContactsSync::decode(&unhex(VECTOR)).unwrap(), m);
        let r = ContactsRequest { since_version: 41 };
        assert_eq!(hex(&r.encode()), "a1001829");
        assert_eq!(ContactsRequest::decode(&r.encode()).unwrap(), r);
        assert_eq!(
            ContactsRequest::decode(&[0xa0]).unwrap(),
            ContactsRequest { since_version: 0 }
        );
    }

    #[test]
    fn a_vcard_over_the_bound_is_refused() {
        let m = ContactsSync {
            version: 1,
            contacts: vec![Contact {
                id: 1,
                version: 1,
                vcard: "x".repeat(MAX_VCARD + 1),
            }],
            removed: vec![],
            complete: true,
        };
        assert_eq!(
            ContactsSync::decode(&m.encode()),
            Err(DecodeError::TooLong {
                what: "vcard",
                max: MAX_VCARD,
                len: MAX_VCARD + 1
            })
        );
    }
}
