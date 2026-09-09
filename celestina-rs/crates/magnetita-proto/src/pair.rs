//! Pairing: the one moment trust is created, as two state machines.
//!
//! Afterwards a device is only ever its certificate fingerprint, pinned in
//! the trust store; pairing is how a fingerprint earns that pin. Two paths,
//! one outcome:
//!
//! - **QR.** The desktop shows a [`QrPayload`] — its id, its fingerprint,
//!   where it listens, and a one-time secret. The phone scans it, dials with
//!   the fingerprint pinned from the QR, and both sides prove possession of
//!   the secret over the encrypted channel with an HMAC over *both*
//!   fingerprints. The secret never crosses the network; a device that did
//!   not see the QR cannot produce the proof, and a proof cannot be replayed
//!   against another certificate.
//! - **Code.** A six-digit code is shown on one side and typed on the other.
//!   SPAKE2 turns it into a shared key that an eavesdropper cannot learn and
//!   an impostor gets one guess at; the confirmation MACs bind that key to
//!   both fingerprints.
//!
//! This crate holds the rules; it holds no clock, no socket and no random
//! source. The link supplies the fingerprints it saw on the TLS handshake and
//! the randomness SPAKE2 needs, and it pins the [`Pinned`] fingerprint that
//! comes out — or drops the connection on a [`PairError`], which is the only
//! other thing that can come out.

use std::fmt;

use minicbor::{Decoder, Encoder};
use rand_core::{CryptoRng, RngCore};
use spake2::{Ed25519Group, Identity, Password, Spake2};

use crate::bound::{self, MAX_IDENT};
use crate::error::DecodeError;

/// SHA-256 of a device's certificate: what the trust store remembers.
pub type Fingerprint = [u8; 32];

/// The result of pairing: pin this, and only this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pinned {
    pub peer_fingerprint: Fingerprint,
}

/// Why a pairing was refused. Each is terminal: the state machine that
/// returned it accepts nothing further, so a peer gets one attempt per
/// connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PairError {
    /// The QR proof did not match the secret and the two fingerprints.
    WrongProof,
    /// The code confirmation did not match: the other side typed a different
    /// code, or is not who the fingerprints say.
    WrongCode,
    /// A message arrived that this state cannot accept.
    OutOfOrder,
    /// The scanned text is not a Magnetita pairing payload.
    BadPayload(&'static str),
    /// The pairing message itself could not be decoded.
    Decode(DecodeError),
}

impl fmt::Display for PairError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongProof => f.write_str("the pairing proof does not match"),
            Self::WrongCode => f.write_str("the pairing code does not match"),
            Self::OutOfOrder => f.write_str("pairing message out of order"),
            Self::BadPayload(what) => write!(f, "bad pairing payload: {what}"),
            Self::Decode(e) => write!(f, "pairing message: {e}"),
        }
    }
}

impl std::error::Error for PairError {}

impl From<DecodeError> for PairError {
    fn from(e: DecodeError) -> Self {
        Self::Decode(e)
    }
}

/// Message kinds of the [`crate::capability::PAIRING`] capability.
pub mod kind {
    /// Phone → desktop: `{0: proof bytes}`.
    pub const QR_PROOF: u16 = 1;
    /// Desktop → phone: `{0: proof bytes}`.
    pub const QR_REPLY: u16 = 2;
    /// Either direction: `{0: spake2 message bytes}`.
    pub const CODE_EXCHANGE: u16 = 3;
    /// Either direction: `{0: confirmation mac bytes}`.
    pub const CODE_CONFIRM: u16 = 4;
}

fn hmac(key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, key);
    let mut ctx = ring::hmac::Context::with_key(&key);
    for p in parts {
        ctx.update(p);
    }
    let tag = ctx.sign();
    let mut out = [0u8; 32];
    out.copy_from_slice(tag.as_ref());
    out
}

fn hmac_verify(key: &[u8], parts: &[&[u8]], tag: &[u8]) -> bool {
    // Constant-time: the library compares, this code never does.
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, key);
    ring::hmac::verify(&key, &parts.concat(), tag).is_ok()
}

/// SHA-256 of a DER certificate: the one way a [`Fingerprint`] is made.
pub fn fingerprint(certificate_der: &[u8]) -> Fingerprint {
    let d = ring::digest::digest(&ring::digest::SHA256, certificate_der);
    let mut out = [0u8; 32];
    out.copy_from_slice(d.as_ref());
    out
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex_32(s: &str, what: &'static str) -> Result<[u8; 32], PairError> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(PairError::BadPayload(what));
    }
    let mut out = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        let pair = std::str::from_utf8(chunk).map_err(|_| PairError::BadPayload(what))?;
        out[i] = u8::from_str_radix(pair, 16).map_err(|_| PairError::BadPayload(what))?;
    }
    Ok(out)
}

