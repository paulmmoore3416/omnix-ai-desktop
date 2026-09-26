//! One JSON snapshot of the host for the model (`host_status` tool) and for
//! scheduled AI reports. Everything in it is measured; nothing is inferred.

use super::{docker, gpu, processes, services};
use crate::state::AppState;
use serde_json::{json, Value};

/// Which sections to include.
#[derive(Debug, Clone, Copy)]
pub struct Sections {
    /// CPU, memory, disks, network, sensors, GPUs.
    pub metrics: bool,
    /// Top processes.
    pub processes: bool,
    /// Failed / notable services.
    pub services: bool,
    /// Docker containers.
    pub containers: bool,
    /// Ollama models in memory.
    pub models: bool,
    /// Alerts currently firing.
    pub alerts: bool,
}

impl Sections {
    /// Everything.
    pub const ALL: Sections = Sections {
        metrics: true,
        processes: true,
        services: true,
        containers: true,
        models: true,
        alerts: true,
    };

    /// Parse a list like `["metrics","services"]`; empty = all.
    pub fn from_names(names: &[String]) -> Self {
        if names.is_empty() {
            return Self::ALL;
        }
        let has = |n: &str| names.iter().any(|x| x == n);
        Sections {
            metrics: has("metrics") || has("gpus"),
            processes: has("processes"),
            services: has("services"),
            containers: has("containers") || has("docker"),
            models: has("models"),
            alerts: has("alerts"),
        }
    }
}

fn gb(b: u64) -> f64 {
    (b as f64 / 1e9 * 10.0).round() / 10.0
}

/// Build the snapshot.
pub async fn host(state: &AppState, s: Sections) -> Value {
    let mut out = serde_json::Map::new();
    out.insert(
        "time".into(),
        json!(chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)),
    );
    if s.metrics || s.processes {
        let (detailed, top) = match state.monitor() {
            Ok(mut m) => (
                m.detailed().ok(),
                if s.processes {
                    processes::list(&mut m).into_iter().take(10).collect()
                } else {
                    vec![]
                },
            ),
            Err(_) => (None, vec![]),
        };
        if s.metrics {
            if let Some(d) = detailed {
                out.insert(
                    "host".into(),
                    json!({
                        "cpu_percent": (d.cpu * 10.0).round() / 10.0,
                        "cores": d.per_core.len(),
                        "load_avg": d.load,
                        "memory_used_gb": gb(d.memory_used),
                        "memory_total_gb": gb(d.memory_total),
                        "memory_available_gb": gb(d.memory_available),
                        "swap_used_gb": gb(d.swap_used),
                        "disks": d.disks.iter().map(|x| json!({
                            "mount": x.mount, "used_gb": gb(x.used), "total_gb": gb(x.total),
                            "percent": if x.total > 0 { (x.used as f64 / x.total as f64 * 1000.0).round() / 10.0 } else { 0.0 },
                        })).collect::<Vec<_>>(),
                        "hottest_sensor_c": d.sensors.first().map(|x| x.temperature),
                        "uptime_hours": d.uptime / 3600,
                        "processes": d.processes,
                    }),
                );
            }
            let gpus = gpu::snapshot().await;
            out.insert(
                "gpus".into(),
                json!(gpus
                    .iter()
                    .map(|g| json!({
                        "index": g.index, "name": g.name, "vendor": g.vendor,
                        "util_percent": g.utilization,
                        "vram_used_gb": g.memory_used.map(gb), "vram_total_gb": g.memory_total.map(gb),
                        "temp_c": g.temperature, "power_w": g.power_w,
                        "processes": g.processes.iter().map(|p| json!({"pid": p.pid, "name": p.name, "vram_gb": gb(p.memory)})).collect::<Vec<_>>(),
                    }))
                    .collect::<Vec<_>>()),
            );
        }
        if s.processes {
            out.insert(
                "top_processes".into(),
                json!(top
                    .iter()
                    .map(|p| json!({"pid": p.pid, "name": p.name, "cpu": p.cpu, "memory_percent": p.memory}))
                    .collect::<Vec<_>>()),
            );
        }
    }
    if s.services {
        let all = services::list().await;
        let failed: Vec<Value> = all
            .iter()
            .filter(|x| x.active == "failed")
            .map(|x| json!({"unit": x.unit, "scope": x.scope, "description": x.description}))
            .collect();
        out.insert(
            "services".into(),
            json!({
                "failed": failed,
                "running": all.iter().filter(|x| x.sub == "running").count(),
                "total": all.len(),
            }),
        );
    }
    if s.containers {
        let d = docker::list().await;
        out.insert(
            "containers".into(),
            if d.available {
                json!(d
                    .containers
                    .iter()
                    .map(|c| json!({
                        "name": c.name, "image": c.image, "state": c.state, "status": c.status,
                        "cpu": c.cpu, "memory": c.memory,
                    }))
                    .collect::<Vec<_>>())
            } else {
                json!({ "unavailable": d.reason })
            },
        );
    }
    if s.models {
        out.insert(
            "models_loaded".into(),
            match crate::ai::ollama_admin::loaded(state).await {
                Ok(l) => json!(l.iter().map(|m| json!({
                    "name": m.name, "memory_gb": gb(m.size), "on_gpu_percent": m.gpu_percent.round(),
                    "context_length": m.context_length,
                })).collect::<Vec<_>>()),
                Err(e) => json!({ "unavailable": e.to_string() }),
            },
        );
    }
    if s.alerts {
        let f = state.ops.snapshot();
        out.insert(
            "alerts_firing".into(),
            json!(f
                .alerts
                .iter()
                .filter(|a| a.enabled && a.state.firing)
                .map(|a| json!({"name": a.name, "condition": a.condition.describe(), "since": a.state.since}))
                .collect::<Vec<_>>()),
        );
    }
    Value::Object(out)
}

