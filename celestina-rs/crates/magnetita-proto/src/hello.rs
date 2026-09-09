//! The hello: who a device is and what it can do, and how two hellos agree.
//!
//! The first envelope on a connection, both ways, is a [`Hello`] (capability
//! [`capability::HELLO`], kind 0). It names the device and lists the
//! capabilities this build offers with the version of each; [`negotiate`]
//! keeps the ones both sides offer, at the lower of the two versions. A
//! capability the other side does not list is simply not used — never an
//! error — which is how a newer build talks to an older one.
//!
//! ```text
//! {0: device_id text, 1: device_name text, 2: device_kind u8,
//!  3: capabilities [[capability u16, version u16], …]}
//! ```

use minicbor::{Decoder, Encoder};

use crate::bound::{self, MAX_IDENT, MAX_LIST};
use crate::error::DecodeError;

/// The capability ids of the own protocol. Each has a module that owns its
/// message kinds and bodies; the envelope only routes by number.
pub mod capability {
    /// The hello itself.
    pub const HELLO: u16 = 0;
    pub const BATTERY: u16 = 1;
    pub const CLIPBOARD: u16 = 2;
    pub const NOTIFICATIONS: u16 = 3;
    pub const FIND: u16 = 4;
    pub const SHARE: u16 = 5;
    pub const MEDIA: u16 = 6;
    pub const COMMANDS: u16 = 7;
    pub const INPUT: u16 = 8;
    pub const MIRROR: u16 = 9;
    pub const SMS: u16 = 10;
    pub const CONTACTS: u16 = 11;
    pub const TELEPHONY: u16 = 12;
    pub const STORAGE: u16 = 13;
    /// The pairing exchange, before any trust exists; see [`crate::pair`].
    pub const PAIRING: u16 = 14;
}

/// What kind of device sent the hello — decides which side of an asymmetric
/// capability it plays (the phone captures the mirror, the desktop shows it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Desktop,
    Phone,
}

impl DeviceKind {
    fn to_wire(self) -> u8 {
        match self {
            Self::Desktop => 0,
            Self::Phone => 1,
        }
    }

    fn from_wire(v: u8) -> Result<Self, DecodeError> {
        match v {
            0 => Ok(Self::Desktop),
            1 => Ok(Self::Phone),
            _ => Err(DecodeError::OutOfRange("device_kind")),
        }
    }
}

/// One offered capability and the highest version of it this build speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapabilityVersion {
    pub capability: u16,
    pub version: u16,
}

/// The first message on a connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hello {
    /// Stable per device, at most [`MAX_IDENT`] bytes; the trust store keys
    /// on the certificate fingerprint, not on this, so it is a label.
    pub device_id: String,
    /// What a person sees, at most [`MAX_IDENT`] bytes, rendered as plain text.
    pub device_name: String,
    pub device_kind: DeviceKind,
    /// At most [`MAX_LIST`] entries. Duplicates are refused.
    pub capabilities: Vec<CapabilityVersion>,
}

const KEY_ID: u32 = 0;
const KEY_NAME: u32 = 1;
const KEY_KIND: u32 = 2;
const KEY_CAPABILITIES: u32 = 3;

impl Hello {
    /// The kind of the envelope that carries a hello.
    pub const KIND: u16 = 0;

    pub fn encode(&self) -> Vec<u8> {
        let mut e = Encoder::new(Vec::new());
        e.map(4)
            .unwrap()
            .u32(KEY_ID)
            .unwrap()
            .str(&self.device_id)
            .unwrap()
            .u32(KEY_NAME)
            .unwrap()
            .str(&self.device_name)
            .unwrap()
            .u32(KEY_KIND)
            .unwrap()
            .u8(self.device_kind.to_wire())
            .unwrap()
            .u32(KEY_CAPABILITIES)
            .unwrap()
            .array(self.capabilities.len() as u64)
            .unwrap();
        for c in &self.capabilities {
            e.array(2)
                .unwrap()
                .u16(c.capability)
                .unwrap()
                .u16(c.version)
                .unwrap();
        }
        e.into_writer()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        bound::check_message_size(bytes)?;
        let mut d = Decoder::new(bytes);
        let pairs = bound::map(&mut d, "hello", 16)?;
        let (mut id, mut name, mut kind, mut caps) = (None, None, None, None);
        for _ in 0..pairs {
            match bound::key(&mut d)? {
                KEY_ID => id = Some(bound::text(&mut d, "device_id", MAX_IDENT)?),
                KEY_NAME => name = Some(bound::text(&mut d, "device_name", MAX_IDENT)?),
                KEY_KIND => kind = Some(DeviceKind::from_wire(bound::u8(&mut d, "device_kind")?)?),
                KEY_CAPABILITIES => {
                    let n = bound::array(&mut d, "capabilities", MAX_LIST)?;
                    let mut list: Vec<CapabilityVersion> = Vec::with_capacity(n);
                    for _ in 0..n {
                        if bound::array(&mut d, "capability", 2)? != 2 {
                            return Err(DecodeError::Malformed("capability pair"));
                        }
                        let entry = CapabilityVersion {
                            capability: bound::u16(&mut d, "capability")?,
                            version: bound::u16(&mut d, "capability version")?,
                        };
                        if list.iter().any(|c| c.capability == entry.capability) {
                            return Err(DecodeError::Malformed("duplicate capability"));
                        }
                        list.push(entry);
                    }
                    caps = Some(list);
                }
                _ => bound::skip(&mut d)?,
            }
        }
        Ok(Self {
            device_id: id.ok_or(DecodeError::MissingField("device_id"))?,
            device_name: name.ok_or(DecodeError::MissingField("device_name"))?,
            device_kind: kind.ok_or(DecodeError::MissingField("device_kind"))?,
            capabilities: caps.ok_or(DecodeError::MissingField("capabilities"))?,
        })
    }
}

