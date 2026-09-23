//! What the adapter decides before it touches a Qt property.
//!
//! Everything here is a function over plain values, so it is tested without
//! a QObject: which snapshot is stale, what a percentage looks like as a
//! fraction, which numbers each kind of resource publishes and in what order
//! (the kind contract the page reads by index), and the only two policy
//! numbers Hematita has.

use hematita_core::gpu::GpuReading;
use hematita_core::memory::Memory;

use crate::sampler::{CpuReading, DiskInfo, InterfaceInfo};

/// Above this a value is worth noticing; above [`CRITICAL_PERCENT`] it is
/// worth interrupting for. The page maps these to appearance; the numbers are
/// policy and live here, not in the theme.
pub const ELEVATED_PERCENT: u8 = 80;
pub const CRITICAL_PERCENT: u8 = 90;

/// The state name the theme colours by.
#[must_use]
pub fn load_name(percent: u8) -> &'static str {
    if percent >= CRITICAL_PERCENT {
        "critical"
    } else if percent >= ELEVATED_PERCENT {
        "elevated"
    } else {
        "normal"
    }
}

/// The load a temperature paints, from the chip's own critical limit: the
/// same two thresholds as a percentage, applied to the fraction of `crit`.
#[must_use]
pub fn thermal_load(value: f64, crit: Option<f64>) -> &'static str {
    let Some(crit) = crit.filter(|c| *c > 0.0) else {
        return "normal";
    };
    let percent = (value / crit * 100.0).clamp(0.0, 255.0) as u8;
    load_name(percent)
}

/// The load a power reading paints, from the chip's own cap: the same two
/// thresholds as a percentage, applied to the fraction of the cap. A chip
/// that declares no cap says nothing about how hard it is working, so its
/// reading is `normal` rather than guessed against a number from elsewhere.
#[must_use]
pub fn power_load(value: f64, cap: Option<f64>) -> &'static str {
    let Some(cap) = cap.filter(|c| *c > 0.0) else {
        return "normal";
    };
    let percent = (value / cap * 100.0).clamp(0.0, 255.0) as u8;
    load_name(percent)
}

/// A snapshot is applied only if it is newer than the last applied one: the
/// thread publishes in order, but the queue does not promise to.
#[must_use]
pub fn accepts(generation: u64, last: u64) -> bool {
    generation > last
}

#[must_use]
pub fn fraction(percent: u8) -> f32 {
    f32::from(percent.min(100)) / 100.0
}

/// The change ticket QML rebuilds on, folded into the positive half of an
/// `i32` because QML has no 64-bit integer.
#[must_use]
pub fn ticket(generation: u64) -> i32 {
    i32::try_from(generation % u64::from(u32::MAX / 2)).unwrap_or(0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Cpu,
    Memory,
    Gpu,
    Disk,
    Network,
}

impl Kind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Gpu => "gpu",
            Self::Disk => "disk",
            Self::Network => "network",
        }
    }
}

// The kind contract. The page reads these by index, so the order here is the
// order documented in the plan and nothing may be inserted in the middle.

#[must_use]
pub fn cpu_numbers(reading: &CpuReading, cores: usize) -> Vec<f64> {
    vec![
        f64::from(reading.aggregate_percent),
        reading.frequency_mhz.map_or(0.0, f64::from),
        cores as f64,
    ]
}

#[must_use]
pub fn memory_numbers(memory: &Memory) -> Vec<f64> {
    // Kibibytes never reach 2^53, so the doubles are exact.
    vec![
        memory.used_kib as f64,
        memory.total_kib as f64,
        memory.swap_used_kib as f64,
        memory.swap_total_kib as f64,
    ]
}

#[must_use]
pub fn gpu_numbers(reading: &GpuReading) -> Vec<f64> {
    vec![
        f64::from(reading.busy_percent),
        f64::from(reading.memory_busy_percent),
        reading.vram_used as f64,
        reading.vram_total as f64,
        reading.gtt_used as f64,
        reading.gtt_total as f64,
        reading.core_mhz.map_or(0.0, f64::from),
        reading.memory_mhz.map_or(0.0, f64::from),
    ]
}

#[must_use]
pub fn disk_numbers(info: &DiskInfo, rate: Option<[f64; 2]>) -> Vec<f64> {
    let [read, write] = rate.unwrap_or([0.0, 0.0]);
    vec![
        read,
        write,
        info.size_bytes as f64,
        if info.rotational { 1.0 } else { 0.0 },
    ]
}

