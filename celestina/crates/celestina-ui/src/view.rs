//! Views: what a product hands the tree, rebuilt from its state on every
//! change and matched against the live nodes by key.
#![forbid(unsafe_code)]

use celestina_paint::{IconKey, PixmapId, Role, TextStyle};
use taffy::{
    prelude::length, Dimension, Display, FlexDirection, LengthPercentage, LengthPercentageAuto,
    Position,
};

use crate::key::Key;

/// What a node draws.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewKind {
    /// A rounded box: the Haze material when `haze`, then `fill` (an
    /// interactive box lifts it on hover and press).
    Box {
        fill: Option<Role>,
        haze: bool,
        radius: f32,
    },
    /// A run of text, wrapped at `max_width` logical px when given.
    Text {
        text: String,
        style: TextStyle,
        max_width: Option<f32>,
    },
    /// An icon fitted in a `size` square; semantic icons take `tint`.
    Icon {
        key: IconKey,
        tint: Option<Role>,
        size: f32,
    },
    /// An uploaded pixmap stretched over the node at `alpha`.
    Image { id: PixmapId, alpha: f32 },
}

/// One keyed node of a view and its children, in paint order (the last
/// child on top).
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub key: Key,
    pub kind: ViewKind,
    pub style: taffy::Style,
    /// Hit-tested: hover, press, click and scroll.
    pub interactive: bool,
    /// In the Tab order; Enter and Space activate it.
    pub focusable: bool,
    /// For a `Box`: its subtree is clipped to its rounded rect.
    pub clip: bool,
    /// 0..=1, multiplied into the subtree; changes fade linearly.
    pub opacity: f32,
    pub children: Vec<View>,
}

impl View {
    /// A node of `kind` with the default (flex row) style, opaque, neither
    /// interactive nor focusable, with no children.
    pub fn new(key: Key, kind: ViewKind) -> View {
        View {
            key,
            kind,
            style: taffy::Style::default(),
            interactive: false,
            focusable: false,
            clip: false,
            opacity: 1.0,
            children: Vec::new(),
        }
    }

    /// A `Box`'s fill; other kinds are left alone.
    pub fn fill(mut self, role: Role) -> View {
        if let ViewKind::Box { fill, .. } = &mut self.kind {
            *fill = Some(role);
        }
        self
    }

    /// A `Box` drawn in the Haze material with corner `radius`.
    pub fn haze(mut self, radius: f32) -> View {
        if let ViewKind::Box {
            haze, radius: r, ..
        } = &mut self.kind
        {
            *haze = true;
            *r = radius;
        }
        self
    }

    /// A `Box`'s corner radius (fill, clip and focus ring).
    pub fn radius(mut self, radius: f32) -> View {
        if let ViewKind::Box { radius: r, .. } = &mut self.kind {
            *r = radius;
        }
        self
    }

    /// A semantic icon's tint.
    pub fn tint(mut self, role: Role) -> View {
        if let ViewKind::Icon { tint, .. } = &mut self.kind {
            *tint = Some(role);
        }
        self
    }

    /// The wrap width of a `Text`.
    pub fn max_width(mut self, w: f32) -> View {
        if let ViewKind::Text { max_width, .. } = &mut self.kind {
            *max_width = Some(w);
        }
        self
    }

    pub fn size(self, w: f32, h: f32) -> View {
        self.width(w).height(h)
    }

    pub fn width(mut self, w: f32) -> View {
        self.style.size.width = Dimension::length(w);
        self
    }

    pub fn height(mut self, h: f32) -> View {
        self.style.size.height = Dimension::length(h);
        self
    }

    /// The same padding on every side.
    pub fn padding(self, p: f32) -> View {
        self.padding_xy(p, p)
    }

    /// `x` left and right, `y` top and bottom.
    pub fn padding_xy(mut self, x: f32, y: f32) -> View {
        let (x, y) = (LengthPercentage::length(x), LengthPercentage::length(y));
        self.style.padding = taffy::Rect {
            left: x,
            right: x,
            top: y,
            bottom: y,
        };
        self
    }

