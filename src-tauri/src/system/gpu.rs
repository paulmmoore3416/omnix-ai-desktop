//! GPU discovery and live metrics for every GPU in the machine.
//!
//! * **Discovery** (Linux): `/sys/class/drm/card*/device` gives vendor, PCI
//!   slot and bound driver for *every* GPU, including ones a vendor tool
//!   can't see (e.g. an AMD card next to an NVIDIA one).
//! * **NVIDIA**: `nvidia-smi --query-gpu … --format=csv` (fixed argv, see
//!   [`probe`](super::probe)) plus per-process VRAM from `--query-compute-apps`.
//! * **AMD** (`amdgpu`): sysfs counters (`gpu_busy_percent`,
//!   `mem_info_vram_*`, hwmon temperature/power/fan, current `pp_dpm_sclk`).
//!
//! Fields that can't be measured are `None`, never invented.

use super::probe;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// One process using GPU memory.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GpuProcess {
    /// Process id.
    pub pid: u32,
    /// Executable name.
    pub name: String,
    /// VRAM used, bytes.
    pub memory: u64,
}

/// A GPU and its current readings.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct GpuInfo {
    /// Stable index in OMNIX's list (sorted by PCI slot).
    pub index: usize,
    /// `nvidia`, `amd`, `intel` or `other`.
    pub vendor: String,
    /// Marketing name when known.
    pub name: String,
    /// Bound kernel driver (`nvidia`, `amdgpu`, `i915`, …), if any.
    pub driver: Option<String>,
    /// PCI slot, e.g. `0000:65:00.0`.
    pub pci: Option<String>,
    /// Core utilisation %.
    pub utilization: Option<f32>,
    /// VRAM used, bytes.
    pub memory_used: Option<u64>,
    /// VRAM total, bytes.
    pub memory_total: Option<u64>,
    /// Temperature °C.
    pub temperature: Option<f32>,
    /// Power draw, watts.
    pub power_w: Option<f32>,
    /// Power limit, watts.
    pub power_limit_w: Option<f32>,
    /// Fan speed % (NVIDIA) or derived from RPM/PWM (AMD).
    pub fan_percent: Option<f32>,
    /// Current core clock, MHz.
    pub clock_mhz: Option<u32>,
    /// Maximum core clock, MHz.
    pub clock_max_mhz: Option<u32>,
    /// Processes holding VRAM (NVIDIA only).
    pub processes: Vec<GpuProcess>,
    /// Why readings are missing, if they are.
    pub note: Option<String>,
}

const NVIDIA_FIELDS: &str = "pci.bus_id,name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw,power.limit,fan.speed,clocks.sm,clocks.max.sm";

/// Snapshot of all GPUs.
pub async fn snapshot() -> Vec<GpuInfo> {
    let mut gpus = discover_sysfs(Path::new("/sys/class/drm"));
    let nvidia = nvidia().await;

    // Merge NVIDIA readings into sysfs entries by PCI slot; add any NVIDIA
    // GPU sysfs didn't list (non-Linux hosts).
    for n in nvidia {
        let slot = n.pci.as_deref().map(normalise_pci);
        match gpus
            .iter_mut()
            .find(|g| g.pci.as_deref().map(normalise_pci) == slot && slot.is_some())
        {
            Some(g) => {
                let (index, driver) = (g.index, g.driver.clone());
                *g = GpuInfo {
                    index,
                    driver: driver.or(n.driver.clone()),
                    ..n
                };
            }
            None => gpus.push(n),
        }
    }
    for g in gpus.iter_mut().filter(|g| g.vendor == "amd") {
        if let Some(pci) = &g.pci {
            read_amd(&PathBuf::from("/sys/bus/pci/devices").join(pci), g);
        }
        if g.name.is_empty() || g.name.starts_with("AMD GPU") {
            if let Some(n) = lspci_name(g.pci.as_deref()).await {
                g.name = n;
            }
        }
    }
    for g in gpus.iter_mut() {
        if g.vendor == "nvidia" && g.utilization.is_none() && g.note.is_none() {
            g.note = Some(if probe::available("nvidia-smi") {
                "nvidia-smi did not report this GPU".into()
            } else {
                "install the NVIDIA driver utilities (nvidia-smi) for live readings".into()
            });
        }
    }
    gpus.sort_by(|a, b| a.pci.cmp(&b.pci));
    for (i, g) in gpus.iter_mut().enumerate() {
        g.index = i;
    }
    gpus
}

