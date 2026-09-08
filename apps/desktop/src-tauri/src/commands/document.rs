//! Tauri IPC onto the document owner, for the webview replica: the same `doc.*` verbs the socket serves.

use recast_project::Op;
use serde_json::Value;

use super::error::{AppError, AppResult};
use crate::control::doc;

#[tauri::command]
pub async fn doc_show(path: String) -> AppResult<Value> {
    tauri::async_runtime::spawn_blocking(move || doc::show(&path))
        .await
        .map_err(|e| AppError::msg(format!("doc_show join error: {e}")))?
        .map_err(AppError::msg)
}

#[tauri::command]
pub async fn doc_apply(
    path: String,
    ops: Vec<Op>,
    expect_seq: Option<u64>,
    expect_hash: Option<String>,
) -> AppResult<Value> {
    tauri::async_runtime::spawn_blocking(move || {
        let expect = doc::expect_of(&serde_json::json!({
            "expectSeq": expect_seq,
            "expectHash": expect_hash,
        }))?;
        doc::apply(&path, &ops, expect)
    })
    .await
    .map_err(|e| AppError::msg(format!("doc_apply join error: {e}")))?
    .map_err(AppError::msg)
}

#[tauri::command]
pub async fn doc_since(path: String, seq: u64) -> AppResult<Value> {
    tauri::async_runtime::spawn_blocking(move || doc::since(&path, seq))
        .await
        .map_err(|e| AppError::msg(format!("doc_since join error: {e}")))?
        .map_err(AppError::msg)
}

/// The blur and idle hook: the file is exactly current the moment editing stops.
#[tauri::command]
pub async fn doc_flush(path: String) -> AppResult<Value> {
    tauri::async_runtime::spawn_blocking(move || doc::flush(&path))
        .await
        .map_err(|e| AppError::msg(format!("doc_flush join error: {e}")))?
        .map_err(AppError::msg)
}
