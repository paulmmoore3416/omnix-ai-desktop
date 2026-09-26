//! Knowledge base / long-term memory commands, backed by the kb-core adapter
//! when `memory.backend_url` is set. Extension features (analytics, document
//! management, export/import, optimization, folder sync) work against the
//! bundled kb-core and report `not_implemented` against a minimal service.
//! Separate named knowledge bases are not implemented.

use crate::error::{AppError, AppResult};
use crate::memory::{self, NewMemory};
use crate::security::files;
use crate::security::policy::Source;
use crate::state::AppState;
use serde_json::{json, Map, Value};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

/// Largest export file `import_knowledge` accepts.
const MAX_IMPORT_BYTES: u64 = 64 * 1024 * 1024;

/// Summary for the Knowledge view. `available: false` when no memory backend
/// is configured; `extended: false` when the service offers only the core
/// contract (no analytics/documents listing).
#[tauri::command]
pub async fn get_knowledge_data(state: State<'_, AppState>) -> AppResult<Value> {
    let Some(store) = memory::from_state(&state).await? else {
        return Ok(json!({
            "available": false, "extended": false,
            "totalMemories": 0, "totalDocuments": 0, "totalKnowledgeBases": 0,
            "storageUsed": 0, "vectorDimensions": 0, "embeddingModel": "",
            "memories": [], "documents": [], "knowledgeBases": []
        }));
    };
    let list = store.list(200).await?;
    let memories: Vec<Value> = list
        .iter()
        .map(|m| {
            json!({
                "id": m.id, "content": m.content, "tags": m.tags,
                "importance": m.importance, "category": m.category,
                "timestamp": m.created_at, "source": m.source, "pinned": m.pinned,
                "reinforced": m.reinforced, "accessCount": m.access_count,
                "rejected": m.rejected,
                "activation": m.activation, "collection": m.collection,
            })
        })
        .collect();
    let stats = match store.stats().await {
        Ok(s) => Some(s),
        Err(AppError::NotImplemented(_)) => None,
        Err(e) => return Err(e),
    };
    let documents = match store.list_documents().await {
        Ok(d) => d
            .into_iter()
            .map(|d| {
                let name = d.get("name").and_then(Value::as_str).unwrap_or_default();
                let ext = name.rsplit('.').next().unwrap_or_default().to_lowercase();
                json!({
                    "id": d.get("id"), "name": name, "type": ext,
                    "size": d.get("size").cloned().unwrap_or(json!(0)),
                    "chunks": d.get("chunks").cloned().unwrap_or(json!(0)),
                    "indexed": d.get("pending").and_then(Value::as_u64).unwrap_or(0) == 0,
                    "sourcePath": d.get("source_path"),
                    "collection": d.get("collection"),
                    "timestamp": d.get("updated_at").or_else(|| d.get("created_at")),
                })
            })
            .collect(),
        Err(AppError::NotImplemented(_)) => vec![],
        Err(e) => return Err(e),
    };
    let s = stats.clone().unwrap_or(Value::Null);
    let num = |k: &str| s.get(k).cloned().unwrap_or(json!(0));
    Ok(json!({
        "available": true,
        "extended": stats.is_some(),
        "totalMemories": if stats.is_some() { num("memories") } else { json!(memories.len()) },
        "totalDocuments": if stats.is_some() { num("documents") } else { json!(documents.len()) },
        "totalChunks": num("chunks"),
        "storageUsed": num("storage_bytes"),
        "vectorDimensions": num("vector_dimensions"),
        "embeddingModel": s.get("embed_model").cloned().unwrap_or(json!("")),
        "llmModel": s.get("llm_model").cloned().unwrap_or(Value::Null),
        "status": s.get("status").cloned().unwrap_or(json!("ok")),
        "embedError": s.get("embed_error").cloned().unwrap_or(Value::Null),
        "pendingEmbeddings": num("pending_embeddings"),
        "superseded": num("superseded"),
        "needsReview": num("needs_review"),
        "links": num("links"),
        "categories": s.get("categories").cloned().unwrap_or(json!({})),
        "sources": s.get("sources").cloned().unwrap_or(json!({})),
        "searches24h": num("searches_24h"),
        "avgSearchMs": s.get("avg_search_ms").cloned().unwrap_or(Value::Null),
        "lastMaintenance": s.get("last_maintenance").cloned().unwrap_or(Value::Null),
        "watch": s.get("watch").cloned().unwrap_or(json!({})),
        "recentActivity": s.get("recent_activity").cloned().unwrap_or(json!([])),
        "memories": memories, "documents": documents,
        "knowledgeBases": s.get("collection_list").cloned().unwrap_or(json!([])),
        "totalKnowledgeBases": s.get("collection_list").and_then(Value::as_array).map_or(0, |a| a.len()),
    }))
}

