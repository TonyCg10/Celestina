<!-- language-contract: product-copy — the qsTr() samples below are product copy quoted inside code blocks -->
# Hematita H1 — Foundation and the Performance page (CPU and memory)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Register Hematita as a suite project and ship its first working surface: a window with the pill navigation strip and a Performance page showing live CPU and memory with sixty seconds of history.

**Architecture:** A pure crate `hematita-core` parses `/proc` text into typed values with typed errors and owns the history ring. A sampler thread in `hematita/src` reads the files once per second, feeds the crate and queues an immutable snapshot onto the Qt thread, where one `HematitaResources` QObject publishes properties. QML presents those properties: `Main.qml` hosts `NavStrip` and the `PerformancePage`, whose `HistoryGraph` is a GPU-backed `Shape`.

**Tech Stack:** Rust 1.97.1 (pinned by `celestina-rs/rust-toolchain.toml`), cxx-qt 0.9.1 / cxx-qt-lib 0.9.1 / cxx 1.0.176 (exact pins copied from Grafita), zbus 5, Qt 6.9+ QML with `QtQuick.Shapes`, `celestina-style` linked by relative symlinks.

**Spec:** [docs/superpowers/specs/2026-09-21-hematita-design.md](../specs/2026-09-21-hematita-design.md) — sections 3 (data sources), 4 (crate), 5 (adapter), 6 (surface), 7 (phase H1), 8 (verification).

## Global Constraints

- **The Celestina shell is in standby.** Never read, reuse, modify or reference `celestina/` or `celestina-rs/crates/celestina-shell-core`. Hematita owns its own `/proc` parsers.
- **Language contract:** every identifier, comment, doc, test name, script message and commit subject is English. Product copy (what the user reads in the window) is Spanish and appears only as the literal argument of `qsTr()`. A Spanish comment or script echo adds language debt and is a defect.
- **No commit without the author's explicit request.** Each task prepares its unit (code, tests, evidence, inventory) and shows the commit command; run it only when the author has asked for a commit in this session.
- **Commit prefixes are hook-enforced:** `hematita-<kind>:` is authorised only once `docs/projects.toml` at `HEAD` registers the project. The first commit is therefore `suite-maintenance: Register the Hematita project and its core crate` (registry only); every later unit is `hematita-maintenance:` except the closing `hematita-milestone:`.
- **Rust invariants:** no `unsafe` (workspace `forbid`), no production `unwrap`/`expect`/`panic!`, typed errors that name what failed, blocking IO never on the Qt thread, every dependency justified in its manifest with a comment.
- **QML invariants:** every QML file listed in `build.rs`'s `QML_FILES`; colours, spacing, radii, type and motion only from `CelestinaTheme` tokens; `required property` and typed signals, no reaching parent ids, no `x: x` injection; no tooltips; every control keyboard-reachable with an accessible role and name; motion honours `CelestinaTheme.reducedMotion`.
- **Style consumption:** shared QML enters `hematita/qml/` only as relative symlinks to `../../celestina-style/…`; never copy a file.
- **Sampling constants:** `INTERVAL = 1 s` lives once in `hematita/src/sampler.rs`; `HISTORY_SAMPLES = 60` lives once in `hematita-core/src/history.rs`; thresholds `ELEVATED_PERCENT = 80`, `CRITICAL_PERCENT = 90` live once in `hematita/src/resources.rs`.
- **Verification order before any unit is called done:** `bash scripts/check-architecture-contract.sh`, then `bash scripts/check-documentation-contract.sh`, then `python3 scripts/check-language-contract.py`, then the project's `scripts/verify-production.sh` once a release build exists.
- **Never `cargo clean`.** Release targets are reusable monorepo resources.
- **Subagents never use `git stash`** (it reverts the shared worktree while other sessions edit).

---

## File structure

| Path | Responsibility |
|---|---|
| `celestina-rs/crates/hematita-core/Cargo.toml` | Crate manifest; no dependencies |
| `celestina-rs/crates/hematita-core/src/lib.rs` | Crate doc and module list |
| `celestina-rs/crates/hematita-core/src/ratio.rs` | `percent_of(part, whole) -> u8`, the one integer percentage |
| `celestina-rs/crates/hematita-core/src/cpu.rs` | `/proc/stat` parsing (aggregate + per core), `CpuSampler`, frequency and model parsing, `CpuError` |
| `celestina-rs/crates/hematita-core/src/memory.rs` | `/proc/meminfo` parsing, `Memory`, `MemoryError` |
| `celestina-rs/crates/hematita-core/src/history.rs` | `Ring` of the last `HISTORY_SAMPLES` values |
| `celestina-rs/crates/hematita-core/tests/fixtures/{proc-stat.txt,proc-meminfo.txt,proc-cpuinfo.txt}` | Captures from the author's machine |
| `celestina-rs/crates/hematita-core/tests/captures.rs` | Integration tests over the captures |
| `hematita/Cargo.toml`, `hematita/Cargo.lock`, `hematita/build.rs` | Application manifest, lock and CXX-Qt build |
| `hematita/src/main.rs` | Qt application setup, engine, initial properties |
| `hematita/src/activation.rs` | Single instance over the session bus |
| `hematita/src/sampler.rs` | The sampling thread and the `Snapshot` it publishes |
| `hematita/src/resources.rs` | `HematitaResources` QObject: the Performance page state and the load thresholds |
| `hematita/qml/Main.qml` | Window: `NavStrip` above a `StackLayout` of pages |
| `hematita/qml/components/NavStrip.qml`, `NavItem.qml` | Centred pill navigation, icon over label, radio-group keyboard |
| `hematita/qml/components/PerformancePage.qml` | Side list of resources and the selected detail |
| `hematita/qml/components/ResourceRow.qml` | One side-list row: name, value, sparkline |
| `hematita/qml/components/ResourceDetail.qml` | Big graph and key/value grid for the selected resource |
| `hematita/qml/components/HistoryGraph.qml` | The one graph: `Shape` fill + stroke over a normalised series |
| `hematita/qml/Celestina*.qml`, `icons`, `icons.qrc`, `fonts`, `fonts.qrc` | Symlinks into `celestina-style` |
| `hematita/org.celestina.Hematita.desktop` | Desktop entry |
| `celestina-style/icons/apps/org.celestina.Hematita.svg` | App icon |
| `hematita/scripts/{build,verify,deploy,complete,status}-production.sh`, `smoke.sh` | Production workflow, same shape as Grafita |
| `hematita/{AGENTS,README,STATUS,ROADMAP,VALIDATION}.md` | Registered project documents |
| `hematita/docs/{evidence,inventories,plans/active,plans/archive}/README.md` | Registered document roots |
| `hematita/docs/plans/active/2026-09-21-h1-foundation.md` | H1 ledger |
| `docs/projects.toml`, `README.md`, `scripts/qmllint-baseline.tsv`, `celestina-rs/Cargo.toml`, `celestina-rs/Cargo.lock`, `docs/version-history.tsv` | Registration |

Ledger units and their commits:

| Unit | Prefix / kind | Content |
|---|---|---|
| (registry) | `suite-maintenance` | `docs/projects.toml` entry (`versioned = false`), workspace member, `README.md` row, qmllint baseline row — no inventory (a registry commit, like `af5f452`) |
| H1-A | `hematita-maintenance` | Skeleton: crate stub, app building an empty window with `NavStrip`, scripts, documents, plan |
| (baseline) | `suite-maintenance` | `version_source` in the registry and the `hematita 0.1.0 baseline` history row |
| H1-B | `hematita-maintenance` | `hematita-core`: `ratio`, `cpu`, `memory`, `history`, captures |
| H1-C | `hematita-maintenance` | Sampler, `HematitaResources`, activation, Performance page QML |
| H1-Z | `hematita-milestone` | Implementation exit: `complete-production.sh`, version 0.2.0, roadmap and status closed, plan archived |

---

## Inventory generator (used by every unit)

Write this once to the scratchpad as `mkinv.py`; it is not repository content. It stages the unit's paths, computes numstat and SHA-256 from the index, writes the inventory with its own `self` row, and rewrites the ledger diffstat until the self row converges.

```python
#!/usr/bin/env python3
"""Usage: mkinv.py UNIT INVENTORY PLAN PATH... — write an exact change inventory."""
import hashlib, os, re, subprocess, sys

unit, inv, plan, *paths = sys.argv[1:]
paths = sorted(set(paths + [plan]))

def git(*args):
    return subprocess.run(["git", *args], capture_output=True, check=True).stdout.decode()

head = git("rev-parse", "HEAD").strip()
os.makedirs(os.path.dirname(inv), exist_ok=True)

def rows():
    subprocess.run(["git", "add", "--"] + paths, check=True)
    out, added, deleted = [], 0, 0
    for path in paths:
        line = git("diff", "--cached", "--numstat", "--no-renames", "HEAD", "--", path).strip()
        if not line:
            sys.exit(f"{path} has no staged change")
        a, d, _ = line.split("\t")
        mode = git("ls-files", "--stage", "--", path).split()[0]
        if mode == "120000":
            blob = os.readlink(path).encode()
        else:
            blob = subprocess.run(["git", "show", f":{path}"], capture_output=True, check=True).stdout
        out.append((a, d, hashlib.sha256(blob).hexdigest(), path))
        added += int(a); deleted += int(d)
    return out, added, deleted

def write(table, self_added):
    lines = [f"# {unit} exact change inventory", "", f"Base revision\t{head}"]
    lines += [f"Pathspec\t{p}" for p in sorted(paths + [inv])]
    lines += ["Calculation\ttracked paths use git diff --numstat --no-renames; new paths use /dev/null",
              "Hashes\tSHA-256 of final bytes; symlinks hash link-target bytes", "",
              "added\tdeleted\tcontent\tpath"]
    lines += [f"{a}\t{d}\t{h}\t{p}" for a, d, h, p in table]
    lines.append(f"{self_added}\t0\tself\t{inv}")
    with open(inv, "w") as f:
        f.write("\n".join(lines) + "\n")
    subprocess.run(["git", "add", "--", inv], check=True)
    return int(git("diff", "--cached", "--numstat", "--no-renames", "HEAD", "--", inv).split("\t")[0])

table, added, deleted = rows()
self_added = write(table, 0)
for _ in range(5):
    text = open(plan).read()
    stat = f"| {len(paths) + 1} files, +{added + self_added}/-{deleted} |"
    text = re.sub(rf"(\| {re.escape(unit)} \|[^\n]*?\| done \|[^\n]*?\|)[^|]*\|", lambda m: m.group(1) + " " + stat.strip("| ") + " |", text, count=1)
    open(plan, "w").write(text)
    table, added, deleted = rows()
    new_self = write(table, self_added)
    if new_self == self_added:
        break
    self_added = new_self
print(f"{unit}: {len(paths) + 1} files, +{added + self_added}/-{deleted}")
```

Rows in the inventory are one per path in `paths` plus the inventory's own `self` row; the ledger's `Files / areas` cell links the inventory and its `Diffstat` cell carries the printed `N files, +X/-Y`. Verify with:

```bash
python3 scripts/check-staged-units.py hematita/docs/inventories/<plan-slug>/<UNIT>.numstat.tsv
```

---

### Task 1: The `hematita-core` crate stub in the workspace

**Files:**
- Create: `celestina-rs/crates/hematita-core/Cargo.toml`
- Create: `celestina-rs/crates/hematita-core/src/lib.rs`
- Create: `celestina-rs/crates/hematita-core/src/ratio.rs`
- Modify: `celestina-rs/Cargo.toml` (workspace members)
- Modify: `celestina-rs/Cargo.lock` (regenerated by cargo)

**Interfaces:**
- Produces: `hematita_core::ratio::percent_of(part: u64, whole: u64) -> u8`

- [ ] **Step 1: Write the crate manifest**

```toml
# celestina-rs/crates/hematita-core/Cargo.toml
[package]
name = "hematita-core"
version = "0.1.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true

# Deliberately empty. Everything this crate does is parse text the caller
# read from /proc and /sys into typed values; the standard library is the
# whole toolbox and a library would still leave hwmon and amdgpu to us.
[dependencies]

[lints]
workspace = true
```

- [ ] **Step 2: Add the workspace member**

In `celestina-rs/Cargo.toml`, inside `members = [`, add after `"crates/grafita-core",`:

```toml
    "crates/hematita-core",
```

- [ ] **Step 3: Write the failing test for `percent_of`**

```rust
// celestina-rs/crates/hematita-core/src/ratio.rs
//! One integer percentage for the whole crate.
//!
//! Tick and kibibyte counts are exact integers; turning them into floats to
//! divide would only add rounding to a number a bar shows as a whole percent.

/// `part` of `whole` as a whole percent, saturating at 100 and answering 0 for
/// a whole of nothing.
#[must_use]
pub fn percent_of(part: u64, whole: u64) -> u8 {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::percent_of;

    #[test]
    fn a_percentage_never_leaves_its_range() {
        assert_eq!(percent_of(5_000, 1_000), 100);
        assert_eq!(percent_of(0, 0), 0);
        assert_eq!(percent_of(1, 3), 33);
        assert_eq!(percent_of(2, 3), 66);
        assert_eq!(percent_of(u64::MAX, u64::MAX), 100);
    }
}
```

```rust
// celestina-rs/crates/hematita-core/src/lib.rs
//! Hematita's domain: what the kernel reports, as typed values.
//!
//! Every function here takes the text a caller read from `/proc` or `/sys`
//! and answers a value or an error naming what was unreadable. Nothing here
//! opens a file, spawns a thread or knows what a percentage should look like
//! on screen; that is the application's business.

pub mod ratio;
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cd celestina-rs && cargo test -p hematita-core ratio`
Expected: FAIL — panics with `not yet implemented`.

- [ ] **Step 5: Implement `percent_of`**

Replace the `todo!()` body with:

```rust
    if whole == 0 {
        return 0;
    }
    // `part.min(whole) * 100` can overflow a u64 near its top, so divide in
    // u128 and the result is provably within 0..=100.
    let scaled = u128::from(part.min(whole)) * 100 / u128::from(whole);
    u8::try_from(scaled).unwrap_or(100)
```

- [ ] **Step 6: Run the tests, fmt and clippy**

Run: `cd celestina-rs && cargo test -p hematita-core && cargo fmt --all --check && cargo clippy --locked -p hematita-core --all-targets -- -D warnings`
Expected: 1 passed; fmt and clippy clean. `Cargo.lock` gains a `hematita-core` package entry.

---

### Task 2: The application skeleton that opens an empty window

**Files:**
- Create: `hematita/Cargo.toml`, `hematita/build.rs`, `hematita/src/main.rs`
- Create: `hematita/qml/Main.qml`
- Create symlinks in `hematita/qml/`: `CelestinaTheme.qml`, `CelestinaIcons.qml`, `CelestinaButton.qml`, `CelestinaFocusRing.qml`, `CelestinaIcon.qml`, `CelestinaIconButton.qml`, `CelestinaCapsule.qml`, `CelestinaSurface.qml`, `CelestinaSectionLabel.qml`, `icons`, `icons.qrc`, `fonts`, `fonts.qrc`
- Create: `hematita/org.celestina.Hematita.desktop`
- Create: `celestina-style/icons/apps/org.celestina.Hematita.svg`
- Create: `hematita/.qmlls.ini` (git-ignored by the root rules; editor aid only)

