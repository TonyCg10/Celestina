//! What a product needs to know about an output, from sctk's state. Shared by
//! the shell runtime and the session-lock profile so there is one reading of
//! `wl_output` (name, logical size, scale, refresh).
#![forbid(unsafe_code)]

use smithay_client_toolkit::output::OutputState;
use wayland_client::protocol::wl_output;

/// An output, by the global name of its `wl_output`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OutputId(pub u32);

/// What the product needs to know about an output.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputInfo {
    pub id: OutputId,
    /// The connector name (`DP-1`), or `wl_output-<id>` when the compositor
    /// gives none.
    pub name: String,
    /// Logical size: `xdg_output`'s when offered, else the current mode
    /// divided by the integer scale.
    pub logical_size: (u32, u32),
    /// The output's integer scale; the real (fractional) scale of a surface
    /// arrives per surface through `wp_fractional_scale_v1`.
    pub scale: f64,
    /// Refresh rate of the current mode in mHz, 0 when unknown.
    pub refresh_mhz: i32,
}

impl OutputInfo {
    /// The product's view of `output`, from sctk's info; `None` while the
    /// compositor has not described it yet.
    pub(crate) fn read(state: &OutputState, output: &wl_output::WlOutput) -> Option<OutputInfo> {
        let info = state.info(output)?;
        let id = OutputId(info.id);
        let current = info.modes.iter().find(|m| m.current);
        let scale = f64::from(info.scale_factor.max(1));
        let logical_size = match info.logical_size {
            Some((w, h)) => (w.max(0) as u32, h.max(0) as u32),
            None => current.map_or((0, 0), |m| {
                let side = |px: i32| (f64::from(px.max(0)) / scale).round() as u32;
                (side(m.dimensions.0), side(m.dimensions.1))
            }),
        };
        Some(OutputInfo {
            id,
            name: info.name.unwrap_or_else(|| format!("wl_output-{}", id.0)),
            logical_size,
            scale,
            refresh_mhz: current.map_or(0, |m| m.refresh_rate),
        })
    }
}