/// The capabilities both sides offer, each at the lower of the two versions,
/// in the order `local` lists them. Anything only one side offers is left
/// out, which is the whole of forward and backward compatibility.
pub fn negotiate(
    local: &[CapabilityVersion],
    remote: &[CapabilityVersion],
) -> Vec<CapabilityVersion> {
    local
        .iter()
        .filter_map(|l| {
            remote
                .iter()
                .find(|r| r.capability == l.capability)
                .map(|r| CapabilityVersion {
                    capability: l.capability,
                    version: l.version.min(r.version),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    fn sample() -> Hello {
        Hello {
            device_id: "0eb21b28ae74d54c".into(),
            device_name: "Escritorio".into(),
            device_kind: DeviceKind::Desktop,
            capabilities: vec![
                CapabilityVersion {
                    capability: capability::BATTERY,
                    version: 1,
                },
                CapabilityVersion {
                    capability: capability::MIRROR,
                    version: 2,
                },
            ],
        }
    }

    /// Committed wire bytes of [`sample`].
    const VECTOR: &str =
        "a4007030656232316232386165373464353463016a4573637269746f72696f02000382820101820902";

    #[test]
    fn golden_vector_round_trips() {
        assert_eq!(hex(&sample().encode()), VECTOR);
        assert_eq!(Hello::decode(&unhex(VECTOR)).unwrap(), sample());
    }

    #[test]
    fn negotiation_keeps_the_intersection_at_the_lower_version() {
        let local = sample().capabilities;
        let remote = vec![
            CapabilityVersion {
                capability: capability::MIRROR,
                version: 1,
            },
            CapabilityVersion {
                capability: capability::SMS,
                version: 1,
            },
        ];
        assert_eq!(
            negotiate(&local, &remote),
            vec![CapabilityVersion {
                capability: capability::MIRROR,
                version: 1
            }]
        );
        assert!(negotiate(&local, &[]).is_empty());
    }

    #[test]
    fn a_name_over_the_bound_is_refused() {
        let long = Hello {
            device_name: "n".repeat(MAX_IDENT + 1),
            ..sample()
        };
        assert_eq!(
            Hello::decode(&long.encode()),
            Err(DecodeError::TooLong {
                what: "device_name",
                max: MAX_IDENT,
                len: MAX_IDENT + 1
            })
        );
    }

    #[test]
    fn too_many_capabilities_are_refused_at_the_header() {
        let many = Hello {
            capabilities: (0..=MAX_LIST as u16)
                .map(|i| CapabilityVersion {
                    capability: i,
                    version: 1,
                })
                .collect(),
            ..sample()
        };
        assert_eq!(
            Hello::decode(&many.encode()),
            Err(DecodeError::TooMany {
                what: "capabilities",
                max: MAX_LIST,
                len: MAX_LIST + 1
            })
        );
    }

    #[test]
    fn a_duplicate_capability_is_refused() {
        let dup = Hello {
            capabilities: vec![
                CapabilityVersion {
                    capability: 1,
                    version: 1,
                },
                CapabilityVersion {
                    capability: 1,
                    version: 2,
                },
            ],
            ..sample()
        };
        assert_eq!(
            Hello::decode(&dup.encode()),
            Err(DecodeError::Malformed("duplicate capability"))
        );
    }

    #[test]
    fn an_unknown_device_kind_is_refused() {
        let mut bytes = unhex(VECTOR);
        // The kind value sits right after key 2.
        let at = bytes.windows(2).position(|w| w == [0x02, 0x00]).unwrap() + 1;
        bytes[at] = 7;
        assert_eq!(
            Hello::decode(&bytes),
            Err(DecodeError::OutOfRange("device_kind"))
        );
    }

    #[test]
    fn a_hello_with_no_capabilities_is_valid() {
        let none = Hello {
            capabilities: vec![],
            ..sample()
        };
        assert_eq!(Hello::decode(&none.encode()).unwrap(), none);
    }
}