**Interfaces:**
- Produces: QML module `org.celestina.hematita 1.0`; window root `Main.qml` with `required property bool reducedMotion`; `APP_ID = "org.celestina.Hematita"`.

- [ ] **Step 1: Write the application manifest**

```toml
# hematita/Cargo.toml
[package]
name = "hematita"
version = "0.1.0"
edition = "2021"
rust-version = "1.85"
license = "GPL-3.0-or-later"
build = "build.rs"

[[bin]]
name = "hematita"
path = "src/main.rs"

[dependencies]
cxx = "=1.0.176"
cxx-qt = "=0.9.1"
cxx-qt-lib = { version = "=0.9.1", default-features = false, features = [
    "qt_gui",
    "qt_qml",
    "qt_quickcontrols",
] }
# Everything the kernel reports, parsed and tested without Qt. This binary
# adds a window, a sampling thread and nothing else.
hematita-core = { path = "../celestina-rs/crates/hematita-core" }
# Single-instance activation: a second `hematita` raises the running window
# instead of mapping another one. The session bus is how the suite talks
# between processes, so this earns zbus rather than a socket protocol.
zbus = "5"

[build-dependencies]
cxx-qt-build = "=0.9.1"
cxx-gen = "=0.7.176"

[profile.release]
codegen-units = 1
lto = "thin"
panic = "abort"
strip = "symbols"
```

- [ ] **Step 2: Create the style symlinks**

```bash
cd hematita/qml
for f in CelestinaTheme CelestinaIcons CelestinaButton CelestinaFocusRing CelestinaIcon CelestinaIconButton CelestinaCapsule CelestinaSurface CelestinaSectionLabel; do
  ln -s "../../celestina-style/$f.qml" "$f.qml"
done
ln -s ../../celestina-style/icons icons
ln -s ../../celestina-style/icons.qrc icons.qrc
ln -s ../../celestina-style/fonts fonts
ln -s ../../celestina-style/fonts.qrc fonts.qrc
mkdir -p components
ls -l
```

Expected: every link resolves (`ls -lL` shows no `No such file`).

- [ ] **Step 3: Write `build.rs`**

```rust
// hematita/build.rs
use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

// Every QML file in one list, so it is both registered in the module and
// watched for rebuilds: two lists once let an edited file compile "fine"
// without reaching the binary.
const QML_FILES: &[&str] = &[
    // The suite's shared visual language, symlinked from ../celestina-style.
    "qml/CelestinaButton.qml",
    "qml/CelestinaFocusRing.qml",
    "qml/CelestinaIcon.qml",
    "qml/CelestinaIconButton.qml",
    "qml/CelestinaCapsule.qml",
    "qml/CelestinaSurface.qml",
    "qml/CelestinaSectionLabel.qml",
    // Hematita's own composition: Main owns the window, the components own
    // one region each.
    "qml/Main.qml",
];

fn main() {
    // CelestinaTheme and CelestinaIcons are singletons that live canonically
    // in ../celestina-style; symlinked into qml/ so they register under a
    // clean `qml/...` resource path (a `..` in the source path would embed
    // `..` in the qrc alias and break type resolution at run time).
    let module = QmlModule::new("org.celestina.hematita")
        .version(1, 0)
        .qml_file(
            QmlFile::from("qml/CelestinaTheme.qml")
                .version(1, 0)
                .singleton(true),
        )
        .qml_file(
            QmlFile::from("qml/CelestinaIcons.qml")
                .version(1, 0)
                .singleton(true),
        )
        .qml_files(QML_FILES);

    // Naming any rerun-if-changed stops cargo watching the whole package, so
    // every watched file must be listed explicitly.
    for qml in QML_FILES.iter().copied().chain([
        "qml/CelestinaTheme.qml",
        "qml/CelestinaIcons.qml",
        "qml/icons.qrc",
        "qml/fonts.qrc",
    ]) {
        println!("cargo::rerun-if-changed={qml}");
    }

    CxxQtBuilder::new_qml_module(module)
        // Shared icon resources and Inter Variable, compiled in.
        .qrc("qml/icons.qrc")
        .qrc("qml/fonts.qrc")
        .build();
}
```

- [ ] **Step 4: Write `main.rs`**

```rust
// hematita/src/main.rs
use cxx_qt_lib::{
    QGuiApplication, QMap, QMapPair_QString_QVariant, QQmlApplicationEngine, QQuickStyle, QString,
    QUrl, QVariant,
};

/// The freedesktop application ID: the installed `.desktop` basename, the icon
/// name, and — because Qt reports it as the Wayland `app_id` — what the
/// compositor matches a window against. All three must be this one string.
const APP_ID: &str = "org.celestina.Hematita";

fn main() {
    // Without a platform theme Qt has nobody to ask for dialogs and draws its
    // own outside this session's portal route. An explicit choice still wins.
    if std::env::var_os("QT_QPA_PLATFORMTHEME").is_none() {
        std::env::set_var("QT_QPA_PLATFORMTHEME", "xdgdesktopportal");
    }

    let mut app = QGuiApplication::new();

    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("Hematita"));
        app.as_mut()
            .set_application_display_name(&QString::from("Hematita"));
        app.as_mut()
            .set_organization_name(&QString::from("Celestina"));
        app.as_mut()
            .set_organization_domain(&QString::from("celestina.org"));
        QGuiApplication::set_desktop_file_name(&QString::from(APP_ID));
    }

    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        QQuickStyle::set_style(&QString::from("Basic"));
    }

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        let reduced_motion = std::env::var_os("CELESTINA_REDUCED_MOTION").is_some();
        let mut initial_properties = QMap::<QMapPair_QString_QVariant>::default();
        initial_properties.insert(
            QString::from("reducedMotion"),
            QVariant::from(&reduced_motion),
        );
        engine.as_mut().set_initial_properties(&initial_properties);
        engine.load(&QUrl::from(
            "qrc:/qt/qml/org/celestina/hematita/qml/Main.qml",
        ));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }
}
```

- [ ] **Step 5: Write the first `Main.qml`**

```qml
// hematita/qml/Main.qml
import QtQuick
import QtQuick.Controls
import org.celestina.hematita 1.0

// Hematita's window. The sections live in a centred pill strip at the top;
// each page below owns one region and reaches nothing outside itself.
ApplicationWindow {
    id: window

    required property bool reducedMotion

    width: 1000
    height: 680
    minimumWidth: 640
    minimumHeight: 420
    visible: true
    color: CelestinaTheme.canvas
    title: "Hematita"

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = window.reducedMotion
    }
}
```

- [ ] **Step 6: Write the desktop entry and the icon**

```ini
# hematita/org.celestina.Hematita.desktop
[Desktop Entry]
Type=Application
Name=Hematita
GenericName=Monitor de recursos
Comment=Muestra el uso del equipo y permite actuar sobre los procesos
Exec=hematita
Icon=org.celestina.Hematita
Terminal=false
Categories=System;Monitor;
Keywords=recursos;monitor;procesos;cpu;memoria;hematita;celestina;
StartupNotify=true
StartupWMClass=org.celestina.Hematita
```

The icon reuses the suite's squircle tile verbatim from `org.celestina.Grafita.svg` (both `<path d="M496.0 256.0 …">` elements, tile fill and rim stroke) and replaces the glyph and gradients. Copy Grafita's file, keep the two tile paths, and replace everything else with:

```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">
  <title>Hematita</title>
  <!--
    Hematite is blood-red iron oxide, the mineral behind the word "pulse" in
    this icon: one heartbeat trace drawn across a dark tile, the colour of the
    streak the stone leaves on porcelain. The tile is the suite's shared
    superellipse; only the gradients and the trace are Hematita's.
  -->
  <defs>
    <linearGradient id="tile" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#2A1416"/>
      <stop offset="1" stop-color="#120809"/>
    </linearGradient>
    <linearGradient id="rim" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#FFFFFF" stop-opacity="0.32"/>
      <stop offset="0.55" stop-color="#FFFFFF" stop-opacity="0.05"/>
      <stop offset="1" stop-color="#FFFFFF" stop-opacity="0"/>
    </linearGradient>
    <linearGradient id="trace" x1="0" y1="0" x2="1" y2="0">
      <stop offset="0" stop-color="#FF8A7A"/>
      <stop offset="1" stop-color="#C8322B"/>
    </linearGradient>
  </defs>
  <!-- tile path and rim path copied verbatim from org.celestina.Grafita.svg -->
  <path d="M496.0 256.0L495.9 327.9 … Z" fill="url(#tile)"/>
  <path d="M495.2 256.0L495.1 327.6 … Z" fill="none" stroke="url(#rim)" stroke-width="1.5"/>
  <!-- The pulse: flat, one sharp beat, flat. Centred on the tile's midline. -->
  <path d="M96 256 H188 L214 196 L246 330 L278 176 L304 256 H416"
        fill="none" stroke="url(#trace)" stroke-width="30"
        stroke-linecap="round" stroke-linejoin="round"/>
  <circle cx="416" cy="256" r="20" fill="#FF8A7A"/>
</svg>
```

Take the two long `d="…"` attributes from Grafita's file with `grep -o 'd="M49[56]\.[0-9] 256\.0[^"]*"' celestina-style/icons/apps/org.celestina.Grafita.svg`.

- [ ] **Step 7: Write the editor aid**

```ini
# hematita/.qmlls.ini
[General]
buildDir="/home/toni/CODIGO/CELESTINA/hematita/target/cxxqt/qml_modules"
no-cmake-calls=true
```

- [ ] **Step 8: Build and run offscreen**

Run:
```bash
cd hematita && cargo generate-lockfile && cargo build --release --locked --bin hematita
QT_QPA_PLATFORM=offscreen timeout 4 target/release/hematita; echo "rc=$?"
```
Expected: the build succeeds and the run prints no `TypeError`, `ReferenceError`, `is not a type` or `Cannot assign`, exiting with `rc=124` (killed by timeout, still alive).

---

### Task 3: `NavStrip` — the centred pill navigation

**Files:**
- Create: `hematita/qml/components/NavItem.qml`
- Create: `hematita/qml/components/NavStrip.qml`
- Modify: `hematita/qml/Main.qml`
- Modify: `hematita/build.rs` (`QML_FILES`)

**Interfaces:**
- Produces: `NavStrip { model: [...]; currentIndex: int; signal activated(int index) }` where each model entry is `{ key: string, icon: string, label: string }`; `NavItem { required property string iconName; required property string label; required property bool current; signal triggered() }`.

- [ ] **Step 1: Write `NavItem.qml`**

```qml
// hematita/qml/components/NavItem.qml
import QtQuick
import QtQuick.Controls
import org.celestina.hematita 1.0

// One destination in the strip: a glyph over its word. The current one wears a
// lighter pill; the others paint nothing at rest and the suite's hover fill
// under the pointer. It is a radio button for the keyboard and the screen
// reader, and the strip decides which one is checked.
AbstractButton {
    id: item

    required property string iconName
    required property string label
    required property bool current

    implicitWidth: Math.max(CelestinaTheme.controlHeightXl * 2, column.implicitWidth + CelestinaTheme.spaceXl * 2)
    implicitHeight: CelestinaTheme.controlHeightXl + CelestinaTheme.spaceSm

    checkable: true
    checked: item.current
    autoExclusive: true
    hoverEnabled: true
    focusPolicy: Qt.TabFocus

    Accessible.role: Accessible.PageTab
    Accessible.name: item.label
    Accessible.checked: item.current

    background: Rectangle {
        radius: CelestinaTheme.radiusPill
        color: item.current
               ? CelestinaTheme.elevated
               : item.down
                 ? CelestinaTheme.pressedWash
                 : item.hovered
                   ? CelestinaTheme.surfaceHover
                   : CelestinaTheme.clear
        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionFast
            }
        }
    }

    // The ring anchors to its target, so it must be the target's child, not
    // the background's: anchors reach only a parent or a sibling.
    CelestinaFocusRing {
        target: item
        cornerRadius: item.height / 2
        shown: item.visualFocus
    }

    contentItem: Column {
        id: column
        spacing: CelestinaTheme.spaceXs
        anchors.centerIn: parent

        CelestinaIcon {
            anchors.horizontalCenter: parent.horizontalCenter
            name: item.iconName
            width: CelestinaTheme.iconMd
            height: width
            tone: item.current ? CelestinaIcon.Primary : CelestinaIcon.Secondary
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: item.label
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
            font.weight: item.current ? CelestinaTheme.weightDemiBold : CelestinaTheme.weightRegular
            color: item.current ? CelestinaTheme.text : CelestinaTheme.textMuted
        }
    }
}
```

`CelestinaIcon` takes `name`, `width`/`height` and a `tone` from its enum (`Primary` is the text colour, `Secondary` the muted one); `CelestinaFocusRing` takes `target`, `cornerRadius` and `shown`. Both are verified against the style files as of 2026-09-21; never add properties to a shared file.

- [ ] **Step 2: Write `NavStrip.qml`**

```qml
// hematita/qml/components/NavStrip.qml
import QtQuick
import org.celestina.hematita 1.0

// The centred pill that names the sections. Built on the suite's capsule so
// the resting fill and the pill radius are the shared ones; the items inside
// are Hematita's, because a glyph-over-word destination with a lit current
// state is not a control the style ships yet. It stays here until a second
// application needs it.
//
// Keyboard: one radio group. Left/Right move the selection, Tab leaves.
FocusScope {
    id: strip

    // Each entry: { key: string, icon: string, label: string }.
    required property var model
    property int currentIndex: 0

    signal activated(int index)

    implicitWidth: capsule.implicitWidth
    implicitHeight: capsule.implicitHeight

    Accessible.role: Accessible.PageTabList

    function move(delta) {
        const count = strip.model.length
        if (count === 0)
            return
        const next = (strip.currentIndex + delta + count) % count
        strip.activated(next)
    }

    Keys.onLeftPressed: function(event) { strip.move(-1); event.accepted = true }
    Keys.onRightPressed: function(event) { strip.move(1); event.accepted = true }

    CelestinaCapsule {
        id: capsule
        inset: CelestinaTheme.spaceXs
        spacing: CelestinaTheme.spaceXs

        Repeater {
            model: strip.model

            NavItem {
                required property int index
                required property var modelData

                iconName: modelData.icon
                label: modelData.label
                current: index === strip.currentIndex
                // Only the current item is in the Tab order; arrows walk
                // the rest, so Tab lands on the strip once and leaves once.
                focus: current
                onClicked: strip.activated(index)
            }
        }
    }
}
```

`CelestinaCapsule` accepts default children through `actions: row.data`; the `Repeater` lands inside that `Row`.

- [ ] **Step 3: Wire the strip into `Main.qml`**

Replace `Main.qml`'s body with:

