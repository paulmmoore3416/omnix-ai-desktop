//! Rolling metrics history (sparklines, sustained-threshold alerts).
//!
//! The ops engine samples every [`SAMPLE_SECS`] seconds; [`CAPACITY`]
//! samples cover one hour. Memory stays bounded regardless of uptime.

use serde::Serialize;
use std::collections::VecDeque;

/// Seconds between samples.
pub const SAMPLE_SECS: u64 = 5;
/// Samples kept (one hour).
pub const CAPACITY: usize = 720;

/// Per-GPU reading in a sample.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GpuSample {
    /// Utilisation %.
    pub util: Option<f32>,
    /// VRAM used %.
    pub mem: Option<f32>,
    /// °C.
    pub temp: Option<f32>,
}

/// One point in time.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Sample {
    /// Unix time, milliseconds.
    pub ts: i64,
    /// CPU %.
    pub cpu: f32,
    /// RAM %.
    pub memory: f32,
    /// Swap %.
    pub swap: f32,
    /// Disk used %.
    pub disk: Option<f32>,
    /// Received bytes/s.
    pub net_rx: u64,
    /// Transmitted bytes/s.
    pub net_tx: u64,
    /// Hottest sensor °C.
    pub temp: Option<f32>,
    /// One entry per GPU, in [`super::gpu::snapshot`] order.
    pub gpus: Vec<GpuSample>,
}

/// Bounded ring of samples.
#[derive(Debug, Default)]
pub struct History {
    samples: VecDeque<Sample>,
}

impl History {
    /// Append, evicting the oldest sample beyond [`CAPACITY`].
    pub fn push(&mut self, s: Sample) {
        if self.samples.len() == CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(s);
    }

    /// Samples from the last `secs` seconds (all when `None`), oldest first.
    pub fn since(&self, secs: Option<u64>) -> Vec<Sample> {
        let Some(secs) = secs else {
            return self.samples.iter().cloned().collect();
        };
        let cutoff = self
            .samples
            .back()
            .map_or(0, |s| s.ts - (secs as i64) * 1000);
        self.samples
            .iter()
            .filter(|s| s.ts >= cutoff)
            .cloned()
            .collect()
    }

    /// Most recent sample.
    pub fn latest(&self) -> Option<&Sample> {
        self.samples.back()
    }

    /// True when `pred` held for every sample of the last `secs` seconds
    /// *and* the window is actually covered (no firing on a cold start).
    pub fn sustained(&self, secs: u64, pred: impl Fn(&Sample) -> bool) -> bool {
        let Some(last) = self.samples.back() else {
            return false;
        };
        let cutoff = last.ts - (secs as i64) * 1000;
        let covered = self.samples.front().is_some_and(|f| f.ts <= cutoff);
        if secs > 0 && !covered {
            return false;
        }
        self.samples.iter().filter(|s| s.ts >= cutoff).all(pred)
    }

    /// Number of samples held.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// No samples yet.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(ts: i64, cpu: f32) -> Sample {
        Sample {
            ts,
            cpu,
            memory: 0.0,
            swap: 0.0,
            disk: None,
            net_rx: 0,
            net_tx: 0,
            temp: None,
            gpus: vec![],
        }
    }

    #[test]
    fn ring_is_bounded() {
        let mut h = History::default();
        for i in 0..(CAPACITY as i64 + 10) {
            h.push(s(i * 5000, 1.0));
        }
        assert_eq!(h.len(), CAPACITY);
        assert_eq!(h.since(Some(10)).len(), 3);
    }

    #[test]
    fn sustained_requires_full_window() {
        let mut h = History::default();
        h.push(s(0, 95.0));
        h.push(s(5000, 95.0));
        assert!(!h.sustained(60, |x| x.cpu > 90.0), "window not covered yet");
        for i in 2..=13 {
            h.push(s(i * 5000, 95.0));
        }
        assert!(h.sustained(60, |x| x.cpu > 90.0));
        h.push(s(14 * 5000, 10.0));
        assert!(!h.sustained(60, |x| x.cpu > 90.0));
        assert!(h.sustained(0, |x| x.cpu < 50.0));
    }
}