#[must_use]
pub fn network_numbers(info: &InterfaceInfo, rate: Option<[f64; 2]>) -> Vec<f64> {
    let [rx, tx] = rate.unwrap_or([0.0, 0.0]);
    vec![
        rx,
        tx,
        info.speed_mbit.map_or(0.0, f64::from),
        if info.wireless { 1.0 } else { 0.0 },
        if info.up { 1.0 } else { 0.0 },
    ]
}

/// What a throughput row's history records: both directions together.
#[must_use]
pub fn throughput(rate: Option<[f64; 2]>) -> f32 {
    rate.map_or(0.0, |[a, b]| (a + b) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_names_the_state_the_page_paints() {
        assert_eq!(load_name(0), "normal");
        assert_eq!(load_name(79), "normal");
        assert_eq!(load_name(ELEVATED_PERCENT), "elevated");
        assert_eq!(load_name(89), "elevated");
        assert_eq!(load_name(CRITICAL_PERCENT), "critical");
        assert_eq!(load_name(100), "critical");
    }

    #[test]
    fn a_temperature_loads_against_its_own_critical_limit() {
        assert_eq!(thermal_load(50.0, Some(100.0)), "normal");
        assert_eq!(thermal_load(85.0, Some(100.0)), "elevated");
        assert_eq!(thermal_load(100.0, Some(100.0)), "critical");
        assert_eq!(thermal_load(200.0, None), "normal");
        assert_eq!(thermal_load(1.0, Some(0.0)), "normal");
    }

    #[test]
    fn a_power_reading_loads_against_its_own_cap() {
        assert_eq!(power_load(50.0, Some(100.0)), "normal");
        assert_eq!(power_load(80.0, Some(100.0)), "elevated");
        assert_eq!(power_load(95.0, Some(100.0)), "critical");
        assert_eq!(power_load(120.0, Some(100.0)), "critical");
        // No cap and a nonsense cap are both "nothing is known".
        assert_eq!(power_load(200.0, None), "normal");
        assert_eq!(power_load(1.0, Some(0.0)), "normal");
        assert_eq!(power_load(0.0, Some(100.0)), "normal");
    }

    #[test]
    fn only_a_newer_generation_is_applied() {
        assert!(accepts(2, 1));
        assert!(!accepts(1, 1));
        assert!(!accepts(1, 5));
    }

    #[test]
    fn fractions_and_tickets_stay_in_range() {
        assert_eq!(fraction(0), 0.0);
        assert_eq!(fraction(50), 0.5);
        assert_eq!(fraction(200), 1.0);
        assert_eq!(ticket(0), 0);
        assert_eq!(ticket(7), 7);
        assert!(ticket(u64::MAX) >= 0);
    }

    #[test]
    fn each_kind_publishes_its_numbers_in_contract_order() {
        let cpu = CpuReading {
            aggregate_percent: 23,
            core_percents: vec![1, 2],
            frequency_mhz: Some(4658),
        };
        assert_eq!(cpu_numbers(&cpu, 8), vec![23.0, 4658.0, 8.0]);
        let memory = Memory {
            used_kib: 10,
            total_kib: 20,
            swap_used_kib: 1,
            swap_total_kib: 2,
        };
        assert_eq!(memory_numbers(&memory), vec![10.0, 20.0, 1.0, 2.0]);
        let gpu = GpuReading {
            busy_percent: 61,
            memory_busy_percent: 15,
            vram_used: 5,
            vram_total: 10,
            gtt_used: 1,
            gtt_total: 4,
            core_mhz: Some(1568),
            memory_mhz: None,
        };
        assert_eq!(
            gpu_numbers(&gpu),
            vec![61.0, 15.0, 5.0, 10.0, 1.0, 4.0, 1568.0, 0.0]
        );
        let disk = DiskInfo {
            name: "sda".into(),
            model: "x".into(),
            size_bytes: 512,
            rotational: true,
        };
        assert_eq!(
            disk_numbers(&disk, Some([3.0, 4.0])),
            vec![3.0, 4.0, 512.0, 1.0]
        );
        assert_eq!(disk_numbers(&disk, None), vec![0.0, 0.0, 512.0, 1.0]);
        let net = InterfaceInfo {
            name: "wlan0".into(),
            wireless: true,
            up: false,
            speed_mbit: None,
        };
        assert_eq!(
            network_numbers(&net, Some([7.0, 1.0])),
            vec![7.0, 1.0, 0.0, 1.0, 0.0]
        );
        assert_eq!(throughput(Some([7.0, 1.0])), 8.0);
        assert_eq!(throughput(None), 0.0);
    }
}