// ---- one-message bodies ----------------------------------------------------

/// Every pairing message is `{0: bytes}`; this is that one shape.
fn encode_single(bytes: &[u8]) -> Vec<u8> {
    let mut e = Encoder::new(Vec::new());
    e.map(1).unwrap().u32(0).unwrap().bytes(bytes).unwrap();
    e.into_writer()
}

fn decode_single(body: &[u8], what: &'static str, max: usize) -> Result<Vec<u8>, DecodeError> {
    bound::check_message_size(body)?;
    let mut d = Decoder::new(body);
    let pairs = bound::map(&mut d, what, 8)?;
    let mut value = None;
    for _ in 0..pairs {
        match bound::key(&mut d)? {
            0 => value = Some(bound::bytes(&mut d, what, max)?),
            _ => bound::skip(&mut d)?,
        }
    }
    value.ok_or(DecodeError::MissingField(what))
}

/// A proof or a confirmation: exactly 32 bytes on the wire.
fn decode_mac(body: &[u8], what: &'static str) -> Result<[u8; 32], PairError> {
    let bytes = decode_single(body, what, 32)?;
    <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| DecodeError::Malformed(what).into())
}

// ---- QR ------------------------------------------------------------------------

/// What the desktop encodes in the QR: enough for the phone to dial it with
/// its certificate already pinned and to prove it saw this very code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrPayload {
    pub device_id: String,
    pub fingerprint: Fingerprint,
    pub secret: [u8; 32],
    /// `ip:port` strings the desktop listens on, at most [`bound::MAX_LIST`].
    pub addresses: Vec<String>,
}

const QR_SCHEME: &str = "magnetita://pair?";

impl QrPayload {
    /// The text the QR carries. Hex keeps it scannable and unambiguous.
    pub fn to_uri(&self) -> String {
        let mut s = format!(
            "{QR_SCHEME}v=1&id={}&fp={}&secret={}",
            self.device_id,
            hex(&self.fingerprint),
            hex(&self.secret)
        );
        for a in &self.addresses {
            s.push_str("&addr=");
            s.push_str(a);
        }
        s
    }

    /// Parses a scanned text, refusing anything that is not exactly this
    /// shape at version 1. Every field is bounded like a network value: the
    /// camera is a peer too.
    pub fn parse_uri(uri: &str) -> Result<Self, PairError> {
        let query = uri
            .strip_prefix(QR_SCHEME)
            .ok_or(PairError::BadPayload("scheme"))?;
        if query.len() > 4096 {
            return Err(PairError::BadPayload("length"));
        }
        let (mut version, mut id, mut fp, mut secret, mut addresses) =
            (None, None, None, None, Vec::new());
        for field in query.split('&') {
            let (k, v) = field
                .split_once('=')
                .ok_or(PairError::BadPayload("field"))?;
            match k {
                "v" => version = Some(v),
                "id" => {
                    if v.is_empty()
                        || v.len() > MAX_IDENT
                        || !v.bytes().all(|b| b.is_ascii_alphanumeric())
                    {
                        return Err(PairError::BadPayload("id"));
                    }
                    id = Some(v.to_owned());
                }
                "fp" => fp = Some(unhex_32(v, "fingerprint")?),
                "secret" => secret = Some(unhex_32(v, "secret")?),
                "addr" => {
                    if v.is_empty() || v.len() > MAX_IDENT || addresses.len() >= bound::MAX_LIST {
                        return Err(PairError::BadPayload("address"));
                    }
                    addresses.push(v.to_owned());
                }
                _ => {}
            }
        }
        if version != Some("1") {
            return Err(PairError::BadPayload("version"));
        }
        Ok(Self {
            device_id: id.ok_or(PairError::BadPayload("id"))?,
            fingerprint: fp.ok_or(PairError::BadPayload("fingerprint"))?,
            secret: secret.ok_or(PairError::BadPayload("secret"))?,
            addresses,
        })
    }
}