```qml
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.hematita 1.0
import "components"

ApplicationWindow {
    id: window

    required property bool reducedMotion

    width: 1000
    height: 680
    minimumWidth: 640
    minimumHeight: 420
    visible: true
    color: CelestinaTheme.canvas
    title: "Hematita"

    // The sections, in the order the strip shows them. Only Performance has a
    // page in H1; the others are named so the strip is the final one and the
    // pages arrive under it in H3 and H4.
    readonly property var sections: [
        { key: "performance", icon: "gauge", label: qsTr("Rendimiento") },
        { key: "processes", icon: "view-list", label: qsTr("Procesos") },
        { key: "applications", icon: "app-window", label: qsTr("Aplicaciones") },
        { key: "sensors", icon: "zap", label: qsTr("Sensores") }
    ]
    property int currentSection: 0

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: CelestinaTheme.windowMargin
        spacing: CelestinaTheme.spaceLg

        NavStrip {
            id: navStrip
            Layout.alignment: Qt.AlignHCenter
            model: window.sections
            currentIndex: window.currentSection
            onActivated: function(index) { window.currentSection = index }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: window.currentSection

            // Performance lands here in Task 8. Placeholder pages say in the
            // product's words that the section is not built yet.
            Repeater {
                model: window.sections.length
                Item {
                    required property int index
                    Text {
                        anchors.centerIn: parent
                        text: qsTr("Esta sección llega en una fase posterior")
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontRowTitle
                    }
                }
            }
        }
    }

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = window.reducedMotion
        navStrip.forceActiveFocus()
    }
}
```

The icon names `gauge`, `view-list`, `app-window` and `zap` all exist in `celestina-style/icons/`.

- [ ] **Step 4: Register the two files in `build.rs`**

In `QML_FILES`, before `"qml/Main.qml"`, add:

```rust
    "qml/components/NavItem.qml",
    "qml/components/NavStrip.qml",
```

- [ ] **Step 5: Build, run offscreen, and inspect the strip**

```bash
cd hematita && cargo build --release --locked --bin hematita
QT_QPA_PLATFORM=offscreen timeout 4 target/release/hematita 2>&1 | grep -E 'Error|unavailable|is not a type|Cannot' ; echo "rc=${PIPESTATUS[0]}"
```
Expected: no matching lines; `rc=124`.

Then look at it on the nest session, never the live one: follow the memory note *Siderita headless debugging* for the offscreen screenshot recipe, or run `QT_QPA_PLATFORM=wayland target/release/hematita` inside the author's nested niri and capture with `grim`. The strip must be a pill centred at the top with four glyph-over-word items and the first lit.

---

### Task 4: Documents, scripts, registration and the H1-A unit

**Files:**
- Create: `hematita/scripts/build-production.sh`, `verify-production.sh`, `deploy-production.sh`, `complete-production.sh`, `status-production.sh`, `smoke.sh`
- Create: `hematita/AGENTS.md`, `README.md`, `STATUS.md`, `ROADMAP.md`, `VALIDATION.md`
- Create: `hematita/docs/evidence/README.md`, `hematita/docs/inventories/README.md`, `hematita/docs/plans/active/README.md`, `hematita/docs/plans/archive/README.md`
- Create: `hematita/docs/plans/active/2026-09-21-h1-foundation.md`
- Create: `hematita/docs/evidence/2026-09-21-h1-skeleton.md`
- Modify: `docs/projects.toml`, `README.md`, `scripts/qmllint-baseline.tsv`

- [ ] **Step 1: Write the six scripts**

Each is Grafita's with `grafita`→`hematita`, `Grafita`→`Hematita`, `grafita-core`→`hematita-core`, `org.celestina.Grafita`→`org.celestina.Hematita`. Copy them, then apply the substitution and make them executable:

```bash
mkdir -p hematita/scripts
for s in build-production verify-production deploy-production complete-production status-production; do
  sed -e 's/grafita-core/hematita-core/g' -e 's/grafita/hematita/g' -e 's/Grafita/Hematita/g' grafita/scripts/$s.sh > hematita/scripts/$s.sh
done
chmod 0755 hematita/scripts/*.sh
grep -n 'grafita\|Grafita' hematita/scripts/*.sh
```
Expected: the final grep prints nothing. `status-production.sh` carries two Spanish messages copied from Grafita (`--prefix necesita un directorio`, `uso:`); rewrite them in English in the Hematita copy:

```sh
if [ "${1:-}" = "--prefix" ]; then shift; prefix=${1:?--prefix requires a directory}; shift; fi
[ "$#" -eq 0 ] || { echo "usage: scripts/status-production.sh [--prefix DIR]" >&2; exit 2; }
```

