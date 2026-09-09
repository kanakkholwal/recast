//! The webview's copy of a v3 document, the same crate the core holds it in, so both sides apply ops identically.

use wasm_bindgen::prelude::*;

use crate::project_io::Replica;

#[wasm_bindgen]
pub struct ProjectDocument {
    replica: Replica,
}

#[wasm_bindgen]
impl ProjectDocument {
    /// Parses canonical or hand-written document text.
    #[wasm_bindgen]
    pub fn parse(text: &str) -> Result<ProjectDocument, JsValue> {
        Replica::parse(text)
            .map(|replica| Self { replica })
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Canonical text, byte-identical to what the core would write.
    #[wasm_bindgen]
    pub fn text(&self) -> String {
        self.replica.text()
    }

    #[wasm_bindgen]
    pub fn hash(&self) -> String {
        self.replica.hash().to_string()
    }

    /// Applies a JSON array of ops, all or nothing; returns how many landed.
    #[wasm_bindgen]
    pub fn apply(&mut self, ops_json: &str) -> Result<usize, JsValue> {
        self.replica
            .apply(ops_json)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// The editor's render state as JSON, read out of the document.
    #[wasm_bindgen(js_name = renderState)]
    pub fn render_state(&mut self) -> Result<String, JsValue> {
        self.replica
            .render_state_json()
            .map_err(|e| JsValue::from_str(&e))
    }

    /// The ops (JSON array) that take this document to the one `state_json` describes; `[]` when nothing moved.
    #[wasm_bindgen(js_name = opsForState)]
    pub fn ops_for_state(&mut self, state_json: &str) -> Result<String, JsValue> {
        self.replica
            .ops_for_state_json(state_json)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// The ops for a few changed top-level fields merged into the last state this document saw; `[]` when nothing moved.
    /// Errors until a whole state has been handed over (`opsForState` or `renderState`).
    #[wasm_bindgen(js_name = opsForPatch)]
    pub fn ops_for_patch(&mut self, patch_json: &str) -> Result<String, JsValue> {
        self.replica
            .ops_for_patch_json(patch_json)
            .map_err(|e| JsValue::from_str(&e))
    }

    /// Validation findings as JSON.
    #[wasm_bindgen]
    pub fn issues(&self) -> Result<String, JsValue> {
        self.replica
            .issues_json()
            .map_err(|e| JsValue::from_str(&e))
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn duplicate(&self) -> ProjectDocument {
        Self {
            replica: self.replica.clone(),
        }
    }
}
