# Suite conventions — design

- **Date:** 2026-10-09
- **Status:** approved by the author in brainstorming; awaiting spec review
- **Product:** the conventions that make the first-party applications behave
  as one system: a common activation interface, a shared appearance source,
  open-with inside the suite, and drag-and-drop between the applications
- **Identifiers:** program `CONV-1`, plan
  `docs/plans/active/2026-10-09-suite-conventions.md`; new crate
  `celestina-settings`; new modules in `celestina-core`; units are `suite`
  except the Cuprita section (`cuprita`)

## 1. Goal and scope

Five applications share a visual language but still behave as five programs:
three of them carry their own copy of the single-instance hand-off and two
have none; the only appearance setting (reduced motion) is an environment
variable each application copies into the theme by hand; Siderita opens the
other applications by spawning processes although they serve D-Bus; only
Siderita accepts a drop; and "send to the phone" exists on Magnetita's bus
but no application offers it. This program closes those gaps, in order:

1. **Activation.** One shared claim-first hand-off and one interface,
   `org.celestina.<App>`, with `Activate()` and `Open(as paths)`, served by
   every application; Siderita opens Grafita and Fluorita through it.
2. **Appearance.** One file, `~/.config/celestina/appearance.toml`, holding
   `reduced_motion` and `text_scale`, read by the theme and watched for
   changes so every window follows a change at once.
3. **Appearance section in Cuprita**, the one place where those two values
   are edited.
4. **Open-with inside the suite.** Siderita's context menu offers the
   first-party targets directly, and Magnetita's "send to the phone" is an
   open-with handler for every file.
5. **Drag-and-drop.** Grafita, Fluorita and Magnetita accept `text/uri-list`
   drops with the obvious meaning.
6. **Exit.** The conventions written as an ADR and enforced by the
   architecture contract.

Out of scope: a light colour scheme (the style is dark by design), the accent
(sealed by the author to the shell's colour), a settings daemon (a file plus
inotify is enough), portal `Settings` (`org.freedesktop.appearance`) as a
source or a sink (the shell that published it is halted; the file is the
source), per-application settings migration (each application keeps its own
store for its own things), and anything in the halted shell.

## 2. Constraints the design honours

- Root `AGENTS.md`: pure domain in `celestina-rs/`, Qt adaptation in each
  application's `src`, presentation in its `qml`; typed errors; no production
  `unwrap`; no blocking IO on the Qt thread; each dependency justified.
- Style contract (`celestina-style/DESIGN.md`): tokens only; the text scale
  multiplies the font tokens and nothing else; motion within 100–500 ms
  honouring `reducedMotion`.
- Byte-exact paths across the Qt seam (ADR 0008): `Open(as paths)` and
  `text/uri-list` carry bytes through `celestina_core::file_uri`/`pathkey`,
  never lossy strings.
- Privilege: nothing new; `SendFile` runs on the daemon as today.
- The halted shell is never touched; its `org.celestina.Shell1` name and
  its portal `Settings` backend are left alone.
- Commit scope: multi-application units are `suite:`; the Cuprita section is
  `cuprita:`; shared crates get their own component prefixes
  (`celestina-settings:`) registered in `docs/projects.toml` by the first
  unit that creates them.

## 3. Facts the design rests on (verified 2026-10-09)

| Fact | Where |
|---|---|
| Three claim-first copies: Grafita `OpenDocument(path)`, Hematita `Activate()`/`Open(path)`, Cuprita `Activate()`; Siderita claims only `org.freedesktop.FileManager1`; Fluorita claims nothing and serves MPRIS | `grafita/src/activation.rs`, `hematita/src/activation.rs`, `cuprita/src/controller/activation.rs`, `siderita/src/dbus.rs`, `fluorita/src/activation.rs` |
| HEM-16 already proposes sharing the hand-off in `celestina-core` | `hematita/docs/plans/active/2026-09-26-hardening.md:39` |
| Siderita's activator spawns `grafita`/`fluorita` and uses `gtk-launch` for Hematita | `siderita/src/editor.rs:249`, `media.rs:264`, `apps.rs:142` |
| `CelestinaTheme.reducedMotion` is a writable property every app sets from `CELESTINA_REDUCED_MOTION`; font tokens are nine `readonly int`s (`fontMini` 10 … `fontDisplay` 34) | `celestina-style/CelestinaTheme.qml:709-717, 820` |
| `celestina-core` has XDG dirs and atomic replace, no watcher; Siderita and Fluorita already use `notify-debouncer-full 0.5` | `celestina-rs/crates/celestina-core/src/{xdg,atomic_file}.rs`, `siderita/Cargo.toml:53` |
| Siderita's drag carries `text/uri-list` and its `FolderView` accepts external drops | `siderita/qml/Main.qml:449-476`, `views/FolderView.qml:473-492` |
| Magnetita's daemon serves `Devices1.SendFile(device_id, path)` / `SendFileUri` | `celestina-rs/crates/magnetitad/src/devices.rs:742-761` |
| Siderita's «Abrir con…» is built from `desktop_entry::scan` and `xdg-mime`, launching through `gtk-launch` | `siderita/src/apps.rs:47-142` |

