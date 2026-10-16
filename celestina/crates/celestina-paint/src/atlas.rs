//! The glyph atlas bookkeeping: one 1024×1024 page, a shelf packer and a
//! least-recently-used map (design §5.3 and the §9 risk "the atlas is
//! bounded"). Pure: the painter uploads the texels to the GL page.
//!
//! Shelves are horizontal bands whose height is the glyph height rounded up
//! to a multiple of [`SHELF_STEP`]; within a shelf, free x ranges are kept
//! merged. When nothing fits, the least recently used entry is evicted and
//! the insert retried; a shelf left empty returns its band to the free
//! vertical space, so a page of evicted glyphs can be re-cut for any height.
//! Entries drawn since the last [`Atlas::begin_frame`] are never evicted:
//! their texels are referenced by quads still waiting for the flush.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

use cosmic_text::{fontdb, CacheKey, SubpixelBin};

/// Side of the one atlas page in texels.
pub(crate) const PAGE: u32 = 1024;

/// Shelf heights are multiples of this, to limit the variety of shelves.
const SHELF_STEP: u32 = 8;

/// What identifies one rasterised glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct GlyphKey {
    pub font_id: fontdb::ID,
    pub glyph_id: u16,
    /// The `wght` the glyph was rasterised at: Inter Variable is one face id
    /// for every weight.
    pub weight: u16,
    /// Font size in whole device pixels.
    pub size_px: u32,
    /// cosmic-text's subpixel bin of the glyph's x position, 0..=3.
    pub subpixel_quarter: u8,
}

impl GlyphKey {
    pub(crate) fn new(
        font_id: fontdb::ID,
        glyph_id: u16,
        weight: u16,
        size_px: u32,
        subpixel_quarter: u8,
    ) -> GlyphKey {
        GlyphKey {
            font_id,
            glyph_id,
            weight,
            size_px,
            subpixel_quarter,
        }
    }

    /// The key of a glyph the layout produced (text is laid out at whole
    /// device-pixel sizes, so the size is exact).
    pub(crate) fn from_cache_key(key: &CacheKey) -> GlyphKey {
        GlyphKey::new(
            key.font_id,
            key.glyph_id,
            key.font_weight.0,
            f32::from_bits(key.font_size_bits).round().max(0.0) as u32,
            quarter(key.x_bin),
        )
    }
}

/// A logical font size at a device pixel ratio, in whole device pixels (at
/// least 1).
pub(crate) fn size_px(size: f32, dpr: f32) -> u32 {
    let px = (size * dpr).round();
    if px.is_finite() && px >= 1.0 {
        px as u32
    } else {
        1
    }
}

/// The subpixel quarter of an x position in device pixels, as cosmic-text
/// bins it (to the nearest quarter) when it lays a glyph out.
#[cfg(test)]
pub(crate) fn subpixel_quarter(x: f32) -> u8 {
    quarter(SubpixelBin::new(x).1)
}

fn quarter(bin: SubpixelBin) -> u8 {
    match bin {
        SubpixelBin::Zero => 0,
        SubpixelBin::One => 1,
        SubpixelBin::Two => 2,
        SubpixelBin::Three => 3,
    }
}

/// A rectangle of the page, in texels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Slot {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

struct Entry<V> {
    slot: Slot,
    shelf_y: u32,
    /// The use tick, the key of this entry in `Atlas::by_use`.
    tick: u64,
    /// The frame that last drew this entry.
    frame: u64,
    value: V,
}

/// One band of the page: its free x ranges, merged and sorted.
struct Shelf {
    h: u32,
    free: Vec<(u32, u32)>,
    used: u32,
}

impl Shelf {
    fn new(h: u32) -> Shelf {
        Shelf {
            h,
            free: vec![(0, PAGE)],
            used: 0,
        }
    }

    fn alloc(&mut self, w: u32) -> Option<u32> {
        let at = self.free.iter().position(|&(_, len)| len >= w)?;
        let (x, len) = self.free[at];
        if len == w {
            self.free.remove(at);
        } else {
            self.free[at] = (x + w, len - w);
        }
        self.used += 1;
        Some(x)
    }

    fn release(&mut self, x: u32, w: u32) {
        let at = self.free.partition_point(|&(fx, _)| fx < x);
        self.free.insert(at, (x, w));
        // Merge with the next range, then with the previous one.
        if at + 1 < self.free.len() && self.free[at].0 + self.free[at].1 == self.free[at + 1].0 {
            self.free[at].1 += self.free[at + 1].1;
            self.free.remove(at + 1);
        }
        if at > 0 && self.free[at - 1].0 + self.free[at - 1].1 == self.free[at].0 {
            self.free[at - 1].1 += self.free[at].1;
            self.free.remove(at);
        }
        self.used = self.used.saturating_sub(1);
    }
}

