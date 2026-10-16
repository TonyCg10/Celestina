//! Keys: the identity a view node keeps across `Tree::apply` calls.
#![forbid(unsafe_code)]

/// FNV-1a 64: stable across runs, builds and platforms, unlike `std`'s
/// randomly seeded hasher.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv(mut hash: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// The identity of a view node among its siblings. A key that appears in
/// two consecutive views names the same node: it keeps its hover, press,
/// focus and running animations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key(pub(crate) u64);

impl Key {
    /// What `Tree::key_of` answers for an id the tree no longer holds.
    pub const STALE: Key = Key(0);

    /// The key of a node that appears once, such as `"clock"`.
    pub fn named(s: &str) -> Key {
        Key(fnv(FNV_OFFSET, s.as_bytes()))
    }

    /// The key of the `i`-th node of a family, such as a list row keyed by
    /// its item's id: distinct from `named(s)` and from every other `i`.
    pub fn indexed(s: &str, i: u64) -> Key {
        let hash = fnv(fnv(FNV_OFFSET, s.as_bytes()), &[0xff]);
        Key(fnv(hash, &i.to_le_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_stable_and_distinct() {
        assert_eq!(Key::named("row"), Key::named("row"));
        assert_ne!(Key::named("row"), Key::named("rows"));
        assert_eq!(Key::indexed("row", 2), Key::indexed("row", 2));
        assert_ne!(Key::indexed("row", 2), Key::indexed("row", 3));
        assert_ne!(Key::indexed("row", 0), Key::named("row"));
        assert_ne!(Key::indexed("ro", 0), Key::indexed("row", 0));
    }

    #[test]
    fn keys_do_not_depend_on_the_process() {
        // FNV-1a 64 of "" is its offset basis: the value is fixed across
        // runs and builds, so a key can be logged and compared.
        assert_eq!(Key::named(""), Key(0xcbf2_9ce4_8422_2325));
    }
}