#[cfg(test)]
mod live_tests {
    //! Opt-in probes against the real host: `cargo test -- --ignored live_host`.

    #[tokio::test]
    #[ignore = "reads the real host; run manually"]
    async fn live_host_probes() {
        let gpus = super::gpu::snapshot().await;
        for g in &gpus {
            eprintln!(
                "GPU {} {} [{}] util={:?} vram={:?}/{:?} temp={:?} power={:?} clk={:?}/{:?} procs={:?} note={:?}",
                g.index, g.name, g.vendor, g.utilization, g.memory_used, g.memory_total, g.temperature,
                g.power_w, g.clock_mhz, g.clock_max_mhz,
                g.processes.iter().map(|p| (&p.name, p.memory)).collect::<Vec<_>>(), g.note
            );
        }
        let svcs = super::services::list().await;
        eprintln!(
            "services: {} total, {} running, failed: {:?}",
            svcs.len(),
            svcs.iter().filter(|s| s.sub == "running").count(),
            svcs.iter()
                .filter(|s| s.active == "failed")
                .map(|s| &s.unit)
                .collect::<Vec<_>>()
        );
        let d = super::docker::list().await;
        eprintln!(
            "docker available={} reason={:?} containers={:?}",
            d.available,
            d.reason,
            d.containers
                .iter()
                .map(|c| (&c.name, &c.state, c.cpu))
                .collect::<Vec<_>>()
        );
        let mut m = crate::system::metrics::Monitor::new();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        let det = m.detailed().expect("detailed");
        eprintln!(
            "host: cores={} freq={:?} load={:?} disks={:?} nics={} sensors={}",
            det.per_core.len(),
            det.cpu_freq_mhz,
            det.load,
            det.disks.iter().map(|x| &x.mount).collect::<Vec<_>>(),
            det.network.len(),
            det.sensors.len()
        );
        assert!(!gpus.is_empty());
    }
}