fn vendor_name(id: &str) -> &'static str {
    match id.trim().to_ascii_lowercase().as_str() {
        "0x10de" => "nvidia",
        "0x1002" => "amd",
        "0x8086" => "intel",
        _ => "other",
    }
}

/// `0000:65:00.0` and nvidia-smi's `00000000:65:00.0` → `65:00.0`.
fn normalise_pci(s: &str) -> String {
    let s = s.trim().to_ascii_lowercase();
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() >= 2 {
        parts[parts.len() - 2..].join(":")
    } else {
        s
    }
}

/// GPUs from `/sys/class/drm/cardN/device` (display-class PCI devices).
pub fn discover_sysfs(drm: &Path) -> Vec<GpuInfo> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(drm) else {
        return out;
    };
    let mut seen = std::collections::HashSet::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        // card0, card1 … (not card1-DP-1 connectors)
        if !name.starts_with("card") || !name[4..].chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let dev = e.path().join("device");
        let class = read_trim(&dev.join("class")).unwrap_or_default();
        if !class.is_empty() && !class.starts_with("0x03") {
            continue; // not a display controller
        }
        let pci = std::fs::canonicalize(&dev)
            .ok()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_string()));
        if let Some(p) = &pci {
            if !seen.insert(p.clone()) {
                continue;
            }
        }
        let vendor = vendor_name(&read_trim(&dev.join("vendor")).unwrap_or_default()).to_string();
        let driver = std::fs::read_link(dev.join("driver"))
            .ok()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_string()));
        let device_id = read_trim(&dev.join("device")).unwrap_or_default();
        out.push(GpuInfo {
            name: match vendor.as_str() {
                "amd" => format!("AMD GPU ({device_id})"),
                "nvidia" => format!("NVIDIA GPU ({device_id})"),
                "intel" => format!("Intel GPU ({device_id})"),
                _ => format!("GPU ({device_id})"),
            },
            vendor,
            driver,
            pci,
            ..Default::default()
        });
    }
    out
}

fn read_trim(p: &Path) -> Option<String> {
    std::fs::read_to_string(p)
        .ok()
        .map(|s| s.trim().to_string())
}

fn read_num<T: std::str::FromStr>(p: &Path) -> Option<T> {
    read_trim(p)?.parse().ok()
}

/// Fill AMD readings from sysfs (`dev` = the PCI device directory).
pub fn read_amd(dev: &Path, g: &mut GpuInfo) {
    g.utilization = read_num::<f32>(&dev.join("gpu_busy_percent"));
    g.memory_used = read_num(&dev.join("mem_info_vram_used"));
    g.memory_total = read_num(&dev.join("mem_info_vram_total"));
    if let Some(sclk) = read_trim(&dev.join("pp_dpm_sclk")) {
        let (cur, max) = parse_dpm(&sclk);
        g.clock_mhz = cur;
        g.clock_max_mhz = max;
    }
    let hwmon = std::fs::read_dir(dev.join("hwmon"))
        .ok()
        .and_then(|mut d| d.next())
        .and_then(|e| e.ok())
        .map(|e| e.path());
    if let Some(h) = hwmon {
        g.temperature = read_num::<f32>(&h.join("temp1_input")).map(|m| m / 1000.0);
        g.power_w = read_num::<f32>(&h.join("power1_average"))
            .or_else(|| read_num::<f32>(&h.join("power1_input")))
            .map(|uw| uw / 1_000_000.0);
        g.power_limit_w = read_num::<f32>(&h.join("power1_cap")).map(|uw| uw / 1_000_000.0);
        g.fan_percent = match (
            read_num::<f32>(&h.join("pwm1")),
            read_num::<f32>(&h.join("pwm1_max")),
        ) {
            (Some(p), Some(m)) if m > 0.0 => Some(p / m * 100.0),
            _ => None,
        };
    }
    if g.utilization.is_none() && g.memory_total.is_none() {
        g.note = Some("this driver exposes no utilisation counters".into());
    }
}

