//! `commands` (capability 7): the desktop publishes the commands the author
//! registered, by id and name only; the phone runs one by id. No command
//! line ever crosses the wire — that is the whole design.

use crate::bound::{self, MAX_IDENT, MAX_LIST};
use crate::codec::{self, required, Map};
use crate::error::DecodeError;

/// One registered command as the phone sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandEntry {
    pub id: u32,
    pub name: String,
}

/// Desktop → phone: everything that can be run. Kind 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandList {
    /// At most [`MAX_LIST`]; ids unique.
    pub commands: Vec<CommandEntry>,
}

/// Phone → desktop: run this one. Kind 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandRun {
    pub id: u32,
}

/// Desktop → phone: how the run ended. Kind 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandResult {
    pub id: u32,
    /// The process exited with status 0 within its budget.
    pub ok: bool,
}

impl CommandList {
    pub const KIND: u16 = 1;

    pub fn encode(&self) -> Vec<u8> {
        let mut m = Map::new(1).list(0, self.commands.len());
        for c in &self.commands {
            m = m.item(&Map::new(2).u32(0, c.id).text(1, &c.name).finish());
        }
        m.finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let mut commands = None;
        codec::read(body, "commands", |k, d| {
            match k {
                0 => {
                    commands = Some(codec::read_list(d, "commands", MAX_LIST, |d| {
                        let (mut id, mut name) = (None, None);
                        codec::read_map(d, "command", |k, d| {
                            match k {
                                0 => id = Some(bound::u32(d, "id")?),
                                1 => name = Some(bound::text(d, "name", MAX_IDENT)?),
                                _ => return Ok(false),
                            }
                            Ok(true)
                        })?;
                        Ok(CommandEntry {
                            id: required(id, "id")?,
                            name: required(name, "name")?,
                        })
                    })?)
                }
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        let commands = required(commands, "commands")?;
        for (i, c) in commands.iter().enumerate() {
            if commands[..i].iter().any(|o| o.id == c.id) {
                return Err(DecodeError::Malformed("duplicate command id"));
            }
        }
        Ok(Self { commands })
    }
}

fn decode_id(body: &[u8], what: &'static str) -> Result<(u32, Option<bool>), DecodeError> {
    let (mut id, mut ok) = (None, None);
    codec::read(body, what, |k, d| {
        match k {
            0 => id = Some(bound::u32(d, "id")?),
            1 => ok = Some(bound::bool(d, "ok")?),
            _ => return Ok(false),
        }
        Ok(true)
    })?;
    Ok((required(id, "id")?, ok))
}

impl CommandRun {
    pub const KIND: u16 = 2;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(1).u32(0, self.id).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        Ok(Self {
            id: decode_id(body, "run")?.0,
        })
    }
}

impl CommandResult {
    pub const KIND: u16 = 3;

    pub fn encode(&self) -> Vec<u8> {
        Map::new(2).u32(0, self.id).bool(1, self.ok).finish()
    }

    pub fn decode(body: &[u8]) -> Result<Self, DecodeError> {
        let (id, ok) = decode_id(body, "result")?;
        Ok(Self {
            id,
            ok: required(ok, "ok")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::testing::{hex, unhex};

    const VECTOR: &str = "a10082a20001016953757370656e646572a200070168426c6f7175656172";

    #[test]
    fn golden_vector_round_trips() {
        let m = CommandList {
            commands: vec![
                CommandEntry {
                    id: 1,
                    name: "Suspender".into(),
                },
                CommandEntry {
                    id: 7,
                    name: "Bloquear".into(),
                },
            ],
        };
        assert_eq!(hex(&m.encode()), VECTOR);
        assert_eq!(CommandList::decode(&unhex(VECTOR)).unwrap(), m);
        let r = CommandRun { id: 7 };
        assert_eq!(hex(&r.encode()), "a10007");
        assert_eq!(CommandRun::decode(&r.encode()).unwrap(), r);
        let res = CommandResult { id: 7, ok: true };
        assert_eq!(CommandResult::decode(&res.encode()).unwrap(), res);
        assert_eq!(
            CommandResult::decode(&r.encode()),
            Err(DecodeError::MissingField("ok"))
        );
    }

    #[test]
    fn duplicate_ids_are_refused() {
        let m = CommandList {
            commands: vec![
                CommandEntry {
                    id: 1,
                    name: "a".into(),
                },
                CommandEntry {
                    id: 1,
                    name: "b".into(),
                },
            ],
        };
        assert_eq!(
            CommandList::decode(&m.encode()),
            Err(DecodeError::Malformed("duplicate command id"))
        );
    }

    #[test]
    fn a_command_carries_no_command_line() {
        // The name is bounded like an identifier, not like a text body.
        let m = CommandList {
            commands: vec![CommandEntry {
                id: 1,
                name: "x".repeat(MAX_IDENT + 1),
            }],
        };
        assert_eq!(
            CommandList::decode(&m.encode()),
            Err(DecodeError::TooLong {
                what: "name",
                max: MAX_IDENT,
                len: MAX_IDENT + 1
            })
        );
    }
}