    /// The space between children, both ways.
    pub fn gap(mut self, g: f32) -> View {
        self.style.gap = length(g);
        self
    }

    /// Takes the free space along the parent's main axis.
    pub fn grow(mut self) -> View {
        self.style.flex_grow = 1.0;
        self
    }

    pub fn interactive(mut self) -> View {
        self.interactive = true;
        self
    }

    pub fn focusable(mut self) -> View {
        self.focusable = true;
        self
    }

    pub fn clip(mut self) -> View {
        self.clip = true;
        self
    }

    /// Clamped to 0..=1; NaN is fully transparent.
    pub fn opacity(mut self, o: f32) -> View {
        self.opacity = if o.is_nan() { 0.0 } else { o.clamp(0.0, 1.0) };
        self
    }

    pub fn child(mut self, child: View) -> View {
        self.children.push(child);
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = View>) -> View {
        self.children.extend(children);
        self
    }

    /// Out of the flow, at (`x`, `y`) from the parent's padding box (the
    /// parent is the containing block when it is positioned, or the root).
    pub fn absolute(mut self, x: f32, y: f32) -> View {
        self.style.position = Position::Absolute;
        self.style.inset.left = LengthPercentageAuto::length(x);
        self.style.inset.top = LengthPercentageAuto::length(y);
        self
    }

    /// Left out of layout, paint, hit-test and the Tab order while
    /// `hidden`; the node keeps its state.
    pub fn hidden(mut self, hidden: bool) -> View {
        if hidden {
            self.style.display = Display::None;
        }
        self
    }
}

fn flex_box(key: Key, direction: FlexDirection) -> View {
    let mut v = View::new(
        key,
        ViewKind::Box {
            fill: None,
            haze: false,
            radius: 0.0,
        },
    );
    v.style.display = Display::Flex;
    v.style.flex_direction = direction;
    v
}

/// A box laying its children out top to bottom.
pub fn col(key: Key) -> View {
    flex_box(key, FlexDirection::Column)
}

/// A box laying its children out left to right.
pub fn row(key: Key) -> View {
    flex_box(key, FlexDirection::Row)
}

pub fn text(key: Key, s: &str, style: TextStyle) -> View {
    View::new(
        key,
        ViewKind::Text {
            text: s.to_owned(),
            style,
            max_width: None,
        },
    )
}

/// A semantic (Lucide) icon from `celestina-style/icons`, `size` square.
pub fn icon(key: Key, name: &'static str, size: f32) -> View {
    View::new(
        key,
        ViewKind::Icon {
            key: IconKey::Semantic(name),
            tint: None,
            size,
        },
    )
    .size(size, size)
}

/// An uploaded pixmap at `alpha`; size it with `.size(w, h)`.
pub fn image(key: Key, id: PixmapId, alpha: f32) -> View {
    View::new(key, ViewKind::Image { id, alpha })
}

#[cfg(test)]
mod tests {
    use celestina_paint::{Role, TextStyle, Weight};
    use taffy::{
        Dimension, Display, FlexDirection, LengthPercentage, LengthPercentageAuto, Position,
    };

    use super::*;

    fn body() -> TextStyle {
        TextStyle {
            size: 13.0,
            weight: Weight::Regular,
            tabular: false,
            role: Role::TextHi,
        }
    }

    #[test]
    fn col_and_row_are_plain_flex_boxes() {
        let c = col(Key::named("c"));
        assert_eq!(c.style.display, Display::Flex);
        assert_eq!(c.style.flex_direction, FlexDirection::Column);
        assert_eq!(
            c.kind,
            ViewKind::Box {
                fill: None,
                haze: false,
                radius: 0.0
            }
        );
        assert!(!c.interactive && !c.focusable && !c.clip);
        assert_eq!(c.opacity, 1.0);
        let r = row(Key::named("r"));
        assert_eq!(r.style.flex_direction, FlexDirection::Row);
        assert_eq!(r.key, Key::named("r"));
    }

