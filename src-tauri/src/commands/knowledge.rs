//! Knowledge base / long-term memory commands, backed by the kb-core
//! adapter when `memory.backend_url` is set. Knowledge-base management,
//! export/import and optimization are not implemented.

use super::not_implemented;
use crate::error::{AppError, AppResult};
use crate::memory::{self, NewMemory};
use crate::security::files;
use crate::security::policy::Source;
use crate::state::AppState;
use serde_json::{json, Value};
use tauri::{AppHandle, State};

/// Summary for the Knowledge view. `available: false` when no memory backend
/// is configured.
#[tauri::command]
pub async fn get_knowledge_data(state: State<'_, AppState>) -> AppResult<Value> {
    let Some(store) = memory::from_state(&state).await? else {
        return Ok(json!({
            "available": false,
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
                "timestamp": m.created_at,
            })
        })
        .collect();
    Ok(json!({
        "available": true,
        "totalMemories": memories.len(),
        "totalDocuments": 0, "totalKnowledgeBases": 0, "storageUsed": 0,
        "vectorDimensions": 0, "embeddingModel": "",
        "memories": memories, "documents": [], "knowledgeBases": []
    }))
}

/// Store a memory in kb-core.
#[tauri::command]
pub async fn save_memory(
    state: State<'_, AppState>,
    content: String,
    tags: Vec<String>,
    importance: u8,
    category: String,
) -> AppResult<String> {
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
    memory::require(&state)
        .await?
        .save(NewMemory {
            content,
            tags,
            importance: importance.clamp(1, 10),
            category,
        })
        .await
}

/// Delete a memory from kb-core.
#[tauri::command]
pub async fn delete_memory(state: State<'_, AppState>, id: String) -> AppResult<()> {
    memory::require(&state).await?.delete(&id).await
}

/// Semantic search in kb-core.
#[tauri::command]
pub async fn semantic_search(
    state: State<'_, AppState>,
    query: String,
    limit: u32,
) -> AppResult<Vec<Value>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    let hits = memory::require(&state).await?.search(&query, limit).await?;
    Ok(hits
        .into_iter()
        .map(|h| json!({ "id": h.id, "content": h.content, "similarity": h.score, "tags": h.tags, "timestamp": h.created_at }))
        .collect())
}

/// Read a (user-chosen) text document through the guarded file reader and
/// send it to kb-core for chunking and embedding.
#[tauri::command]
pub async fn index_document(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> AppResult<u32> {
    let store = memory::require(&state).await?;
    let content = files::read_file(&app, &state, &path, Source::User).await?;
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "document".into());
    Ok(store.index_document(&name, &content).await?.chunks)
}

/// Not implemented: create a knowledge base.
#[tauri::command]
pub async fn create_knowledge_base() -> AppResult<()> {
    not_implemented("create_knowledge_base")
}

/// Not implemented: export knowledge.
#[tauri::command]
pub async fn export_knowledge() -> AppResult<()> {
    not_implemented("export_knowledge")
}

/// Not implemented: import knowledge.
#[tauri::command]
pub async fn import_knowledge() -> AppResult<()> {
    not_implemented("import_knowledge")
}

/// Not implemented: optimize the vector store.
#[tauri::command]
pub async fn optimize_vector_db() -> AppResult<()> {
    not_implemented("optimize_vector_db")
}
