//! Content hashes: the document's over its canonical text, a track's over its bytes.
//! The document hash is what a read reports and a write names, so it must depend only on the canonical bytes.

use sha2::{Digest, Sha256};

use crate::document::Document;
use crate::serialize::serialize;

/// Hex sha256, shortened to 16 characters on the wire; collisions at that length are not a concern for one library.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct DocHash(String);

impl DocHash {
    #[must_use]
    pub fn of(doc: &Document) -> Self {
        Self::of_text(&serialize(doc))
    }

    #[must_use]
    pub fn of_text(text: &str) -> Self {
        Self(hex::encode(Sha256::digest(text.as_bytes()))[..16].to_owned())
    }

    #[must_use]
    pub fn of_bytes(bytes: &[u8]) -> Self {
        Self(hex::encode(Sha256::digest(bytes))[..16].to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// # Errors When `text` is not 16 lowercase hex characters.
    pub fn parse(text: &str) -> Result<Self, String> {
        let ok = text.len() == 16
            && text
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
        if !ok {
            return Err(format!(
                "'{text}' is not a document hash (16 hex characters)"
            ));
        }
        Ok(Self(text.to_owned()))
    }
}

impl std::fmt::Display for DocHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    #[test]
    fn the_hash_ignores_spelling_differences_the_canonical_form_removes() {
        let a = parse("<recast v=\"3\"><screen id=\"s\"><zoom id=\"z\" at=\"4.5\" dur=\"1\"/></screen></recast>").unwrap();
        let b = parse("<recast v=\"3\">\n  <screen id=\"s\">\n    <zoom dur=\"1.000\" id=\"z\" at=\"4.500\"/>\n  </screen>\n</recast>\n").unwrap();
        assert_eq!(DocHash::of(&a), DocHash::of(&b));
        let c = parse("<recast v=\"3\"><screen id=\"s\"><zoom id=\"z\" at=\"4.6\" dur=\"1\"/></screen></recast>").unwrap();
        assert_ne!(DocHash::of(&a), DocHash::of(&c));
        assert_eq!(DocHash::of(&a).as_str().len(), 16);
    }
}
