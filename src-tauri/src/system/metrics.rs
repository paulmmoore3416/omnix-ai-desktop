//! System metrics via `sysinfo`.
//!
//! [`Monitor`] owns the sysinfo handles and refreshes **only** what each call
//! needs (never `refresh_all()` on a timer). Values that cannot be measured on
//! this machine are `None` (serialized as `null`) rather than invented.

use crate::error::AppResult;
use serde::Serialize;
use std::collections::HashSet;
use std::time::Instant;
use sysinfo::{Components, Disks, Networks, System, MINIMUM_CPU_UPDATE_INTERVAL};

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
    /// Number of processes (from the last process refresh).
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
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct NetworkStats {
    /// Received bytes/s (all non-loopback interfaces).
    pub rx: u64,
    /// Transmitted bytes/s (all non-loopback interfaces).
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
    /// Used space across local disks, % (`None` if no disks are reported).
    pub disk: Option<f32>,
    /// Network throughput (`None` until two samples exist).
    pub network: Option<NetworkStats>,
    /// Hottest sensor in °C (`None` when no sensors are exposed).
    pub temperature: Option<f32>,
}

/// One mounted local disk.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DiskInfo {
    /// Device name.
    pub name: String,
    /// Mount point.
    pub mount: String,
    /// File system type.
    pub fs: String,
    /// Total bytes.
    pub total: u64,
    /// Used bytes.
    pub used: u64,
    /// Removable media.
    pub removable: bool,
}

/// Throughput of one network interface.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct NicStats {
    /// Interface name.
    pub name: String,
    /// Received bytes/s.
    pub rx: u64,
    /// Transmitted bytes/s.
    pub tx: u64,
    /// Total received since boot.
    pub total_rx: u64,
    /// Total transmitted since boot.
    pub total_tx: u64,
}

/// One temperature sensor.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SensorReading {
    /// Sensor label.
    pub label: String,
    /// °C.
    pub temperature: f32,
    /// Critical threshold °C, if known.
    pub critical: Option<f32>,
}

/// Everything the Performance view shows about the host (GPUs are in
/// [`super::gpu`]).
#[derive(Debug, Clone, Serialize)]
pub struct DetailedStats {
    /// Global CPU %.
    pub cpu: f32,
    /// Per logical core %.
    pub per_core: Vec<f32>,
    /// Average current core frequency, MHz.
    pub cpu_freq_mhz: Option<u64>,
    /// 1/5/15-minute load average (Unix; zeros elsewhere).
    pub load: [f64; 3],
    /// RAM total/used/available, bytes.
    pub memory_total: u64,
    /// Used RAM.
    pub memory_used: u64,
    /// Available RAM (includes reclaimable cache).
    pub memory_available: u64,
    /// Swap total.
    pub swap_total: u64,
    /// Swap used.
    pub swap_used: u64,
    /// Local disks.
    pub disks: Vec<DiskInfo>,
    /// Per-interface throughput (non-loopback).
    pub network: Vec<NicStats>,
    /// Temperature sensors, hottest first.
    pub sensors: Vec<SensorReading>,
    /// Seconds since boot.
    pub uptime: u64,
    /// Process count.
    pub processes: usize,
}

/// File systems that do not represent user storage (and would double-count
/// or skew disk usage).
const PSEUDO_FS: &[&str] = &[
    "tmpfs", "devtmpfs", "squashfs", "overlay", "proc", "sysfs", "ramfs", "efivarfs", "autofs",
];