Write `smoke.sh` by hand (Grafita's is Spanish and opens a document; Hematita opens nothing):

```sh
#!/bin/sh
set -u

# Hematita smoke: the fast gate without a window.
#
#  1) The shared static check for the `x: x` auto-binding: an injected
#     property that shadows the id resolves to itself and stays undefined.
#     Legal for the engine and for qmllint, so it is caught by pattern.
#  2) An 8-second offscreen start: the binary must still be alive (timeout
#     answers 124) and the QML runtime must not report errors. Looking only
#     for TypeError/ReferenceError would let the worst case through — an
#     object that does not construct — so "Cannot create delegate" and its
#     relatives fail too.
#
# This catches *startup* errors only. Keyboard, focus and accessibility need a
# real Wayland session.

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin=$root/target/release/hematita
scanner=$root/../scripts/architecture_scanners.py
if [ "${1:-}" = "--binary" ]; then
    shift
    bin=${1:?--binary requires a path}
    shift
fi
if [ "$#" -ne 0 ]; then
    echo "usage: scripts/smoke.sh [--binary PATH]" >&2
    exit 2
fi

if ! autos=$(python3 "$scanner" qml-auto-bindings "$root/qml"); then
    echo "smoke: the auto-binding scanner could not complete" >&2
    exit 1
fi
if [ -n "$autos" ]; then
    echo "smoke: auto-binding 'x: x' (the property shadows the id):" >&2
    echo "$autos" >&2
    exit 1
fi

if [ ! -x "$bin" ]; then
    echo "smoke: the binary is missing: $bin" >&2
    exit 1
fi

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT HUP INT TERM
mkdir -p "$scratch/config" "$scratch/data" "$scratch/cache" \
    "$scratch/state" "$scratch/run"
chmod 0700 "$scratch/run"
log=$scratch/output.log

XDG_CONFIG_HOME=$scratch/config \
XDG_DATA_HOME=$scratch/data \
XDG_CACHE_HOME=$scratch/cache \
XDG_STATE_HOME=$scratch/state \
XDG_RUNTIME_DIR=$scratch/run \
DBUS_SESSION_BUS_ADDRESS=unix:path=$scratch/run/no-session-bus \
QT_QPA_PLATFORM=offscreen \
QT_ASSUME_STDERR_HAS_CONSOLE=1 \
    timeout 8 "$bin" >"$log" 2>&1
rc=$?
if [ "$rc" -ne 124 ]; then
    echo "smoke: the binary exited on its own (rc=$rc); last lines:" >&2
    tail -20 "$log" >&2
    exit 1
fi

errors=$(grep -E 'TypeError|ReferenceError|SyntaxError|Cannot create delegate|Cannot set properties on|Cannot assign|Unable to assign|Type [A-Za-z_][A-Za-z0-9_]* unavailable|is not a type|Binding loop detected' "$log" || true)
if [ -n "$errors" ]; then
    echo "smoke: QML errors at startup:" >&2
    echo "$errors" | sort | uniq -c | sort -rn >&2
    exit 1
fi

echo "smoke: OK — binary alive for 8 s, no QML errors, no auto-bindings"
```

`chmod 0755 hematita/scripts/smoke.sh`.

- [ ] **Step 2: Write the project documents**

`hematita/AGENTS.md`:

```markdown
# Hematita — local contract

This file inherits the root [`AGENTS.md`](../AGENTS.md) in full. It only adds
Hematita constraints; it cannot relax the root or grant authority.

## Required context

- [README.md](README.md), [STATUS.md](STATUS.md), [ROADMAP.md](ROADMAP.md), and
  [VALIDATION.md](VALIDATION.md)
- [The design](../docs/superpowers/specs/2026-09-21-hematita-design.md)
- [Production artifacts](../docs/contracts/production-artifacts.md)
- [Architecture](../docs/standards/architecture.md)
- [Rust, C++, Qt, and QML](../docs/standards/rust-cpp-qt-qml.md)
- [Verification](../docs/standards/verification.md)
- [Visual design](../celestina-style/DESIGN.md) for visual changes

## Local boundary

- `celestina-rs/crates/hematita-core` is the only owner of `/proc` and `/sys`
  parsing, rate computation between samples and the history ring. It takes
  text and answers typed values or typed errors; it opens no file and holds
  no policy.
- `src/` owns the sampling thread, the snapshot it publishes, the load
  thresholds and every Qt object; `qml/` presents. QML never reads a file,
  spawns a process or decides what "high" means.
- The Celestina shell is in standby by the author's order: never read, reuse
  or reference `celestina/` or `celestina-shell-core`, however similar.
- Absence is a state: a source that cannot be read leaves its section of the
  snapshot unavailable with a reason while the others keep publishing.
- Per-process network throughput is out of scope for every phase (the kernel
  does not expose it without root or eBPF); per-process disk IO exists only
  for the user's own processes.

## Local verification

- `hematita/scripts/build-production.sh`
- `hematita/scripts/verify-production.sh`
- `hematita/scripts/status-production.sh`

Verification exercises the canonical release artifact without touching the
installed binary. Closing a bug or milestone runs
`hematita/scripts/complete-production.sh`. Graph smoothness, idle cost,
keyboard and assistive-technology checks belong in `VALIDATION.md`.
```

`hematita/README.md`:

```markdown
# Hematita

Celestina's resource monitor: what the machine is doing, and the means to act
on it. It replaces Mission Center.

## User contract

- Performance: CPU, memory, and in later phases disks, network and GPU, each
  with sixty seconds of history.
- Processes and applications (H3), sensors (H4) and systemd services (H5)
  follow the [design](../docs/superpowers/specs/2026-09-21-hematita-design.md).
- Hematita reads `/proc` and `/sys` directly; it needs no daemon and no
  privilege to observe. Acting on other users' processes and on services
  arrives with polkit in H5.

## Architecture

| Area | Responsibility |
|---|---|
| `../celestina-rs/crates/hematita-core` | Parsers over `/proc` and `/sys` text, rate samplers, the history ring; no Qt, no IO |
| `src/` | The sampling thread, its immutable snapshot, load thresholds, CXX-Qt objects, single-instance activation |
| `qml/` | The window, the pill navigation strip and the pages |
| `../celestina-style` | Canonical visual tokens, controls and assets, linked |
| `org.celestina.Hematita.desktop` | Desktop discovery |

## Build and use

Hematita needs Rust and a Qt 6 development environment visible to CXX-Qt. The
canonical production workflow is:

```sh
scripts/build-production.sh
scripts/verify-production.sh
scripts/status-production.sh
scripts/complete-production.sh # canonical agent completion; updates ~/.local
```

After completion, launch `hematita` or use the desktop entry.

## Project documents

- [Current status](STATUS.md)
- [Implementation roadmap](ROADMAP.md)
- [Author validation](VALIDATION.md)
- [Local agent delta](AGENTS.md)
```

`hematita/STATUS.md`:

```markdown
# Hematita status

- **Updated:** 2026-09-21
- **Implementation:** H1 is active; the skeleton builds and opens the window
  with the navigation strip
- **Author validation:** `VAL-H1` requested, not run

## Current checkout truth

- The project is registered and builds a release binary. The window shows the
  pill strip with four sections; only Performance gains a page in H1.
- `hematita-core` exists as a stub with the shared percentage.

## Blockers

None recorded.
```

`hematita/ROADMAP.md`:

```markdown
# Hematita implementation roadmap

- **Status:** active
- **Active implementation checkpoint:** H1
- **Related author validation:** `VAL-H1` in [VALIDATION.md](VALIDATION.md)
  (does not block)

## Hypothesis and tangible outcome

A window that samples `/proc` once per second on its own thread can show CPU
and memory with sixty seconds of history, through parsers tested on text
alone, without the machine noticing the monitor.

## Scope

| Phase | Outcome |
|---|---|
| H1 | Registered project, skeleton, navigation strip, Performance page with CPU and memory |
| H2 | Disks, network, GPU and swap; the per-core grid |
| H3 | Processes and applications |
| H4 | Sensors (all of hwmon) |
| H5 | Services and privileged actions |

## Exclusions

- Per-process network; non-AMD GPUs; history persistence; alerts; any shell
  integration.

## Build order

| Unit | Status | Dependency | Implementation result | Agent evidence |
|---|---|---|---|---|
| H1-A | done | none | crate stub, application skeleton, strip, scripts, documents | `scripts/smoke.sh`, guards |
| H1-B | planned | H1-A | `hematita-core`: ratio, cpu, memory, history with captures | `cargo test -p hematita-core` |
| H1-C | planned | H1-B | sampler, `HematitaResources`, activation, Performance page | `scripts/verify-production.sh` |
| H1-Z | planned | H1-C | implementation exit and 0.2.0 | `scripts/complete-production.sh` |

## Implementation exit

`scripts/complete-production.sh` succeeds: the release build, its verification
(crate tests, clippy, fmt, qmllint ratchet, smoke) and the deployment to the
author's prefix; the installed binary shows live CPU and memory graphs.

## Closed evidence

- H1-A: [skeleton](docs/evidence/2026-09-21-h1-skeleton.md)
```

`hematita/VALIDATION.md`:

```markdown
# Author validation — Hematita

This queue contains no implementation work and never blocks `ROADMAP.md`.

## VAL-H1 — Live CPU and memory on the real session

- **Status:** pending
- **Related implementation:** H1
- **Requires:** the deployed Hematita on the real session
- **Procedure:** launch `hematita`; watch the CPU graph for a minute while
  compiling something; switch to Memory with the pointer and with the arrow
  keys from the strip; read the strip with the screen reader; check the
  monitor's own CPU in another tool while idle
- **Pass condition:** the graph advances once per second without stutter;
  values match another monitor within a few percent; the strip is reachable by
  Tab and walkable by arrows; the screen reader names each section and its
  checked state; Hematita idles under 1 % CPU
- **Result:** not run
- **Evidence:** none
```

`hematita/docs/evidence/README.md`:

```markdown
# Hematita evidence

Dated records in this directory contain agent-executable evidence for Hematita
implementation units. Author-only checks remain in `hematita/VALIDATION.md`.

Use `YYYY-MM-DD-short-topic.md` and the suite
[evidence template](../../../docs/templates/evidence.md).
```

`hematita/docs/inventories/README.md`:

```markdown
# Hematita inventories

Immutable exact change inventories, one per closed ledger unit, under
`<plan-slug>/<unit>.numstat.tsv`. Never edit, move, rename or reuse one.
```

`hematita/docs/plans/active/README.md`:

```markdown
# Active Hematita plans

The active plan is [H1 — foundation](2026-09-21-h1-foundation.md), which the
project roadmap names as its active implementation checkpoint.

Unit inventories live under
[`../../inventories/<plan-slug>/<unit>.numstat.tsv`](../../inventories/) and do
not move when the completed plan moves to `../archive/`.
```

`hematita/docs/plans/archive/README.md`:

```markdown
# Archived Hematita plans

Completed plans move here alone with the same basename, checkpoint, `Plan ID`,
units and links. Exact inventories remain immutable under
[`../../inventories/`](../../inventories/) and evidence remains in its stable
root.
```

`hematita/docs/plans/active/2026-09-21-h1-foundation.md`:

```markdown
# H1 — Foundation and the Performance page

- **Opened:** 2026-09-21
- **Plan ID:** h1-foundation
- **Status:** active
- **Authorization:** the author approved the design and asked for the H1 plan
  on 2026-09-21
- **Scope:** hematita
- **Implementation checkpoint:** H1
- **Author-validation checkpoint:** `VAL-H1` in [VALIDATION.md](../../../VALIDATION.md)

## Hypothesis

A window that samples `/proc` once per second on its own thread can show CPU
and memory with sixty seconds of history, through parsers tested on text
alone, without the machine noticing the monitor.

## Tangible outcome

The registered project, a release binary installed under the author's prefix,
whose Performance page shows live CPU and memory with a one-minute graph and a
pill navigation strip naming the later sections.

## Scope

- `H1-A` — the skeleton: crate stub, application, strip, scripts, documents.
- `H1-B` — `hematita-core`: `ratio`, `cpu`, `memory`, `history`, captures.
- `H1-C` — sampler thread, `HematitaResources`, activation, Performance page.
- `H1-Z` — implementation exit and 0.2.0.

## Exclusions

- Everything in phases H2 to H5 of the
  [design](../../../../docs/superpowers/specs/2026-09-21-hematita-design.md).
- Any reference to the Celestina shell.

## Build order

1. `H1-A`, then `-B`, `-C`, `-Z`.

## Implementation exit

`scripts/complete-production.sh` succeeds and the installed binary shows live
CPU and memory graphs.

## Change and commit ledger

| Unit | Commit prefix | Status | Files / areas | Diffstat | Intended change | Automated evidence | Author validation |
|---|---|---|---|---|---|---|---|
| H1-A | `hematita:` | done | [inventory](../../inventories/2026-09-21-h1-foundation/H1-A.numstat.tsv) | 0 files, +0/-0 | Crate stub with the shared percentage, application skeleton opening the window with the pill navigation strip, production scripts, smoke, the document set | [skeleton](../../evidence/2026-09-21-h1-skeleton.md) | `VAL-H1` |
| H1-B | `hematita:` | planned | `celestina-rs/crates/hematita-core/` | — | `/proc/stat`, `/proc/meminfo`, frequency and model parsers; samplers; the ring; captures | `cargo test -p hematita-core` | `VAL-H1` |
| H1-C | `hematita:` | planned | `hematita/src/`, `hematita/qml/`, `hematita/build.rs` | — | Sampler thread, snapshot, `HematitaResources`, activation, Performance page with graph | `scripts/verify-production.sh` | `VAL-H1` |
| H1-Z | `hematita:` | planned | `hematita/`, `docs/version-history.tsv` | — | Implementation exit, 0.2.0, status and roadmap closed, plan archived | `scripts/complete-production.sh` | `VAL-H1` |
```

`hematita/docs/evidence/2026-09-21-h1-skeleton.md`:

```markdown
# The skeleton builds and opens the window with the strip — H1-A

- **Date:** 2026-09-21
- **Scope:** `H1-A` of
  [`../plans/active/2026-09-21-h1-foundation.md`](../plans/active/2026-09-21-h1-foundation.md):
  `celestina-rs/crates/hematita-core/` as a stub, the whole of `hematita/`,
  the app icon, and the registration rows
- **Environment:** Rust 1.97.1 (pinned), cxx-qt 0.9.1, the author's Qt 6
  development environment
- **Artifact:** `hematita/target/release/hematita`, not installed

## Procedure

```sh
(cd celestina-rs && cargo test -p hematita-core && cargo clippy --locked -p hematita-core --all-targets -- -D warnings)
(cd hematita && cargo build --release --locked --bin hematita && cargo clippy --all-targets --locked -- -D warnings && cargo fmt --all --check)
hematita/scripts/smoke.sh
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
```

## Result

- **Exit:** 0 for every command
- **Observed:** FILL IN AFTER RUNNING — the crate's one test passes; the
  release binary starts offscreen and stays alive 8 s with no QML error; the
  guards report OK; the strip renders as one centred pill with four
  glyph-over-word items on the nested session

## Limits

- No graph yet; no sampling; nothing installed.
- Keyboard, screen reader and appearance are `VAL-H1`.

## Follow-up

`H1-B`.
```

Replace `FILL IN AFTER RUNNING` with the observed facts in Step 5 — the evidence must not be committed with that text.

- [ ] **Step 3: Register the project**

Append to `docs/projects.toml` after the `fluorita` project (end of file):

```toml

[[projects]]
id = "hematita"
name = "Hematita"
path = "hematita"
kind = "cxx-qt-application"
commit_prefix = "hematita"
# Registered unversioned first so the registry commit carries no history row;
# the baseline commit that follows the skeleton declares the Cargo source.
versioned = false
agents = "hematita/AGENTS.md"
readme = "hematita/README.md"
status = "hematita/STATUS.md"
roadmap = "hematita/ROADMAP.md"
validation = "hematita/VALIDATION.md"
active_plans = "hematita/docs/plans/active"
context_documents = ["celestina-style/DESIGN.md", "docs/superpowers/specs/2026-09-21-hematita-design.md"]
source_roots = ["hematita/src", "hematita/qml", "celestina-rs/crates/hematita-core"]
commit_roots = ["hematita/", "celestina-rs/crates/hematita-core/"]
include_workspace_manifests = true
production_role = "desktop-application"
deployable = true
build_script = "hematita/scripts/build-production.sh"
complete_script = "hematita/scripts/complete-production.sh"
verify_script = "hematita/scripts/verify-production.sh"
deploy_script = "hematita/scripts/deploy-production.sh"
status_script = "hematita/scripts/status-production.sh"
artifact_manifest = "hematita/target/production-artifact.toml"
artifact_paths = ["hematita/target/release/hematita"]
production_inputs = ["hematita/Cargo.toml", "hematita/Cargo.lock", "hematita/build.rs", "hematita/src", "hematita/qml", "hematita/org.celestina.Hematita.desktop", "celestina-style/icons/apps/org.celestina.Hematita.svg", "celestina-rs/crates/hematita-core"]
verification_inputs = ["hematita/scripts/smoke.sh", "celestina-rs/crates/hematita-core/tests", "celestina-rs/rust-toolchain.toml"]
component_commit_scopes = [
  { prefix = "hematita-core", roots = ["celestina-rs/crates/hematita-core/"], include_workspace_manifests = true },
]
```

In `README.md`, add after the `fluorita` row of the project table:

```markdown
| [hematita](hematita/) | Resource monitor: performance, processes, sensors | Rust · CXX-Qt · QML |
```

In `scripts/qmllint-baseline.tsv`, append (the count is filled in Step 5):

```text
0	hematita
```

- [ ] **Step 4: Run the guards and the full verification**

```bash
bash scripts/check-architecture-contract.sh
bash scripts/check-documentation-contract.sh
python3 scripts/check-language-contract.py
hematita/scripts/build-production.sh
hematita/scripts/verify-production.sh
```
Expected: architecture, documentation and language OK; build seals; verify runs `test-production-artifacts.sh`, the architecture guard, fmt, clippy and tests in both `hematita/` and `celestina-rs`, then `qmllint-cxxqt.sh hematita` and `smoke.sh`.

If qmllint reports `hematita has no row` or `warnings grew from 0 to N`, set the row in `scripts/qmllint-baseline.tsv` to the printed `N` (a fresh application starts at its real count) and rerun verify. If the language guard flags a file, fix the wording; do not add a baseline row.

- [ ] **Step 5: Fill the evidence and compute the H1-A inventory**

Update `hematita/docs/evidence/2026-09-21-h1-skeleton.md`'s `Observed` line with the real outputs (the qmllint count, the test count, the smoke line).

Then compute the inventory. The registry commit must land first (it authorises the `hematita:` prefix), so the base of H1-A is that commit. Prepare both:

```bash
# Registry commit — only when the author has requested a commit.
git add docs/projects.toml README.md scripts/qmllint-baseline.tsv celestina-rs/Cargo.toml celestina-rs/Cargo.lock
python3 scripts/version_tool.py check
git commit -m "$(cat <<'EOF'
suite-maintenance: Register the Hematita project and its core crate

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

```bash
# H1-A inventory, computed against the new HEAD.
python3 "$SCRATCH/mkinv.py" H1-A \
  hematita/docs/inventories/2026-09-21-h1-foundation/H1-A.numstat.tsv \
  hematita/docs/plans/active/2026-09-21-h1-foundation.md \
  celestina-rs/crates/hematita-core/Cargo.toml \
  celestina-rs/crates/hematita-core/src/lib.rs \
  celestina-rs/crates/hematita-core/src/ratio.rs \
  celestina-style/icons/apps/org.celestina.Hematita.svg \
  $(git ls-files --others --exclude-standard hematita/)
python3 scripts/check-staged-units.py hematita/docs/inventories/2026-09-21-h1-foundation/H1-A.numstat.tsv
bash scripts/check-documentation-contract.sh
```

Expected: the generator prints `H1-A: N files, +X/-0`, the staged-units guard prints OK, and the ledger's H1-A row now carries that diffstat.

- [ ] **Step 6: Commit H1-A (only on the author's request)**

```bash
git commit -m "$(cat <<'EOF'
hematita-maintenance: Add the application skeleton with the crate stub, the strip and the document set

H1-A: hematita-core as a stub with the shared percentage; the CXX-Qt
application opening the window with the centred pill navigation strip;
production scripts, smoke and the project documents; the H1 plan.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

- [ ] **Step 7: The baseline commit (only on the author's request)**

In `docs/projects.toml`, replace the two comment lines and `versioned = false` under `id = "hematita"` with:

```toml
version_source = { kind = "cargo-package", path = "hematita/Cargo.toml", package = "hematita" }
version_mirrors = [
  { kind = "cargo-lock", path = "hematita/Cargo.lock", package = "hematita" },
]
```

Append to `docs/version-history.tsv`:

```text
hematita	0.1.0	baseline	H1-A	Adopt the Hematita Cargo version as the verified baseline
```

```bash
python3 scripts/version_tool.py check
git add docs/projects.toml docs/version-history.tsv
git commit -m "$(cat <<'EOF'
suite-maintenance: Adopt the Hematita Cargo version as the verified baseline

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: `hematita-core::cpu`

**Files:**
- Create: `celestina-rs/crates/hematita-core/src/cpu.rs`
- Modify: `celestina-rs/crates/hematita-core/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub struct CpuTicks { pub idle: u64, pub total: u64 }`
  - `pub struct CpuStat { pub aggregate: CpuTicks, pub cores: Vec<CpuTicks> }`
  - `pub fn parse_stat(stat: &str) -> Result<CpuStat, CpuError>`
  - `pub struct CpuSampler` with `new()`, `sample(&mut self, stat: &CpuStat) -> Result<Option<CpuSample>, CpuError>`, `reset(&mut self)`
  - `pub struct CpuSample { pub aggregate_percent: u8, pub core_percents: Vec<u8> }`
  - `pub fn parse_frequency_khz(text: &str) -> Result<u64, CpuError>`
  - `pub fn parse_model(cpuinfo: &str) -> Option<String>`
  - `pub enum CpuError { NoAggregateLine, TooFewFields { line: String }, UnreadableNumber { line: String }, NoElapsedTime, CoreCountChanged { before: usize, after: usize } }`

- [ ] **Step 1: Write the failing tests**

```rust
// celestina-rs/crates/hematita-core/src/cpu.rs
//! CPU, as `/proc/stat` and `cpufreq` report it.
//!
//! CPU is a rate, not a reading: `/proc/stat` counts ticks since boot, so a
//! percentage only exists between two samples. [`CpuSampler`] holds the
//! previous reading and refuses to invent a number for the first.

use std::fmt;

use crate::ratio::percent_of;

/// One reading of a `cpu` line: ticks spent idle, and ticks spent at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuTicks {
    pub idle: u64,
    pub total: u64,
}

/// The aggregate line and every `cpuN` line, in kernel order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CpuStat {
    pub aggregate: CpuTicks,
    pub cores: Vec<CpuTicks>,
}

/// Busy percentages between two readings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CpuSample {
    pub aggregate_percent: u8,
    pub core_percents: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CpuError {
    NoAggregateLine,
    TooFewFields { line: String },
    UnreadableNumber { line: String },
    /// Two samples that are not apart in time say nothing about a rate.
    NoElapsedTime,
    /// The kernel hot-plugged a core between two readings; the rate restarts.
    CoreCountChanged { before: usize, after: usize },
}

impl fmt::Display for CpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoAggregateLine => write!(formatter, "/proc/stat has no aggregate cpu line"),
            Self::TooFewFields { line } => write!(formatter, "/proc/stat line is too short: {line}"),
            Self::UnreadableNumber { line } => {
                write!(formatter, "/proc/stat carries an unreadable number: {line}")
            }
            Self::NoElapsedTime => write!(formatter, "two cpu samples with no time between them"),
            Self::CoreCountChanged { before, after } => {
                write!(formatter, "core count changed from {before} to {after} between samples")
            }
        }
    }
}

impl std::error::Error for CpuError {}

/// Parses the aggregate `cpu` line and every `cpuN` line of `/proc/stat`.
///
/// # Errors
///
/// Refuses a file with no aggregate line, a line too short to carry idle and
/// iowait, or any field that is not a number.
pub fn parse_stat(stat: &str) -> Result<CpuStat, CpuError> {
    todo!()
}

/// Turns successive `/proc/stat` readings into busy percentages.
#[derive(Debug, Default)]
pub struct CpuSampler {
    previous: Option<CpuStat>,
}

impl CpuSampler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The busy percentages since the previous sample, or `None` for the
    /// first one.
    ///
    /// # Errors
    ///
    /// [`CpuError::NoElapsedTime`] when no aggregate tick passed;
    /// [`CpuError::CoreCountChanged`] when the core list changed length, in
    /// which case the new reading becomes the baseline.
    pub fn sample(&mut self, stat: &CpuStat) -> Result<Option<CpuSample>, CpuError> {
        todo!()
    }

    /// Forgets the previous reading, so the next sample starts a fresh rate.
    pub fn reset(&mut self) {
        self.previous = None;
    }
}

/// Parses a `cpufreq/scaling_cur_freq` file: one integer in kHz.
///
/// # Errors
///
/// [`CpuError::UnreadableNumber`] when the text is not one integer.
pub fn parse_frequency_khz(text: &str) -> Result<u64, CpuError> {
    todo!()
}