/// Store a memory in kb-core. Returns the service's receipt
/// (`{id, status, related}`; status is `created`, `reinforced` or `updated`).
#[tauri::command]
pub async fn save_memory(
    state: State<'_, AppState>,
    content: String,
    tags: Vec<String>,
    importance: u8,
    category: String,
    collection: Option<String>,
) -> AppResult<Value> {
    let content = content.trim().to_string();
    if content.is_empty() || content.len() > 20_000 {
        return Err(AppError::InvalidInput(
            "memory must be 1–20000 characters".into(),
        ));
    }
    if tags.len() > 50 || tags.iter().any(|t| t.len() > 64) || category.len() > 64 {
        return Err(AppError::InvalidInput(
            "too many or too long tags/category".into(),
        ));
    }
    let store = memory::require(&state).await?;
    memory::check_capacity(&state, store.as_ref()).await?;
    store
        .save_detailed(NewMemory {
            content,
            tags,
            importance: importance.clamp(1, 10),
            category,
            source: Some("user".into()),
            collection: collection
                .filter(|c| !c.trim().is_empty())
                .map(|c| check_kb_name(&c))
                .transpose()?,
        })
        .await
}

/// Delete a memory from kb-core.
#[tauri::command]
pub async fn delete_memory(state: State<'_, AppState>, id: String) -> AppResult<()> {
    memory::require(&state).await?.delete(&id).await
}

/// Merged and superseded memories: hidden from recall but kept, so a wrong
/// consolidation ruling can be reviewed and undone (restore = `update_memory`
/// with `superseded_by: null`).
#[tauri::command]
pub async fn list_hidden_memories(state: State<'_, AppState>, limit: u32) -> AppResult<Vec<Value>> {
    memory::require(&state).await?.list_hidden(limit).await
}

/// Pin/unpin, re-categorize, restore a merged/superseded memory, or keep the
/// model's ruling (`reviewed: true`). Only these fields are forwarded;
/// anything else is refused.
#[tauri::command]
pub async fn update_memory(
    state: State<'_, AppState>,
    id: String,
    patch: Map<String, Value>,
) -> AppResult<Value> {
    let clean = clean_memory_patch(patch)?;
    memory::require(&state)
        .await?
        .update_memory(&id, Value::Object(clean))
        .await
}

/// "This recalled memory was wrong / helped". Safe to take from the webview:
/// feedback only changes ranking (a flagged memory still answers when it is
/// the only match) and never hides, edits or deletes anything.
#[tauri::command]
pub async fn memory_feedback(
    state: State<'_, AppState>,
    id: String,
    helpful: bool,
) -> AppResult<Value> {
    memory::require(&state).await?.feedback(&id, helpful).await
}

/// The webview is untrusted: forward only known memory fields with
/// well-formed values, and refuse the whole patch otherwise.
fn clean_memory_patch(patch: Map<String, Value>) -> AppResult<Map<String, Value>> {
    let mut clean = Map::new();
    for (k, v) in patch {
        let ok = match k.as_str() {
            "pinned" => v.is_boolean(),
            // Restoring is the only change allowed: hiding a memory is a
            // consolidation decision, never something the UI can set.
            "superseded_by" => v.is_null(),
            "reviewed" => v.is_boolean(),
            "importance" => v.as_u64().is_some_and(|n| (1..=10).contains(&n)),
            "category" => v.as_str().is_some_and(|s| s.len() <= 64),
            "tags" => v.as_array().is_some_and(|a| {
                a.len() <= 50 && a.iter().all(|t| t.as_str().is_some_and(|s| s.len() <= 64))
            }),
            "content" => v
                .as_str()
                .is_some_and(|s| !s.trim().is_empty() && s.len() <= 20_000),
            _ => false,
        };
        if !ok {
            return Err(AppError::InvalidInput(format!(
                "cannot update `{k}` with that value"
            )));
        }
        clean.insert(k, v);
    }
    Ok(clean)
}

