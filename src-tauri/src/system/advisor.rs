//! Optimization advisor: findings from measured host state, each with an
//! optional one-click fix that runs through an existing guarded path
//! (model unload via the audited Ollama API, service restart via the
//! executor with confirmation, kb-core maintenance, cleanup view).
//!
//! It deliberately doesn't do "optimisations" with no measurable effect
//! (dropping page caches, renicing at random).

use super::{docker, gpu, services};
use crate::state::AppState;
use serde::Serialize;
use serde_json::{json, Value};

/// A finding.
#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    /// Stable id (used to apply it).
    pub id: String,
    /// `info`, `warning`, `critical`.
    pub severity: String,
    /// Area: `models`, `memory`, `disk`, `gpu`, `hardware`, `services`, `containers`, `memory_store`.
    pub area: String,
    /// Headline.
    pub title: String,
    /// Explanation with the measured numbers.
    pub detail: String,
    /// Fix, if one exists: `{kind, label, params}`.
    pub action: Option<Value>,
}

fn rec(
    id: String,
    severity: &str,
    area: &str,
    title: String,
    detail: String,
    action: Option<Value>,
) -> Recommendation {
    Recommendation {
        id,
        severity: severity.into(),
        area: area.into(),
        title,
        detail,
        action,
    }
}

