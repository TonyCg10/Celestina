//! The phone's raw stream cut into access units, and the least a new
//! reader needs to start decoding at once: the parameter sets and the last
//! key frame. The phone ends every access unit with a delimiter, which is
//! where this cuts; a reader that opens mid-stream would otherwise wait for
//! the next key frame, ten seconds apart at most, and show nothing until
//! then.

use magnetita_proto::mirror::Codec;

/// Cuts the stream at access unit delimiters and keeps what a new reader
/// needs.
pub(crate) struct AccessUnits {
    codec: Codec,
    pending: Vec<u8>,
    /// The parameter sets seen (VPS, SPS, PPS), in order, deduplicated by
    /// their bytes.
    params: Vec<Vec<u8>>,
    /// The last access unit that started with a key frame.
    last_key: Option<Vec<u8>>,
}

impl AccessUnits {
    pub(crate) fn new(codec: Codec) -> Self {
        Self {
            codec,
            pending: Vec::new(),
            params: Vec::new(),
            last_key: None,
        }
    }

    /// Feeds bytes as they arrive; returns every access unit they complete.
    pub(crate) fn push(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.pending.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some(end) = self.delimiter_end() {
            let unit: Vec<u8> = self.pending.drain(..end).collect();
            self.note(&unit);
            out.push(unit);
        }
        out
    }

    /// What a reader opening now must see first: the parameter sets, then
    /// the last key frame, or nothing when no key frame has passed yet.
    pub(crate) fn snapshot(&self) -> Vec<u8> {
        let Some(key) = &self.last_key else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for p in &self.params {
            out.extend_from_slice(p);
        }
        out.extend_from_slice(key);
        out
    }

    /// The end of the first complete access unit in `pending`: just after
    /// the delimiter NAL that closes it.
    fn delimiter_end(&self) -> Option<usize> {
        let mut at = 0;
        while let Some(start) = find_start_code(&self.pending, at) {
            let header = start + start_code_len(&self.pending, start);
            if header >= self.pending.len() {
                return None;
            }
            let kind = self.nal_type(&self.pending[header..]);
            if kind == self.delimiter_type() {
                // The delimiter's own bytes end the unit: two bytes of header
                // for HEVC (plus its one payload byte), one plus one for H.264.
                let len = match self.codec {
                    Codec::Hevc => 3,
                    Codec::H264 => 2,
                };
                let end = (header + len).min(self.pending.len());
                if end == self.pending.len() && self.pending.len() < header + len {
                    return None;
                }
                return Some(end);
            }
            at = header;
        }
        None
    }

    fn delimiter_type(&self) -> u8 {
        match self.codec {
            Codec::Hevc => 35,
            Codec::H264 => 9,
        }
    }

    fn nal_type(&self, nal: &[u8]) -> u8 {
        match self.codec {
            Codec::Hevc => (nal[0] >> 1) & 0x3f,
            Codec::H264 => nal[0] & 0x1f,
        }
    }

    /// Records the unit's parameter sets and whether it is a key frame.
    fn note(&mut self, unit: &[u8]) {
        let mut key = false;
        let mut at = 0;
        let mut starts = Vec::new();
        while let Some(start) = find_start_code(unit, at) {
            starts.push(start);
            at = start + start_code_len(unit, start);
        }
        for (i, &start) in starts.iter().enumerate() {
            let end = starts.get(i + 1).copied().unwrap_or(unit.len());
            let header = start + start_code_len(unit, start);
            if header >= end {
                continue;
            }
            let kind = self.nal_type(&unit[header..]);
            let is_param = match self.codec {
                Codec::Hevc => (32..=34).contains(&kind),
                Codec::H264 => kind == 7 || kind == 8,
            };
            let is_key = match self.codec {
                Codec::Hevc => (16..=21).contains(&kind),
                Codec::H264 => kind == 5,
            };
            if is_param {
                let nal = unit[start..end].to_vec();
                if !self.params.contains(&nal) {
                    self.params.push(nal);
                }
            }
            key |= is_key;
        }
        if key {
            self.last_key = Some(unit.to_vec());
        }
    }
}

/// The offset of the next Annex B start code (`00 00 01`, with or without a
/// leading zero) at or after `from`.
fn find_start_code(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 3 <= bytes.len() {
        if bytes[i] == 0 && bytes[i + 1] == 0 && bytes[i + 2] == 1 {
            // A four-byte code starts one earlier.
            if i > from && bytes[i - 1] == 0 {
                return Some(i - 1);
            }
            return Some(i);
        }
        i += 1;
    }
    None
}

fn start_code_len(bytes: &[u8], start: usize) -> usize {
    if bytes.len() >= start + 4 && bytes[start..start + 4] == [0, 0, 0, 1] {
        4
    } else {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nal(kind: u8, payload: &[u8]) -> Vec<u8> {
        let mut v = vec![0, 0, 0, 1, kind << 1, 1];
        v.extend_from_slice(payload);
        v
    }

    fn aud() -> Vec<u8> {
        vec![0, 0, 0, 1, 0x46, 0x01, 0x50]
    }

    #[test]
    fn units_are_cut_at_the_delimiter_across_chunk_boundaries() {
        let mut units = AccessUnits::new(Codec::Hevc);
        let mut stream = Vec::new();
        stream.extend(nal(32, b"vps"));
        stream.extend(nal(33, b"sps"));
        stream.extend(nal(34, b"pps"));
        stream.extend(nal(19, b"idr-frame"));
        stream.extend(aud());
        stream.extend(nal(1, b"p-frame"));
        stream.extend(aud());
        let mut out = Vec::new();
        for chunk in stream.chunks(5) {
            out.extend(units.push(chunk));
        }
        assert_eq!(out.len(), 2);
        assert!(out[0].ends_with(&aud()));
        assert!(out[1].starts_with(&nal(1, b"p-frame")));
        // A reader opening now gets the parameter sets and the key unit.
        let snapshot = units.snapshot();
        assert!(snapshot.starts_with(&nal(32, b"vps")));
        assert!(snapshot.ends_with(&aud()));
        assert!(snapshot.windows(9).any(|w| w == b"idr-frame"));
        assert!(!snapshot.windows(7).any(|w| w == b"p-frame"));
    }

    #[test]
    fn no_key_frame_yet_means_no_snapshot() {
        let mut units = AccessUnits::new(Codec::Hevc);
        units.push(&nal(1, b"p"));
        units.push(&aud());
        assert!(units.snapshot().is_empty());
    }
}
