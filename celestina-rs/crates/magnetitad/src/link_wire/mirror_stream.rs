//! The phone's raw stream cut into access units, each marked as a key
//! frame or not. The phone ends every access unit with a delimiter, which
//! is where this cuts. A reader that opens mid-stream is started at a key
//! frame the daemon asks the phone for, so it decodes from its next frame.

use magnetita_proto::mirror::Codec;

/// Cuts the stream at access unit delimiters and keeps the parameter
/// sets, which the phone's encoder sends once at its start and a reader
/// starting later cannot decode without.
pub(crate) struct AccessUnits {
    codec: Codec,
    pending: Vec<u8>,
    /// The parameter sets seen (VPS, SPS, PPS), in order, deduplicated by
    /// their bytes; each with its start code.
    params: Vec<Vec<u8>>,
}

impl AccessUnits {
    pub(crate) fn new(codec: Codec) -> Self {
        Self {
            codec,
            pending: Vec::new(),
            params: Vec::new(),
        }
    }

    /// Every parameter set seen so far, in order, as one Annex B run.
    pub(crate) fn params(&self) -> Vec<u8> {
        self.params.concat()
    }

    /// Feeds bytes as they arrive; returns every access unit they complete
    /// with whether it carries a key frame.
    pub(crate) fn push(&mut self, bytes: &[u8]) -> Vec<(Vec<u8>, bool)> {
        self.pending.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some(end) = self.delimiter_end() {
            let unit: Vec<u8> = self.pending.drain(..end).collect();
            let key = self.note(&unit);
            self.remember_params(&unit);
            out.push((unit, key));
        }
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

    /// Records the parameter set NALs the unit carries.
    fn remember_params(&mut self, unit: &[u8]) {
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
            if is_param {
                let nal = unit[start..end].to_vec();
                if !self.params.contains(&nal) {
                    self.params.push(nal);
                }
            }
        }
    }

    /// Whether the unit carries a key frame.
    fn note(&self, unit: &[u8]) -> bool {
        let mut at = 0;
        let mut starts = Vec::new();
        while let Some(start) = find_start_code(unit, at) {
            starts.push(start);
            at = start + start_code_len(unit, start);
        }
        starts.iter().enumerate().any(|(i, &start)| {
            let end = starts.get(i + 1).copied().unwrap_or(unit.len());
            let header = start + start_code_len(unit, start);
            if header >= end {
                return false;
            }
            let kind = self.nal_type(&unit[header..]);
            match self.codec {
                Codec::Hevc => (16..=21).contains(&kind),
                Codec::H264 => kind == 5,
            }
        })
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
        assert!(
            out[0].0.ends_with(&aud()) && out[0].1,
            "the first unit is the key frame"
        );
        assert!(out[1].0.starts_with(&nal(1, b"p-frame")) && !out[1].1);
    }

    #[test]
    fn a_unit_without_a_key_frame_is_not_one() {
        let mut units = AccessUnits::new(Codec::Hevc);
        units.push(&nal(1, b"p"));
        let out = units.push(&aud());
        assert_eq!(out.len(), 1);
        assert!(!out[0].1);
    }
}