/// One bounded atlas page: keys to slots with an LRU order.
pub(crate) struct Atlas<K, V> {
    entries: HashMap<K, Entry<V>>,
    /// Use tick → key, oldest first.
    by_use: BTreeMap<u64, K>,
    /// Shelves by their top y.
    shelves: BTreeMap<u32, Shelf>,
    /// Free vertical bands `(y, h)`, merged and sorted.
    bands: Vec<(u32, u32)>,
    tick: u64,
    frame: u64,
}

impl<K: Clone + Eq + Hash, V> Atlas<K, V> {
    pub(crate) fn new() -> Atlas<K, V> {
        Atlas {
            entries: HashMap::new(),
            by_use: BTreeMap::new(),
            shelves: BTreeMap::new(),
            bands: vec![(0, PAGE)],
            tick: 0,
            frame: 1,
        }
    }

    /// Starts a frame: what earlier frames drew may be evicted again.
    pub(crate) fn begin_frame(&mut self) {
        self.frame += 1;
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Texels the live entries occupy, out of `PAGE`².
    pub(crate) fn occupied_texels(&self) -> u64 {
        self.entries
            .values()
            .map(|e| u64::from(e.slot.w) * u64::from(e.slot.h))
            .sum()
    }

    /// The slot and value of `key`, marking it used by this frame.
    pub(crate) fn get(&mut self, key: &K) -> Option<(Slot, &V)> {
        let tick = self.next_tick();
        let entry = self.entries.get_mut(key)?;
        self.by_use.remove(&entry.tick);
        self.by_use.insert(tick, key.clone());
        entry.tick = tick;
        entry.frame = self.frame;
        Some((entry.slot, &entry.value))
    }

    /// The slot and value of `key` without touching its recency.
    #[cfg(test)]
    pub(crate) fn get_untouched(&self, key: &K) -> Option<(Slot, &V)> {
        self.entries.get(key).map(|e| (e.slot, &e.value))
    }

    /// Every occupied slot.
    #[cfg(test)]
    pub(crate) fn slots(&self) -> impl Iterator<Item = Slot> + '_ {
        self.entries.values().map(|e| e.slot)
    }

    /// Places a `w`×`h` rectangle for `key`, evicting the least recently used
    /// entries of earlier frames until it fits. `None` when it is empty,
    /// larger than the page, or only fits by evicting this frame's glyphs.
    /// An existing entry for `key` is replaced.
    pub(crate) fn insert(&mut self, key: K, w: u32, h: u32, value: V) -> Option<Slot> {
        if w == 0 || h == 0 || w > PAGE || h > PAGE {
            return None;
        }
        self.remove(&key);
        let slot = loop {
            if let Some(slot) = self.place(w, h) {
                break slot;
            }
            if !self.evict_oldest() {
                return None;
            }
        };
        let tick = self.next_tick();
        self.by_use.insert(tick, key.clone());
        let shelf_y = slot.y;
        self.entries.insert(
            key,
            Entry {
                slot,
                shelf_y,
                tick,
                frame: self.frame,
                value,
            },
        );
        Some(slot)
    }

    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    /// Finds room in a shelf of the right height, or cuts a new shelf.
    fn place(&mut self, w: u32, h: u32) -> Option<Slot> {
        let shelf_h = h.div_ceil(SHELF_STEP).saturating_mul(SHELF_STEP).min(PAGE);
        // Best fit by height among shelves no taller than twice the glyph.
        let candidate = self
            .shelves
            .iter()
            .filter(|(_, s)| s.h >= h && s.h <= shelf_h.saturating_mul(2))
            .filter(|(_, s)| s.free.iter().any(|&(_, len)| len >= w))
            .min_by_key(|(_, s)| s.h)
            .map(|(&y, _)| y);
        if let Some(y) = candidate {
            let shelf = self.shelves.get_mut(&y)?;
            let x = shelf.alloc(w)?;
            return Some(Slot { x, y, w, h });
        }
        let y = self.cut_band(shelf_h)?;
        let mut shelf = Shelf::new(shelf_h);
        let x = shelf.alloc(w)?;
        self.shelves.insert(y, shelf);
        Some(Slot { x, y, w, h })
    }