/// Semantic search in kb-core.
#[tauri::command]
pub async fn semantic_search(
    state: State<'_, AppState>,
    query: String,
    limit: u32,
    collection: Option<String>,
) -> AppResult<Vec<Value>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    let store = memory::require(&state).await?;
    let hits = match collection.filter(|c| !c.trim().is_empty()) {
        Some(c) => store.search_in(&query, limit, &check_kb_name(&c)?).await?,
        None => store.search(&query, limit).await?,
    };
    Ok(hits
        .into_iter()
        .map(|h| {
            json!({
                "id": h.id, "content": h.content, "score": h.score, "similarity": h.similarity,
                "tags": h.tags, "timestamp": h.created_at,
                "kind": h.kind.unwrap_or_else(|| "memory".into()),
                "collection": h.collection,
                "source": h.source.unwrap_or_else(|| "memory".into()),
            })
        })
        .collect())
}

/// Read a (user-chosen) text document through the guarded file reader and
/// send it to kb-core for chunking and embedding.
#[tauri::command]
pub async fn index_document(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    collection: Option<String>,
) -> AppResult<u32> {
    let store = memory::require(&state).await?;
    let collection = collection
        .filter(|c| !c.trim().is_empty())
        .map(|c| check_kb_name(&c))
        .transpose()?;
    let content = files::read_file(&app, &state, &path, Source::User).await?;
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".into());
    Ok(match collection {
        Some(c) => store.index_document_in(&name, &content, &c).await?.chunks,
        None => store.index_document(&name, &content).await?.chunks,
    })
}

/// Knowledge base that workspace notes are synced into.
const NOTES_KB: &str = "notes";
/// Largest note that can be synced, in bytes (the notepad keeps up to
/// 200 000 characters).
const MAX_NOTE_BYTES: usize = 800_000;

/// kb-core document name for a workspace note. The id comes from the
/// webview, so it is limited to a short alphanumeric token, and the `::`
/// prefix can't occur in the `folder/relative/path` names of watched files:
/// syncing a note can never replace a document it doesn't own.
fn note_doc_name(id: &str) -> AppResult<String> {
    if id.is_empty() || id.len() > 40 || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(AppError::InvalidInput("invalid note id".into()));
    }
    Ok(format!("omnix-notepad::{id}.md"))
}

/// Keep a workspace note indexed in kb-core (the `notes` knowledge base) so
/// recall and search find it. Re-syncing replaces the previous version, and
/// kb-core only re-embeds the chunks that changed. Returns `{id, chunks}`.
#[tauri::command]
pub async fn sync_note(
    state: State<'_, AppState>,
    id: String,
    content: String,
) -> AppResult<Value> {
    let name = note_doc_name(&id)?;
    if content.trim().is_empty() || content.len() > MAX_NOTE_BYTES {
        return Err(AppError::InvalidInput(
            "a synced note must have text and be under 800 KB".into(),
        ));
    }
    let store = memory::require(&state).await?;
    let receipt = store.index_document_in(&name, &content, NOTES_KB).await?;
    tracing::info!(note = %id, chunks = receipt.chunks, "note synced to kb-core");
    Ok(json!({ "id": receipt.id, "chunks": receipt.chunks }))
}

