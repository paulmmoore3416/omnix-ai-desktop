//! System metrics via `sysinfo`.
//!
//! Values that are not measured yet are reported as `None` (serialized as
//! `null`) rather than invented; the UI shows "n/a".

use crate::error::AppResult;
use serde::Serialize;
use sysinfo::System;

/// Compact status for the sidebar.
#[derive(Debug, Clone, Serialize)]
pub struct SystemStatus {
    /// Global CPU usage in percent.
    pub cpu: f32,
    /// Memory usage in percent.
    pub memory: f32,
    /// Always `online` when the backend answers.
    pub status: String,
    /// Seconds since boot.
    pub uptime: u64,
    /// Number of processes.
    pub processes: usize,
}

/// Static host information.
#[derive(Debug, Clone, Serialize)]
pub struct SystemInfo {
    /// OS name.
    pub os: Option<String>,
    /// Kernel version.
    pub kernel: Option<String>,
    /// OS version.
    pub os_version: Option<String>,
    /// Host name.
    pub hostname: Option<String>,
    /// Seconds since boot.
    pub uptime: u64,
    /// CPU brand string.
    pub cpu_model: Option<String>,
    /// Logical CPU count.
    pub cpu_cores: usize,
    /// Total RAM in bytes.
    pub total_memory: u64,
    /// Used RAM in bytes.
    pub used_memory: u64,
    /// Total swap in bytes.
    pub total_swap: u64,
    /// Used swap in bytes.
    pub used_swap: u64,
}

/// Network throughput sample.
#[derive(Debug, Clone, Serialize)]
pub struct NetworkStats {
    /// Received bytes/s.
    pub rx: u64,
    /// Transmitted bytes/s.
    pub tx: u64,
}

/// Live metrics for the System Control view.
#[derive(Debug, Clone, Serialize)]
pub struct RealTimeStats {
    /// CPU usage %.
    pub cpu: f32,
    /// Memory usage %.
    pub memory: f32,
    /// Swap usage %.
    pub swap: f32,
    /// Disk usage % (`None` until measured).
    pub disk: Option<f32>,
    /// Network throughput (`None` until measured).
    pub network: Option<NetworkStats>,
    /// Hottest sensor in °C (`None` when unavailable).
    pub temperature: Option<f32>,
}

/// Percentage helper that never divides by zero.
pub fn percent(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (used as f64 / total as f64 * 100.0) as f32
    }
}

/// CPU + memory snapshot.
pub fn status(sys: &mut System) -> AppResult<SystemStatus> {
    sys.refresh_cpu();
    sys.refresh_memory();
    Ok(SystemStatus {
        cpu: sys.global_cpu_info().cpu_usage(),
        memory: percent(sys.used_memory(), sys.total_memory()),
        status: "online".into(),
        uptime: System::uptime(),
        processes: sys.processes().len(),
    })
}

/// Static host info.
pub fn info(sys: &mut System) -> AppResult<SystemInfo> {
    sys.refresh_memory();
    sys.refresh_cpu();
    Ok(SystemInfo {
        os: System::name(),
        kernel: System::kernel_version(),
        os_version: System::os_version(),
        hostname: System::host_name(),
        uptime: System::uptime(),
        cpu_model: sys.cpus().first().map(|c| c.brand().trim().to_string()),
        cpu_cores: sys.cpus().len(),
        total_memory: sys.total_memory(),
        used_memory: sys.used_memory(),
        total_swap: sys.total_swap(),
        used_swap: sys.used_swap(),
    })
}

/// Live stats.
pub fn real_time(sys: &mut System) -> AppResult<RealTimeStats> {
    sys.refresh_cpu();
    sys.refresh_memory();
    Ok(RealTimeStats {
        cpu: sys.global_cpu_info().cpu_usage(),
        memory: percent(sys.used_memory(), sys.total_memory()),
        swap: percent(sys.used_swap(), sys.total_swap()),
        disk: None,
        network: None,
        temperature: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_handles_zero() {
        assert_eq!(percent(5, 0), 0.0);
        assert!((percent(1, 4) - 25.0).abs() < f32::EPSILON);
    }
}
