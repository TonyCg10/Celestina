//! The argv of the capture tools: `grim` takes the picture, `slurp` lets the
//! person draw a region, `wl-copy` puts the PNG on the clipboard, and niri's
//! own `screenshot-window` action takes a tiled window (whose position niri's
//! IPC does not give). Each builder returns the whole argv, the program
//! first, so [`resolve`] can point it at a stub folder and the runner can
//! start it as is.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use crate::target::Target;

/// The `grim` argv that writes `target` as a PNG to `out`.
///
/// A screen is `-o <output>`: `output_name` when given, else the target's
/// own output; with neither, every output. A window or a region is
/// `-g "x,y wxh"` in absolute logical coordinates.
#[must_use]
pub fn grim_argv(target: &Target, out: &Path, output_name: Option<&str>) -> Vec<OsString> {
    let mut argv: Vec<OsString> = vec!["grim".into(), "-t".into(), "png".into()];
    match target {
        Target::Screen { output } => {
            let output = output_name.unwrap_or(output);
            if !output.is_empty() {
                argv.push("-o".into());
                argv.push(output.into());
            }
        }
        Target::Window { geometry, .. } | Target::Region { geometry } => {
            argv.push("-g".into());
            argv.push(geometry.to_string().into());
        }
    }
    argv.push(out.as_os_str().to_owned());
    argv
}

/// The `slurp` argv that draws its selection in the suite's colours: the
/// border in `accent`, the dimmed outside in `background`, both `#RRGGBBAA`
/// (see [`slurp_colour`]). It prints `x,y wxh` on success.
#[must_use]
pub fn slurp_argv(accent: &str, background: &str) -> Vec<OsString> {
    vec![
        "slurp".into(),
        "-c".into(),
        accent.into(),
        "-b".into(),
        background.into(),
        "-f".into(),
        "%x,%y %wx%h".into(),
    ]
}

/// The `wl-copy` argv that takes a PNG on its standard input.
#[must_use]
pub fn wl_copy_argv() -> Vec<OsString> {
    vec!["wl-copy".into(), "--type".into(), "image/png".into()]
}

/// The niri argv that writes the window `id` (the focused one when `None`) to
/// `out` (an absolute path). niri also puts the picture on the clipboard,
/// whatever is asked: its action has no switch for that.
#[must_use]
pub fn niri_window_argv(out: &Path, id: Option<u64>) -> Vec<OsString> {
    let mut argv: Vec<OsString> = ["niri", "msg", "action", "screenshot-window"]
        .into_iter()
        .map(OsString::from)
        .collect();
    if let Some(id) = id {
        argv.push("--id".into());
        argv.push(id.to_string().into());
    }
    argv.extend(
        [
            "--write-to-disk",
            "true",
            "--show-pointer",
            "false",
            "--path",
        ]
        .into_iter()
        .map(OsString::from),
    );
    argv.push(out.as_os_str().to_owned());
    argv
}

/// A Qt colour name (`#RRGGBB`, or `#AARRGGBB` when translucent) as the
/// `#RRGGBBAA` slurp reads; `None` for anything else.
#[must_use]
pub fn slurp_colour(qt: &str) -> Option<String> {
    let hex = qt.strip_prefix('#')?;
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let hex = hex.to_ascii_lowercase();
    match hex.len() {
        6 => Some(format!("#{hex}ff")),
        8 => Some(format!("#{}{}", &hex[2..], &hex[..2])),
        _ => None,
    }
}

/// `argv` with its program looked up in `tools_dir` when one is given (the
/// `SELENITA_TOOLS_DIR` test seam: shell stubs named `grim`, `slurp`,
/// `wl-copy`, `niri`), else left for `PATH`.
#[must_use]
pub fn resolve(mut argv: Vec<OsString>, tools_dir: Option<&Path>) -> Vec<OsString> {
    if let (Some(dir), Some(program)) = (tools_dir, argv.first_mut()) {
        let name = Path::new(program.as_os_str())
            .file_name()
            .map(OsStr::to_owned)
            .unwrap_or_default();
        *program = dir.join(name).into_os_string();
    }
    argv
}