/// Stop syncing a note: remove its document from kb-core. Looked up by name,
/// so only the note's own document can be removed this way.
#[tauri::command]
pub async fn unsync_note(state: State<'_, AppState>, id: String) -> AppResult<bool> {
    let name = note_doc_name(&id)?;
    let store = memory::require(&state).await?;
    let doc = store
        .list_documents()
        .await?
        .into_iter()
        .find(|d| d.get("name").and_then(Value::as_str) == Some(name.as_str()));
    match doc
        .as_ref()
        .and_then(|d| d.get("id"))
        .and_then(Value::as_str)
    {
        Some(doc_id) => {
            store.delete_document(doc_id).await?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Remove an indexed document (and its chunks) from kb-core.
#[tauri::command]
pub async fn delete_document(state: State<'_, AppState>, id: String) -> AppResult<()> {
    memory::require(&state).await?.delete_document(&id).await
}

/// Re-scan the folders kb-core watches (`KB_CORE_WATCH`) now.
#[tauri::command]
pub async fn sync_knowledge_folders(state: State<'_, AppState>) -> AppResult<Value> {
    memory::require(&state).await?.sync_folders().await
}

/// Collection names: what kb-core accepts (lowercase, digits, `-`, `_`).
fn check_kb_name(name: &str) -> AppResult<String> {
    let n = name.trim().to_lowercase().replace(' ', "-");
    let ok = !n.is_empty()
        && n.len() <= 64
        && n.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && n.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if ok {
        Ok(n)
    } else {
        Err(AppError::InvalidInput(
            "knowledge base names use letters, digits, - and _ (max 64)".into(),
        ))
    }
}

/// Knowledge bases (kb-core collections) with counts.
#[tauri::command]
pub async fn list_knowledge_bases(state: State<'_, AppState>) -> AppResult<Vec<Value>> {
    memory::require(&state).await?.list_collections().await
}

/// Create a knowledge base.
#[tauri::command]
pub async fn create_knowledge_base(
    state: State<'_, AppState>,
    name: String,
    description: String,
) -> AppResult<Value> {
    let n = check_kb_name(&name)?;
    if description.len() > 500 {
        return Err(AppError::InvalidInput(
            "description is limited to 500 characters".into(),
        ));
    }
    memory::require(&state)
        .await?
        .create_collection(&n, description.trim())
        .await
}

/// Delete a knowledge base and everything in it (native confirmation, audited).
#[tauri::command]
pub async fn delete_knowledge_base(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> AppResult<Value> {
    use crate::security::audit::{AuditRecord, Decision};
    use crate::security::confirm::{self, ConfirmRequest};
    use crate::security::policy::RiskTier;
    let n = check_kb_name(&name)?;
    let store = memory::require(&state).await?;
    let info = store
        .list_collections()
        .await?
        .into_iter()
        .find(|c| c.get("name").and_then(Value::as_str) == Some(n.as_str()))
        .ok_or_else(|| AppError::InvalidInput(format!("no knowledge base `{n}`")))?;
    let count = |k: &str| info.get(k).and_then(Value::as_u64).unwrap_or(0);
    let c = confirm::require(
        &app,
        &state,
        &ConfirmRequest {
            title: "Delete knowledge base?".into(),
            subject: format!("Knowledge base:\n{n}"),
            details: vec![format!(
                "Deletes {} memories and {} documents in it. This can't be undone (export first to keep a copy).",
                count("memories"),
                count("documents")
            )],
            tier: RiskTier::Mutating,
            source: Source::User,
            reasons: vec!["permanently deletes stored memories and documents".into()],
            approve_label: "Delete".into(),
        },
    )
    .await;
    let (decision, conf) = match c {
        Ok(c) => (Decision::Allowed, c),
        Err(c) => (Decision::NotApproved, c),
    };
    let result = if decision == Decision::Allowed {
        Some(store.delete_collection(&n).await)
    } else {
        None
    };
    state
        .audit
        .record(AuditRecord {
            id: uuid::Uuid::new_v4().to_string(),
            source: Source::User,
            action: "kb_delete".into(),
            command: n.clone(),
            cwd: None,
            tier: RiskTier::Mutating,
            decision: match &result {
                Some(Err(_)) => Decision::Failed,
                _ => decision,
            },
            confirmation: conf,
            exit_code: None,
            duration_ms: None,
            detail: result
                .as_ref()
                .and_then(|r| r.as_ref().err())
                .map(ToString::to_string),
        })
        .await?;
    match result {
        Some(r) => r,
        None => Err(AppError::NotApproved(
            "the knowledge base was not deleted".into(),
        )),
    }
}

/// Export memories and documents to an NDJSON file the user picks in a native
/// save dialog. The file is written mode 600 (it holds personal data) and the
/// export is audited. Returns the path, or `None` if cancelled.
#[tauri::command]
pub async fn export_knowledge(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<String>> {
    let store = memory::require(&state).await?;
    let data = store.export().await?;
    let default_name = format!(
        "omnix-memory-{}.jsonl",
        chrono::Local::now().format("%Y%m%d")
    );
    let picked = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Export long-term memory")
            .set_file_name(&default_name)
            .add_filter("JSON Lines", &["jsonl"])
            .blocking_save_file()
    })
    .await?;
    let Some(fp) = picked else { return Ok(None) };
    let path = fp
        .into_path()
        .map_err(|e| AppError::InvalidInput(format!("unsupported file location: {e}")))?;
    files::write_user_chosen_private(&state, &path, &data, "memory_export").await?;
    Ok(Some(path.display().to_string()))
}

/// Import (merge) an export file the user picks in a native open dialog.
/// Returns kb-core's counts, or `None` if cancelled.
#[tauri::command]
pub async fn import_knowledge(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Option<Value>> {
    let store = memory::require(&state).await?;
    let picked = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Import long-term memory")
            .add_filter("JSON Lines", &["jsonl", "json", "ndjson"])
            .blocking_pick_file()
    })
    .await?;
    let Some(fp) = picked else { return Ok(None) };
    let path = fp
        .into_path()
        .map_err(|e| AppError::InvalidInput(format!("unsupported file location: {e}")))?;
    let text = files::read_user_chosen(&state, &path, MAX_IMPORT_BYTES, "memory_import").await?;
    let records = parse_ndjson(&text)?;
    Ok(Some(store.import(records).await?))
}

/// Parse NDJSON (one JSON object per line; blank lines ignored).
fn parse_ndjson(text: &str) -> AppResult<Vec<Value>> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(n, l)| {
            let v: Value = serde_json::from_str(l)
                .map_err(|e| AppError::InvalidInput(format!("line {}: {e}", n + 1)))?;
            if v.is_object() {
                Ok(v)
            } else {
                Err(AppError::InvalidInput(format!(
                    "line {}: not a JSON object",
                    n + 1
                )))
            }
        })
        .collect()
}