/// The QR path, either role. Built once per connection; consumed by its
/// terminal step.
#[derive(Debug)]
pub struct QrPairing {
    secret: [u8; 32],
    my_fingerprint: Fingerprint,
    peer_fingerprint: Fingerprint,
    role: QrRole,
    done: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum QrRole {
    Phone,
    Desktop,
}

impl QrPairing {
    /// The phone, after dialling the QR's address and seeing `peer` on the
    /// handshake. The link must already have refused a certificate whose
    /// fingerprint is not the QR's; this checks it again because two
    /// checks cost nothing and one forgotten check costs the trust store.
    pub fn phone(
        payload: &QrPayload,
        my_fingerprint: Fingerprint,
        peer: Fingerprint,
    ) -> Result<Self, PairError> {
        if peer != payload.fingerprint {
            return Err(PairError::WrongProof);
        }
        Ok(Self {
            secret: payload.secret,
            my_fingerprint,
            peer_fingerprint: peer,
            role: QrRole::Phone,
            done: false,
        })
    }

    /// The desktop, once a connection presenting `peer` has arrived.
    pub fn desktop(secret: [u8; 32], my_fingerprint: Fingerprint, peer: Fingerprint) -> Self {
        Self {
            secret,
            my_fingerprint,
            peer_fingerprint: peer,
            role: QrRole::Desktop,
            done: false,
        }
    }

    fn proof_of(&self, prover: &Fingerprint, verifier: &Fingerprint) -> [u8; 32] {
        hmac(&self.secret, &[b"magnetita-qr-proof", prover, verifier])
    }

    /// Phone only: the body of the [`kind::QR_PROOF`] message to send first.
    pub fn proof_to_send(&self) -> Result<Vec<u8>, PairError> {
        if self.role != QrRole::Phone || self.done {
            return Err(PairError::OutOfOrder);
        }
        Ok(encode_single(
            &self.proof_of(&self.my_fingerprint, &self.peer_fingerprint),
        ))
    }

    /// Desktop only: verifies the phone's proof and returns the
    /// [`kind::QR_REPLY`] body plus the fingerprint to pin. A wrong proof
    /// ends the pairing.
    pub fn accept_proof(&mut self, body: &[u8]) -> Result<(Vec<u8>, Pinned), PairError> {
        if self.role != QrRole::Desktop || self.done {
            return Err(PairError::OutOfOrder);
        }
        self.done = true;
        let proof = decode_mac(body, "proof")?;
        let ok = hmac_verify(
            &self.secret,
            &[
                b"magnetita-qr-proof",
                &self.peer_fingerprint,
                &self.my_fingerprint,
            ],
            &proof,
        );
        if !ok {
            return Err(PairError::WrongProof);
        }
        let reply = self.proof_of(&self.my_fingerprint, &self.peer_fingerprint);
        Ok((
            encode_single(&reply),
            Pinned {
                peer_fingerprint: self.peer_fingerprint,
            },
        ))
    }

    /// Phone only: verifies the desktop's reply; the fingerprint to pin.
    pub fn accept_reply(&mut self, body: &[u8]) -> Result<Pinned, PairError> {
        if self.role != QrRole::Phone || self.done {
            return Err(PairError::OutOfOrder);
        }
        self.done = true;
        let reply = decode_mac(body, "proof")?;
        let ok = hmac_verify(
            &self.secret,
            &[
                b"magnetita-qr-proof",
                &self.peer_fingerprint,
                &self.my_fingerprint,
            ],
            &reply,
        );
        if !ok {
            return Err(PairError::WrongProof);
        }
        Ok(Pinned {
            peer_fingerprint: self.peer_fingerprint,
        })
    }
}

// ---- six-digit code ------------------------------------------------------------

/// The code path, either role. The desktop always plays SPAKE2's side A and
/// the phone side B, so the two derive the same key from the same code.
pub struct CodePairing {
    state: Option<Spake2<Ed25519Group>>,
    key: Option<Vec<u8>>,
    my_fingerprint: Fingerprint,
    peer_fingerprint: Fingerprint,
    outbound: Vec<u8>,
    role: CodeRole,
    done: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CodeRole {
    Desktop,
    Phone,
}

impl fmt::Debug for CodePairing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CodePairing")
            .field("role", &self.role)
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}

impl CodePairing {
    /// Checks a code is exactly six ASCII digits; nothing else is typed.
    pub fn check_code(code: &str) -> Result<(), PairError> {
        if code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()) {
            Ok(())
        } else {
            Err(PairError::BadPayload("code"))
        }
    }

