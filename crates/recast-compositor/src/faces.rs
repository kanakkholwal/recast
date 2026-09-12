//! The faces one session draws with, and the ids the glyph atlas keys them by.
//! A session used to hold exactly one, which made every string in a frame share a font whatever it asked for.

use recast_text::FontFace;

/// What a caller asks for: a CSS family stack and a weight.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    family: String,
    weight: u16,
}

/// The face the host supplied, if any, plus everything resolved from the system.
/// Ids are handed out once per key and never reused, so a glyph cached under one
/// can never be read back as another's.
#[derive(Default)]
pub struct Faces {
    /// Used when a family resolves to nothing, and the only source on wasm,
    /// where there is no filesystem to search.
    host: Option<FontFace>,
    entries: Vec<(Key, u32, Option<FontFace>)>,
    next_id: u32,
}

/// The id the host's own face is packed under. Fixed, so the caption path keeps
/// the ids it had before there was a registry.
pub const HOST_FACE: u32 = 0;

impl Faces {
    #[must_use]
    pub fn new() -> Self {
        Self {
            host: None,
            entries: Vec::new(),
            next_id: HOST_FACE + 1,
        }
    }

    /// Replaces the host's face. Returns true when it changed, which invalidates
    /// every glyph packed under `HOST_FACE`.
    pub fn set_host(&mut self, face: Option<FontFace>) -> bool {
        let changed = self.host.is_some() != face.is_some();
        self.host = face;
        changed
    }

    #[must_use]
    pub fn host(&self) -> Option<&FontFace> {
        self.host.as_ref()
    }

    /// Puts a face the HOST resolved under `family` and `weight`. The browser has
    /// no font database, so this is the only way a page gets a second face.
    /// Returns the id it was packed under, replacing any face already there.
    pub fn insert(&mut self, family: &str, weight: u16, face: FontFace) -> u32 {
        let key = Key {
            family: first_family(family),
            weight,
        };
        if let Some(entry) = self.entries.iter_mut().find(|(k, _, _)| *k == key) {
            entry.2 = Some(face);
            return entry.1;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push((key, id, Some(face)));
        id
    }

    /// The face to draw `family` at `weight` with, and the id to pack it under.
    /// An empty family, or one that resolves to nothing, falls back to the host's.
    pub fn face_for(&mut self, family: &str, weight: u16) -> Option<(u32, FontFace)> {
        let name = first_family(family);
        if name.is_empty() {
            return self.host.clone().map(|face| (HOST_FACE, face));
        }
        let key = Key {
            family: name,
            weight,
        };
        if let Some((_, id, face)) = self.entries.iter().find(|(k, _, _)| *k == key) {
            return match face {
                Some(face) => Some((*id, face.clone())),
                None => self.host.clone().map(|face| (HOST_FACE, face)),
            };
        }
        let resolved = resolve(&key.family, weight);
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push((key, id, resolved.clone()));
        match resolved {
            Some(face) => Some((id, face)),
            None => self.host.clone().map(|face| (HOST_FACE, face)),
        }
    }

    /// Forgets every resolved face, for when the atlas is reset under them.
    pub fn clear_resolved(&mut self) {
        self.entries.clear();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve(family: &str, weight: u16) -> Option<FontFace> {
    recast_text::resolve_face(family, weight, None).map(|resolved| resolved.face)
}

/// The bytes of an installed family, for a host with no font database of its own.
/// A webview cannot look one up; the side that can hands the file over so both
/// shape from the same face. `None` when nothing matches.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub fn installed_font_bytes(family: &str, weight: u16) -> Option<Vec<u8>> {
    resolve(family, weight).map(|face| {
        let (data, _) = face.source();
        data.as_ref().clone()
    })
}

/// No filesystem to search: the host's face is the only one there is.
#[cfg(target_arch = "wasm32")]
fn resolve(_family: &str, _weight: u16) -> Option<FontFace> {
    None
}

/// The first family of a CSS stack, unquoted. fontdb matches one name, not a
/// fallback list.
#[must_use]
pub fn first_family(stack: &str) -> String {
    stack
        .split(',')
        .next()
        .unwrap_or(stack)
        .trim()
        .trim_matches(['\'', '"'])
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_css_stack_resolves_by_its_first_name_unquoted() {
        assert_eq!(first_family("\"Inter\", sans-serif"), "Inter");
        assert_eq!(first_family("'Anton', Impact"), "Anton");
        assert_eq!(first_family("  Georgia  "), "Georgia");
        assert_eq!(first_family(""), "");
    }

    /// Two families sharing an id would read each other's glyphs out of the
    /// atlas, which keys on it. Ids are handed out whether or not the family
    /// resolves, so this holds on a machine with no fonts at all.
    #[test]
    fn every_distinct_request_gets_its_own_id_and_a_repeat_reuses_it() {
        let mut faces = Faces::new();

        faces.face_for("Alpha", 400);
        faces.face_for("Beta", 400);
        faces.face_for("Alpha", 700);
        faces.face_for("Alpha", 400);

        let ids: Vec<u32> = faces.entries.iter().map(|(_, id, _)| *id).collect();
        assert_eq!(ids, [1, 2, 3], "three distinct keys, the repeat reused");
        assert!(ids.iter().all(|id| *id != HOST_FACE));
    }

    #[test]
    fn an_empty_family_asks_for_the_host_face_rather_than_searching() {
        let mut faces = Faces::new();

        assert!(
            faces.face_for("", 400).is_none(),
            "and there is no host face"
        );
        assert!(faces.entries.is_empty(), "nothing was cached for it");
    }

    /// The browser resolves nothing, so a supplied face is the only way a second
    /// family draws there. It must win over a search that would fail.
    #[test]
    fn a_supplied_face_is_used_for_its_family_and_keeps_one_id_when_replaced() {
        let Some(face) = any_system_face() else {
            return;
        };
        let mut faces = Faces::new();

        let first = faces.insert("Anton", 700, face.clone());
        let (found, _) = faces.face_for("Anton", 700).expect("the supplied face");
        let again = faces.insert("Anton", 700, face);

        assert_eq!(
            found, first,
            "asked for by name, answered with the supplied one"
        );
        assert_eq!(
            again, first,
            "replacing it keeps the id its glyphs are under"
        );
        assert_ne!(first, HOST_FACE);
    }

    #[test]
    fn clearing_forgets_resolved_faces_so_they_are_asked_for_again() {
        let mut faces = Faces::new();
        faces.face_for("Alpha", 400);
        assert_eq!(faces.entries.len(), 1);

        faces.clear_resolved();

        assert!(faces.entries.is_empty());
    }

    /// Needs a real face, so it runs only where one resolves. The fallback is
    /// the whole point of the host slot: a family nobody has must still draw.
    #[test]
    fn an_unresolvable_family_falls_back_to_the_host_face_and_its_id() {
        let Some(face) = any_system_face() else {
            return;
        };
        let mut faces = Faces::new();
        faces.set_host(Some(face));

        let (id, _) = faces
            .face_for("Definitely Not A Font 9000", 400)
            .expect("the host face stands in");

        assert_eq!(
            id, HOST_FACE,
            "so glyphs pack under the face they came from"
        );
        assert!(faces.host().is_some());
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn any_system_face() -> Option<FontFace> {
        ["Segoe UI", "Arial", "DejaVu Sans", "Helvetica"]
            .into_iter()
            .find_map(|family| resolve(family, 400))
    }

    #[cfg(target_arch = "wasm32")]
    fn any_system_face() -> Option<FontFace> {
        None
    }
}
