//! Element ids: nanoid-shaped, eight characters, unique within a document.
//! Migration mints them from a seeded generator so the same bundle always yields the same ids; the editor mints them from entropy the host supplies.

use std::collections::BTreeSet;

pub const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
pub const ID_LEN: usize = 8;

/// A validated id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(String);

impl Id {
    /// # Errors When `text` is empty, longer than 32, or holds a character outside the alphabet.
    pub fn parse(text: &str) -> Result<Self, IdError> {
        let ok =
            !text.is_empty() && text.len() <= 32 && text.bytes().all(|b| ALPHABET.contains(&b));
        if ok {
            Ok(Self(text.to_owned()))
        } else {
            Err(IdError(text.to_owned()))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("'{0}' is not an id: 1 to 32 characters of A-Z a-z 0-9 _ -")]
pub struct IdError(String);

/// Mints ids that are unique against everything it has seen. Deterministic for a given seed.
pub struct IdGen {
    state: u64,
    taken: BTreeSet<String>,
    /// Ids a previous document gave to rows the state cannot carry ids for, keyed by what identifies the row.
    known: std::collections::BTreeMap<String, String>,
}

impl IdGen {
    /// A generator whose sequence is a pure function of `seed`, for migration.
    #[must_use]
    pub fn seeded(seed: u64) -> Self {
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
            taken: BTreeSet::new(),
            known: std::collections::BTreeMap::new(),
        }
    }

    /// Ids already in the document, so a minted one can never collide.
    pub fn reserve<'a>(&mut self, existing: impl IntoIterator<Item = &'a str>) {
        self.taken.extend(existing.into_iter().map(str::to_owned));
    }

    /// Records that the row identified by `key` already carries `id`, so a rewrite keeps it instead of minting.
    pub fn remember(&mut self, key: &str, id: &str) {
        self.taken.insert(id.to_owned());
        self.known.insert(key.to_owned(), id.to_owned());
    }

    /// The remembered id for `key`, or a fresh one that is remembered from now on.
    pub fn mint_for(&mut self, key: &str) -> Id {
        if let Some(id) = self.known.get(key) {
            return Id(id.clone());
        }
        let id = self.next_id();
        self.known.insert(key.to_owned(), id.0.clone());
        id
    }

    pub fn next_id(&mut self) -> Id {
        loop {
            let candidate = self.candidate();
            if self.taken.insert(candidate.clone()) {
                return Id(candidate);
            }
        }
    }

    // SplitMix64: small, well-distributed, and identical on every platform.
    fn candidate(&mut self) -> String {
        let mut out = String::with_capacity(ID_LEN);
        let mut bits = self.next_u64();
        for _ in 0..ID_LEN {
            out.push(ALPHABET[(bits & 63) as usize] as char);
            bits >>= 6;
        }
        out
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seeded_generator_is_deterministic_and_unique() {
        let a: Vec<Id> = {
            let mut g = IdGen::seeded(42);
            (0..100).map(|_| g.next_id()).collect()
        };
        let b: Vec<Id> = {
            let mut g = IdGen::seeded(42);
            (0..100).map(|_| g.next_id()).collect()
        };
        assert_eq!(a, b);
        let set: BTreeSet<&Id> = a.iter().collect();
        assert_eq!(set.len(), 100);
        assert!(a.iter().all(|id| id.as_str().len() == ID_LEN));
    }

    #[test]
    fn reserved_ids_are_never_minted() {
        let mut probe = IdGen::seeded(7);
        let first = probe.next_id();
        let mut g = IdGen::seeded(7);
        g.reserve([first.as_str()]);
        assert_ne!(g.next_id(), first);
    }

    #[test]
    fn a_remembered_key_returns_its_id_and_a_new_key_mints_once() {
        let mut g = IdGen::seeded(1);
        g.remember("split@30.000", "keep1234");
        assert_eq!(g.mint_for("split@30.000").as_str(), "keep1234");
        let fresh = g.mint_for("clip@30.000");
        assert_eq!(g.mint_for("clip@30.000"), fresh);
        assert_ne!(g.next_id(), fresh);
    }

    #[test]
    fn ids_are_validated_against_the_alphabet() {
        assert!(Id::parse("tr703cp").is_ok());
        assert!(Id::parse("with space").is_err());
        assert!(Id::parse("").is_err());
        assert!(Id::parse("a.b").is_err());
    }
}