## 4. Activation (`celestina-core::activation`)

- **Module** `celestina-rs/crates/celestina-core/src/activation.rs` (zbus 5,
  blocking; `celestina-core` gains `zbus` behind a feature `activation`,
  default off, so the crates that do not need the bus keep their closure).
- **API:**

```rust
pub struct ActivationName(&'static str);          // "org.celestina.Grafita"
pub enum Claim { Owner(Owned), HandedOff, Unsettled(ActivationError) }
pub trait Activatable: Send + 'static {
    fn activate(&self);                    // raise the window (queued to Qt)
    fn open(&self, paths: Vec<PathBuf>);   // open these paths (queued to Qt)
}
pub fn claim(name: ActivationName, served: impl Activatable, argv_paths: &[PathBuf])
    -> Claim;
```

  `claim` requests the name with `DoNotQueue` on a connection that already
  serves the object at `/org/celestina/<App>` (interface
  `org.celestina.Application1`: `Activate()`, `Open(as paths)` where each
  element is the byte-exact path as `celestina_core::pathkey` encodes it).
  On `NameTaken` it calls `Open(argv_paths)` when there are paths, else
  `Activate()`, on the owner with a 3 s timeout, and returns `HandedOff` so
  `main` exits 0; on a bus error it logs and returns `Unsettled` so the
  launch carries on standalone. Requests that arrive before the Qt side is
  attached wait in a bounded inbox (16) replayed by `attach()`, exactly as
  Hematita's HEM-H1-F does today. The owner keeps the connection for the
  process lifetime.
- **Adoption:** Grafita (`Open` = open documents in tabs; `OpenDocument`
  retired), Hematita (`Open` = show the storage view at that folder),
  Cuprita (`Open` ignored — no paths), Siderita (new name
  `org.celestina.Siderita`; `Open` = one tab per folder, or the parent
  folder with the file selected; `FileManager1` stays as it is), Fluorita
  (new; `Open` = play the first file, queue the rest; a folder becomes the
  library's current folder; the second launch no longer opens a second
  player). Every `.desktop` keeps its `Exec … %U`/`%F`; `DBusActivatable`
  stays unset (the suite starts applications by process, the bus only
  reaches a running one).
- **Siderita's activator** (`editor.rs`, `media.rs`, `usage.rs`) calls
  `celestina_core::activation::open_in(ActivationName, paths)` first — a
  blocking call with a 3 s timeout on a worker — and spawns the process only
  when nothing owns the name; the spawn passes the paths so the new
  instance opens them itself.
- **Architecture contract:** a new scanner refuses an `activation.rs` under
  an application that requests a bus name itself; the three old copies are
  deleted in the same unit.

## 5. Appearance (`celestina-settings`)

- **Crate** `celestina-rs/crates/celestina-settings` (no Qt):
  - `Appearance { reduced_motion: bool, text_scale: TextScale }` with
    `enum TextScale { Compact /*0.9*/, Normal /*1.0*/, Large /*1.15*/, Larger /*1.3*/ }`
    and `fn factor(self) -> f64`.
  - `load() -> Appearance` reads `~/.config/celestina/appearance.toml`
    (`celestina_core::xdg::config_home`), tolerating a missing file (defaults)
    and unknown keys (kept on write); a malformed file logs and yields the
    defaults without overwriting it.
  - `save(&Appearance) -> Result<(), SettingsError>` writes atomically
    through `celestina_core::atomic_file::replace`, preserving unknown
    keys and comments by rewriting only the two known keys (the file is
    small; a line-based rewrite keeps everything else).
  - `watch(on_change: impl Fn(Appearance) + Send + 'static) -> Result<WatchHandle, SettingsError>`
    using `notify-debouncer-full` (300 ms) on the directory (so an atomic
    replace is seen), re-reading and calling back only when the value
    changed; the handle stops the watcher on drop.
  - Environment: `CELESTINA_REDUCED_MOTION` still forces `reduced_motion`
    on (the file cannot turn it off while the variable is set), documented
    as the development and accessibility override.
  - Tests: round trip, defaults, unknown keys preserved, malformed file,
    the factor table, and a watcher test on a temporary directory.
- **Theme:** `CelestinaTheme` gains `property real textScale: 1.0` and the
  nine font tokens become `readonly property int fontX: Math.round(base * textScale)`
  with the bases kept as private constants; `rowHeight` and the other
  layout tokens do not scale (rows grow only through their text's implicit
  height where they already do). `reducedMotion` stays as it is.
- **Adapter:** one shared QML-facing object per application is not needed:
  each application's existing controller gains `appearanceReducedMotion`
  and `appearanceTextScale` properties fed by the crate's watcher on a
  worker thread (queued to Qt), and `Main.qml` binds the theme's two
  properties to them. The six `CELESTINA_REDUCED_MOTION` reads collapse into
  the crate.
- **Siderita's own scales** (`interfaceTextScale`, `contentTextScale`…) stay
  as they are: they multiply on top, and the author can reset them from
  Siderita as today. The ADR records that a later unit may fold them.