/// Consolidate duplicate memories, embed anything pending and compact the
/// store. Returns kb-core's report.
#[tauri::command]
pub async fn optimize_vector_db(state: State<'_, AppState>) -> AppResult<Value> {
    memory::require(&state).await?.maintenance().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_patch_whitelist() {
        let ok = |v: Value| clean_memory_patch(v.as_object().cloned().expect("object"));
        assert!(ok(json!({"reviewed": true})).is_ok());
        assert!(ok(json!({"superseded_by": null, "pinned": false})).is_ok());
        assert!(ok(json!({"reviewed": "yes"})).is_err());
        assert!(
            ok(json!({"superseded_by": "m_other"})).is_err(),
            "the UI can't hide memories"
        );
        assert!(ok(json!({"importance": 11})).is_err());
        assert!(ok(json!({"vec": [0.1]})).is_err());
    }

    #[test]
    fn note_names_are_confined() {
        assert_eq!(
            note_doc_name("nabc123").expect("ok"),
            "omnix-notepad::nabc123.md"
        );
        for bad in ["", "../x", "a/b", "a.md", "n x", &"a".repeat(41)] {
            assert!(note_doc_name(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn ndjson_parsing() {
        let v = parse_ndjson("{\"type\":\"memory\"}\n\n{\"type\":\"document\"}\n").expect("parse");
        assert_eq!(v.len(), 2);
        assert!(parse_ndjson("{\"a\":1}\n[1]").is_err());
        assert!(parse_ndjson("{oops").is_err());
    }
}