/// The first `model name` of `/proc/cpuinfo`, trimmed; `None` when absent.
#[must_use]
pub fn parse_model(cpuinfo: &str) -> Option<String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = "cpu  100 20 50 800 30 0 0 0 0 0\ncpu0 50 10 25 400 15 0 0 0 0 0\ncpu1 50 10 25 400 15 0 0 0 0 0\nintr 1\nctxt 2\n";

    fn stat(aggregate: (u64, u64), cores: &[(u64, u64)]) -> CpuStat {
        CpuStat {
            aggregate: CpuTicks { idle: aggregate.0, total: aggregate.1 },
            cores: cores.iter().map(|&(idle, total)| CpuTicks { idle, total }).collect(),
        }
    }

    #[test]
    fn the_aggregate_line_counts_idle_and_iowait_as_idle() {
        let parsed = parse_stat(STAT).expect("a readable stat");
        assert_eq!(parsed.aggregate, CpuTicks { idle: 830, total: 1000 });
        assert_eq!(parsed.cores.len(), 2);
        assert_eq!(parsed.cores[1], CpuTicks { idle: 415, total: 500 });
    }

    #[test]
    fn an_unreadable_stat_is_refused_rather_than_guessed() {
        assert_eq!(parse_stat("intr 1\n"), Err(CpuError::NoAggregateLine));
        assert_eq!(
            parse_stat("cpu  1 2 3\n"),
            Err(CpuError::TooFewFields { line: "cpu  1 2 3".to_owned() })
        );
        assert_eq!(
            parse_stat("cpu  1 2 3 four 5\n"),
            Err(CpuError::UnreadableNumber { line: "cpu  1 2 3 four 5".to_owned() })
        );
        // A broken core line is refused too; the aggregate does not excuse it.
        assert!(matches!(
            parse_stat("cpu  1 2 3 4 5\ncpu0 1 2\n"),
            Err(CpuError::TooFewFields { .. })
        ));
    }

    #[test]
    fn the_first_sample_reports_nothing_because_a_rate_needs_two() {
        let mut sampler = CpuSampler::new();
        assert_eq!(sampler.sample(&stat((830, 1000), &[(415, 500), (415, 500)])), Ok(None));
        // Half the new aggregate ticks were busy; core 0 was fully busy, core 1 idle.
        let sample = sampler
            .sample(&stat((880, 1100), &[(415, 550), (465, 550)]))
            .expect("second sample")
            .expect("a rate");
        assert_eq!(sample.aggregate_percent, 50);
        assert_eq!(sample.core_percents, vec![100, 0]);
    }

    #[test]
    fn two_samples_with_no_time_between_them_report_no_rate() {
        let mut sampler = CpuSampler::new();
        let reading = stat((830, 1000), &[(415, 500)]);
        sampler.sample(&reading).expect("first sample");
        assert_eq!(sampler.sample(&reading), Err(CpuError::NoElapsedTime));
    }

    #[test]
    fn a_changed_core_count_restarts_the_rate_from_the_new_reading() {
        let mut sampler = CpuSampler::new();
        sampler.sample(&stat((830, 1000), &[(415, 500)])).expect("first");
        assert_eq!(
            sampler.sample(&stat((880, 1100), &[(415, 550), (465, 550)])),
            Err(CpuError::CoreCountChanged { before: 1, after: 2 })
        );
        // The next reading measures against the two-core one, not the old.
        let sample = sampler
            .sample(&stat((930, 1200), &[(415, 600), (515, 600)]))
            .expect("third")
            .expect("a rate");
        assert_eq!(sample.core_percents, vec![100, 0]);
    }

    #[test]
    fn counters_that_go_backwards_saturate_instead_of_wrapping() {
        let mut sampler = CpuSampler::new();
        sampler.sample(&stat((830, 1000), &[])).expect("first");
        // idle moved backwards; busy cannot exceed the total.
        let sample = sampler.sample(&stat((800, 1100), &[])).expect("second").expect("rate");
        assert_eq!(sample.aggregate_percent, 100);
    }

    #[test]
    fn a_reset_sampler_measures_from_the_next_reading_only() {
        let mut sampler = CpuSampler::new();
        sampler.sample(&stat((830, 1000), &[])).expect("first");
        sampler.reset();
        assert_eq!(sampler.sample(&stat((880, 1100), &[])), Ok(None));
    }

    #[test]
    fn frequency_is_one_integer_in_khz() {
        assert_eq!(parse_frequency_khz("4658104\n"), Ok(4_658_104));
        assert!(matches!(parse_frequency_khz("fast\n"), Err(CpuError::UnreadableNumber { .. })));
        assert!(matches!(parse_frequency_khz(""), Err(CpuError::UnreadableNumber { .. })));
    }

    #[test]
    fn the_model_is_the_first_model_name_trimmed() {
        let cpuinfo = "processor\t: 0\nmodel name\t: AMD Ryzen 7 9800X3D 8-Core Processor\nprocessor\t: 1\nmodel name\t: other\n";
        assert_eq!(parse_model(cpuinfo).as_deref(), Some("AMD Ryzen 7 9800X3D 8-Core Processor"));
        assert_eq!(parse_model("processor : 0\n"), None);
    }
}
```

Add `pub mod cpu;` to `lib.rs` after `pub mod ratio;`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd celestina-rs && cargo test -p hematita-core cpu`
Expected: every `cpu::tests` test panics with `not yet implemented`.

- [ ] **Step 3: Implement**

Replace the three `todo!()` bodies:

```rust
pub fn parse_stat(stat: &str) -> Result<CpuStat, CpuError> {
    let mut aggregate = None;
    let mut cores = Vec::new();
    for line in stat.lines() {
        if let Some(rest) = line.strip_prefix("cpu") {
            let is_aggregate = rest.starts_with(' ');
            let is_core = rest.starts_with(|c: char| c.is_ascii_digit());
            if !is_aggregate && !is_core {
                continue;
            }
            let ticks = parse_ticks(line)?;
            if is_aggregate {
                aggregate = Some(ticks);
            } else {
                cores.push(ticks);
            }
        }
    }
    let aggregate = aggregate.ok_or(CpuError::NoAggregateLine)?;
    Ok(CpuStat { aggregate, cores })
}

/// user, nice, system, idle, iowait — everything after is optional and still
/// counts toward the total, which keeps this correct on a kernel that adds a
/// column.
fn parse_ticks(line: &str) -> Result<CpuTicks, CpuError> {
    let mut ticks = Vec::with_capacity(10);
    for field in line.split_whitespace().skip(1) {
        let value = field
            .parse::<u64>()
            .map_err(|_| CpuError::UnreadableNumber { line: line.to_owned() })?;
        ticks.push(value);
    }
    if ticks.len() < 5 {
        return Err(CpuError::TooFewFields { line: line.to_owned() });
    }
    Ok(CpuTicks {
        idle: ticks[3].saturating_add(ticks[4]),
        total: ticks.iter().fold(0u64, |sum, tick| sum.saturating_add(*tick)),
    })
}

fn busy_percent(previous: CpuTicks, current: CpuTicks) -> Option<u8> {
    let total = current.total.saturating_sub(previous.total);
    if total == 0 {
        return None;
    }
    let idle = current.idle.saturating_sub(previous.idle).min(total);
    Some(percent_of(total - idle, total))
}
```

```rust
    pub fn sample(&mut self, stat: &CpuStat) -> Result<Option<CpuSample>, CpuError> {
        let Some(previous) = self.previous.replace(stat.clone()) else {
            return Ok(None);
        };
        if previous.cores.len() != stat.cores.len() {
            return Err(CpuError::CoreCountChanged {
                before: previous.cores.len(),
                after: stat.cores.len(),
            });
        }
        let aggregate_percent =
            busy_percent(previous.aggregate, stat.aggregate).ok_or(CpuError::NoElapsedTime)?;
        // A core with no ticks between samples is a core that did nothing.
        let core_percents = previous
            .cores
            .iter()
            .zip(&stat.cores)
            .map(|(before, after)| busy_percent(*before, *after).unwrap_or(0))
            .collect();
        Ok(Some(CpuSample { aggregate_percent, core_percents }))
    }
```

```rust
pub fn parse_frequency_khz(text: &str) -> Result<u64, CpuError> {
    text.trim()
        .parse::<u64>()
        .map_err(|_| CpuError::UnreadableNumber { line: text.trim().to_owned() })
}

pub fn parse_model(cpuinfo: &str) -> Option<String> {
    cpuinfo
        .lines()
        .find_map(|line| line.strip_prefix("model name"))
        .and_then(|rest| rest.split_once(':'))
        .map(|(_, model)| model.trim().to_owned())
        .filter(|model| !model.is_empty())
}
```

- [ ] **Step 4: Run tests, fmt and clippy**

Run: `cd celestina-rs && cargo test -p hematita-core && cargo fmt --all --check && cargo clippy --locked -p hematita-core --all-targets -- -D warnings`
Expected: all `cpu` tests pass; clean.

---

### Task 6: `hematita-core::memory`

**Files:**
- Create: `celestina-rs/crates/hematita-core/src/memory.rs`
- Modify: `celestina-rs/crates/hematita-core/src/lib.rs`

**Interfaces:**
- Produces: `pub struct Memory { pub used_kib, pub total_kib, pub swap_used_kib, pub swap_total_kib: u64 }` with `used_percent() -> u8` and `swap_percent() -> u8`; `pub fn parse_meminfo(text: &str) -> Result<Memory, MemoryError>`; `pub enum MemoryError { MissingField(&'static str), UnreadableNumber { field: &'static str } }`.

- [ ] **Step 1: Write the failing tests**

```rust
// celestina-rs/crates/hematita-core/src/memory.rs
//! Memory and swap, as `/proc/meminfo` reports them.
//!
//! Used memory is total minus *available*, not minus free: `MemAvailable`
//! already accounts for reclaimable cache, and reporting cache as used is the
//! classic way to tell someone their memory is full when it is not.

use std::fmt;

use crate::ratio::percent_of;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Memory {
    pub used_kib: u64,
    pub total_kib: u64,
    pub swap_used_kib: u64,
    pub swap_total_kib: u64,
}

impl Memory {
    #[must_use]
    pub fn used_percent(&self) -> u8 {
        percent_of(self.used_kib, self.total_kib)
    }

    #[must_use]
    pub fn swap_percent(&self) -> u8 {
        percent_of(self.swap_used_kib, self.swap_total_kib)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemoryError {
    MissingField(&'static str),
    UnreadableNumber { field: &'static str },
}

impl fmt::Display for MemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(field) => write!(formatter, "/proc/meminfo has no {field}"),
            Self::UnreadableNumber { field } => {
                write!(formatter, "/proc/meminfo carries an unreadable {field}")
            }
        }
    }
}

impl std::error::Error for MemoryError {}

/// Parses `/proc/meminfo`.
///
/// # Errors
///
/// Refuses a file missing `MemTotal`, `MemAvailable`, `SwapTotal` or
/// `SwapFree`, or carrying one that is not a number.
pub fn parse_meminfo(text: &str) -> Result<Memory, MemoryError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMINFO: &str = "MemTotal:       16000 kB\nMemFree:         1000 kB\nMemAvailable:    8000 kB\nSwapTotal:       4000 kB\nSwapFree:        3000 kB\n";

    #[test]
    fn used_memory_leaves_reclaimable_cache_out_of_it() {
        let memory = parse_meminfo(MEMINFO).expect("readable meminfo");
        assert_eq!(memory.used_kib, 8000);
        assert_eq!(memory.total_kib, 16000);
        assert_eq!(memory.used_percent(), 50);
        assert_eq!(memory.swap_used_kib, 1000);
        assert_eq!(memory.swap_total_kib, 4000);
        assert_eq!(memory.swap_percent(), 25);
    }

    #[test]
    fn meminfo_without_the_fields_it_needs_is_refused() {
        assert_eq!(parse_meminfo("MemFree: 1000 kB\n"), Err(MemoryError::MissingField("MemTotal")));
        assert_eq!(
            parse_meminfo("MemTotal: 16000 kB\n"),
            Err(MemoryError::MissingField("MemAvailable"))
        );
        // A prefix is not a field: `MemTotalish` must not answer for MemTotal.
        assert_eq!(
            parse_meminfo("MemTotalish: 1 kB\nMemAvailable: 1 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n"),
            Err(MemoryError::MissingField("MemTotal"))
        );
        assert_eq!(
            parse_meminfo("MemTotal: lots kB\nMemAvailable: 1 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n"),
            Err(MemoryError::UnreadableNumber { field: "MemTotal" })
        );
    }

    #[test]
    fn a_machine_without_swap_reports_zero_rather_than_dividing_by_it() {
        let memory = parse_meminfo("MemTotal: 10 kB\nMemAvailable: 5 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n")
            .expect("readable");
        assert_eq!(memory.swap_percent(), 0);
    }

    #[test]
    fn available_above_total_saturates_used_at_zero() {
        let memory = parse_meminfo("MemTotal: 10 kB\nMemAvailable: 50 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n")
            .expect("readable");
        assert_eq!(memory.used_kib, 0);
    }
}
```

Add `pub mod memory;` to `lib.rs`.

- [ ] **Step 2: Run to verify failure**

Run: `cd celestina-rs && cargo test -p hematita-core memory`
Expected: the four tests panic with `not yet implemented`.

- [ ] **Step 3: Implement**

```rust
pub fn parse_meminfo(text: &str) -> Result<Memory, MemoryError> {
    let field = |name: &'static str| -> Result<u64, MemoryError> {
        let line = text
            .lines()
            .find(|line| line.strip_prefix(name).is_some_and(|rest| rest.starts_with(':')))
            .ok_or(MemoryError::MissingField(name))?;
        line.split_whitespace()
            .nth(1)
            .ok_or(MemoryError::MissingField(name))?
            .parse::<u64>()
            .map_err(|_| MemoryError::UnreadableNumber { field: name })
    };

    let total = field("MemTotal")?;
    let available = field("MemAvailable")?;
    let swap_total = field("SwapTotal")?;
    let swap_free = field("SwapFree")?;
    Ok(Memory {
        used_kib: total.saturating_sub(available),
        total_kib: total,
        swap_used_kib: swap_total.saturating_sub(swap_free),
        swap_total_kib: swap_total,
    })
}
```

- [ ] **Step 4: Run tests, fmt and clippy**

Run: `cd celestina-rs && cargo test -p hematita-core && cargo fmt --all --check && cargo clippy --locked -p hematita-core --all-targets -- -D warnings`
Expected: pass; clean.

---

### Task 7: `hematita-core::history` and the captures

**Files:**
- Create: `celestina-rs/crates/hematita-core/src/history.rs`
- Create: `celestina-rs/crates/hematita-core/tests/fixtures/proc-stat.txt`, `proc-meminfo.txt`, `proc-cpuinfo.txt`
- Create: `celestina-rs/crates/hematita-core/tests/captures.rs`
- Modify: `celestina-rs/crates/hematita-core/src/lib.rs`

**Interfaces:**
- Produces: `pub const HISTORY_SAMPLES: usize = 60;` `pub struct Ring` with `new()`, `push(&mut self, value: f32)`, `values(&self) -> Vec<f32>` (oldest first, exactly `HISTORY_SAMPLES` long, zero-padded at the front), `latest(&self) -> Option<f32>`.

- [ ] **Step 1: Write the failing ring tests**

