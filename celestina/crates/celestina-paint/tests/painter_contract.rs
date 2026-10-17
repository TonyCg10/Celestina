//! The painter's public contract, compiled without a GL context: every entry
//! point the UI tree and the product use exists with the brief's signature.
#![forbid(unsafe_code)]

use std::ffi::c_void;
use std::path::PathBuf;

use celestina_paint::{
    silhouette_slices, Color, ColorError, IconKey, PaintError, Painter, PixmapId, Rect, Role,
    TextError, TextRun, TextStyle, TextSystem, Theme, Weight,
};

/// What `FrameCx::proc_address` is.
type ProcAddress<'a> = &'a dyn Fn(&str) -> *const c_void;

/// `Painter::new`'s signature.
type NewPainter = fn(ProcAddress<'_>, Theme, TextSystem) -> Result<Painter, PaintError>;

/// Calls every drawing entry point; type-checked, never run (no GL here).
fn frame(p: &mut Painter, run: &TextRun, pixmap: PixmapId) -> Result<(), PaintError> {
    let r = Rect {
        x: 0.0,
        y: 0.0,
        w: 280.0,
        h: 40.0,
    };
    p.begin((760.0, 560.0), 1.5, true);
    let accent = p.theme().color(Role::Accent);
    p.set_theme(Theme::dark().scaled(1.15));
    let _: &mut TextSystem = p.text();
    p.push_state();
    p.opacity(0.5);
    p.scale_about(140.0, 20.0, 0.96);
    p.haze(r, 20.0);
    p.fill_rounded(r, 12.0, accent);
    p.stroke_rounded(r, 12.0, 2.0, accent);
    p.push_clip(r, 20.0);
    p.glyphs(run, 12.0, 10.0, accent);
    p.icon(&IconKey::Semantic("bell"), r, Some(accent));
    p.icon(&IconKey::Theme("firefox".to_owned()), r, None);
    p.icon(&IconKey::Pixmap(pixmap), r, None);
    p.image(pixmap, r, 0.8);
    p.pop_clip();
    p.pop_state();
    let id = p.upload_pixmap(&[0; 16], 2, 2)?;
    p.release_pixmap(id);
    let wallpaper = p.upload_wallpaper(&[0xff; 16], 2, 2)?;
    p.draw_wallpaper(wallpaper, r);
    p.release_pixmap(wallpaper);
    p.end();
    Ok(())
}

#[test]
fn the_contract_types_line_up() {
    let _new: NewPainter = Painter::new;
    let _frame: fn(&mut Painter, &TextRun, PixmapId) -> Result<(), PaintError> = frame;
    let _text: fn(&[PathBuf]) -> Result<TextSystem, TextError> = TextSystem::new;
    let _hex: fn(&str) -> Result<Color, ColorError> = Color::hex;
    let style = TextStyle {
        size: 14.0,
        weight: Weight::Semibold,
        tabular: true,
        role: Role::TextHi,
    };
    assert_eq!(style.weight, Weight::Semibold);
    let oversize = PaintError::WallpaperTooLarge {
        width: 16385,
        height: 1,
        max_side: 16384,
    };
    assert!(oversize.to_string().contains("16384"));
    let texels = PaintError::WallpaperTexels {
        width: 2,
        height: 2,
        len: 15,
    };
    assert!(texels.to_string().contains("15 bytes"));
    let theme = Theme::dark();
    assert_eq!(theme.color(Role::GlassFallback).3, 1.0);
    assert_eq!(Color(1, 2, 3, 1.0).with_alpha(0.5), Color(1, 2, 3, 0.5));
}

#[test]
fn silhouette_slices_are_exported() {
    let slices = silhouette_slices(
        Rect {
            x: 0.0,
            y: 0.0,
            w: 280.0,
            h: 40.0,
        },
        20.0,
        2.0,
    );
    assert_eq!(slices.len(), 20);
}