## 6. Cuprita's «Apariencia» section

- Fourth strip section (icon `preferences-desktop-theme` or the closest
  glyph in `CelestinaIcons`), one card «Apariencia»: a `SettingRow`
  for reduced motion and a row for the text size with a segmented choice of
  the four `TextScale` values (Spanish copy chosen in the unit) — `CelestinaSegmentedControl`
  if the style has it, else four `CelestinaButton`s in a `Row` with the
  current one in the selected emphasis.
- A change calls `celestina_settings::save` on Cuprita's worker; Cuprita's
  own theme follows through the same watcher as every other window, so
  the section is a true preview. When the environment variable forces
  reduced motion, the switch is on and disabled with the hint «Forzado por
  el entorno».
- The accessible names and keyboard rules of CUP-1-F apply.

## 7. Open-with inside the suite

- **Siderita's context menu** (`SidebarContextMenus`/the folder-view menu):
  a `GlassMenuSection` «Abrir en» with the targets that apply to the
  selection: «Grafita» (any file: Grafita decides by bytes and says so if
  it is not text), «Fluorita» (media and images, or a folder as a source),
  «Hematita» (a folder: see its usage), and send-to-phone (any file or
  files; one entry per paired device when `Devices1.ListDevices` returns
  more than one, none when Magnetita's daemon is absent). The entries use
  the activation module (§4) and `Devices1.SendFile`; the generic «Abrir
  con…» dialog stays below them.
- **Magnetita as a handler:** `org.celestina.Magnetita.desktop` gains a
  second action entry (`[Desktop Action send]`, `Exec=magnetita --send %F`,
  Name = the send-to-phone copy) and `MimeType=application/octet-stream;` plus the
  common families, so other applications' "open with" lists it; `magnetita
  --send` calls `SendFile` for each path on the one paired device, or shows
  Magnetita's device chooser when there are several, then exits.

## 8. Drag-and-drop

- **Grafita:** a `DropArea` over the document area accepting `text/uri-list`;
  each dropped file opens as a tab (bytes decide; a non-text file is
  refused with the existing notice). Dragging text selections is unchanged.
- **Fluorita:** a `DropArea` over the window: a dropped media file plays and
  the rest are queued; a dropped folder is offered as a new source with the
  existing source dialog prefilled; images open in the viewer.
- **Magnetita:** a `DropArea` on each device row and on the device page:
  dropped files are sent to that device through `Devices1.SendFile`, with
  the existing transfer notice.
- **Cuprita and Hematita:** no drops (nothing to receive); Hematita's
  storage view could accept a folder later, not planned.
- URIs are decoded with `celestina_core::file_uri::to_path` (byte-exact,
  local only); remote URIs are ignored with a notice.

## 9. Delivery phases

| Unit | Prefix | Outcome |
|---|---|---|
| CONV-1-A | `suite` | `celestina-core::activation` (feature `activation`), the five adoptions (three replacements, Siderita and Fluorita new), Siderita's activator over the bus, the architecture scanner, `OpenDocument` retired |
| CONV-1-B | `suite` | `celestina-settings` crate registered in `projects.toml`, `appearance.toml`, the theme's `textScale` and scaled font tokens, the six applications fed by the watcher, env override kept |
| CONV-1-C | `cuprita` | The «Apariencia» section |
| CONV-1-D | `suite` | Siderita's open-in section and send-to-phone entries; Magnetita's `--send` action and MIME registration |
| CONV-1-E | `suite` | The three drop areas |
| CONV-1-F | `suite` | ADR 0012 «Suite conventions», STATUS/ROADMAP per application, the plan archived |

Each unit lands alone; A, B and D are independent of each other, C needs B,
E needs A (Fluorita's `Open` is what a drop calls), F closes.

## 10. Verification

- Crate tests in `celestina-core::activation` (inbox replay, hand-off
  decision, `Open` path encoding) and `celestina-settings` (§5).
- Per application: the existing QML tests plus one per adoption (an `Open`
  request reaching the controller over the fake queue); Siderita's
  activator test with a fake owner on a private `dbus-run-session`.
- Live, by the author (recorded in each application's `VALIDATION.md`):
  open a second file from Siderita into a running Grafita and Fluorita;
  change the text size in Cuprita and watch every open window follow;
  drag a file from Siderita into Grafita, Fluorita and a Magnetita device;
  send-to-phone from Siderita's menu.
- Guards as always: architecture (with the new scanner), qmllint, language,
  documentation, radius and glass-canvas ratchets, `cargo fmt`, `clippy`.

## 11. Decisions recorded here

- The file is the appearance source; the portal `Settings` is neither read
  nor written (the shell is halted; a later shell may mirror the file).
- Text scale multiplies font tokens only; layout tokens stay fixed so the
  concentric-radius rules and row heights hold.
- Applications keep their own stores; `appearance.toml` holds only what
  must be the same everywhere.
- Process start stays the way to launch an application; the bus reaches a
  running one. `DBusActivatable` stays off.
- Siderita's own scales remain until a later unit decides their fate.