```rust
// celestina-rs/crates/hematita-core/src/history.rs
//! The last minute of a value.
//!
//! The graph always shows a full minute: a ring that is not yet full answers
//! zeros for the seconds it has not lived, so the trace starts flat at the
//! left edge and grows to the right instead of stretching to fit.

/// How many one-second samples a graph shows. The one place this number lives.
pub const HISTORY_SAMPLES: usize = 60;

#[derive(Clone, Debug, PartialEq)]
pub struct Ring {
    samples: [f32; HISTORY_SAMPLES],
    next: usize,
    filled: usize,
}

impl Default for Ring {
    fn default() -> Self {
        Self::new()
    }
}

impl Ring {
    #[must_use]
    pub fn new() -> Self {
        Self { samples: [0.0; HISTORY_SAMPLES], next: 0, filled: 0 }
    }

    pub fn push(&mut self, value: f32) {
        todo!()
    }

    /// Oldest first, always `HISTORY_SAMPLES` long.
    #[must_use]
    pub fn values(&self) -> Vec<f32> {
        todo!()
    }

    #[must_use]
    pub fn latest(&self) -> Option<f32> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_ring_is_a_flat_minute() {
        let ring = Ring::new();
        assert_eq!(ring.values(), vec![0.0; HISTORY_SAMPLES]);
        assert_eq!(ring.latest(), None);
    }

    #[test]
    fn samples_arrive_at_the_right_edge_and_age_leftwards() {
        let mut ring = Ring::new();
        ring.push(0.5);
        ring.push(1.0);
        let values = ring.values();
        assert_eq!(values.len(), HISTORY_SAMPLES);
        assert_eq!(values[HISTORY_SAMPLES - 2], 0.5);
        assert_eq!(values[HISTORY_SAMPLES - 1], 1.0);
        assert_eq!(values[0], 0.0);
        assert_eq!(ring.latest(), Some(1.0));
    }

    #[test]
    fn a_full_ring_forgets_the_oldest_sample() {
        let mut ring = Ring::new();
        for index in 0..=HISTORY_SAMPLES {
            ring.push(index as f32);
        }
        let values = ring.values();
        assert_eq!(values[0], 1.0);
        assert_eq!(values[HISTORY_SAMPLES - 1], HISTORY_SAMPLES as f32);
    }
}
```

Add `pub mod history;` to `lib.rs`.

- [ ] **Step 2: Run to verify failure**

Run: `cd celestina-rs && cargo test -p hematita-core history`
Expected: three failures with `not yet implemented`.

- [ ] **Step 3: Implement**

```rust
    pub fn push(&mut self, value: f32) {
        self.samples[self.next] = value;
        self.next = (self.next + 1) % HISTORY_SAMPLES;
        self.filled = (self.filled + 1).min(HISTORY_SAMPLES);
    }

    pub fn values(&self) -> Vec<f32> {
        let mut out = vec![0.0; HISTORY_SAMPLES];
        let start = HISTORY_SAMPLES - self.filled;
        for (offset, slot) in out[start..].iter_mut().enumerate() {
            let index = (self.next + HISTORY_SAMPLES - self.filled + offset) % HISTORY_SAMPLES;
            *slot = self.samples[index];
        }
        out
    }

    pub fn latest(&self) -> Option<f32> {
        (self.filled > 0).then(|| self.samples[(self.next + HISTORY_SAMPLES - 1) % HISTORY_SAMPLES])
    }
```

- [ ] **Step 4: Capture the fixtures and write the capture tests**

```bash
mkdir -p celestina-rs/crates/hematita-core/tests/fixtures
cat /proc/stat > celestina-rs/crates/hematita-core/tests/fixtures/proc-stat.txt
cat /proc/meminfo > celestina-rs/crates/hematita-core/tests/fixtures/proc-meminfo.txt
cat /proc/cpuinfo > celestina-rs/crates/hematita-core/tests/fixtures/proc-cpuinfo.txt
grep -c '^cpu[0-9]' celestina-rs/crates/hematita-core/tests/fixtures/proc-stat.txt
grep -m1 'MemTotal' celestina-rs/crates/hematita-core/tests/fixtures/proc-meminfo.txt
```

Note the printed core count (`CORES`) and `MemTotal` value (`TOTAL_KIB`), then write:

```rust
// celestina-rs/crates/hematita-core/tests/captures.rs
//! The parsers against text captured from the author's machine (an AMD Ryzen
//! 7 9800X3D, 2026-09-21). Inline fixtures prove arithmetic; these prove the
//! real files have the shape the parsers expect.

use hematita_core::cpu::{parse_model, parse_stat};
use hematita_core::memory::parse_meminfo;

const STAT: &str = include_str!("fixtures/proc-stat.txt");
const MEMINFO: &str = include_str!("fixtures/proc-meminfo.txt");
const CPUINFO: &str = include_str!("fixtures/proc-cpuinfo.txt");

#[test]
fn the_captured_stat_has_the_aggregate_and_every_core() {
    let stat = parse_stat(STAT).expect("the captured /proc/stat parses");
    assert_eq!(stat.cores.len(), CORES);
    assert!(stat.aggregate.total >= stat.aggregate.idle);
    for core in &stat.cores {
        assert!(core.total >= core.idle);
        assert!(core.total <= stat.aggregate.total);
    }
}

#[test]
fn the_captured_meminfo_has_memory_and_swap() {
    let memory = parse_meminfo(MEMINFO).expect("the captured /proc/meminfo parses");
    assert_eq!(memory.total_kib, TOTAL_KIB);
    assert!(memory.used_kib <= memory.total_kib);
    assert!(memory.swap_used_kib <= memory.swap_total_kib);
}

#[test]
fn the_captured_cpuinfo_names_the_processor() {
    assert_eq!(parse_model(CPUINFO).as_deref(), Some("AMD Ryzen 7 9800X3D 8-Core Processor"));
}
```

Replace `CORES` and `TOTAL_KIB` with the two literals you noted (for example `8` and `65_000_000`).

- [ ] **Step 5: Run the whole crate**

Run: `cd celestina-rs && cargo test -p hematita-core && cargo fmt --all --check && cargo clippy --locked -p hematita-core --all-targets -- -D warnings`
Expected: unit tests and the three capture tests pass; clean.

- [ ] **Step 6: Close H1-B**

Write `hematita/docs/evidence/2026-09-21-h1-core.md` from the evidence template with the exact commands above, the test count printed, and `Limits: no consumer yet; per-core percentages are computed but not shown until H2`. Update the ledger row H1-B to `done` with the evidence link, set the roadmap's H1-B row to `done`, update `STATUS.md`'s `Updated` date and implementation line, then:

```bash
python3 "$SCRATCH/mkinv.py" H1-B \
  hematita/docs/inventories/2026-09-21-h1-foundation/H1-B.numstat.tsv \
  hematita/docs/plans/active/2026-09-21-h1-foundation.md \
  hematita/docs/evidence/2026-09-21-h1-core.md hematita/ROADMAP.md hematita/STATUS.md \
  celestina-rs/crates/hematita-core/src/lib.rs celestina-rs/crates/hematita-core/src/cpu.rs \
  celestina-rs/crates/hematita-core/src/memory.rs celestina-rs/crates/hematita-core/src/history.rs \
  celestina-rs/crates/hematita-core/tests/captures.rs \
  celestina-rs/crates/hematita-core/tests/fixtures/proc-stat.txt \
  celestina-rs/crates/hematita-core/tests/fixtures/proc-meminfo.txt \
  celestina-rs/crates/hematita-core/tests/fixtures/proc-cpuinfo.txt
python3 scripts/check-staged-units.py hematita/docs/inventories/2026-09-21-h1-foundation/H1-B.numstat.tsv
bash scripts/check-documentation-contract.sh && python3 scripts/check-language-contract.py
```

Commit (only on request):

```bash
git commit -m "$(cat <<'EOF'
hematita-maintenance: Add the CPU, memory and history parsers with their captures

H1-B: /proc/stat aggregate and per-core ticks with a sampler that refuses a
first rate, /proc/meminfo with used as total minus available, cpufreq and
model parsing, the sixty-sample ring, and captures from the author's machine.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

### Task 8: The sampler and `HematitaResources`

**Files:**
- Create: `hematita/src/sampler.rs`, `hematita/src/resources.rs`
- Modify: `hematita/src/main.rs` (`mod` lines), `hematita/build.rs` (`.files([...])`)

**Interfaces:**
- Produces QML type `HematitaResources` (from `org.celestina.hematita`) with properties `available: bool`, `unavailableReason: QString`, `generation: i32`, `cpuPercent: i32`, `cpuLoad: QString`, `cpuModel: QString`, `cpuFrequencyMhz: i32`, `cpuCores: i32`, `cpuHistory: QList<f64>` (60 values in 0..=1, oldest first), `memoryPercent: i32`, `memoryLoad: QString`, `memoryUsedKib: f64`, `memoryTotalKib: f64`, `memoryHistory: QList<f64>`, `swapUsedKib: f64`, `swapTotalKib: f64`, and invokable `start()`.
- Consumes: `hematita_core::{cpu, memory, history}` from Tasks 5–7.

- [ ] **Step 1: Write `sampler.rs`**

```rust
// hematita/src/sampler.rs
//! The thread that reads the machine, and what it hands the window.
//!
//! One thread, one second, one immutable [`Snapshot`]. Every source is read
//! and parsed here, off the Qt thread; the snapshot crosses to Qt by value and
//! is applied whole, so the window never shows one second's CPU beside another
//! second's memory. A source that cannot be read leaves its section
//! [`Section::Unavailable`] with the reason while the others keep publishing.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use hematita_core::cpu::{self, CpuSampler};
use hematita_core::memory::{self, Memory};

/// A number nobody stares at, and rare enough that the monitor is not a reason
/// the machine is busy. The one place the cadence lives.
pub const INTERVAL: Duration = Duration::from_secs(1);

const STAT_PATH: &str = "/proc/stat";
const MEMINFO_PATH: &str = "/proc/meminfo";
const CPUINFO_PATH: &str = "/proc/cpuinfo";
const FREQUENCY_PATH: &str = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq";

#[derive(Clone, Debug)]
pub enum Section<T> {
    Available(T),
    Unavailable(String),
}

#[derive(Clone, Debug)]
pub struct CpuReading {
    pub aggregate_percent: u8,
    pub core_percents: Vec<u8>,
    pub frequency_mhz: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub generation: u64,
    /// `None` on the first sample: a rate needs two readings.
    pub cpu: Section<Option<CpuReading>>,
    pub memory: Section<Memory>,
}

/// Facts that do not change while the machine is up, read once.
#[derive(Clone, Debug, Default)]
pub struct Identity {
    pub cpu_model: String,
    pub cpu_cores: usize,
}

/// Reads the identity once, tolerating absence.
#[must_use]
pub fn read_identity() -> Identity {
    let cpu_model = read(CPUINFO_PATH)
        .ok()
        .and_then(|text| cpu::parse_model(&text))
        .unwrap_or_default();
    let cpu_cores = read(STAT_PATH)
        .ok()
        .and_then(|text| cpu::parse_stat(&text).ok())
        .map(|stat| stat.cores.len())
        .unwrap_or(0);
    Identity { cpu_model, cpu_cores }
}

fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(Path::new(path)).map_err(|error| format!("{path}: {error}"))
}

/// Owns the thread; dropping it asks the thread to stop and waits for it.
pub struct Sampler {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Sampler {
    /// Starts sampling; `publish` runs on the sampler thread with each
    /// snapshot and is expected to queue it onto the Qt thread.
    ///
    /// # Errors
    ///
    /// The OS refused to create the thread.
    pub fn spawn(
        publish: impl Fn(Snapshot) + Send + 'static,
    ) -> std::io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let handle = thread::Builder::new()
            .name("hematita-sampler".to_owned())
            .spawn(move || run(&stop_flag, &publish))?;
        Ok(Self { stop, handle: Some(handle) })
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            // The thread checks the flag once per interval; a join that
            // returns an error means it panicked, and there is nothing left to
            // do about that during shutdown.
            let _ = handle.join();
        }
    }
}

fn run(stop: &AtomicBool, publish: &dyn Fn(Snapshot)) {
    let mut cpu_sampler = CpuSampler::new();
    let mut generation = 0u64;
    while !stop.load(Ordering::Relaxed) {
        generation += 1;
        publish(Snapshot {
            generation,
            cpu: sample_cpu(&mut cpu_sampler),
            memory: sample_memory(),
        });
        // Sleep in short slices so a close does not wait a whole interval.
        let mut slept = Duration::ZERO;
        while slept < INTERVAL && !stop.load(Ordering::Relaxed) {
            let slice = Duration::from_millis(100);
            thread::sleep(slice);
            slept += slice;
        }
    }
}

fn sample_cpu(sampler: &mut CpuSampler) -> Section<Option<CpuReading>> {
    let stat = match read(STAT_PATH).and_then(|text| cpu::parse_stat(&text).map_err(|e| e.to_string())) {
        Ok(stat) => stat,
        Err(reason) => {
            // A machine whose counters went away is not a machine at 0 %.
            sampler.reset();
            return Section::Unavailable(reason);
        }
    };
    let sample = match sampler.sample(&stat) {
        Ok(sample) => sample,
        // A hot-plugged core restarts the rate; the next second answers.
        Err(cpu::CpuError::CoreCountChanged { .. }) => None,
        Err(error) => return Section::Unavailable(error.to_string()),
    };
    // Frequency is optional: a VM or a locked governor has no such file.
    let frequency_mhz = read(FREQUENCY_PATH)
        .ok()
        .and_then(|text| cpu::parse_frequency_khz(&text).ok())
        .and_then(|khz| u32::try_from(khz / 1000).ok());
    Section::Available(sample.map(|sample| CpuReading {
        aggregate_percent: sample.aggregate_percent,
        core_percents: sample.core_percents,
        frequency_mhz,
    }))
}

fn sample_memory() -> Section<Memory> {
    match read(MEMINFO_PATH).and_then(|text| memory::parse_meminfo(&text).map_err(|e| e.to_string())) {
        Ok(memory) => Section::Available(memory),
        Err(reason) => Section::Unavailable(reason),
    }
}
```

- [ ] **Step 2: Write the failing test for the load policy in `resources.rs`**

```rust
// hematita/src/resources.rs
//! The Performance page's state, as Qt properties.
//!
//! This object owns the only policy numbers in Hematita — what counts as
//! elevated and critical — and the two history rings. It receives whole
//! snapshots from the sampler on the Qt thread and republishes them as typed
//! properties; nothing here reads a file.

use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QString};

use hematita_core::history::Ring;

use crate::sampler::{self, Identity, Sampler, Section, Snapshot};

/// Above this a value is worth noticing; above [`CRITICAL_PERCENT`] it is
/// worth interrupting for. The page maps these to appearance; the numbers are
/// policy and live here, not in the theme.
pub const ELEVATED_PERCENT: u8 = 80;
pub const CRITICAL_PERCENT: u8 = 90;

/// The state name the theme colours by.
#[must_use]
pub fn load_name(percent: u8) -> &'static str {
    todo!()
}

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qlist.h");
        type QList_f64 = cxx_qt_lib::QList<f64>;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        // available / unavailableReason — the last snapshot had CPU and
        //   memory, or why it did not
        // generation — bumped per applied snapshot so QML can animate
        //   "a new sample arrived" on one signal
        // cpu* — aggregate percent (-1 before the first rate), load name,
        //   model, MHz (0 when unknown), core count, one minute of history
        //   as fractions 0..=1 oldest first
        // memory* / swap* — kibibytes as doubles (QML has no 64-bit int)
        #[qobject]
        #[qml_element]
        #[qproperty(bool, available)]
        #[qproperty(QString, unavailable_reason)]
        #[qproperty(i32, generation)]
        #[qproperty(i32, cpu_percent)]
        #[qproperty(QString, cpu_load)]
        #[qproperty(QString, cpu_model)]
        #[qproperty(i32, cpu_frequency_mhz)]
        #[qproperty(i32, cpu_cores)]
        #[qproperty(QList_f64, cpu_history)]
        #[qproperty(i32, memory_percent)]
        #[qproperty(QString, memory_load)]
        #[qproperty(f64, memory_used_kib)]
        #[qproperty(f64, memory_total_kib)]
        #[qproperty(QList_f64, memory_history)]
        #[qproperty(f64, swap_used_kib)]
        #[qproperty(f64, swap_total_kib)]
        type HematitaResources = super::HematitaResourcesRust;

        /// Starts the sampler, once. The window calls it when it is up.
        #[qinvokable]
        fn start(self: Pin<&mut HematitaResources>);
    }

    impl cxx_qt::Threading for HematitaResources {}
}