/// Owns all sysinfo state and caches rate-limited samples.
pub struct Monitor {
    sys: System,
    disks: Disks,
    networks: Networks,
    components: Components,
    /// When CPU usage was last refreshed, to honour sysinfo's minimum interval
    /// (refreshing faster yields meaningless values).
    last_cpu: Option<Instant>,
    cpu_cache: f32,
    last_net: Instant,
    /// Last per-interface sample, reused for ~1 s so two callers in quick
    /// succession don't reset each other's rate window.
    nics: Option<(Instant, Vec<NicStats>)>,
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Monitor {
    /// Create the monitor and take baseline samples.
    pub fn new() -> Self {
        let mut sys = System::new();
        // First CPU sample establishes the baseline for usage deltas.
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        Self {
            sys,
            disks: Disks::new_with_refreshed_list(),
            networks: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            last_cpu: Some(Instant::now()),
            cpu_cache: 0.0,
            last_net: Instant::now(),
            nics: None,
        }
    }

    /// Mutable access to the underlying [`System`] (process listing/killing).
    pub fn sys(&mut self) -> &mut System {
        &mut self.sys
    }

    /// Global CPU usage, refreshed at most every `MINIMUM_CPU_UPDATE_INTERVAL`.
    pub fn cpu(&mut self) -> f32 {
        let due = self
            .last_cpu
            .is_none_or(|t| t.elapsed() >= MINIMUM_CPU_UPDATE_INTERVAL);
        if due {
            self.sys.refresh_cpu_usage();
            self.cpu_cache = self.sys.global_cpu_usage();
            self.last_cpu = Some(Instant::now());
        }
        self.cpu_cache
    }

    fn memory_percent(&mut self) -> (f32, f32) {
        self.sys.refresh_memory();
        (
            percent(self.sys.used_memory(), self.sys.total_memory()),
            percent(self.sys.used_swap(), self.sys.total_swap()),
        )
    }

    /// Used space across real local disks, de-duplicated by device.
    pub fn disk_percent(&mut self) -> Option<f32> {
        self.disks.refresh(true);
        let mut seen = HashSet::new();
        let (mut total, mut avail) = (0u64, 0u64);
        for d in self.disks.list() {
            let fs = d.file_system().to_string_lossy().to_lowercase();
            if PSEUDO_FS.contains(&fs.as_str()) || d.is_removable() || d.total_space() == 0 {
                continue;
            }
            // The same device can be mounted several times (btrfs subvolumes, bind mounts).
            if !seen.insert(d.name().to_os_string()) {
                continue;
            }
            total = total.saturating_add(d.total_space());
            avail = avail.saturating_add(d.available_space());
        }
        (total > 0).then(|| percent(total.saturating_sub(avail), total))
    }

    /// Per-interface rates since the previous sample (non-loopback).
    pub fn nic_stats(&mut self) -> Option<Vec<NicStats>> {
        if let Some((t, v)) = &self.nics {
            if t.elapsed() < std::time::Duration::from_millis(1000) {
                return Some(v.clone());
            }
        }
        let elapsed = self.last_net.elapsed().as_secs_f64();
        self.networks.refresh(true);
        self.last_net = Instant::now();
        if elapsed < 0.05 {
            return None;
        }
        let mut v: Vec<NicStats> = self
            .networks
            .list()
            .iter()
            .filter(|(name, _)| !is_loopback_iface(name))
            .map(|(name, n)| NicStats {
                name: name.clone(),
                rx: (n.received() as f64 / elapsed) as u64,
                tx: (n.transmitted() as f64 / elapsed) as u64,
                total_rx: n.total_received(),
                total_tx: n.total_transmitted(),
            })
            .collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        self.nics = Some((Instant::now(), v.clone()));
        Some(v)
    }

    /// Bytes/s since the previous sample, summed over non-loopback interfaces.
    pub fn network_rate(&mut self) -> Option<NetworkStats> {
        self.nic_stats().map(|v| {
            v.iter()
                .fold(NetworkStats { rx: 0, tx: 0 }, |acc, n| NetworkStats {
                    rx: acc.rx.saturating_add(n.rx),
                    tx: acc.tx.saturating_add(n.tx),
                })
        })
    }

    /// Local disks (pseudo file systems skipped, one entry per device).
    pub fn disks(&mut self) -> Vec<DiskInfo> {
        self.disks.refresh(true);
        let mut seen = HashSet::new();
        self.disks
            .list()
            .iter()
            .filter(|d| {
                let fs = d.file_system().to_string_lossy().to_lowercase();
                !PSEUDO_FS.contains(&fs.as_str()) && d.total_space() > 0
            })
            .filter(|d| seen.insert(d.name().to_os_string()))
            .map(|d| DiskInfo {
                name: d.name().to_string_lossy().to_string(),
                mount: d.mount_point().to_string_lossy().to_string(),
                fs: d.file_system().to_string_lossy().to_string(),
                total: d.total_space(),
                used: d.total_space().saturating_sub(d.available_space()),
                removable: d.is_removable(),
            })
            .collect()
    }

    /// All finite temperature sensors, hottest first.
    pub fn sensors(&mut self) -> Vec<SensorReading> {
        self.components.refresh(true);
        let mut v: Vec<SensorReading> = self
            .components
            .list()
            .iter()
            .filter_map(|c| {
                let t = c.temperature().filter(|t| t.is_finite())?;
                Some(SensorReading {
                    label: c.label().to_string(),
                    temperature: t,
                    critical: c.critical().filter(|t| t.is_finite()),
                })
            })
            .collect();
        v.sort_by(|a, b| b.temperature.total_cmp(&a.temperature));
        v
    }

    /// Full host snapshot for the Performance view.
    pub fn detailed(&mut self) -> AppResult<DetailedStats> {
        let cpu = self.cpu();
        let per_core: Vec<f32> = self.sys.cpus().iter().map(|c| c.cpu_usage()).collect();
        self.sys.refresh_cpu_frequency();
        let freqs: Vec<u64> = self
            .sys
            .cpus()
            .iter()
            .map(|c| c.frequency())
            .filter(|f| *f > 0)
            .collect();
        self.sys.refresh_memory();
        let l = System::load_average();
        Ok(DetailedStats {
            cpu,
            per_core,
            cpu_freq_mhz: (!freqs.is_empty())
                .then(|| freqs.iter().sum::<u64>() / freqs.len() as u64),
            load: [l.one, l.five, l.fifteen],
            memory_total: self.sys.total_memory(),
            memory_used: self.sys.used_memory(),
            memory_available: self.sys.available_memory(),
            swap_total: self.sys.total_swap(),
            swap_used: self.sys.used_swap(),
            disks: self.disks(),
            network: self.nic_stats().unwrap_or_default(),
            sensors: self.sensors(),
            uptime: System::uptime(),
            processes: self.sys.processes().len(),
        })
    }

    /// Highest finite sensor temperature in °C.
    pub fn temperature(&mut self) -> Option<f32> {
        self.components.refresh(true);
        max_finite(
            self.components
                .list()
                .iter()
                .filter_map(|c| c.temperature()),
        )
    }

    /// CPU + memory snapshot.
    pub fn status(&mut self) -> AppResult<SystemStatus> {
        let cpu = self.cpu();
        let (memory, _) = self.memory_percent();
        Ok(SystemStatus {
            cpu,
            memory,
            status: "online".into(),
            uptime: System::uptime(),
            processes: self.sys.processes().len(),
        })
    }

    /// Static host info.
    pub fn info(&mut self) -> AppResult<SystemInfo> {
        self.sys.refresh_memory();
        Ok(SystemInfo {
            os: System::name(),
            kernel: System::kernel_version(),
            os_version: System::os_version(),
            hostname: System::host_name(),
            uptime: System::uptime(),
            cpu_model: self
                .sys
                .cpus()
                .first()
                .map(|c| c.brand().trim().to_string()),
            cpu_cores: self.sys.cpus().len(),
            total_memory: self.sys.total_memory(),
            used_memory: self.sys.used_memory(),
            total_swap: self.sys.total_swap(),
            used_swap: self.sys.used_swap(),
        })
    }

    /// Live stats.
    pub fn real_time(&mut self) -> AppResult<RealTimeStats> {
        let cpu = self.cpu();
        let (memory, swap) = self.memory_percent();
        Ok(RealTimeStats {
            cpu,
            memory,
            swap,
            disk: self.disk_percent(),
            network: self.network_rate(),
            temperature: self.temperature(),
        })
    }
}

/// Percentage helper that never divides by zero.
pub fn percent(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (used as f64 / total as f64 * 100.0) as f32
    }
}