    #[test]
    fn builders_set_box_paint_and_flags() {
        let v = col(Key::named("c"))
            .fill(Role::Card)
            .haze(20.0)
            .interactive()
            .focusable()
            .clip()
            .opacity(0.5);
        assert_eq!(
            v.kind,
            ViewKind::Box {
                fill: Some(Role::Card),
                haze: true,
                radius: 20.0
            }
        );
        assert!(v.interactive && v.focusable && v.clip);
        assert_eq!(v.opacity, 0.5);
        let v = col(Key::named("c")).radius(8.0);
        assert!(matches!(v.kind, ViewKind::Box { radius, haze: false, .. } if radius == 8.0));
        // Opacity is clamped to 0..=1; NaN is fully transparent.
        assert_eq!(col(Key::named("c")).opacity(3.0).opacity, 1.0);
        assert_eq!(col(Key::named("c")).opacity(f32::NAN).opacity, 0.0);
    }

    #[test]
    fn builders_set_the_layout_style() {
        let v = row(Key::named("r"))
            .size(10.0, 20.0)
            .padding(4.0)
            .gap(8.0)
            .grow();
        assert_eq!(v.style.size.width, Dimension::length(10.0));
        assert_eq!(v.style.size.height, Dimension::length(20.0));
        assert_eq!(v.style.padding.left, LengthPercentage::length(4.0));
        assert_eq!(v.style.padding.bottom, LengthPercentage::length(4.0));
        assert_eq!(v.style.gap.width, LengthPercentage::length(8.0));
        assert_eq!(v.style.flex_grow, 1.0);
        let v = row(Key::named("r"))
            .padding_xy(12.0, 6.0)
            .width(5.0)
            .height(7.0);
        assert_eq!(v.style.padding.left, LengthPercentage::length(12.0));
        assert_eq!(v.style.padding.top, LengthPercentage::length(6.0));
        assert_eq!(v.style.size.width, Dimension::length(5.0));
        assert_eq!(v.style.size.height, Dimension::length(7.0));
        let v = row(Key::named("r")).absolute(3.0, 4.0);
        assert_eq!(v.style.position, Position::Absolute);
        assert_eq!(v.style.inset.left, LengthPercentageAuto::length(3.0));
        assert_eq!(v.style.inset.top, LengthPercentageAuto::length(4.0));
        assert_eq!(
            row(Key::named("r")).hidden(true).style.display,
            Display::None
        );
        assert_eq!(
            row(Key::named("r")).hidden(false).style.display,
            Display::Flex
        );
    }

    #[test]
    fn children_keep_their_order() {
        let v = col(Key::named("c"))
            .child(row(Key::indexed("r", 0)))
            .children([row(Key::indexed("r", 1)), row(Key::indexed("r", 2))]);
        let keys: Vec<Key> = v.children.iter().map(|c| c.key).collect();
        assert_eq!(
            keys,
            vec![
                Key::indexed("r", 0),
                Key::indexed("r", 1),
                Key::indexed("r", 2)
            ]
        );
    }

    #[test]
    fn text_and_icon_leaves() {
        let t = text(Key::named("t"), "Isla", body()).max_width(100.0);
        assert_eq!(
            t.kind,
            ViewKind::Text {
                text: "Isla".into(),
                style: body(),
                max_width: Some(100.0)
            }
        );
        let i = icon(Key::named("i"), "bell", 16.0).tint(Role::TextLo);
        assert_eq!(
            i.kind,
            ViewKind::Icon {
                key: IconKey::Semantic("bell"),
                tint: Some(Role::TextLo),
                size: 16.0
            }
        );
        assert_eq!(i.style.size.width, Dimension::length(16.0));
        assert_eq!(i.style.size.height, Dimension::length(16.0));
        // Box-only builders leave other kinds alone.
        assert_eq!(t.clone().fill(Role::Card).kind, t.kind);
    }
}
