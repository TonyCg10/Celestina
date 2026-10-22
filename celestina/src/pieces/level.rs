//! The widget the Volume and Brightness pieces share: an icon and the level
//! as `NN %`, one interactive node. Not a piece itself; the two pieces differ
//! in what they read and what their gestures ask for, not in how they look.

use celestina_paint::{Role, Weight};
use celestina_ui::{view, Key, View};

use super::BarCx;
use crate::{
    copy,
    island::view::{centred, text_style},
};

/// The icon's size, logical px.
const ICON: f32 = 18.0;
/// The space around the widget that still counts as it.
const HIT_PAD: f32 = 5.0;
/// The space between the icon and the percentage.
const GAP: f32 = 4.0;

/// The widget keyed `key`: `icon` and `percent`, the text faint while
/// `muted` (the level is the one the device remembers).
pub(super) fn widget(
    cx: &BarCx<'_>,
    key: Key,
    icon: &'static str,
    percent: u8,
    muted: bool,
) -> View {
    let role = if muted { Role::TextFaint } else { Role::TextHi };
    centred(view::row(key))
        .padding(HIT_PAD)
        .gap(GAP)
        .radius(cx.theme.radius_md)
        .interactive()
        .focusable()
        .child(view::icon(Key::named("level-icon"), icon, ICON).tint(role))
        .child(view::text(
            Key::named("level-percent"),
            &copy::osd_percent(percent.min(100)),
            text_style(cx.theme.type_scale.body, Weight::Regular, true, role),
        ))
}