/// Analyse the host.
pub async fn analyse(state: &AppState) -> Vec<Recommendation> {
    let mut out = vec![];
    let configured = state.settings.read().await.ai.ollama_model.clone();

    // Models: partly on CPU, or loaded but not the one in use.
    if let Ok(loaded) = crate::ai::ollama_admin::loaded(state).await {
        for m in &loaded {
            if m.size > 0 && m.gpu_percent < 99.0 {
                out.push(rec(
                    format!("model_cpu:{}", m.name),
                    if m.gpu_percent < 60.0 { "warning" } else { "info" },
                    "models",
                    format!("{} is partly in system RAM", m.name),
                    format!(
                        "{:.0}% of its {:.1} GB is on the GPU; the rest runs on the CPU, which is much slower. \
                         Unload other models, reduce the context window, or use a smaller quantization.",
                        m.gpu_percent,
                        m.size as f64 / 1e9
                    ),
                    None,
                ));
            }
            if !configured.is_empty() && m.name != configured && !m.name.contains("embed") {
                out.push(rec(
                    format!("model_idle:{}", m.name),
                    "info",
                    "models",
                    format!("Unload {} to free {:.1} GB", m.name, m.size as f64 / 1e9),
                    format!("It's loaded but OMNIX chats with {configured}. It reloads automatically if something uses it."),
                    Some(json!({ "kind": "unload_model", "label": "Unload", "params": { "model": m.name } })),
                ));
            }
        }
    }

    // Memory pressure.
    if let Ok(d) = state.monitor().and_then(|mut m| m.detailed()) {
        let avail = d.memory_available as f64 / d.memory_total.max(1) as f64;
        if avail < 0.10 {
            out.push(rec(
                "memory_low".into(),
                "warning",
                "memory",
                "Memory is nearly exhausted".into(),
                format!(
                    "Only {:.1} GB of {:.1} GB is available. Check the Processes tab for the largest programs.",
                    d.memory_available as f64 / 1e9,
                    d.memory_total as f64 / 1e9
                ),
                None,
            ));
        }
        if d.swap_total > 0 && d.swap_used as f64 / d.swap_total as f64 > 0.5 && avail < 0.2 {
            out.push(rec(
                "swap_heavy".into(),
                "warning",
                "memory",
                "Heavy swapping".into(),
                format!(
                    "{:.1} GB of swap in use while RAM is tight; everything slows down.",
                    d.swap_used as f64 / 1e9
                ),
                None,
            ));
        }
        for disk in &d.disks {
            if disk.total == 0 || disk.removable {
                continue;
            }
            let pct = disk.used as f64 / disk.total as f64 * 100.0;
            if pct >= 90.0 {
                out.push(rec(
                    format!("disk_full:{}", disk.mount),
                    if pct >= 95.0 { "critical" } else { "warning" },
                    "disk",
                    format!("{} is {:.0}% full", disk.mount, pct),
                    format!(
                        "{:.1} GB free of {:.1} GB.",
                        (disk.total - disk.used) as f64 / 1e9,
                        disk.total as f64 / 1e9
                    ),
                    Some(json!({ "kind": "open_cleanup", "label": "Find space", "params": {} })),
                ));
            }
        }
        if let Some(s) = d.sensors.first() {
            if s.critical.is_some_and(|c| s.temperature >= c - 5.0) || s.temperature >= 90.0 {
                out.push(rec(
                    "hot_sensor".into(),
                    "critical",
                    "hardware",
                    format!("{} is at {:.0} °C", s.label, s.temperature),
                    "Check cooling and dust; sustained heat throttles performance.".into(),
                    None,
                ));
            }
        }
    }

    // GPUs.
    for g in gpu::snapshot().await {
        if g.temperature.is_some_and(|t| t >= 85.0) {
            out.push(rec(
                format!("gpu_hot:{}", g.index),
                "warning",
                "gpu",
                format!(
                    "GPU {} ({}) is at {:.0} °C",
                    g.index,
                    g.name,
                    g.temperature.unwrap_or_default()
                ),
                "High GPU temperature reduces clocks; check airflow.".into(),
                None,
            ));
        }
        if let (Some(u), Some(t)) = (g.memory_used, g.memory_total) {
            if t > 0 && u as f64 / t as f64 > 0.97 {
                out.push(rec(
                    format!("gpu_full:{}", g.index),
                    "info",
                    "gpu",
                    format!("GPU {} memory is full", g.index),
                    "New models or longer contexts will spill to the CPU. Unload models you aren't using.".into(),
                    None,
                ));
            }
        }
    }

    // Failed services.
    for s in services::list()
        .await
        .into_iter()
        .filter(|s| s.active == "failed")
    {
        out.push(rec(
            format!("service_failed:{}/{}", s.scope, s.unit),
            "warning",
            "services",
            format!("{} has failed", s.unit),
            format!("{} ({} service). Check its logs in the Services tab before restarting.", s.description, s.scope),
            Some(json!({ "kind": "restart_service", "label": "Restart", "params": { "scope": s.scope, "unit": s.unit } })),
        ));
    }

    // Exited containers.
    let d = docker::list().await;
    let exited: Vec<&docker::Container> = d
        .containers
        .iter()
        .filter(|c| c.state == "exited")
        .collect();
    if exited.len() >= 3 {
        out.push(rec(
            "containers_exited".into(),
            "info",
            "containers",
            format!("{} stopped containers", exited.len()),
            format!(
                "{} … Remove the ones you no longer need with `docker rm`.",
                exited
                    .iter()
                    .take(5)
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            None,
        ));
    }

    // Memory store upkeep.
    if let Ok(Some(store)) = crate::memory::from_state(state).await {
        if let Ok(s) = store.stats().await {
            let pending = s
                .get("pending_embeddings")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let stale = s
                .get("last_maintenance")
                .and_then(Value::as_str)
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .is_none_or(|t| (chrono::Utc::now().fixed_offset() - t).num_days() >= 7);
            if pending > 0 || stale {
                out.push(rec(
                    "kb_maintenance".into(),
                    "info",
                    "memory_store",
                    "Tidy up long-term memory".into(),
                    if pending > 0 {
                        format!("{pending} items are waiting for embeddings; maintenance also merges duplicates and compacts the database.")
                    } else {
                        "Not optimised in the last week: merges duplicate memories and compacts the database.".into()
                    },
                    Some(json!({ "kind": "kb_maintenance", "label": "Optimize", "params": {} })),
                ));
            }
        }
    }

    let order = |s: &str| match s {
        "critical" => 0,
        "warning" => 1,
        _ => 2,
    };
    out.sort_by_key(|r| order(&r.severity));
    out
}
