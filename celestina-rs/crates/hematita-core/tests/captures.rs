//! The parsers against text captured from the author's machine (an AMD Ryzen
//! 7 9800X3D, 2026-09-21). Inline fixtures prove arithmetic; these prove the
//! real files have the shape the parsers expect.

use hematita_core::cpu::{parse_model, parse_stat};
use hematita_core::disk::parse_diskstats;
use hematita_core::memory::parse_meminfo;
use hematita_core::network::parse_net_dev;

const STAT: &str = include_str!("fixtures/proc-stat.txt");
const MEMINFO: &str = include_str!("fixtures/proc-meminfo.txt");
const CPUINFO: &str = include_str!("fixtures/proc-cpuinfo.txt");
const DISKSTATS: &str = include_str!("fixtures/proc-diskstats.txt");
const NET_DEV: &str = include_str!("fixtures/proc-net-dev.txt");
const CORES: usize = 8;
const TOTAL_KIB: u64 = 65_403_392;
const DISKS: usize = 4;
const INTERFACES: usize = 2;

#[test]
fn the_captured_stat_has_the_aggregate_and_every_core() {
    let stat = parse_stat(STAT).expect("the captured /proc/stat parses");
    assert_eq!(stat.cores.len(), CORES);
    assert!(stat.aggregate.total >= stat.aggregate.idle);
    for core in &stat.cores {
        assert!(core.total >= core.idle);
        assert!(core.total <= stat.aggregate.total);
    }
}

#[test]
fn the_captured_meminfo_has_memory_and_swap() {
    let memory = parse_meminfo(MEMINFO).expect("the captured /proc/meminfo parses");
    assert_eq!(memory.total_kib, TOTAL_KIB);
    assert!(memory.used_kib <= memory.total_kib);
    assert!(memory.swap_used_kib <= memory.swap_total_kib);
}

#[test]
fn the_captured_cpuinfo_names_the_processor() {
    assert_eq!(
        parse_model(CPUINFO).as_deref(),
        Some("AMD Ryzen 7 9800X3D 8-Core Processor")
    );
}

#[test]
fn the_captured_diskstats_lists_the_whole_disks_only() {
    let disks = parse_diskstats(DISKSTATS).expect("the captured /proc/diskstats parses");
    assert_eq!(disks.len(), DISKS);
    assert!(disks
        .iter()
        .all(|disk| !disk.name.contains('p') || disk.name.starts_with("sd")));
    assert!(disks.iter().any(|disk| disk.name == "nvme0n1"));
}

#[test]
fn the_captured_net_dev_lists_the_real_interfaces_without_loopback() {
    let interfaces = parse_net_dev(NET_DEV).expect("the captured /proc/net/dev parses");
    assert_eq!(interfaces.len(), INTERFACES);
    assert!(interfaces.iter().all(|interface| interface.name != "lo"));
}