    /// Starts as the desktop (SPAKE2 side A) with the code it displays.
    pub fn desktop(
        code: &str,
        my: Fingerprint,
        peer: Fingerprint,
        rng: impl RngCore + CryptoRng,
    ) -> Result<Self, PairError> {
        Self::check_code(code)?;
        let (state, outbound) = Spake2::<Ed25519Group>::start_a_with_rng(
            &Password::new(code.as_bytes()),
            &Identity::new(b"magnetita-desktop"),
            &Identity::new(b"magnetita-phone"),
            rng,
        );
        Ok(Self {
            state: Some(state),
            key: None,
            my_fingerprint: my,
            peer_fingerprint: peer,
            outbound,
            role: CodeRole::Desktop,
            done: false,
        })
    }

    /// Starts as the phone (SPAKE2 side B) with the code the author typed.
    pub fn phone(
        code: &str,
        my: Fingerprint,
        peer: Fingerprint,
        rng: impl RngCore + CryptoRng,
    ) -> Result<Self, PairError> {
        Self::check_code(code)?;
        let (state, outbound) = Spake2::<Ed25519Group>::start_b_with_rng(
            &Password::new(code.as_bytes()),
            &Identity::new(b"magnetita-desktop"),
            &Identity::new(b"magnetita-phone"),
            rng,
        );
        Ok(Self {
            state: Some(state),
            key: None,
            my_fingerprint: my,
            peer_fingerprint: peer,
            outbound,
            role: CodeRole::Phone,
            done: false,
        })
    }

    /// The [`kind::CODE_EXCHANGE`] body to send; both sides send one.
    pub fn exchange_to_send(&self) -> Vec<u8> {
        encode_single(&self.outbound)
    }

    /// Takes the other side's exchange and returns the [`kind::CODE_CONFIRM`]
    /// body to send. A wrong code is not detected here — SPAKE2 yields *a*
    /// key either way — but at [`accept_confirm`](Self::accept_confirm).
    pub fn accept_exchange(&mut self, body: &[u8]) -> Result<Vec<u8>, PairError> {
        let state = self.state.take().ok_or(PairError::OutOfOrder)?;
        let inbound = decode_single(body, "exchange", 64)?;
        let key = state.finish(&inbound).map_err(|_| PairError::WrongCode)?;
        let confirm = hmac(
            &key,
            &[
                self.label(self.role),
                &self.my_fingerprint,
                &self.peer_fingerprint,
            ],
        );
        self.key = Some(key);
        Ok(encode_single(&confirm))
    }

    /// Verifies the other side's confirmation; the fingerprint to pin.
    pub fn accept_confirm(&mut self, body: &[u8]) -> Result<Pinned, PairError> {
        if self.done {
            return Err(PairError::OutOfOrder);
        }
        let key = self.key.take().ok_or(PairError::OutOfOrder)?;
        self.done = true;
        let confirm = decode_mac(body, "confirm")?;
        let other = match self.role {
            CodeRole::Desktop => CodeRole::Phone,
            CodeRole::Phone => CodeRole::Desktop,
        };
        if !hmac_verify(
            &key,
            &[
                self.label(other),
                &self.peer_fingerprint,
                &self.my_fingerprint,
            ],
            &confirm,
        ) {
            return Err(PairError::WrongCode);
        }
        Ok(Pinned {
            peer_fingerprint: self.peer_fingerprint,
        })
    }

    fn label(&self, role: CodeRole) -> &'static [u8] {
        match role {
            CodeRole::Desktop => b"magnetita-code-confirm-desktop",
            CodeRole::Phone => b"magnetita-code-confirm-phone",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic, for tests only: SPAKE2 needs *a* scalar, not a secret
    /// one, to prove the protocol's logic.
    struct Counter(u64);
    impl RngCore for Counter {
        fn next_u32(&mut self) -> u32 {
            self.next_u64() as u32
        }
        fn next_u64(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0
        }
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            for chunk in dest.chunks_mut(8) {
                let v = self.next_u64().to_le_bytes();
                chunk.copy_from_slice(&v[..chunk.len()]);
            }
        }
        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }
    impl CryptoRng for Counter {}

    const DESKTOP: Fingerprint = [0x11; 32];
    const PHONE: Fingerprint = [0x22; 32];
    const IMPOSTOR: Fingerprint = [0x33; 32];

    fn payload() -> QrPayload {
        QrPayload {
            device_id: "0eb21b28ae74d54c".into(),
            fingerprint: DESKTOP,
            secret: [0xab; 32],
            addresses: vec!["10.0.0.134:1762".into()],
        }
    }

