//! The bar (parity spec §6): the pure half of the Island's wide surface.
//! [`layout`] places the zone capsules and the Island on the output and is
//! the one place that knows how the surface is anchored, [`view`] builds one
//! capsule [`View`](celestina_ui::View) per non-empty zone from the pieces
//! table, and [`silhouette`] turns the painted capsules into the blur and
//! input regions.
//!
//! Nothing here touches a surface, a painter or the event loop: every
//! function takes measured widths, a theme or a context and answers with
//! rects or views, so the Island (Task 6) only measures, places and paints.

pub mod layout;
pub mod silhouette;
pub mod view;