pub struct HematitaResourcesRust {
    available: bool,
    unavailable_reason: QString,
    generation: i32,
    cpu_percent: i32,
    cpu_load: QString,
    cpu_model: QString,
    cpu_frequency_mhz: i32,
    cpu_cores: i32,
    cpu_history: QList<f64>,
    memory_percent: i32,
    memory_load: QString,
    memory_used_kib: f64,
    memory_total_kib: f64,
    memory_history: QList<f64>,
    swap_used_kib: f64,
    swap_total_kib: f64,
    cpu_ring: Ring,
    memory_ring: Ring,
    last_generation: u64,
    sampler: Option<Sampler>,
}

impl Default for HematitaResourcesRust {
    fn default() -> Self {
        let identity: Identity = sampler::read_identity();
        Self {
            available: false,
            unavailable_reason: QString::default(),
            generation: 0,
            cpu_percent: -1,
            cpu_load: QString::from("normal"),
            cpu_model: QString::from(identity.cpu_model.as_str()),
            cpu_frequency_mhz: 0,
            cpu_cores: i32::try_from(identity.cpu_cores).unwrap_or(i32::MAX),
            cpu_history: ring_list(&Ring::new()),
            memory_percent: 0,
            memory_load: QString::from("normal"),
            memory_used_kib: 0.0,
            memory_total_kib: 0.0,
            memory_history: ring_list(&Ring::new()),
            swap_used_kib: 0.0,
            swap_total_kib: 0.0,
            cpu_ring: Ring::new(),
            memory_ring: Ring::new(),
            last_generation: 0,
            sampler: None,
        }
    }
}

fn ring_list(ring: &Ring) -> QList<f64> {
    let mut list = QList::<f64>::default();
    for value in ring.values() {
        list.append(f64::from(value));
    }
    list
}

