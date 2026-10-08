//! The window-wide state: the motion preference. Each section has its own
//! controller beside this one.

#[cxx_qt::bridge]
pub mod qobject {
    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, reduced_motion, cxx_name = "reducedMotion")]
        #[qproperty(bool, smoke_report, cxx_name = "smokeReport")]
        type CupritaController = super::CupritaControllerRust;
    }
}

/// Whether motion should be skipped: `CELESTINA_REDUCED_MOTION` set in the
/// environment, as in the rest of the suite.
///
/// `smokeReport` is `CUPRITA_SMOKE_REPORT` set: `scripts/smoke.sh` asks the
/// window to print its list models' row counts once the pages are up, so the
/// smoke checks the controller-to-model wiring over the fakes.
pub struct CupritaControllerRust {
    reduced_motion: bool,
    smoke_report: bool,
}

impl Default for CupritaControllerRust {
    fn default() -> Self {
        Self {
            reduced_motion: std::env::var_os("CELESTINA_REDUCED_MOTION").is_some(),
            smoke_report: std::env::var_os("CUPRITA_SMOKE_REPORT").is_some(),
        }
    }
}