fn is_loopback_iface(name: &str) -> bool {
    name == "lo" || name.starts_with("lo0") || name.eq_ignore_ascii_case("loopback")
}

fn max_finite(values: impl Iterator<Item = f32>) -> Option<f32> {
    values.filter(|v| v.is_finite()).max_by(f32::total_cmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_handles_zero() {
        assert_eq!(percent(5, 0), 0.0);
        assert!((percent(1, 4) - 25.0).abs() < f32::EPSILON);
    }

    #[test]
    fn max_finite_ignores_nan() {
        assert_eq!(
            max_finite([40.0, f32::NAN, 55.5, 12.0].into_iter()),
            Some(55.5)
        );
        assert_eq!(max_finite(std::iter::empty()), None);
        assert_eq!(max_finite([f32::NAN].into_iter()), None);
    }

    #[test]
    fn monitor_produces_sane_values() {
        let mut m = Monitor::new();
        std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
        let s = m.real_time().expect("stats");
        assert!((0.0..=100.0).contains(&s.cpu));
        assert!((0.0..=100.0).contains(&s.memory));
        if let Some(d) = s.disk {
            assert!((0.0..=100.0).contains(&d));
        }
        assert!(s.network.is_some(), "second sample should yield a rate");
    }

    #[test]
    fn loopback_detection() {
        assert!(is_loopback_iface("lo"));
        assert!(is_loopback_iface("lo0"));
        assert!(!is_loopback_iface("eth0"));
        assert!(!is_loopback_iface("wlan0"));
    }
}