    /// Takes `h` rows from the first free band tall enough.
    fn cut_band(&mut self, h: u32) -> Option<u32> {
        let at = self.bands.iter().position(|&(_, len)| len >= h)?;
        let (y, len) = self.bands[at];
        if len == h {
            self.bands.remove(at);
        } else {
            self.bands[at] = (y + h, len - h);
        }
        Some(y)
    }

    /// Returns a shelf's rows to the free bands, merging neighbours.
    fn return_band(&mut self, y: u32, h: u32) {
        let at = self.bands.partition_point(|&(by, _)| by < y);
        self.bands.insert(at, (y, h));
        if at + 1 < self.bands.len() && self.bands[at].0 + self.bands[at].1 == self.bands[at + 1].0
        {
            self.bands[at].1 += self.bands[at + 1].1;
            self.bands.remove(at + 1);
        }
        if at > 0 && self.bands[at - 1].0 + self.bands[at - 1].1 == self.bands[at].0 {
            self.bands[at - 1].1 += self.bands[at].1;
            self.bands.remove(at);
        }
    }

    /// Evicts the least recently used entry unless this frame drew it.
    fn evict_oldest(&mut self) -> bool {
        let Some((_, key)) = self.by_use.first_key_value() else {
            return false;
        };
        let key = key.clone();
        match self.entries.get(&key) {
            Some(entry) if entry.frame == self.frame => false,
            _ => {
                self.remove(&key);
                true
            }
        }
    }

