//! The `--metrics` report: frame counts, rest frames, frame timing, late
//! frames and RSS, kept by the runtime (`celestina_wayland::metrics`, ported
//! from the spike's final wave) and printed by the binary at exit. The
//! glyph atlas's occupancy is logged by the app when it is dropped.

pub use celestina_wayland::metrics::{period_from_mhz, Metrics, LATE_FACTOR, REST_AFTER};
