//! The state every page reads. For now only the motion preference; the
//! section controllers arrive with CUP-1-B.

#[cxx_qt::bridge]
pub mod qobject {
    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, reduced_motion, cxx_name = "reducedMotion")]
        type CupritaController = super::CupritaControllerRust;
    }
}

/// Whether motion should be skipped: `CELESTINA_REDUCED_MOTION` set in the
/// environment, as in the rest of the suite.
pub struct CupritaControllerRust {
    reduced_motion: bool,
}

impl Default for CupritaControllerRust {
    fn default() -> Self {
        Self {
            reduced_motion: std::env::var_os("CELESTINA_REDUCED_MOTION").is_some(),
        }
    }
}