    /// Drops `key`'s entry and frees its slot (an upload that failed rolls
    /// its insert back with this).
    pub(crate) fn remove(&mut self, key: &K) {
        let Some(entry) = self.entries.remove(key) else {
            return;
        };
        self.by_use.remove(&entry.tick);
        let empty = match self.shelves.get_mut(&entry.shelf_y) {
            Some(shelf) => {
                shelf.release(entry.slot.x, entry.slot.w);
                shelf.used == 0
            }
            None => false,
        };
        if empty {
            if let Some(shelf) = self.shelves.remove(&entry.shelf_y) {
                self.return_band(entry.shelf_y, shelf.h);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::{fontdb, CacheKey, CacheKeyFlags, Weight};

    fn key(size: f32, dpr: f32, x: f32) -> GlyphKey {
        GlyphKey::new(
            fontdb::ID::dummy(),
            7,
            400,
            size_px(size, dpr),
            subpixel_quarter(x),
        )
    }

    #[test]
    fn glyph_key_includes_font_size_in_device_px_and_subpixel_quarter() {
        assert_eq!(size_px(14.0, 1.5), 21);
        assert_eq!(size_px(14.0, 1.0), 14);
        assert_ne!(key(14.0, 1.5, 0.0), key(14.0, 1.0, 0.0));
        // Quarters are cosmic-text's bins (nearest quarter), the same bins the
        // rasteriser offsets the glyph by.
        assert_eq!(key(14.0, 1.0, 0.30), key(14.0, 1.0, 0.36));
        assert_eq!(key(14.0, 1.0, 0.05), key(14.0, 1.0, 0.10));
        assert_ne!(key(14.0, 1.0, 0.10), key(14.0, 1.0, 0.40));
        assert_ne!(key(14.0, 1.0, 0.10), key(14.0, 1.0, 0.30));
        // The variable font shares one face id across weights.
        let id = fontdb::ID::dummy();
        assert_ne!(
            GlyphKey::new(id, 7, 400, 14, 0),
            GlyphKey::new(id, 7, 600, 14, 0)
        );
    }

    #[test]
    fn glyph_key_agrees_with_the_cache_key_the_layout_produces() {
        let id = fontdb::ID::dummy();
        let (ck, _, _) =
            CacheKey::new(id, 7, 21.0, (3.3, 0.0), Weight(600), CacheKeyFlags::empty());
        assert_eq!(
            GlyphKey::from_cache_key(&ck),
            GlyphKey::new(id, 7, 600, 21, subpixel_quarter(3.3))
        );
    }

    #[test]
    fn atlas_evicts_least_recent_when_a_page_is_full() {
        let mut atlas = Atlas::<u32, ()>::new();
        atlas.begin_frame();
        for k in 0..256 {
            assert!(atlas.insert(k, 64, 64, ()).is_some(), "cell {k} fits");
        }
        assert_eq!(atlas.len(), 256);
        atlas.begin_frame();
        assert!(atlas.get(&0).is_some(), "0 is now the most recent");
        let slot_of_1 = atlas.get_untouched(&1).map(|(slot, _)| slot);
        let slot = atlas.insert(256, 64, 64, ());
        assert!(slot.is_some(), "the 257th insert fits after an eviction");
        assert_eq!(slot, slot_of_1, "it reuses the evicted cell");
        assert!(atlas.get_untouched(&1).is_none(), "1 was the least recent");
        assert!(atlas.get_untouched(&0).is_some());
        assert!(atlas.get_untouched(&2).is_some());
        assert_eq!(atlas.len(), 256);
    }

    #[test]
    fn occupancy_is_the_area_of_the_live_entries() {
        let mut atlas = Atlas::<u32, ()>::new();
        assert_eq!(atlas.occupied_texels(), 0);
        atlas.begin_frame();
        assert!(atlas.insert(1, 10, 20, ()).is_some());
        assert!(atlas.insert(2, 5, 5, ()).is_some());
        assert_eq!(atlas.occupied_texels(), 225);
        atlas.remove(&1);
        assert_eq!(atlas.occupied_texels(), 25);
    }

    #[test]
    fn atlas_never_evicts_what_the_current_frame_drew() {
        let mut atlas = Atlas::<u32, ()>::new();
        atlas.begin_frame();
        for k in 0..256 {
            assert!(atlas.insert(k, 64, 64, ()).is_some());
        }
        assert!(
            atlas.insert(256, 64, 64, ()).is_none(),
            "every cell is in use this frame"
        );
        assert_eq!(atlas.len(), 256);
        assert!(atlas.get_untouched(&0).is_some());
    }

    #[test]
    fn atlas_refuses_a_glyph_larger_than_the_page() {
        let mut atlas = Atlas::<u32, ()>::new();
        atlas.begin_frame();
        assert!(atlas.insert(1, PAGE + 1, 8, ()).is_none());
        assert!(atlas.insert(2, 8, PAGE + 1, ()).is_none());
        assert!(atlas.insert(3, 0, 8, ()).is_none());
        assert_eq!(atlas.len(), 0);
    }

    fn assert_disjoint_and_inside(atlas: &Atlas<u32, u32>) {
        let slots: Vec<Slot> = atlas.slots().collect();
        for s in &slots {
            assert!(
                s.x + s.w <= PAGE && s.y + s.h <= PAGE,
                "{s:?} leaves the page"
            );
        }
        for (i, a) in slots.iter().enumerate() {
            for b in &slots[i + 1..] {
                let apart =
                    a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
                assert!(apart, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn atlas_never_exceeds_one_page() {
        let mut atlas = Atlas::<u32, u32>::new();
        // Numerical Recipes LCG: deterministic glyph sizes from 4 to 67 px.
        let mut state = 0x9e37_79b9_u32;
        let mut next = move || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state >> 16
        };
        let mut placed = 0;
        for k in 0..20_000u32 {
            if k % 40 == 0 {
                atlas.begin_frame();
            }
            if k % 3 == 0 {
                let old = next() % (k + 1);
                let _ = atlas.get(&old);
            }
            let (w, h) = (4 + next() % 64, 4 + next() % 64);
            if let Some(slot) = atlas.insert(k, w, h, k) {
                placed += 1;
                assert_eq!((slot.w, slot.h), (w, h));
            }
            if k % 2_000 == 0 {
                assert_disjoint_and_inside(&atlas);
            }
        }
        assert_disjoint_and_inside(&atlas);
        assert!(placed > 19_000, "only {placed} of 20000 placed");
        let area: u32 = atlas.slots().map(|s| s.w * s.h).sum();
        assert!(area <= PAGE * PAGE);
    }

    #[test]
    fn a_rolled_back_insert_leaves_no_entry_and_frees_its_slot() {
        let mut atlas = Atlas::<u32, ()>::new();
        atlas.begin_frame();
        for k in 0..256 {
            assert!(atlas.insert(k, 64, 64, ()).is_some());
        }
        let slot_of_5 = atlas.get_untouched(&5).map(|(slot, _)| slot);
        atlas.remove(&5);
        assert!(atlas.get(&5).is_none(), "no entry survives the rollback");
        assert_eq!(atlas.len(), 255);
        let slot = atlas.insert(256, 64, 64, ());
        assert_eq!(
            slot, slot_of_5,
            "the freed slot is reused within the same frame"
        );
        assert_eq!(atlas.len(), 256);
    }
}