impl qobject::HematitaResources {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().sampler.is_some() {
            return;
        }
        let qt = self.qt_thread();
        match Sampler::spawn(move |snapshot| {
            let _ = qt.queue(move |resources: Pin<&mut qobject::HematitaResources>| {
                resources.apply(snapshot);
            });
        }) {
            Ok(sampler) => self.as_mut().rust_mut().sampler = Some(sampler),
            Err(error) => {
                self.as_mut().set_available(false);
                self.as_mut().set_unavailable_reason(QString::from(
                    format!("No se pudo iniciar la lectura: {error}").as_str(),
                ));
            }
        }
    }

    /// Applies one whole snapshot. A snapshot older than the last applied one
    /// is dropped: the thread publishes in order, but the queue does not
    /// promise to.
    fn apply(mut self: Pin<&mut Self>, snapshot: Snapshot) {
        if snapshot.generation <= self.rust().last_generation {
            return;
        }
        self.as_mut().rust_mut().last_generation = snapshot.generation;

        let mut reasons = Vec::new();
        match &snapshot.cpu {
            Section::Available(Some(reading)) => {
                let percent = reading.aggregate_percent;
                self.as_mut().rust_mut().cpu_ring.push(f32::from(percent) / 100.0);
                let history = ring_list(&self.rust().cpu_ring);
                self.as_mut().set_cpu_percent(i32::from(percent));
                self.as_mut().set_cpu_load(QString::from(load_name(percent)));
                let mhz = reading.frequency_mhz.map_or(0, |mhz| i32::try_from(mhz).unwrap_or(i32::MAX));
                self.as_mut().set_cpu_frequency_mhz(mhz);
                self.as_mut().set_cpu_history(history);
            }
            Section::Available(None) => {}
            Section::Unavailable(reason) => reasons.push(reason.clone()),
        }
        match &snapshot.memory {
            Section::Available(memory) => {
                let percent = memory.used_percent();
                self.as_mut().rust_mut().memory_ring.push(f32::from(percent) / 100.0);
                let history = ring_list(&self.rust().memory_ring);
                self.as_mut().set_memory_percent(i32::from(percent));
                self.as_mut().set_memory_load(QString::from(load_name(percent)));
                self.as_mut().set_memory_used_kib(memory.used_kib as f64);
                self.as_mut().set_memory_total_kib(memory.total_kib as f64);
                self.as_mut().set_swap_used_kib(memory.swap_used_kib as f64);
                self.as_mut().set_swap_total_kib(memory.swap_total_kib as f64);
                self.as_mut().set_memory_history(history);
            }
            Section::Unavailable(reason) => reasons.push(reason.clone()),
        }

        let available = reasons.is_empty();
        self.as_mut().set_available(available);
        self.as_mut().set_unavailable_reason(QString::from(reasons.join("; ").as_str()));
        let generation = i32::try_from(snapshot.generation % i64::from(i32::MAX) as u64).unwrap_or(0);
        self.as_mut().set_generation(generation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_names_the_state_the_page_paints() {
        assert_eq!(load_name(0), "normal");
        assert_eq!(load_name(79), "normal");
        assert_eq!(load_name(ELEVATED_PERCENT), "elevated");
        assert_eq!(load_name(89), "elevated");
        assert_eq!(load_name(CRITICAL_PERCENT), "critical");
        assert_eq!(load_name(100), "critical");
    }
}
```

`u64 as f64` for kibibytes is an accepted lossless-enough conversion (memory sizes are far below 2^53); clippy's `cast_precision_loss` is pedantic and not enabled by the workspace lints. If clippy still objects, use `f64::from(u32::try_from(x).unwrap_or(u32::MAX))`-free form: `x as f64` with a `// kibibytes never reach 2^53` comment is the intended shape.

- [ ] **Step 3: Register modules and build files**

In `main.rs`, at the top:

```rust
mod resources;
mod sampler;
```

In `build.rs`, replace the final builder call with:

```rust
    CxxQtBuilder::new_qml_module(module)
        .qrc("qml/icons.qrc")
        .qrc("qml/fonts.qrc")
        .files(["src/resources.rs"])
        .build();
```

- [ ] **Step 4: Run the test to verify it fails, implement, rerun**

Run: `cd hematita && cargo test --locked load_names`
Expected: FAIL with `not yet implemented`.

Implement:

```rust
pub fn load_name(percent: u8) -> &'static str {
    if percent >= CRITICAL_PERCENT {
        "critical"
    } else if percent >= ELEVATED_PERCENT {
        "elevated"
    } else {
        "normal"
    }
}
```

Run: `cd hematita && cargo test --locked && cargo clippy --all-targets --locked -- -D warnings && cargo fmt --all --check`
Expected: PASS; clean. If the bridge refuses `QList_f64` as a property type, the fallback is `QVariantList` built from `QVariant::from(&value)` per sample: change the two `#[qproperty]` lines and `ring_list` accordingly and note the reason in the evidence.

---

### Task 9: The Performance page

**Files:**
- Create: `hematita/qml/components/HistoryGraph.qml`, `ResourceRow.qml`, `ResourceDetail.qml`, `PerformancePage.qml`
- Modify: `hematita/qml/Main.qml`, `hematita/build.rs`

**Interfaces:**
- Consumes: `HematitaResources` from Task 8.
- Produces: `PerformancePage { required property HematitaResources resources }`.

- [ ] **Step 1: Write `HistoryGraph.qml`**

```qml
// hematita/qml/components/HistoryGraph.qml
import QtQuick
import QtQuick.Shapes
import org.celestina.hematita 1.0

// The one graph in Hematita: a minute of a fraction, oldest at the left. A
// GPU-backed Shape draws a filled area and the trace over it from a series the
// adapter already normalised, so the page never does arithmetic on samples.
//
// Colour follows the load state through theme tokens; the fill is the same
// colour at the suite's soft opacity.
Item {
    id: graph

    // Fractions 0..=1, oldest first. Any length; an empty series draws nothing.
    required property var series
    // "normal", "elevated" or "critical".
    required property string load

    readonly property color trace: graph.load === "critical"
                                   ? CelestinaTheme.danger
                                   : graph.load === "elevated"
                                     ? CelestinaTheme.warning
                                     : CelestinaTheme.accent

    implicitHeight: CelestinaTheme.rowHeightLg * 2

    function pointX(index) {
        const count = graph.series.length
        return count <= 1 ? 0 : index * graph.width / (count - 1)
    }

    function pointY(value) {
        const clamped = Math.max(0, Math.min(1, value))
        return graph.height - clamped * graph.height
    }

    // Both paths are rebuilt from the series once per sample. A PathPolyline
    // takes the points in one assignment, which is the cheap way to redraw.
    readonly property var tracePoints: {
        const points = []
        for (let index = 0; index < graph.series.length; ++index)
            points.push(Qt.point(graph.pointX(index), graph.pointY(graph.series[index])))
        return points
    }

    readonly property var areaPoints: {
        if (graph.series.length === 0)
            return []
        const points = [Qt.point(0, graph.height)]
        for (let index = 0; index < graph.series.length; ++index)
            points.push(Qt.point(graph.pointX(index), graph.pointY(graph.series[index])))
        points.push(Qt.point(graph.width, graph.height))
        return points
    }

    Rectangle {
        anchors.fill: parent
        radius: CelestinaTheme.radiusSm
        color: CelestinaTheme.inputFill
    }

    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer
        visible: graph.series.length > 1

        ShapePath {
            strokeWidth: 0
            strokeColor: CelestinaTheme.clear
            fillColor: Qt.rgba(graph.trace.r, graph.trace.g, graph.trace.b, CelestinaTheme.accentSoftOpacity)
            PathPolyline { path: graph.areaPoints }
        }

        ShapePath {
            strokeWidth: 2
            strokeColor: graph.trace
            fillColor: CelestinaTheme.clear
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            PathPolyline { path: graph.tracePoints }
        }
    }

    Accessible.role: Accessible.Graphic
    Accessible.name: qsTr("Historial del último minuto")
}
```

- [ ] **Step 2: Write `ResourceRow.qml`**

```qml
// hematita/qml/components/ResourceRow.qml
import QtQuick
import QtQuick.Controls
import org.celestina.hematita 1.0

// One resource in the side list: its name, its current value and a sparkline.
// Selecting it puts its detail on the right. Content family: the row paints
// the accent ramp, never the grey lift.
AbstractButton {
    id: row

    required property string name
    required property string value
    required property var series
    required property string load
    required property bool selected

    implicitHeight: CelestinaTheme.rowHeightLg
    hoverEnabled: true
    focusPolicy: Qt.TabFocus
    checkable: true
    checked: row.selected
    autoExclusive: true

    Accessible.role: Accessible.ListItem
    Accessible.name: row.name + ", " + row.value
    Accessible.selected: row.selected

    background: Rectangle {
        radius: CelestinaTheme.radiusSm
        color: row.selected
               ? CelestinaTheme.surfaceSelected
               : row.down
                 ? CelestinaTheme.pressedWash
                 : row.hovered
                   ? CelestinaTheme.contentHover
                   : CelestinaTheme.clear
    }

    CelestinaFocusRing {
        target: row
        cornerRadius: CelestinaTheme.radiusSm
        shown: row.visualFocus
    }

    contentItem: Row {
        spacing: CelestinaTheme.spaceMd
        leftPadding: CelestinaTheme.spaceMd
        rightPadding: CelestinaTheme.spaceMd

        Column {
            width: parent.width - sparkline.width - parent.spacing - parent.leftPadding - parent.rightPadding
            anchors.verticalCenter: parent.verticalCenter
            spacing: CelestinaTheme.spaceXs

            Text {
                text: row.name
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                font.weight: CelestinaTheme.weightDemiBold
                elide: Text.ElideRight
                width: parent.width
            }
            Text {
                text: row.value
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                font.features: CelestinaTheme.fontFeaturesTabular
                elide: Text.ElideRight
                width: parent.width
            }
        }

        HistoryGraph {
            id: sparkline
            width: CelestinaTheme.controlHeightXl * 2
            height: CelestinaTheme.controlHeightSm
            anchors.verticalCenter: parent.verticalCenter
            series: row.series
            load: row.load
        }
    }
}
```

- [ ] **Step 3: Write `ResourceDetail.qml`**

```qml
// hematita/qml/components/ResourceDetail.qml
import QtQuick
import QtQuick.Layouts
import org.celestina.hematita 1.0

// The selected resource, large: a title line, the minute graph, and the facts
// beneath it as label/value pairs. The page hands in everything; this draws.
CelestinaSurface {
    id: detail

    required property string title
    required property string subtitle
    required property var series
    required property string load
    // Flat list of alternating label, value strings.
    required property var facts

    role: CelestinaSurface.Grouped
    padding: CelestinaTheme.spaceXl

    contentItem: ColumnLayout {
        spacing: CelestinaTheme.spaceLg

        RowLayout {
            Layout.fillWidth: true
            Text {
                text: detail.title
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontTitle
                font.weight: CelestinaTheme.weightDemiBold
            }
            Item { Layout.fillWidth: true }
            Text {
                text: detail.subtitle
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                elide: Text.ElideLeft
                Layout.maximumWidth: detail.width / 2
            }
        }

        HistoryGraph {
            Layout.fillWidth: true
            Layout.fillHeight: true
            series: detail.series
            load: detail.load
        }

        GridLayout {
            Layout.fillWidth: true
            columns: 4
            columnSpacing: CelestinaTheme.space2xl
            rowSpacing: CelestinaTheme.spaceSm

            Repeater {
                model: detail.facts.length
                Text {
                    required property int index
                    readonly property bool isLabel: index % 2 === 0
                    text: detail.facts[index]
                    color: isLabel ? CelestinaTheme.textFaint : CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: isLabel ? CelestinaTheme.fontCaption : CelestinaTheme.fontRowTitle
                    font.features: CelestinaTheme.fontFeaturesTabular
                }
            }
        }
    }
}
```

- [ ] **Step 4: Write `PerformancePage.qml`**

```qml
// hematita/qml/components/PerformancePage.qml
import QtQuick
import QtQuick.Layouts
import org.celestina.hematita 1.0

// Rendimiento: the resources on the left, the chosen one on the right. In H1
// the list is CPU and memory; H2 makes it a model when disks and interfaces
// bring a count nobody can write down in advance.
Item {
    id: page

    required property HematitaResources resources

    // 0 = CPU, 1 = memory.
    property int selected: 0

    function gib(kib) {
        return (kib / 1048576).toLocaleString(Qt.locale(), "f", 1) + " GiB"
    }

    function percent(value) {
        return value < 0 ? "—" : value + " %"
    }

    readonly property string cpuValue: page.percent(page.resources.cpuPercent)
    readonly property string memoryValue: page.gib(page.resources.memoryUsedKib)
                                          + " / " + page.gib(page.resources.memoryTotalKib)

    RowLayout {
        anchors.fill: parent
        spacing: CelestinaTheme.spaceLg

        CelestinaSurface {
            Layout.preferredWidth: page.width * 0.32
            Layout.fillHeight: true
            role: CelestinaSurface.Panel
            padding: CelestinaTheme.spaceXs

            contentItem: Column {
                spacing: CelestinaTheme.spaceXs

                ResourceRow {
                    width: parent.width
                    name: qsTr("Procesador")
                    value: page.cpuValue
                    series: page.resources.cpuHistory
                    load: page.resources.cpuLoad
                    selected: page.selected === 0
                    onClicked: page.selected = 0
                }
                ResourceRow {
                    width: parent.width
                    name: qsTr("Memoria")
                    value: page.memoryValue
                    series: page.resources.memoryHistory
                    load: page.resources.memoryLoad
                    selected: page.selected === 1
                    onClicked: page.selected = 1
                }
            }
        }

        ResourceDetail {
            Layout.fillWidth: true
            Layout.fillHeight: true
            title: page.selected === 0 ? qsTr("Procesador") : qsTr("Memoria")
            subtitle: page.selected === 0 ? page.resources.cpuModel : ""
            series: page.selected === 0 ? page.resources.cpuHistory : page.resources.memoryHistory
            load: page.selected === 0 ? page.resources.cpuLoad : page.resources.memoryLoad
            facts: page.selected === 0
                   ? [qsTr("Uso"), page.cpuValue,
                      qsTr("Frecuencia"), page.resources.cpuFrequencyMhz > 0
                          ? (page.resources.cpuFrequencyMhz / 1000).toLocaleString(Qt.locale(), "f", 2) + " GHz"
                          : "—",
                      qsTr("Núcleos"), String(page.resources.cpuCores)]
                   : [qsTr("En uso"), page.gib(page.resources.memoryUsedKib),
                      qsTr("Total"), page.gib(page.resources.memoryTotalKib),
                      qsTr("Intercambio"), page.gib(page.resources.swapUsedKib)
                          + " / " + page.gib(page.resources.swapTotalKib)]
        }
    }

    // The truthful empty state: a source that could not be read says so in
    // the page rather than freezing the last number.
    Text {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.spaceSm
        visible: !page.resources.available && page.resources.unavailableReason.length > 0
        text: qsTr("No se pudo leer: ") + page.resources.unavailableReason
        color: CelestinaTheme.danger
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
    }
}
```

- [ ] **Step 5: Put the page into `Main.qml`**

Replace the `StackLayout` block in `Main.qml` with:

```qml
        HematitaResources {
            id: resources
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: window.currentSection

            PerformancePage {
                resources: resources
            }

            // Processes, applications and sensors arrive in H3 and H4.
            Repeater {
                model: window.sections.length - 1
                Item {
                    required property int index
                    Text {
                        anchors.centerIn: parent
                        text: qsTr("Esta sección llega en una fase posterior")
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontRowTitle
                    }
                }
            }
        }
```

and add `resources.start()` to `Component.onCompleted`.

- [ ] **Step 6: Register the four files in `build.rs`**

Before `"qml/Main.qml"` in `QML_FILES`:

```rust
    "qml/components/HistoryGraph.qml",
    "qml/components/ResourceRow.qml",
    "qml/components/ResourceDetail.qml",
    "qml/components/PerformancePage.qml",
```

- [ ] **Step 7: Build, smoke and look**

```bash
hematita/scripts/build-production.sh
hematita/scripts/smoke.sh
```
Expected: `smoke: OK`. Then open the binary on the nested session (memory *Nido en el 4K a escala 1.5*) and capture with `grim`: the side list shows Procesador and Memoria with sparklines that start advancing after two seconds; the detail shows the big graph and the facts; switching rows changes the detail; arrows on the strip move the lit pill.

---

### Task 10: Single-instance activation and the H1-C unit

**Files:**
- Create: `hematita/src/activation.rs`
- Modify: `hematita/src/main.rs`, `hematita/build.rs`, `hematita/qml/Main.qml`

**Interfaces:**
- Produces: `activation::hand_off() -> bool` (true when a running Hematita took the launch); QML type `HematitaActivation` with `signal raiseRequested()` and `start()`.

- [ ] **Step 1: Write `activation.rs`**

```rust
// hematita/src/activation.rs
//! One Hematita.
//!
//! The first instance takes a bus name and serves `Activate`; every later
//! launch finds the name owned, asks the running window to raise itself and
//! exits without building one. Failing to reach the bus is never fatal: the
//! launch carries on and opens its own window.

use std::pin::Pin;

use cxx_qt::{CxxQtType, Threading};

const SERVICE: &str = "org.celestina.Hematita";
const OBJECT: &str = "/org/celestina/Hematita";
const INTERFACE: &str = "org.celestina.Hematita";

#[cxx_qt::bridge]
pub mod qobject {
    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type HematitaActivation = super::HematitaActivationRust;

        /// Another launch asked this window to come to the front.
        #[qsignal]
        fn raise_requested(self: Pin<&mut HematitaActivation>);

        /// Starts serving the activation name, once. Best-effort.
        #[qinvokable]
        fn start(self: Pin<&mut HematitaActivation>);
    }

    impl cxx_qt::Threading for HematitaActivation {}
}

#[derive(Default)]
pub struct HematitaActivationRust {
    started: bool,
}

impl qobject::HematitaActivation {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().started {
            return;
        }
        self.as_mut().rust_mut().started = true;
        let qt = self.qt_thread();
        std::thread::spawn(move || {
            if let Err(error) = serve(qt) {
                eprintln!("hematita: D-Bus activation unavailable: {error}");
            }
        });
    }
}

struct Activation {
    qt: cxx_qt::CxxQtThread<qobject::HematitaActivation>,
}

#[zbus::interface(name = "org.celestina.Hematita")]
impl Activation {
    fn activate(&self) {
        let _ = self
            .qt
            .queue(|activation: Pin<&mut qobject::HematitaActivation>| {
                activation.raise_requested();
            });
    }
}

fn serve(qt: cxx_qt::CxxQtThread<qobject::HematitaActivation>) -> zbus::Result<()> {
    // `DoNotQueue`: without it a second instance sits in the name's queue and
    // inherits the name the moment the first exits, stranding a process.
    let connection = zbus::blocking::connection::Builder::session()?
        .serve_at(OBJECT, Activation { qt })?
        .build()?;
    connection.request_name_with_flags(SERVICE, zbus::fdo::RequestNameFlags::DoNotQueue.into())?;
    let _connection = connection;
    loop {
        std::thread::park();
    }
}

/// Asks a running Hematita to raise itself. `true` means it did and this
/// launch should exit; any failure answers `false` and the launch opens its
/// own window.
#[must_use]
pub fn hand_off() -> bool {
    let Ok(connection) = zbus::blocking::Connection::session() else {
        return false;
    };
    let Ok(proxy) = zbus::blocking::Proxy::<'_>::new(&connection, SERVICE, OBJECT, INTERFACE) else {
        return false;
    };
    proxy.call::<_, _, ()>("Activate", &()).is_ok()
}
```

- [ ] **Step 2: Wire it**

`main.rs`: add `mod activation;` and, as the first statement of `main()`:

```rust
    // A Hematita already running takes this launch: it raises itself and
    // this process leaves without building a window.
    if activation::hand_off() {
        return;
    }
```

`build.rs`: `.files(["src/activation.rs", "src/resources.rs"])`.

`Main.qml`: inside the window add

```qml
    HematitaActivation {
        id: activation
        onRaiseRequested: {
            window.show()
            window.raise()
            window.requestActivate()
        }
    }
```

and `activation.start()` in `Component.onCompleted`.

- [ ] **Step 3: Verify twice-launch behaviour**

```bash
hematita/scripts/build-production.sh && hematita/scripts/verify-production.sh
```
Expected: verify passes end to end (tests, clippy, fmt, qmllint ratchet, smoke). Then on the nested session launch the binary, launch it again from another terminal, and confirm the second exits immediately (`echo $?` prints 0 within a second) while the first raises.

If qmllint's count differs from the baseline row, set `scripts/qmllint-baseline.tsv`'s `hematita` row to the new count in this same unit (the row may fall; if it rose, fix the warnings instead).

- [ ] **Step 4: Close H1-C**

Write `hematita/docs/evidence/2026-09-21-h1-performance-page.md` (template: exact commands, printed test counts, the qmllint line, the smoke line, the grim capture path under `hematita/docs/evidence/` if kept, and the twice-launch observation). Update the ledger H1-C row to `done`, the roadmap's H1-C row, and `STATUS.md`. Then:

```bash
python3 "$SCRATCH/mkinv.py" H1-C \
  hematita/docs/inventories/2026-09-21-h1-foundation/H1-C.numstat.tsv \
  hematita/docs/plans/active/2026-09-21-h1-foundation.md \
  hematita/docs/evidence/2026-09-21-h1-performance-page.md hematita/ROADMAP.md hematita/STATUS.md \
  hematita/build.rs hematita/src/main.rs hematita/src/activation.rs hematita/src/sampler.rs hematita/src/resources.rs \
  hematita/qml/Main.qml hematita/qml/components/HistoryGraph.qml hematita/qml/components/ResourceRow.qml \
  hematita/qml/components/ResourceDetail.qml hematita/qml/components/PerformancePage.qml \
  hematita/Cargo.lock $( [ -n "$(git diff --name-only scripts/qmllint-baseline.tsv)" ] && echo scripts/qmllint-baseline.tsv )
python3 scripts/check-staged-units.py hematita/docs/inventories/2026-09-21-h1-foundation/H1-C.numstat.tsv
bash scripts/check-architecture-contract.sh && bash scripts/check-documentation-contract.sh && python3 scripts/check-language-contract.py
```

Commit (only on request):

```bash
git commit -m "$(cat <<'EOF'
hematita-maintenance: Add the sampler, the resources object and the Performance page

H1-C: a one-second sampling thread publishing whole snapshots to the Qt
thread; HematitaResources with the load thresholds and two history rings;
single-instance activation over the session bus; the Performance page with
the side list, the Shape graph and the detail facts.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

---

### Task 11: Implementation exit — H1-Z

**Files:**
- Modify: `hematita/Cargo.toml`, `hematita/Cargo.lock` (version 0.2.0), `docs/version-history.tsv`
- Modify: `hematita/ROADMAP.md`, `hematita/STATUS.md`, `hematita/docs/plans/active/README.md`, `hematita/docs/plans/archive/README.md`
- Move: `hematita/docs/plans/active/2026-09-21-h1-foundation.md` → `hematita/docs/plans/archive/`
- Create: `hematita/docs/evidence/2026-09-21-h1-production-completion.md`

- [ ] **Step 1: Bump the version**

```bash
python3 scripts/version_tool.py bump hematita milestone --unit H1-Z --summary "Add the foundation and the Performance page with CPU and memory"
python3 scripts/version_tool.py check
grep -n '^version' hematita/Cargo.toml
```
Expected: `0.2.0` in `Cargo.toml` and `Cargo.lock`, one appended `hematita	0.2.0	milestone	H1-Z	Add the foundation and the Performance page with CPU and memory` row. If `bump`'s flags differ, run `python3 scripts/version_tool.py bump --help` and use the documented spelling; the history row's summary must equal the commit subject after the colon.

- [ ] **Step 2: Run the canonical completion**

```bash
hematita/scripts/complete-production.sh
hematita/scripts/status-production.sh
ls -l ~/.local/bin/hematita ~/.local/share/applications/org.celestina.Hematita.desktop
```
Expected: build, verify, deploy and status succeed; the installed binary and desktop entry exist. Launch `hematita` from the nested session once to confirm the installed bytes show the graphs.

- [ ] **Step 3: Close the documents**

- `ROADMAP.md`: `Status: idle`, `Active implementation checkpoint: none`, H1 rows `done`, add `## H1 — closed 2026-09-21` naming the delivered result and linking the completion evidence; keep the H2–H5 table.
- `STATUS.md`: `Updated`, `Delivered as 0.2.0: H1`, installed state, next phase H2 planned.
- Plan: add `- **Closed:** 2026-09-21`, `- **Successor:** H2`, `Status: done`, H1-Z row `done`; then `git mv` it to `docs/plans/archive/` and update both plan READMEs (active: "No plan is active…"; archive: list H1).
- Evidence `2026-09-21-h1-production-completion.md`: the completion command, the sealed manifest path, the installed paths, `VAL-H1` pending.

- [ ] **Step 4: Inventory and commit (only on request)**

```bash
python3 "$SCRATCH/mkinv.py" H1-Z \
  hematita/docs/inventories/2026-09-21-h1-foundation/H1-Z.numstat.tsv \
  hematita/docs/plans/archive/2026-09-21-h1-foundation.md \
  hematita/Cargo.toml hematita/Cargo.lock docs/version-history.tsv \
  hematita/ROADMAP.md hematita/STATUS.md \
  hematita/docs/plans/active/README.md hematita/docs/plans/archive/README.md \
  hematita/docs/evidence/2026-09-21-h1-production-completion.md
git add -A hematita/docs/plans   # records the move
python3 scripts/check-staged-units.py hematita/docs/inventories/2026-09-21-h1-foundation/H1-Z.numstat.tsv
python3 scripts/version_tool.py check
bash scripts/check-documentation-contract.sh
git commit -m "$(cat <<'EOF'
hematita-milestone: Add the foundation and the Performance page with CPU and memory

H1-Z: the implementation exit of H1 — release built, verified and deployed to
the author's prefix at 0.2.0; roadmap and status closed; the plan archived.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>
EOF
)"
```

The moved plan's inventory row uses the archive path; the generator handles a rename as a delete of the old path plus an add of the new one only if both are listed — list only the archive path and let `git diff --numstat --no-renames` report the deletion under `hematita/docs/plans/active/…` as its own row by adding that path to the generator's arguments too.

---

## Self-review

**Spec coverage (H1 only):** §3 sources for CPU, memory, frequency, model → Tasks 5–8. §4 modules `cpu`, `memory`, `history`, `ratio` (named `error` per module instead of one `error` module: each error enum lives beside its parser, which the spec allows since it asks for "one enum per module") → Tasks 5–7. §5 `sampler.rs`, `resources.rs`, `activation.rs`, `main.rs` → Tasks 8, 10, 2. §6 `Main`, `NavStrip`, `PerformancePage`, `ResourceRow`, `ResourceDetail`, `HistoryGraph` → Tasks 3, 9. §7 H1 skeleton items (crate, app, `build.rs`, links, `.desktop`, icon, registry, scripts, baseline row) → Tasks 1, 2, 4. §8 crate tests, adapter test (load policy; snapshot diffing is H3 when a model exists), smoke, guards, `VALIDATION.md` → Tasks 4–11. Not in H1 by design: per-core grid (H2), process model (H3).

**Placeholder scan:** the only intentional fill-ins are `CORES`/`TOTAL_KIB` (captured at execution) and `FILL IN AFTER RUNNING` in the skeleton evidence, both with explicit instructions to replace before the unit closes.

**Type consistency:** `CpuStat`/`CpuTicks`/`CpuSample`/`CpuSampler::sample(&CpuStat)` match between Task 5 and `sampler.rs`; `Memory` fields match Task 6 and `resources.rs`; `Ring::values()`/`push()` match Task 7 and `ring_list`; `Section`, `Snapshot`, `Identity`, `read_identity`, `Sampler::spawn` match between `sampler.rs` and `resources.rs`; QML property names (`cpuHistory`, `memoryLoad`, `unavailableReason`, …) are the camelCase of the `#[qproperty]` snake_case names and are what `PerformancePage.qml` reads; `NavStrip.activated(int)` is what `Main.qml` handles.
