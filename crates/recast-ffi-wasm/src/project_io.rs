//! The replica's document logic, kept off the `wasm_bindgen` layer so it runs in host tests.
//! A replica holds the document, turns a whole editor state into the ops that reach it, and reads a state back out.

use recast_project::scene::{from_render_state_public, to_render_state, MediaRefs};
use recast_project::{apply_all, diff, parse, serialize, DocHash, Document, IdGen, Op};
use recast_scene::v1::RenderState;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct Replica {
    doc: Document,
    /// The last whole state the editor handed over, so a patch of a few fields can be merged rather than re-sent.
    state: crate::scene_io::PatchableState,
}

impl PartialEq for Replica {
    fn eq(&self, other: &Self) -> bool {
        self.doc == other.doc
    }
}

impl Replica {
    /// # Errors When the text is not a v3 document.
    pub fn parse(text: &str) -> Result<Self, String> {
        parse(text)
            .map(|doc| Self {
                doc,
                state: Default::default(),
            })
            .map_err(|e| e.to_string())
    }

    #[must_use]
    pub fn text(&self) -> String {
        serialize(&self.doc)
    }

    #[must_use]
    pub fn hash(&self) -> DocHash {
        DocHash::of(&self.doc)
    }

    /// Applies one batch (JSON array of ops), all or nothing.
    /// # Errors When the JSON is not an op array or an op cannot apply; the replica is untouched.
    pub fn apply(&mut self, ops_json: &str) -> Result<usize, String> {
        let ops: Vec<Op> = serde_json::from_str(ops_json).map_err(|e| format!("ops: {e}"))?;
        self.doc = apply_all(&self.doc, &ops).map_err(|(i, e)| format!("op {i}: {e}"))?;
        Ok(ops.len())
    }

    /// The editor's whole state, read out of the document.
    /// # Errors When the document does not map (missing required attribute, wrong version).
    pub fn render_state_json(&mut self) -> Result<String, String> {
        let state = to_render_state(&self.doc).map_err(|e| e.to_string())?;
        let json = serde_json::to_string(&state).map_err(|e| e.to_string())?;
        self.state.remember(&json);
        Ok(json)
    }

    /// The ops that take this document to the one the editor's state describes, as JSON. Empty array when nothing moved.
    /// Media and tracks are read from the document itself; the state never names files. Ids recycle from the document.
    /// # Errors When the state JSON does not parse or the diff cannot be formed.
    pub fn ops_for_state_json(&mut self, state_json: &str) -> Result<String, String> {
        let state: RenderState =
            serde_json::from_str(state_json).map_err(|e| format!("state: {e}"))?;
        self.state.remember(state_json);
        self.ops_for(&state, state_json)
    }

    /// The ops for a few changed fields merged into the last whole state (`ops_for_state_json` or `render_state_json`
    /// must have run first). The seed for minted ids comes from the patch, so the same patch mints the same ids.
    /// # Errors When no whole state was handed over yet, or the patch does not merge.
    pub fn ops_for_patch_json(&mut self, patch_json: &str) -> Result<String, String> {
        let state = self.state.apply(patch_json).map_err(|e| e.to_string())?;
        self.ops_for(&state, patch_json)
    }

    fn ops_for(&self, state: &RenderState, seed: &str) -> Result<String, String> {
        let refs = MediaRefs::from_document(&self.doc);
        let mut ids = IdGen::seeded(seed_of(seed));
        let target = from_render_state_public(state, &refs, Some(&self.doc), &mut ids);
        let ops = diff(&self.doc, &target).map_err(|e| e.to_string())?;
        serde_json::to_string(&ops).map_err(|e| e.to_string())
    }

    /// Validation findings as JSON, so the inspector can show what the core would refuse.
    /// # Errors When the report cannot be encoded.
    pub fn issues_json(&self) -> Result<String, String> {
        serde_json::to_string(&recast_project::validate(&self.doc).issues)
            .map_err(|e| e.to_string())
    }
}

/// Same seeding as the desktop save path, so both sides mint the same ids for the same state.
fn seed_of(state_json: &str) -> u64 {
    let digest = Sha256::digest(state_json.as_bytes());
    u64::from_le_bytes(digest[..8].try_into().unwrap_or([0; 8]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r##"<recast v="3" timebase="source-seconds">
  <media id="rec" kind="video" src="media/recording.mp4" w="1920" h="1080" fps="60" dur="11.350"/>
  <track id="cur" kind="cursor" src="tracks/cursor.json" n="10" ro="true"/>
  <timeline src="rec" in="0" out="11.350"/>
  <background><solid color="#0f172a"/></background>
  <screen id="scr"><zooms/></screen>
</recast>
"##;

    #[test]
    fn a_state_edit_becomes_ops_that_reach_the_same_document_the_core_would_write() {
        let mut replica = Replica::parse(DOC).unwrap();
        let mut state: RenderState =
            serde_json::from_str(&replica.render_state_json().unwrap()).unwrap();
        state.padding = 12.0;
        state.trim_end = 9.0;
        let ops_json = replica
            .ops_for_state_json(&serde_json::to_string(&state).unwrap())
            .unwrap();
        let ops: Vec<Op> = serde_json::from_str(&ops_json).unwrap();
        assert!(ops
            .iter()
            .any(|o| matches!(o, Op::Set { id, attr, .. } if id == "/" && attr == "pad")));
        assert!(ops
            .iter()
            .any(|o| matches!(o, Op::Set { id, attr, .. } if id == "/timeline" && attr == "out")));
        let mut after = replica.clone();
        after.apply(&ops_json).unwrap();
        assert!(after.text().contains("pad=\"12\""), "{}", after.text());
        assert!(
            after.text().contains("src=\"media/recording.mp4\""),
            "media refs survive: {}",
            after.text()
        );
        let again: RenderState = serde_json::from_str(&after.render_state_json().unwrap()).unwrap();
        assert_eq!(again.padding, 12.0);
        assert_eq!(again.trim_end, 9.0);
        assert_eq!(
            after
                .ops_for_state_json(&serde_json::to_string(&again).unwrap())
                .unwrap(),
            "[]"
        );
    }

    #[test]
    fn a_patch_yields_the_same_ops_as_the_whole_state_would() {
        let mut replica = Replica::parse(DOC).unwrap();
        let full = replica.render_state_json().unwrap();
        let mut state: RenderState = serde_json::from_str(&full).unwrap();
        state.padding = 12.0;
        let whole = replica
            .ops_for_state_json(&serde_json::to_string(&state).unwrap())
            .unwrap();
        let mut fresh = Replica::parse(DOC).unwrap();
        fresh.render_state_json().unwrap();
        let patched = fresh.ops_for_patch_json(r#"{"padding": 12.0}"#).unwrap();
        assert_eq!(whole, patched);
        assert!(
            Replica::parse(DOC)
                .unwrap()
                .ops_for_patch_json(r#"{"padding": 1}"#)
                .is_err(),
            "no base yet"
        );
    }

    #[test]
    fn a_bad_batch_leaves_the_replica_untouched() {
        let mut replica = Replica::parse(DOC).unwrap();
        let before = replica.hash();
        let err = replica
            .apply(
                r#"[{"op":"set","id":"/","attr":"pad","value":"1"},{"op":"remove","id":"nope"}]"#,
            )
            .unwrap_err();
        assert!(err.contains("op 1"), "{err}");
        assert_eq!(replica.hash(), before);
        assert!(Replica::parse("<nope/>").is_err());
    }
}
