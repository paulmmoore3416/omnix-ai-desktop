//! Knowledge base / long-term memory commands.
//!
//! No memory backend exists yet, so everything except the (honest, empty)
//! summary returns `NotImplemented`.

use super::not_implemented;
use crate::error::AppResult;
use serde_json::{json, Value};

/// Summary for the Knowledge view. `available: false` tells the UI to show
/// the "not yet available" state.
#[tauri::command]
pub async fn get_knowledge_data() -> AppResult<Value> {
    Ok(json!({
        "available": false,
        "totalMemories": 0,
        "totalDocuments": 0,
        "totalKnowledgeBases": 0,
        "storageUsed": 0,
        "vectorDimensions": 0,
        "embeddingModel": "",
        "memories": [],
        "documents": [],
        "knowledgeBases": []
    }))
}

/// Not implemented: store a memory.
#[tauri::command]
pub async fn save_memory() -> AppResult<()> {
    not_implemented("save_memory")
}

/// Not implemented: delete a memory.
#[tauri::command]
pub async fn delete_memory() -> AppResult<()> {
    not_implemented("delete_memory")
}

/// Not implemented: semantic search.
#[tauri::command]
pub async fn semantic_search() -> AppResult<Vec<Value>> {
    not_implemented("semantic_search")
}

/// Not implemented: index a document.
#[tauri::command]
pub async fn index_document() -> AppResult<()> {
    not_implemented("index_document")
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
