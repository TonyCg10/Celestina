//! The AMD GPU, as `amdgpu`'s sysfs files report it.
//!
//! Only AMD in this phase: it is the card the author has, and its driver
//! exposes busy percentages, memory and clock levels as plain files. The
//! caller reads the files; this module turns their text into one reading.
//! Clock levels are optional: a driver in a power-saving mode marks no level
//! active, and that is not an error.

use std::fmt;

pub struct AmdgpuFiles<'a> {
    pub busy_percent: &'a str,
    pub memory_busy_percent: &'a str,
    pub vram_used: &'a str,
    pub vram_total: &'a str,
    pub gtt_used: &'a str,
    pub gtt_total: &'a str,
    pub sclk: &'a str,
    pub mclk: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuReading {
    pub busy_percent: u8,
    pub memory_busy_percent: u8,
    pub vram_used: u64,
    pub vram_total: u64,
    pub gtt_used: u64,
    pub gtt_total: u64,
    pub core_mhz: Option<u32>,
    pub memory_mhz: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GpuError {
    UnreadableNumber { file: &'static str, text: String },
}

impl fmt::Display for GpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnreadableNumber { file, text } => {
                write!(formatter, "amdgpu {file} is not a number: {text}")
            }
        }
    }
}

impl std::error::Error for GpuError {}

/// Whether a `/sys/class/drm/cardN/device/driver` link points at `amdgpu`.
#[must_use]
pub fn is_amdgpu_driver_link(target: &str) -> bool {
    target.rsplit('/').next() == Some("amdgpu")
}

/// Parses the eight files into one reading. Percentages saturate at 100.
///
/// # Errors
///
/// A busy or memory file that is not an integer.
pub fn parse_amdgpu(files: &AmdgpuFiles<'_>) -> Result<GpuReading, GpuError> {
    fn number(file: &'static str, text: &str) -> Result<u64, GpuError> {
        text.trim()
            .parse::<u64>()
            .map_err(|_| GpuError::UnreadableNumber {
                file,
                text: text.trim().to_owned(),
            })
    }
    fn percent(file: &'static str, text: &str) -> Result<u8, GpuError> {
        Ok(u8::try_from(number(file, text)?.min(100)).unwrap_or(100))
    }
    Ok(GpuReading {
        busy_percent: percent("gpu_busy_percent", files.busy_percent)?,
        memory_busy_percent: percent("mem_busy_percent", files.memory_busy_percent)?,
        vram_used: number("mem_info_vram_used", files.vram_used)?,
        vram_total: number("mem_info_vram_total", files.vram_total)?,
        gtt_used: number("mem_info_gtt_used", files.gtt_used)?,
        gtt_total: number("mem_info_gtt_total", files.gtt_total)?,
        core_mhz: parse_dpm_active_mhz(files.sclk),
        memory_mhz: parse_dpm_active_mhz(files.mclk),
    })
}

/// The active level of a `pp_dpm_sclk`/`pp_dpm_mclk` file: the line marked
/// with `*`, as MHz.
#[must_use]
pub fn parse_dpm_active_mhz(text: &str) -> Option<u32> {
    text.lines()
        .find(|line| line.trim_end().ends_with('*'))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|token| {
            token
                .strip_suffix("Mhz")
                .or_else(|| token.strip_suffix("MHz"))
        })
        .and_then(|digits| digits.parse::<u32>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCLK: &str = "0: 500Mhz \n1: 1568Mhz *\n2: 2400Mhz \n";
    const MCLK: &str = "0: 96Mhz \n1: 456Mhz \n5: 1258Mhz *\n";

    fn files<'a>(busy: &'a str, vram_used: &'a str) -> AmdgpuFiles<'a> {
        AmdgpuFiles {
            busy_percent: busy,
            memory_busy_percent: "15\n",
            vram_used,
            vram_total: "17095983104\n",
            gtt_used: "1299664896\n",
            gtt_total: "33486536704\n",
            sclk: SCLK,
            mclk: MCLK,
        }
    }

    #[test]
    fn the_eight_files_become_one_reading() {
        let reading = parse_amdgpu(&files("100\n", "11859542016\n")).expect("readable");
        assert_eq!(
            reading,
            GpuReading {
                busy_percent: 100,
                memory_busy_percent: 15,
                vram_used: 11_859_542_016,
                vram_total: 17_095_983_104,
                gtt_used: 1_299_664_896,
                gtt_total: 33_486_536_704,
                core_mhz: Some(1568),
                memory_mhz: Some(1258),
            }
        );
    }

    #[test]
    fn percentages_saturate_and_bad_numbers_are_refused() {
        assert_eq!(
            parse_amdgpu(&files("250\n", "0\n"))
                .expect("readable")
                .busy_percent,
            100
        );
        assert_eq!(
            parse_amdgpu(&files("busy\n", "0\n")),
            Err(GpuError::UnreadableNumber {
                file: "gpu_busy_percent",
                text: "busy".to_owned()
            })
        );
        assert!(matches!(
            parse_amdgpu(&files("1\n", "lots\n")),
            Err(GpuError::UnreadableNumber {
                file: "mem_info_vram_used",
                ..
            })
        ));
    }

    #[test]
    fn the_active_clock_level_is_the_starred_line() {
        assert_eq!(parse_dpm_active_mhz(SCLK), Some(1568));
        assert_eq!(parse_dpm_active_mhz("0: 500Mhz \n1: 1568Mhz \n"), None);
        assert_eq!(parse_dpm_active_mhz(""), None);
        assert_eq!(parse_dpm_active_mhz("1: fastMhz *\n"), None);
    }

    #[test]
    fn the_driver_link_names_amdgpu_at_its_end() {
        assert!(is_amdgpu_driver_link(
            "../../../../../../bus/pci/drivers/amdgpu"
        ));
        assert!(!is_amdgpu_driver_link("../../bus/pci/drivers/nvidia"));
        assert!(!is_amdgpu_driver_link(""));
    }
}