/// `pp_dpm_sclk` lines like `7: 1370Mhz *` → (current, max).
pub fn parse_dpm(s: &str) -> (Option<u32>, Option<u32>) {
    let mut cur = None;
    let mut max = None;
    for line in s.lines() {
        let mhz = line
            .split(':')
            .nth(1)
            .and_then(|v| {
                v.trim()
                    .trim_end_matches('*')
                    .trim()
                    .to_ascii_lowercase()
                    .strip_suffix("mhz")
                    .map(str::to_string)
            })
            .and_then(|v| v.trim().parse::<u32>().ok());
        if let Some(m) = mhz {
            max = Some(max.map_or(m, |x: u32| x.max(m)));
            if line.trim_end().ends_with('*') {
                cur = Some(m);
            }
        }
    }
    (cur, max)
}

async fn nvidia() -> Vec<GpuInfo> {
    let Some(out) = probe::run(
        "nvidia-smi",
        &[
            &format!("--query-gpu={NVIDIA_FIELDS}"),
            "--format=csv,noheader,nounits",
        ],
        Duration::from_secs(5),
    )
    .await
    else {
        return vec![];
    };
    let mut gpus = parse_nvidia(&out);
    // Per-process VRAM, mapped to GPUs by bus id.
    if let Some(apps) = probe::run(
        "nvidia-smi",
        &[
            "--query-compute-apps=gpu_bus_id,pid,process_name,used_memory",
            "--format=csv,noheader,nounits",
        ],
        Duration::from_secs(5),
    )
    .await
    {
        let mut by_bus: HashMap<String, Vec<GpuProcess>> = HashMap::new();
        for line in apps.lines() {
            let f: Vec<&str> = line.split(',').map(str::trim).collect();
            if f.len() < 4 {
                continue;
            }
            if let (Ok(pid), Ok(mem)) = (f[1].parse(), f[3].parse::<u64>()) {
                let name = Path::new(f[2])
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| f[2].to_string());
                by_bus
                    .entry(normalise_pci(f[0]))
                    .or_default()
                    .push(GpuProcess {
                        pid,
                        name,
                        memory: mem * 1024 * 1024,
                    });
            }
        }
        for g in gpus.iter_mut() {
            if let Some(p) = g
                .pci
                .as_deref()
                .and_then(|b| by_bus.remove(&normalise_pci(b)))
            {
                g.processes = p;
            }
        }
    }
    gpus
}

fn opt_f32(s: &str) -> Option<f32> {
    s.trim().parse::<f32>().ok().filter(|v| v.is_finite())
}

/// Parse `nvidia-smi --query-gpu=<NVIDIA_FIELDS> --format=csv,noheader,nounits`.
pub fn parse_nvidia(out: &str) -> Vec<GpuInfo> {
    out.lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .filter_map(|(i, line)| {
            let f: Vec<&str> = line.split(',').map(str::trim).collect();
            if f.len() < 11 {
                return None;
            }
            let mib = |s: &str| opt_f32(s).map(|v| (v as u64) * 1024 * 1024);
            Some(GpuInfo {
                index: i,
                vendor: "nvidia".into(),
                name: f[1].to_string(),
                driver: Some("nvidia".into()),
                pci: Some(f[0].to_string()),
                utilization: opt_f32(f[2]),
                memory_used: mib(f[3]),
                memory_total: mib(f[4]),
                temperature: opt_f32(f[5]),
                power_w: opt_f32(f[6]),
                power_limit_w: opt_f32(f[7]),
                fan_percent: opt_f32(f[8]),
                clock_mhz: opt_f32(f[9]).map(|v| v as u32),
                clock_max_mhz: opt_f32(f[10]).map(|v| v as u32),
                processes: vec![],
                note: None,
            })
        })
        .collect()
}

