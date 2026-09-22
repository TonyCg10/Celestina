//! The parsers against text captured from the author's machine (an AMD Ryzen
//! 7 9800X3D, 2026-09-21). Inline fixtures prove arithmetic; these prove the
//! real files have the shape the parsers expect.

use hematita_core::cpu::{parse_model, parse_stat};
use hematita_core::disk::parse_diskstats;
use hematita_core::memory::parse_meminfo;
use hematita_core::network::parse_net_dev;
use hematita_core::passwd;
use hematita_core::process::{
    parse_cgroup, parse_stat as parse_process_stat, parse_status as parse_process_status,
};
use hematita_core::sensors::{discover, ChannelKind, ChipListing};

const STAT: &str = include_str!("fixtures/proc-stat.txt");
const MEMINFO: &str = include_str!("fixtures/proc-meminfo.txt");
const CPUINFO: &str = include_str!("fixtures/proc-cpuinfo.txt");
const DISKSTATS: &str = include_str!("fixtures/proc-diskstats.txt");
const NET_DEV: &str = include_str!("fixtures/proc-net-dev.txt");
const CORES: usize = 8;
const TOTAL_KIB: u64 = 65_403_392;
const INTERFACES: usize = 2;
const PID_STAT: &str = include_str!("fixtures/proc-pid-stat.txt");
const PID_STATUS: &str = include_str!("fixtures/proc-pid-status.txt");
const PID_CGROUP: &str = include_str!("fixtures/proc-pid-cgroup.txt");
const PASSWD: &str = include_str!("fixtures/etc-passwd.txt");

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
    // Sorted, as the sampler sorts them: the assertion is the exact set the
    // capture holds, not the order `/proc/diskstats` happened to list.
    let mut names: Vec<&str> = disks.iter().map(|disk| disk.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["nvme0n1", "nvme1n1", "sda", "sdb"]);
}

#[test]
fn the_captured_net_dev_lists_the_real_interfaces_without_loopback() {
    let interfaces = parse_net_dev(NET_DEV).expect("the captured /proc/net/dev parses");
    assert_eq!(interfaces.len(), INTERFACES);
    assert!(interfaces.iter().all(|interface| interface.name != "lo"));
}

#[test]
fn the_captured_process_files_parse_and_agree_on_the_pid() {
    let stat = parse_process_stat(PID_STAT).expect("the captured stat parses");
    let status = parse_process_status(PID_STATUS).expect("the captured status parses");
    assert!(stat.pid > 1);
    assert_eq!(status.uid, 1000);
    assert!(status.threads >= 1);
    assert!(status.rss_kib.is_some());
}

#[test]
fn the_captured_cgroup_names_a_desktop_application() {
    let scope = parse_cgroup(PID_CGROUP).expect("the shell ran under an app scope");
    assert!(!scope.desktop_id.is_empty());
    assert!(scope.unit.starts_with("app-"));
}

#[test]
fn the_captured_passwd_names_root_and_the_author() {
    let users = passwd::parse(PASSWD);
    assert_eq!(users.get(&0).map(String::as_str), Some("root"));
    assert_eq!(users.get(&1000).map(String::as_str), Some("toni"));
}

fn chip_listing(key: &str, text: &str) -> ChipListing {
    let mut name = String::new();
    let mut files = Vec::new();
    for line in text.lines() {
        let Some((file, contents)) = line.split_once('\t') else {
            continue;
        };
        if file == "name" {
            name = contents.to_owned();
        }
        files.push((file.to_owned(), contents.to_owned()));
    }
    ChipListing {
        key: key.to_owned(),
        name,
        files,
    }
}

#[test]
fn the_captured_processor_chip_has_tctl_and_tccd() {
    let chip = discover(&chip_listing(
        "hwmon6",
        include_str!("fixtures/hwmon-k10temp.txt"),
    ));
    assert_eq!(chip.name, "k10temp");
    let labels: Vec<&str> = chip.channels.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, vec!["Tctl", "Tccd1"]);
    assert!(chip
        .channels
        .iter()
        .all(|c| c.kind == ChannelKind::Temperature && c.value > 0.0 && c.value < 120.0));
}

#[test]
fn the_captured_gpu_chip_reads_temperatures_fan_voltage_and_power() {
    let chip = discover(&chip_listing(
        "hwmon3",
        include_str!("fixtures/hwmon-amdgpu.txt"),
    ));
    assert_eq!(chip.channels.len(), 6);
    let power = chip
        .channels
        .iter()
        .find(|c| c.kind == ChannelKind::Power)
        .expect("a power channel");
    assert_eq!(power.label, "PPT");
    assert!(power.limit_max.is_some(), "the power cap is the max");
    let junction = chip
        .channels
        .iter()
        .find(|c| c.label == "junction")
        .expect("junction");
    assert!(junction.limit_crit.is_some());
}

#[test]
fn the_captured_board_chip_has_six_fans_and_ten_voltages() {
    let chip = discover(&chip_listing(
        "hwmon4",
        include_str!("fixtures/hwmon-it8696.txt"),
    ));
    assert_eq!(
        chip.channels
            .iter()
            .filter(|c| c.kind == ChannelKind::Fan)
            .count(),
        6
    );
    assert_eq!(
        chip.channels
            .iter()
            .filter(|c| c.kind == ChannelKind::Voltage)
            .count(),
        10
    );
    assert_eq!(
        chip.channels
            .iter()
            .filter(|c| c.kind == ChannelKind::Temperature)
            .count(),
        6
    );
    let labelled: Vec<&str> = chip
        .channels
        .iter()
        .filter(|c| !c.label.is_empty())
        .map(|c| c.label.as_str())
        .collect();
    assert_eq!(labelled, vec!["3VSB", "Vbat", "+3.3V"]);
}
