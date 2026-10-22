//! What the connectivity pieces (network, Bluetooth, battery, power
//! profile) share: a small clickable glyph in the bar, built the way the
//! notification bell is, and the accessible name their states give it.

use celestina_paint::{Role, Theme};
use celestina_ui::{view, Key, View};
use taffy::JustifyContent;

use crate::island::view::centred;

/// A glyph's size, logical px.
pub const ICON: f32 = 18.0;
/// The space around a glyph that still counts as the glyph.
pub const HIT_PAD: f32 = 5.0;

/// The clickable node `key` around `children` (already sized to the glyph
/// box): keyed, interactive and focusable.
#[must_use]
pub fn tap(theme: &Theme, key: Key, children: impl IntoIterator<Item = View>) -> View {
    centred(view::row(key))
        .padding(HIT_PAD)
        .gap(theme.spacing.xs)
        .radius(theme.radius_md)
        .interactive()
        .focusable()
        .children(children)
}

/// An icon `name` in `role`, in a [`ICON`] square.
#[must_use]
pub fn icon(key: Key, name: &'static str, role: Role) -> View {
    view::icon(key, name, ICON).tint(role)
}

/// `inner` centred in a [`ICON`] square, for a glyph drawn from boxes.
#[must_use]
pub fn boxed(key: Key, inner: View) -> View {
    let mut square = centred(view::row(key)).size(ICON, ICON);
    square.style.justify_content = JustifyContent::CENTER;
    square.child(inner)
}