async fn lspci_name(pci: Option<&str>) -> Option<String> {
    let slot = pci?;
    // Hardcoded argv; the slot comes from sysfs (hex/colon/dot only).
    if !slot
        .chars()
        .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
    {
        return None;
    }
    let out = probe::run("lspci", &["-mm", "-s", slot], Duration::from_secs(3)).await?;
    // "65:00.0" "VGA compatible controller" "Vendor" "Device name" ...
    let fields: Vec<&str> = out.split('"').filter(|s| !s.trim().is_empty()).collect();
    let device = fields.get(3)?.trim();
    let short = device
        .rsplit_once('[')
        .map(|(_, r)| r.trim_end_matches(']').to_string())
        .unwrap_or_else(|| device.to_string());
    Some(format!("AMD Radeon {short}").replace("AMD Radeon Radeon", "AMD Radeon"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nvidia_csv() {
        let g = parse_nvidia(
            "00000000:65:00.0, NVIDIA GeForce GTX 1060 6GB, 7, 3452, 6144, 41, 6.47, 120.00, 28, 139, 1974\n",
        );
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].name, "NVIDIA GeForce GTX 1060 6GB");
        assert_eq!(g[0].memory_total, Some(6144 * 1024 * 1024));
        assert_eq!(g[0].utilization, Some(7.0));
        assert_eq!(g[0].clock_max_mhz, Some(1974));
        let na = parse_nvidia("0:1:0.0, X, [N/A], 1, 2, [N/A], [N/A], [N/A], [N/A], 3, 4\n");
        assert_eq!(na[0].utilization, None);
        assert_eq!(na[0].temperature, None);
    }

    #[test]
    fn pci_normalisation_matches_both_formats() {
        assert_eq!(
            normalise_pci("00000000:65:00.0"),
            normalise_pci("0000:65:00.0")
        );
        assert_eq!(normalise_pci("0000:17:00.0"), "17:00.0");
    }

    #[test]
    fn parses_amd_dpm() {
        assert_eq!(
            parse_dpm("0: 300Mhz\n1: 600Mhz\n7: 1370Mhz *\n"),
            (Some(1370), Some(1370))
        );
        assert_eq!(
            parse_dpm("0: 300Mhz *\n1: 1200Mhz\n"),
            (Some(300), Some(1200))
        );
    }

    #[test]
    fn reads_amd_sysfs_and_discovers_cards() {
        let dir = tempfile::tempdir().expect("tmp");
        let dev = dir.path().join("0000:17:00.0");
        let hw = dev.join("hwmon").join("hwmon3");
        std::fs::create_dir_all(&hw).expect("mkdir");
        for (f, v) in [
            ("gpu_busy_percent", "12"),
            ("mem_info_vram_used", "3784916992"),
            ("mem_info_vram_total", "8589934592"),
            ("pp_dpm_sclk", "0: 300Mhz\n7: 1370Mhz *\n"),
            ("vendor", "0x1002"),
            ("device", "0x67df"),
            ("class", "0x030000"),
        ] {
            std::fs::write(dev.join(f), v).expect("write");
        }
        for (f, v) in [
            ("temp1_input", "50000"),
            ("power1_average", "34000000"),
            ("pwm1", "51"),
            ("pwm1_max", "255"),
        ] {
            std::fs::write(hw.join(f), v).expect("write");
        }
        let mut g = GpuInfo::default();
        read_amd(&dev, &mut g);
        assert_eq!(g.utilization, Some(12.0));
        assert_eq!(g.memory_total, Some(8_589_934_592));
        assert_eq!(g.temperature, Some(50.0));
        assert_eq!(g.power_w, Some(34.0));
        assert_eq!(g.fan_percent.map(|f| f.round()), Some(20.0));

        // discovery: drm/card1/device -> the device dir; connectors ignored
        let drm = dir.path().join("drm");
        std::fs::create_dir_all(drm.join("card1")).expect("mkdir");
        std::fs::create_dir_all(drm.join("card1-DP-1")).expect("mkdir");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&dev, drm.join("card1").join("device")).expect("link");
        #[cfg(unix)]
        {
            let found = discover_sysfs(&drm);
            assert_eq!(found.len(), 1);
            assert_eq!(found[0].vendor, "amd");
            assert_eq!(found[0].pci.as_deref(), Some("0000:17:00.0"));
        }
    }
}