    #[test]
    fn qr_payload_round_trips_through_its_uri() {
        let uri = payload().to_uri();
        assert!(uri.starts_with("magnetita://pair?v=1&id=0eb21b28ae74d54c&fp=1111"));
        assert_eq!(QrPayload::parse_uri(&uri).unwrap(), payload());
    }

    #[test]
    fn qr_payload_refuses_what_is_not_one() {
        assert_eq!(
            QrPayload::parse_uri("https://example.com").unwrap_err(),
            PairError::BadPayload("scheme")
        );
        let v2 = payload().to_uri().replace("v=1", "v=2");
        assert_eq!(
            QrPayload::parse_uri(&v2).unwrap_err(),
            PairError::BadPayload("version")
        );
        let short_fp = payload().to_uri().replace(&hex(&DESKTOP), "abcd");
        assert_eq!(
            QrPayload::parse_uri(&short_fp).unwrap_err(),
            PairError::BadPayload("fingerprint")
        );
        let no_secret = payload()
            .to_uri()
            .replace(&format!("&secret={}", hex(&[0xab; 32])), "");
        assert_eq!(
            QrPayload::parse_uri(&no_secret).unwrap_err(),
            PairError::BadPayload("secret")
        );
        let bad_id = payload()
            .to_uri()
            .replace("id=0eb21b28ae74d54c", "id=../etc");
        assert_eq!(
            QrPayload::parse_uri(&bad_id).unwrap_err(),
            PairError::BadPayload("id")
        );
    }

    #[test]
    fn qr_pairing_pins_both_ways() {
        let p = payload();
        let mut phone = QrPairing::phone(&p, PHONE, DESKTOP).unwrap();
        let mut desktop = QrPairing::desktop(p.secret, DESKTOP, PHONE);
        let proof = phone.proof_to_send().unwrap();
        let (reply, pinned_by_desktop) = desktop.accept_proof(&proof).unwrap();
        assert_eq!(
            pinned_by_desktop,
            Pinned {
                peer_fingerprint: PHONE
            }
        );
        assert_eq!(
            phone.accept_reply(&reply).unwrap(),
            Pinned {
                peer_fingerprint: DESKTOP
            }
        );
        // Terminal: nothing more is accepted from either side.
        assert_eq!(
            phone.accept_reply(&reply).unwrap_err(),
            PairError::OutOfOrder
        );
        assert_eq!(
            desktop.accept_proof(&proof).unwrap_err(),
            PairError::OutOfOrder
        );
    }

    #[test]
    fn qr_proof_is_bound_to_the_certificates() {
        let p = payload();
        // An impostor who saw the QR but presents another certificate.
        let phone = QrPairing::phone(&p, IMPOSTOR, DESKTOP).unwrap();
        let mut desktop = QrPairing::desktop(p.secret, DESKTOP, PHONE);
        assert_eq!(
            desktop
                .accept_proof(&phone.proof_to_send().unwrap())
                .unwrap_err(),
            PairError::WrongProof
        );
        // A phone that reached a desktop other than the QR's.
        assert_eq!(
            QrPairing::phone(&p, PHONE, IMPOSTOR).unwrap_err(),
            PairError::WrongProof
        );
        // The right phone with the wrong secret.
        let phone = QrPairing::phone(
            &QrPayload {
                secret: [0xcd; 32],
                ..p.clone()
            },
            PHONE,
            DESKTOP,
        )
        .unwrap();
        let mut desktop = QrPairing::desktop(p.secret, DESKTOP, PHONE);
        assert_eq!(
            desktop
                .accept_proof(&phone.proof_to_send().unwrap())
                .unwrap_err(),
            PairError::WrongProof
        );
    }

    #[test]
    fn qr_roles_refuse_the_other_roles_steps() {
        let p = payload();
        let mut phone = QrPairing::phone(&p, PHONE, DESKTOP).unwrap();
        let desktop = QrPairing::desktop(p.secret, DESKTOP, PHONE);
        assert_eq!(desktop.proof_to_send().unwrap_err(), PairError::OutOfOrder);
        assert_eq!(phone.accept_proof(&[]).unwrap_err(), PairError::OutOfOrder);
    }

    #[test]
    fn a_proof_of_the_wrong_size_is_refused_at_decode() {
        let p = payload();
        let mut desktop = QrPairing::desktop(p.secret, DESKTOP, PHONE);
        assert_eq!(
            desktop
                .accept_proof(&encode_single(&[1, 2, 3]))
                .unwrap_err(),
            PairError::Decode(DecodeError::Malformed("proof"))
        );
        let mut desktop = QrPairing::desktop(p.secret, DESKTOP, PHONE);
        assert_eq!(
            desktop.accept_proof(&encode_single(&[0; 33])).unwrap_err(),
            PairError::Decode(DecodeError::TooLong {
                what: "proof",
                max: 32,
                len: 33
            })
        );
    }

    fn run_code(
        desktop_code: &str,
        phone_code: &str,
    ) -> (Result<Pinned, PairError>, Result<Pinned, PairError>) {
        let mut d = CodePairing::desktop(desktop_code, DESKTOP, PHONE, Counter(1)).unwrap();
        let mut p = CodePairing::phone(phone_code, PHONE, DESKTOP, Counter(2)).unwrap();
        let (dx, px) = (d.exchange_to_send(), p.exchange_to_send());
        let dc = d.accept_exchange(&px).unwrap();
        let pc = p.accept_exchange(&dx).unwrap();
        (d.accept_confirm(&pc), p.accept_confirm(&dc))
    }

    #[test]
    fn code_pairing_pins_both_ways_with_the_same_code() {
        let (d, p) = run_code("282191", "282191");
        assert_eq!(
            d.unwrap(),
            Pinned {
                peer_fingerprint: PHONE
            }
        );
        assert_eq!(
            p.unwrap(),
            Pinned {
                peer_fingerprint: DESKTOP
            }
        );
    }

    #[test]
    fn code_pairing_rejects_a_code_off_by_one_on_both_sides() {
        let (d, p) = run_code("935810", "935811");
        assert_eq!(d.unwrap_err(), PairError::WrongCode);
        assert_eq!(p.unwrap_err(), PairError::WrongCode);
    }

    #[test]
    fn code_confirmation_is_bound_to_the_certificates() {
        // Same code, but the phone believes it is talking to another desktop.
        let mut d = CodePairing::desktop("123456", DESKTOP, PHONE, Counter(1)).unwrap();
        let mut p = CodePairing::phone("123456", PHONE, IMPOSTOR, Counter(2)).unwrap();
        let (dx, px) = (d.exchange_to_send(), p.exchange_to_send());
        let dc = d.accept_exchange(&px).unwrap();
        let pc = p.accept_exchange(&dx).unwrap();
        assert_eq!(d.accept_confirm(&pc).unwrap_err(), PairError::WrongCode);
        assert_eq!(p.accept_confirm(&dc).unwrap_err(), PairError::WrongCode);
    }

    #[test]
    fn code_pairing_is_single_use_and_ordered() {
        let mut d = CodePairing::desktop("123456", DESKTOP, PHONE, Counter(1)).unwrap();
        assert_eq!(
            d.accept_confirm(&encode_single(&[0; 32])).unwrap_err(),
            PairError::OutOfOrder
        );
        let p = CodePairing::phone("123456", PHONE, DESKTOP, Counter(2)).unwrap();
        let px = p.exchange_to_send();
        d.accept_exchange(&px).unwrap();
        assert_eq!(d.accept_exchange(&px).unwrap_err(), PairError::OutOfOrder);
        let dc = d.accept_exchange(&px).unwrap_err();
        assert_eq!(dc, PairError::OutOfOrder);
    }

    #[test]
    fn only_six_digits_are_a_code() {
        assert!(CodePairing::check_code("000000").is_ok());
        for bad in ["12345", "1234567", "12345a", "１２３４５６", ""] {
            assert_eq!(
                CodePairing::check_code(bad).unwrap_err(),
                PairError::BadPayload("code")
            );
        }
    }

    #[test]
    fn a_garbage_exchange_is_refused() {
        let mut d = CodePairing::desktop("123456", DESKTOP, PHONE, Counter(1)).unwrap();
        assert_eq!(
            d.accept_exchange(&encode_single(&[0; 33])).unwrap_err(),
            PairError::WrongCode
        );
        let mut d = CodePairing::desktop("123456", DESKTOP, PHONE, Counter(1)).unwrap();
        assert_eq!(
            d.accept_exchange(&encode_single(&[0; 65])).unwrap_err(),
            PairError::Decode(DecodeError::TooLong {
                what: "exchange",
                max: 64,
                len: 65
            })
        );
    }

    #[test]
    fn fingerprint_is_sha256_of_the_der() {
        assert_eq!(
            hex(&fingerprint(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
