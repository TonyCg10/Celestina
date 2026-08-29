//! Niri event-stream adapter for the shell.
//!
//! Niri owns the event model and already provides a state reducer. This helper
//! keeps those compositor types out of Qt, reduces every relevant event to a
//! small workspace snapshot and reconnects after IPC loss. The C++ host only
//! marshals this line-delimited protocol onto the GUI thread.
//!
//! The protocol has two directions. Downstream (helper → host) carries
//! `snapshot`, `unavailable` and `request` frames. Upstream (host → helper)
//! carries bounded focus requests: a request is read by a dedicated stdin
//! reader, queued on a bounded channel and performed by one worker over a
//! short-lived second socket, so a slow action never stalls the event stream.
//! A single shared writer serializes every frame — snapshots and request
//! results can never interleave on stdout.

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::{self, BufRead, BufReader, BufWriter, Stdout, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::process::{self, ExitCode};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use celestina_shell_core::bounded;
// Aliased: `niri_ipc::Event` is the compositor's event and is already the
// `Event` this file means everywhere else.
use celestina_shell_core::diagnostics::{Event as Record, Level, Value as Field};
use celestina_shell_core::journal::{self, Journal};
use celestina_shell_core::lines::{read_bounded_line, HostLine, SharedWriter, WriteError};
use celestina_shell_core::settings::Settings;
use celestina_shell_core::workspace_groups::{self, Homes, Workspace as CoreWorkspace};
use celestina_shell_core::workspace_map;
use niri_ipc::socket::Socket;
use niri_ipc::state::{EventStreamState, EventStreamStatePart};
use niri_ipc::{Action, Event, Reply, Request, Response, WorkspaceReferenceArg};
use serde::{Deserialize, Serialize};

const RECONNECT_DELAY: Duration = Duration::from_secs(1);
/// The host may not outrun the compositor: further requests are refused with a
/// visible failure instead of growing an unbounded backlog.
const COMMAND_QUEUE_CAPACITY: usize = 32;
/// Request ids are opaque to this helper — it only echoes them back — but a
/// bounded length keeps a hostile id out of the downstream frames.
const MAX_ID_CHARS: usize = 32;
const MAX_REASON_CHARS: usize = 200;
/// Workspace labels, output names, window titles and the workspace count are
/// compositor state, and a window title is whatever its client decided to set —
/// it can be megabytes long. The host discards any protocol line above its own
/// line limit as a whole and then declares the shell unavailable, so a single
/// hostile title would blank the workspace strip on every event for as long as
/// that window exists. These four limits are therefore the host's own
/// `maxLabelLength`, `maxTitleLength` and `maxWorkspaceCount` in
/// `src/niriclient.cpp`: a snapshot this helper publishes must be one the host
/// accepts, and a value that exceeds them there is rejected, not trimmed. They
/// are counted in UTF-16 code units because that is what `QString::size()`
/// measures on the far side.
const MAX_LABEL_UNITS: usize = 128;
const MAX_TITLE_UNITS: usize = 512;
const MAX_WORKSPACES: usize = 512;
// How many windows and columns one workspace publishes is
// `celestina_shell_core::workspace_map`'s to say, not this file's: it is the
// module that folds them, and a second constant here would be a second owner
// for the same bound quietly disagreeing with the first.

/// Every frame leaves through this one writer, so a request result can never
/// land in the middle of a snapshot line.
type AdapterWriter = Arc<SharedWriter<BufWriter<Stdout>>>;

/// How a strip is treating the monitor group a workspace belongs to.
///
/// [`Self::Expanded`] is the ordinary case and the wire default: a strip whose
/// workspaces all belong to one monitor has one group, opens it, and draws the
/// plain row it drew before grouping existed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum GroupState {
    /// Its workspaces are shown.
    Expanded,
    /// Its workspaces are behind a capsule.
    Collapsed,
    /// Behind a capsule, and this is the workspace that capsule asks for: the
    /// one that was active on that monitor, or the group's first if none was.
    /// Exactly one workspace of a collapsed group carries it.
    CollapsedTarget,
}

/// Enough of a window to draw where it is, and nothing about what it contains.
///
/// There are no pixels here and there is no way to get them: Wayland gives a
/// client no access to another client's buffers, and Niri composites its own
/// overview inside the compositor. What the compositor does publish is where a
/// window sits and how big it is, which is what makes a truthful map possible
/// without a picture.
#[derive(Debug, PartialEq, Serialize)]
struct WindowSnapshot {
    /// The compositor's own id, as a decimal string. It is what a surface sends
    /// back to focus this window rather than the workspace it sits on, and it
    /// travels as text for the same reason a workspace id does: JSON numbers
    /// reach the Qt host as doubles and this is a `u64`.
    id: String,
    /// Whatever the client set. Bounded like every other producer string, and
    /// rendered as characters rather than markup on the far side.
    title: String,
    /// The application's own id. The closest thing to a description this
    /// protocol has, and the key an icon would later be looked up by.
    app_id: String,
    /// Where the window sits in the scrolling layout, as Niri counts it. Zero
    /// means it is not in that layout at all — a floating window has no column,
    /// and the surface reads `floating` rather than inventing a position for it.
    column: u16,
    row: u16,
    /// This window's share of its column's height, between 0 and 1, computed by
    /// [`celestina_shell_core::workspace_map`]. The surface multiplies it by
    /// whatever room it has.
    ///
    /// A share rather than a measure, so no surface ever receives a pixel count
    /// it might be tempted to use as one, and none of them has to decide what an
    /// impossible size means. Never `NaN`: the frames either side of one would
    /// compare unequal for ever and republish the snapshot on every compositor
    /// event.
    height_share: f64,
    focused: bool,
    floating: bool,
    urgent: bool,
}

/// One column of a workspace's layout.
#[derive(Debug, PartialEq, Serialize)]
struct ColumnSnapshot {
    /// This column's share of the map's width, between 0 and 1. The shares of a
    /// map always sum to 1.
    width_share: f64,
    /// Its windows, top to bottom.
    windows: Vec<WindowSnapshot>,
}

/// What a workspace holds, folded into the arrangement it really has.
///
/// The fold is [`celestina_shell_core::workspace_map`] and it happens here
/// rather than in a surface, so the rule has one owner: two surfaces grouping
/// the same windows separately could show the same session two ways.
#[derive(Debug, Default, PartialEq, Serialize)]
struct MapSnapshot {
    /// The scrolling layout, left to right.
    columns: Vec<ColumnSnapshot>,
    /// Windows with no place in that layout. They sit over the arrangement
    /// rather than in it.
    floating: Vec<WindowSnapshot>,
    /// How many windows the bounds dropped. A surface that shows four of nine
    /// must be able to say so; showing four silently is the map lying about the
    /// one thing it exists to answer.
    hidden: usize,
}

#[derive(Debug, PartialEq, Serialize)]
struct WorkspaceSnapshot {
    /// Niri's stable workspace id, carried as a decimal string: it is a `u64`
    /// whose values the compositor explicitly refuses to constrain, and JSON
    /// numbers reach the Qt host as doubles. The host never interprets it — it
    /// echoes the string back when it requests focus.
    id: String,
    index: u8,
    label: String,
    output: String,
    /// Which monitor this workspace belongs to, which is only the same as
    /// `output` while that monitor is connected. Niri moves a workspace to a
    /// surviving output when its own goes away and then stops saying where it
    /// came from, so the panel would otherwise have no way to keep three
    /// monitors' worth of workspaces apart. Remembered or declared rather than
    /// read — see [`celestina_shell_core::workspace_groups`].
    ///
    /// Always present and never empty: when nothing is known it is `output`,
    /// which is the honest answer and the one that groups a strip exactly as it
    /// grouped before this field existed.
    home: String,
    /// What the strip showing this workspace does with its monitor group.
    ///
    /// Grouping itself is a fold over [`Self::home`], but *which* group opens is
    /// policy, and it is decided here — by the process that links the core that
    /// owns it — so no surface has to reimplement it. One value rather than two
    /// booleans because the two facts are not independent: being the workspace a
    /// capsule asks for only means anything while that capsule exists.
    group: GroupState,
    active: bool,
    focused: bool,
    urgent: bool,
    active_window_title: Option<String>,
    /// What this workspace holds, folded into the columns and rows it really
    /// has. Additive: a host that predates this field ignores it and keeps the
    /// snapshot it always had.
    map: MapSnapshot,
    /// The name the compositor gave it, kept out of the wire: the label above
    /// always carries something displayable, and this is what may be looked up
    /// in the homes memory. `None` is niri's unnamed spare, whose index
    /// fallback must never borrow a named workspace's home.
    #[serde(skip)]
    identity: Option<String>,
}

#[derive(Debug, PartialEq, Serialize)]
struct ShellSnapshot {
    kind: &'static str,
    workspaces: Vec<WorkspaceSnapshot>,
    /// Outputs whose active workspace holds a tile the size of the output
    /// itself — a fullscreen window, and therefore the one tenant the shell's
    /// parked surfaces yield direct scanout to (SURF-1-C).
    ///
    /// A semantic fact rather than a measure, deliberately: the map already
    /// refuses to publish pixel counts, and this field keeps that rule — the
    /// comparison against each output's logical size happens here, where both
    /// numbers live. Additive: a host that predates it ignores it.
    fullscreen_outputs: Vec<String>,
}

#[derive(Serialize)]
struct Unavailable<'a> {
    kind: &'static str,
    reason: &'a str,
}

/// The result of one host request. `accepted` means the compositor handled the
/// request; it is not proof that the requested workspace is now active — only a
/// later snapshot can show that, and only the host correlates the two.
#[derive(Debug, PartialEq, Serialize)]
struct RequestFrame<'a> {
    kind: &'static str,
    id: &'a str,
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl<'a> RequestFrame<'a> {
    fn accepted(id: &'a str) -> Self {
        Self {
            kind: "request",
            id,
            state: "accepted",
            reason: None,
        }
    }

    fn failed(id: &'a str, reason: &str) -> Self {
        Self {
            kind: "request",
            id,
            state: "failed",
            reason: Some(bounded(reason, MAX_REASON_CHARS)),
        }
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum HostCommand {
    /// Focus the workspace whose Niri id the host read from a snapshot.
    FocusWorkspace { id: String, workspace: String },
    /// Focus one window by the id a snapshot published. A workspace focus moves
    /// to a place; this moves to a thing, which is what a map of windows is for.
    FocusWindow { id: String, window: String },
    /// Open Niri's own screenshot UI. The shell asks the compositor to
    /// capture; it never captures anything itself.
    Screenshot { id: String },
    /// Blank the outputs now. There is no matching "on": any input wakes them,
    /// and the compositor owns that, not this shell.
    PowerOffMonitors { id: String },
    /// End the session. The compositor owns it; this shell only asks.
    Quit { id: String },
}

impl HostCommand {
    fn id(&self) -> &str {
        match self {
            Self::FocusWorkspace { id, .. }
            | Self::FocusWindow { id, .. }
            | Self::Screenshot { id }
            | Self::PowerOffMonitors { id }
            | Self::Quit { id } => id,
        }
    }
}

/// Enough of a command to answer a malformed one. A host that sends an
/// unreadable frame still learns its request failed instead of waiting out the
/// host-side timeout.
#[derive(Debug, Deserialize)]
struct CommandEnvelope {
    id: Option<String>,
}

#[derive(Debug, PartialEq)]
struct Rejection {
    id: Option<String>,
    reason: String,
}

#[derive(Debug)]
enum AdapterError {
    Connect(io::Error),
    Request(io::Error),
    Rejected(String),
    Stream(io::Error),
    Emit(WriteError),
    Spawn(io::Error),
}

impl Display for AdapterError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect(error) => write!(formatter, "cannot connect to Niri IPC: {error}"),
            Self::Request(error) => {
                write!(formatter, "cannot request Niri's event stream: {error}")
            }
            Self::Rejected(message) => {
                write!(formatter, "Niri rejected the event stream: {message}")
            }
            Self::Stream(error) => write!(formatter, "Niri event stream ended: {error}"),
            Self::Emit(error) => write!(formatter, "cannot publish a shell frame: {error}"),
            Self::Spawn(error) => write!(formatter, "cannot start an adapter thread: {error}"),
        }
    }
}

impl Error for AdapterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Connect(error)
            | Self::Request(error)
            | Self::Stream(error)
            | Self::Spawn(error) => Some(error),
            Self::Emit(error) => Some(error),
            Self::Rejected(_) => None,
        }
    }
}

/// Where the learned homes live. State rather than configuration: this is what
/// the shell observed, not what the person chose, and a person who deletes it
/// loses nothing they wrote.
fn homes_path() -> Option<std::path::PathBuf> {
    celestina_core::xdg::state_home().map(|dir| dir.join("celestina").join("workspace-homes.json"))
}

/// The persisted memory, or an empty one.
///
/// Every failure here returns an empty memory rather than propagating: a
/// missing, unreadable, oversized or corrupt file means the strip groups by the
/// output each workspace is on, which is exactly how it behaved before this
/// existed. A shell that refused to start over its own cache would be worse than
/// one that forgot.
fn load_homes() -> Homes {
    let Some(path) = homes_path() else {
        return Homes::new();
    };
    // Read bounded rather than whole: the size limit and every parsing rule
    // belong to the core that owns what a memory is, so this reads bytes and
    // decides nothing about them.
    let Ok(file) = std::fs::File::open(&path) else {
        return Homes::new();
    };
    let ceiling = u64::try_from(workspace_groups::MAX_FILE_BYTES).unwrap_or(u64::MAX);
    let mut bytes = Vec::new();
    if io::Read::read_to_end(&mut io::Read::take(file, ceiling + 1), &mut bytes).is_err() {
        return Homes::new();
    }

    Homes::from_bytes(&bytes).unwrap_or_default()
}

/// Writes the memory atomically. A failure is reported and dropped: losing what
/// was learned costs the next session one multi-output frame to learn it again,
/// and is not worth interrupting an event stream for.
fn save_homes(homes: &Homes) {
    let Some(path) = homes_path() else {
        return;
    };
    let Ok(bytes) = homes.to_bytes() else {
        return;
    };
    if let Err(error) = celestina_core::atomic_file::replace(&path, &bytes) {
        eprintln!("celestina-niri-adapter: could not persist workspace homes: {error}");
    }
}

/// The person's own declarations, read from the shell's settings file.
///
/// Read-only and best-effort. The settings file belongs to the aggregate
/// provider helper, which is the only process that writes it; this one consumes
/// the single field that answers a question only it can act on, through the same
/// [`celestina_shell_core::settings`] schema, so there is no second idea of what
/// the file contains. A missing, unreadable or older file simply declares
/// nothing, which leaves the strip on what it learned by watching.
fn load_declarations(homes: &mut Homes) {
    let Some(path) = celestina_core::xdg::config_home() else {
        return;
    };
    let Ok(bytes) = std::fs::read(path.join("celestina").join("settings.json")) else {
        return;
    };
    let Some(settings) = Settings::from_bytes(&bytes) else {
        return;
    };

    homes.set_declarations(
        settings
            .workspace_homes
            .iter()
            .map(|(label, output)| (label.as_str(), output.as_str())),
    );
}

/// Marks which monitor group is open, and which workspace each closed one asks
/// for, once every home is known.
///
/// Grouped **per output**, because the shell draws one strip per physical
/// monitor and each strip shows only the workspaces that are on it. A session
/// with all three monitors connected therefore has one group per strip and every
/// workspace comes back expanded, which is the case that must look untouched.
///
/// The decision itself is [`celestina_shell_core::workspace_groups::group`]. It
/// is made here rather than in the surface so that one implementation answers
/// it: a strip folds equal `home` values together and renders what this said.
fn publish_grouping(workspaces: &mut [WorkspaceSnapshot], homes: &Homes) {
    let outputs: Vec<String> = {
        let mut seen: Vec<String> = Vec::new();
        for workspace in workspaces.iter() {
            if !seen.contains(&workspace.output) {
                seen.push(workspace.output.clone());
            }
        }
        seen
    };

    for output in outputs {
        let strip: Vec<CoreWorkspace> = workspaces
            .iter()
            .filter(|workspace| workspace.output == output)
            .map(|workspace| {
                CoreWorkspace::new(
                    // Grouping keys on the home, so the core is handed the home
                    // as the placement it should group by. The label stays the
                    // identity, which is what `focus_target` is matched on.
                    &workspace.label,
                    &workspace.home,
                    workspace.active,
                    workspace.urgent,
                    workspace.active_window_title.is_some(),
                )
            })
            .collect();

        for group in workspace_groups::group(&strip, homes) {
            let target = group
                .focus_target()
                .map(|workspace| workspace.label.clone());
            for workspace in workspaces
                .iter_mut()
                .filter(|workspace| workspace.output == output && workspace.home == group.key)
            {
                workspace.group = if group.expanded {
                    GroupState::Expanded
                } else if target.as_deref() == Some(workspace.label.as_str()) {
                    GroupState::CollapsedTarget
                } else {
                    GroupState::Collapsed
                };
            }
        }
    }
}

/// Reduces the compositor's state to the strip's snapshot, teaching the memory
/// whatever this frame is in a position to teach.
///
/// What one workspace holds, folded into the arrangement it really has.
///
/// Every decision about that arrangement — which column a window is in, what
/// order the rows go in, what share of the space each takes and what an
/// unusable measure means — belongs to
/// [`celestina_shell_core::workspace_map`]. This function's whole job is to
/// carry the compositor's types across to it and its answer back out, so the
/// rule has exactly one owner and a second consumer cannot get a different
/// arrangement from the same session.
fn folded_map(state: &EventStreamState, workspace_id: u64) -> MapSnapshot {
    let windows = state
        .windows
        .windows
        .values()
        .filter(|window| window.workspace_id == Some(workspace_id))
        .map(|window| {
            let (column, row) =
                window
                    .layout
                    .pos_in_scrolling_layout
                    .map_or((0, 0), |(column, row)| {
                        (
                            u16::try_from(column).unwrap_or(u16::MAX),
                            u16::try_from(row).unwrap_or(u16::MAX),
                        )
                    });

            workspace_map::Window::new(
                &window.id.to_string(),
                window.title.as_deref().unwrap_or_default(),
                window.app_id.as_deref().unwrap_or_default(),
                column,
                row,
            )
            .sized(window.layout.tile_size.0, window.layout.tile_size.1)
            .with_states(window.is_focused, window.is_floating, window.is_urgent)
        })
        .collect::<Vec<_>>();

    let folded = workspace_map::map(&windows);
    MapSnapshot {
        columns: folded
            .columns
            .into_iter()
            .map(|column| ColumnSnapshot {
                width_share: column.width_share,
                windows: column
                    .tiles
                    .into_iter()
                    .map(|tile| published(&tile))
                    .collect(),
            })
            .collect(),
        floating: folded
            .floating
            .into_iter()
            .map(|window| {
                published(&workspace_map::Tile {
                    window,
                    // A floating window is not stacked against siblings, so it has
                    // no share of a column to report. One is the honest answer: it
                    // occupies all of whatever room the surface gives it.
                    height_share: 1.0,
                })
            })
            .collect(),
        hidden: folded.hidden,
    }
}

/// One folded tile, in the shape the wire carries.
fn published(tile: &workspace_map::Tile) -> WindowSnapshot {
    WindowSnapshot {
        id: tile.window.id.clone(),
        title: bounded(&tile.window.title, MAX_TITLE_UNITS),
        app_id: bounded(&tile.window.app_id, MAX_LABEL_UNITS),
        column: tile.window.column,
        row: tile.window.row,
        height_share: tile.height_share,
        focused: tile.window.focused,
        floating: tile.window.floating,
        urgent: tile.window.urgent,
    }
}

/// Returns whether anything new was learned, so the caller writes the file when
/// there is something to write rather than on every compositor event.
/// The logical size of each connected output, from the request socket.
///
/// The event stream never carries outputs, so the one comparison this adapter
/// needs them for — whether a tile is the size of its whole output — reads a
/// cache fetched over the same socket the actions use. Refetched only when
/// something says the answer may have moved: session start, a config reload
/// (scale lives in the config), or a workspace naming an output the cache does
/// not know. A fetch that fails keeps the previous map and waits for the next
/// of those moments rather than hammering a socket that just refused.
struct OutputSizes {
    by_name: HashMap<String, (f64, f64)>,
    stale: bool,
}

impl OutputSizes {
    fn new() -> Self {
        Self {
            by_name: HashMap::new(),
            stale: true,
        }
    }

    fn mark_stale(&mut self) {
        self.stale = true;
    }

    fn ensure(&mut self, state: &EventStreamState) {
        if !self.stale {
            let unknown = state
                .workspaces
                .workspaces
                .values()
                .filter(|workspace| workspace.is_active)
                .filter_map(|workspace| workspace.output.as_deref())
                .any(|output| !self.by_name.contains_key(output));
            if !unknown {
                return;
            }
        }
        self.stale = false;

        match Socket::connect().and_then(|mut socket| socket.send(Request::Outputs)) {
            Ok(Ok(Response::Outputs(outputs))) => {
                self.by_name = outputs
                    .into_iter()
                    .filter_map(|(name, output)| {
                        let logical = output.logical?;
                        Some((name, (f64::from(logical.width), f64::from(logical.height))))
                    })
                    .collect();
            }
            answer => {
                journal::record(
                    Record::new(Level::Warn, "niri.outputs.unavailable").with_text(
                        "reason",
                        &match answer {
                            Ok(Ok(response)) => format!("unexpected response: {response:?}"),
                            Ok(Err(message)) => message,
                            Err(error) => error.to_string(),
                        },
                    ),
                );
            }
        }
    }
}

/// Outputs whose active workspace holds a tile the size of the output itself.
///
/// A maximized tile never matches: it excludes the panel's exclusive zone, so
/// only a really fullscreened window — the direct-scanout tenant — reaches the
/// full logical height. The tolerance absorbs fractional logical sizes; an
/// output whose size is unknown reports nothing rather than guessing.
fn fullscreen_outputs(
    state: &EventStreamState,
    sizes: &HashMap<String, (f64, f64)>,
) -> Vec<String> {
    const TOLERANCE: f64 = 1.0;

    let mut outputs: Vec<String> = state
        .workspaces
        .workspaces
        .values()
        .filter(|workspace| workspace.is_active)
        .filter_map(|workspace| {
            let output = workspace.output.as_deref()?;
            let (width, height) = sizes.get(output)?;
            let holds = state.windows.windows.values().any(|window| {
                window.workspace_id == Some(workspace.id)
                    && (window.layout.tile_size.0 - width).abs() <= TOLERANCE
                    && (window.layout.tile_size.1 - height).abs() <= TOLERANCE
            });
            holds.then(|| bounded(output, MAX_LABEL_UNITS))
        })
        .collect();
    outputs.sort();
    outputs.dedup();
    outputs.truncate(MAX_WORKSPACES);
    outputs
}

fn shell_snapshot(
    state: &EventStreamState,
    homes: &mut Homes,
    sizes: &HashMap<String, (f64, f64)>,
) -> (ShellSnapshot, bool) {
    let mut workspaces = state
        .workspaces
        .workspaces
        .values()
        // The shell renders one strip per physical output. Niri can briefly
        // retain an unassigned workspace while outputs change; that state is
        // valid compositor data but has no panel consumer, so keep it out of
        // the inter-process contract instead of serializing a nullable output.
        .filter_map(|workspace| {
            let output = workspace.output.clone()?;
            let active_window = workspace
                .active_window_id
                .and_then(|id| state.windows.windows.get(&id));
            // The identity, apart from the label. The label below falls back
            // to the index so a dot can still be spoken, but that fallback is
            // a *display* name: the homes memory keys on identities, and an
            // unnamed spare wearing "6" as its label would borrow the home of
            // a named workspace "6" — which put one monitor's group capsule on
            // every other monitor's strip.
            let identity = workspace
                .name
                .as_deref()
                .filter(|name| !name.is_empty())
                .map(|name| bounded(name, MAX_LABEL_UNITS));

            Some(WorkspaceSnapshot {
                id: workspace.id.to_string(),
                index: workspace.idx,
                label: workspace
                    .name
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .map_or_else(
                        || workspace.idx.to_string(),
                        |name| bounded(name, MAX_LABEL_UNITS),
                    ),
                output: bounded(&output, MAX_LABEL_UNITS),
                // Filled in below, once the whole frame is known: a home cannot
                // be decided from one workspace, because whether this frame may
                // teach anything at all depends on how many outputs it carries.
                home: String::new(),
                group: GroupState::Expanded,
                active: workspace.is_active,
                focused: workspace.is_focused,
                urgent: workspace.is_urgent,
                active_window_title: active_window
                    .and_then(|window| window.title.as_deref())
                    .map(|title| bounded(title, MAX_TITLE_UNITS)),
                map: folded_map(state, workspace.id),
                identity,
            })
        })
        .collect::<Vec<_>>();

    workspaces.sort_by(|left, right| {
        left.output
            .cmp(&right.output)
            .then(left.index.cmp(&right.index))
    });
    // Truncating after the sort keeps a stable prefix of the strip instead of
    // whichever workspaces the compositor's map happened to yield first, so a
    // session past the cap still shows the same outputs from one event to the
    // next rather than a reshuffling list.
    workspaces.truncate(MAX_WORKSPACES);

    // Learn from the truncated strip rather than the whole compositor state, so
    // the memory only ever records workspaces the panel could actually show.
    let observed = workspaces
        .iter()
        .map(|workspace| {
            CoreWorkspace::new(
                // The core's contract: an unnamed workspace has an empty label
                // and is never remembered. The display fallback stays out of
                // the memory's key space.
                workspace.identity.as_deref().unwrap_or(""),
                &workspace.output,
                workspace.active,
                workspace.urgent,
                workspace.active_window_title.is_some(),
            )
        })
        .collect::<Vec<_>>();
    let learned = homes.learn(&observed);

    for workspace in &mut workspaces {
        // An unnamed workspace has no identity to look up: it belongs where it
        // is, which folds it into its own output's group.
        workspace.home = workspace
            .identity
            .as_deref()
            .and_then(|identity| homes.home_of(identity))
            .unwrap_or(&workspace.output)
            .to_owned();
    }
    publish_grouping(&mut workspaces, homes);

    (
        ShellSnapshot {
            kind: "snapshot",
            workspaces,
            fullscreen_outputs: fullscreen_outputs(state, sizes),
        },
        learned,
    )
}

fn emit_json<T: Serialize>(writer: &AdapterWriter, value: &T) -> Result<(), AdapterError> {
    writer.emit(value).map_err(AdapterError::Emit)
}

fn protocol_io(error: serde_json::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

fn open_event_stream() -> Result<BufReader<UnixStream>, AdapterError> {
    let path = std::env::var_os(niri_ipc::socket::SOCKET_PATH_ENV).ok_or_else(|| {
        AdapterError::Connect(io::Error::new(
            io::ErrorKind::NotFound,
            "NIRI_SOCKET is not set",
        ))
    })?;
    let stream = UnixStream::connect(path).map_err(AdapterError::Connect)?;
    let mut reader = BufReader::new(stream);
    let mut request = serde_json::to_vec(&Request::EventStream)
        .map_err(protocol_io)
        .map_err(AdapterError::Request)?;
    request.push(b'\n');
    reader
        .get_mut()
        .write_all(&request)
        .map_err(AdapterError::Request)?;

    let mut line = String::new();
    if reader.read_line(&mut line).map_err(AdapterError::Request)? == 0 {
        return Err(AdapterError::Request(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Niri closed the event-stream handshake",
        )));
    }
    let reply: Reply = serde_json::from_str(&line)
        .map_err(protocol_io)
        .map_err(AdapterError::Request)?;
    match reply {
        Ok(Response::Handled) => {}
        Ok(response) => {
            return Err(AdapterError::Rejected(format!(
                "unexpected response: {response:?}"
            )));
        }
        Err(message) => return Err(AdapterError::Rejected(message)),
    }

    reader
        .get_mut()
        .shutdown(Shutdown::Write)
        .map_err(AdapterError::Request)?;
    Ok(reader)
}

fn parse_shell_event(line: &str) -> io::Result<Option<Event>> {
    let value: serde_json::Value = serde_json::from_str(line).map_err(protocol_io)?;
    if value
        .as_object()
        .is_some_and(|object| object.len() == 1 && object.contains_key("MinimizedWindowsChanged"))
    {
        // Melibea is the sole consumer and reducer for compositor-native
        // minimized state. This adapter keeps serving ordinary workspace and
        // window state when that independent extension shares Niri's global
        // event stream.
        return Ok(None);
    }
    match serde_json::from_value(value) {
        Ok(event) => Ok(Some(event)),
        // A compositor newer than this adapter emits events it has never
        // heard of — `BindingModeChanged` on the patched 26.04 killed the
        // whole stream and the shell's workspaces flickered on a
        // four-second reconnect loop, because an unknown *variant* ended
        // the session as if the protocol itself had broken. A foreign
        // event is not a broken stream: skip it and keep reading. A known
        // event that fails to decode still errors, because that is real
        // corruption this adapter must not paper over.
        Err(error)
            if error.is_data()
                && error.to_string().starts_with("unknown variant") =>
        {
            Ok(None)
        }
        Err(error) => Err(protocol_io(error)),
    }
}

fn stream_session(
    writer: &AdapterWriter,
    emitted_snapshot: &mut bool,
    homes: &mut Homes,
) -> Result<(), AdapterError> {
    let mut stream = open_event_stream()?;
    let mut state = EventStreamState::default();
    let mut sizes = OutputSizes::new();
    let mut have_workspaces = false;
    let mut have_windows = false;
    let mut last_snapshot = None;

    loop {
        let mut line = String::new();
        if stream.read_line(&mut line).map_err(AdapterError::Stream)? == 0 {
            return Err(AdapterError::Stream(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Niri closed the event stream",
            )));
        }
        let Some(event) = parse_shell_event(&line).map_err(AdapterError::Stream)? else {
            continue;
        };
        match &event {
            Event::WorkspacesChanged { .. } => have_workspaces = true,
            Event::WindowsChanged { .. } => have_windows = true,
            // Output scale — and with it every logical size — lives in the
            // compositor's config, so a reload is the one mid-session moment
            // the cached sizes can silently change under this adapter.
            Event::ConfigLoaded { .. } => sizes.mark_stale(),
            _ => {}
        }
        state.apply(event);

        if have_workspaces && have_windows {
            sizes.ensure(&state);
            let (snapshot, learned) = shell_snapshot(&state, homes, &sizes.by_name);
            // Persisted on learning rather than on publication: a frame that
            // taught nothing leaves the file alone, so an ordinary session of
            // switching workspaces never touches the disk.
            if learned {
                save_homes(homes);
            }
            if last_snapshot.as_ref() != Some(&snapshot) {
                // A frame the host would discard whole is skipped, not treated
                // as the end of the session. Ending it here would tear down the
                // compositor connection, publish `unavailable`, reconnect,
                // rebuild the same state and refuse the same frame again — a
                // reconnect loop where the previous behaviour was one dropped
                // line. Only a real write failure means this channel is gone.
                match writer.emit(&snapshot) {
                    Ok(()) => {
                        *emitted_snapshot = true;
                        last_snapshot = Some(snapshot);
                    }
                    Err(error) if error.is_fatal() => {
                        return Err(AdapterError::Emit(error));
                    }
                    Err(error) => {
                        eprintln!("celestina-niri-adapter: {error}");
                        // Deliberately not remembered as the last snapshot: the
                        // next state change should be offered rather than
                        // suppressed as a duplicate of one that never landed.
                    }
                }
            }
        }
    }
}

fn stream_forever(writer: &AdapterWriter) -> Result<(), AdapterError> {
    let mut last_error: Option<String> = None;
    // Loaded once and carried across reconnections. A compositor that went away
    // and came back has not forgotten which monitor a workspace belongs to, and
    // neither should this.
    let mut homes = load_homes();
    // Read once beside the memory, and again on every reconnection, which is
    // the cheap moment this helper already has. A declaration edited by hand
    // therefore takes effect at the next shell start rather than instantly; the
    // alternative is watching a file the other helper owns, which would be a
    // second idea of who owns settings for a repair that is made once.
    load_declarations(&mut homes);

    let mut attempt: u64 = 0;
    loop {
        let mut emitted_snapshot = false;
        attempt += 1;
        journal::record(
            Record::new(Level::Info, "niri.connect.attempt").with("attempt", Field::Uint(attempt)),
        );
        let error = match stream_session(writer, &mut emitted_snapshot, &mut homes) {
            Ok(()) => AdapterError::Rejected("event stream ended without an error".into()),
            Err(error) => error,
        };
        let reason = error.to_string();
        // A compositor that went away is adjacent to everything this journal is
        // trying to place: the same card drives the outputs Niri is describing.
        journal::record(
            Record::new(Level::Critical, "niri.disconnected")
                .with("attempt", Field::Uint(attempt))
                .with("published_snapshot", Field::Bool(emitted_snapshot))
                .with_text("reason", &reason),
        );

        if emitted_snapshot || last_error.as_deref() != Some(reason.as_str()) {
            eprintln!("celestina-niri-adapter: {reason}");
            emit_json(
                writer,
                &Unavailable {
                    kind: "unavailable",
                    reason: &reason,
                },
            )?;
        }
        last_error = Some(reason);
        thread::sleep(RECONNECT_DELAY);
    }
}

fn parse_command(line: &[u8]) -> Result<HostCommand, Rejection> {
    let command = serde_json::from_slice::<HostCommand>(line).map_err(|error| Rejection {
        id: serde_json::from_slice::<CommandEnvelope>(line)
            .ok()
            .and_then(|envelope| envelope.id)
            .filter(|id| !id.is_empty() && id.chars().count() <= MAX_ID_CHARS),
        reason: bounded(&error.to_string(), MAX_REASON_CHARS),
    })?;

    let id = command.id();
    if id.is_empty() || id.chars().count() > MAX_ID_CHARS {
        return Err(Rejection {
            // An unusable id cannot be echoed back: answering it would put
            // unbounded host input into a downstream frame.
            id: None,
            reason: "the command carries no usable request id".to_owned(),
        });
    }

    Ok(command)
}

/// One action, one short-lived socket. Every compositor request the host makes
/// goes through here, so "accepted" means the same thing for all of them: Niri
/// took it, not that the session already looks different.
fn perform(action: Action) -> Result<(), String> {
    // The action's kind, not its payload. A `FocusWorkspace` names a workspace
    // id the host already published; nothing here carries a window title.
    let kind = match &action {
        Action::FocusWorkspace { .. } => "focus-workspace",
        Action::Screenshot { .. } => "screenshot",
        Action::PowerOffMonitors { .. } => "power-off-monitors",
        Action::Quit { .. } => "quit",
        _ => "other",
    };
    // Critical: `power-off-monitors` and `quit` are the two things this shell
    // asks for that change what the graphics card is doing.
    journal::record_from(
        "niri-actions",
        Record::new(Level::Critical, "niri.action.start").with_text("action", kind),
    );
    let started = std::time::Instant::now();
    let answer = perform_inner(action);
    journal::record_from(
        "niri-actions",
        Record::new(Level::Critical, "niri.action.end")
            .with_text("action", kind)
            .with("ok", Field::Bool(answer.is_ok()))
            .with(
                "elapsed_ms",
                Field::Millis(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)),
            )
            .with_text("error", answer.as_ref().err().map_or("", String::as_str)),
    );
    answer
}

fn perform_inner(action: Action) -> Result<(), String> {
    let mut socket =
        Socket::connect().map_err(|error| format!("cannot connect to Niri: {error}"))?;
    let reply = socket
        .send(Request::Action(action))
        .map_err(|error| format!("cannot send the request: {error}"))?;

    match reply {
        Ok(Response::Handled) => Ok(()),
        Ok(response) => Err(format!("unexpected response: {response:?}")),
        Err(message) => Err(message),
    }
}

fn focus_workspace(workspace: &str) -> Result<(), String> {
    let id = workspace
        .parse::<u64>()
        .map_err(|_| "the request names an invalid workspace id".to_owned())?;

    perform(Action::FocusWorkspace {
        reference: WorkspaceReferenceArg::Id(id),
    })
}

/// Focuses one window by the id a snapshot published.
///
/// The id is parsed rather than forwarded: it arrives as text from the host and
/// the compositor takes a number, so a value that is not one is refused here
/// with a visible failure instead of being handed on.
fn focus_window(window: &str) -> Result<(), String> {
    let id = window
        .parse::<u64>()
        .map_err(|_| "the request names an invalid window id".to_owned())?;

    perform(Action::FocusWindow { id })
}

/// Asks Niri to start its own screenshot UI, which saves where the session's
/// `screenshot-path` already points. Capture belongs to the compositor: the
/// shell asks for it and reimplements none of it.
fn screenshot() -> Result<(), String> {
    perform(Action::Screenshot {
        // Niri's own default, so the panel's button does exactly what the
        // compositor's screenshot action does; its UI still toggles the pointer.
        show_pointer: true,
        // `None` means the configured `screenshot-path`.
        path: None,
    })
}

/// Performs one queued action at a time over its own short-lived socket, so a
/// blocked or slow compositor round-trip cannot stall the event stream.
fn run_commands(receiver: &Receiver<HostCommand>, writer: &AdapterWriter) {
    while let Ok(command) = receiver.recv() {
        let outcome = match &command {
            HostCommand::FocusWorkspace { workspace, .. } => focus_workspace(workspace),
            HostCommand::FocusWindow { window, .. } => focus_window(window),
            HostCommand::Screenshot { .. } => screenshot(),
            HostCommand::PowerOffMonitors { .. } => perform(Action::PowerOffMonitors {}),
            // The confirmation prompt is skipped because the shell already
            // asked: a second prompt the compositor draws over everything would
            // be answering a question nobody saw asked.
            HostCommand::Quit { .. } => perform(Action::Quit {
                skip_confirmation: true,
            }),
        };
        let frame = match &outcome {
            Ok(()) => RequestFrame::accepted(command.id()),
            Err(reason) => RequestFrame::failed(command.id(), reason),
        };
        if let Err(error) = emit_json(writer, &frame) {
            eprintln!("celestina-niri-adapter: {error}");
            return;
        }
    }
}

fn reject(writer: &AdapterWriter, rejection: &Rejection) {
    let Some(id) = rejection.id.as_deref() else {
        eprintln!(
            "celestina-niri-adapter: ignored an unusable host command: {}",
            rejection.reason
        );
        return;
    };
    if let Err(error) = emit_json(writer, &RequestFrame::failed(id, &rejection.reason)) {
        eprintln!("celestina-niri-adapter: {error}");
    }
}

fn queue_command(sender: &SyncSender<HostCommand>, writer: &AdapterWriter, command: HostCommand) {
    match sender.try_send(command) {
        Ok(()) => {}
        Err(TrySendError::Full(command)) => reject(
            writer,
            &Rejection {
                id: Some(command.id().to_owned()),
                reason: "the adapter's request queue is full".to_owned(),
            },
        ),
        Err(TrySendError::Disconnected(command)) => reject(
            writer,
            &Rejection {
                id: Some(command.id().to_owned()),
                reason: "the adapter's request worker is gone".to_owned(),
            },
        ),
    }
}

fn read_host_commands(sender: &SyncSender<HostCommand>, writer: &AdapterWriter) {
    let mut reader = BufReader::new(io::stdin());

    loop {
        match read_bounded_line(&mut reader) {
            Ok(HostLine::End) => return,
            Ok(HostLine::Oversized) => {
                eprintln!("celestina-niri-adapter: discarded an oversized host command");
            }
            Ok(HostLine::Complete(line)) => {
                if line.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                match parse_command(&line) {
                    Ok(command) => queue_command(sender, writer, command),
                    Err(rejection) => reject(writer, &rejection),
                }
            }
            Err(error) => {
                eprintln!("celestina-niri-adapter: cannot read host commands: {error}");
                return;
            }
        }
    }
}

fn run() -> Result<(), AdapterError> {
    let writer: AdapterWriter = Arc::new(SharedWriter::new(BufWriter::new(io::stdout())));
    let (sender, receiver) = sync_channel::<HostCommand>(COMMAND_QUEUE_CAPACITY);

    let worker_writer = Arc::clone(&writer);
    let worker = thread::Builder::new()
        .name("niri-actions".to_owned())
        .spawn(move || run_commands(&receiver, &worker_writer))
        .map_err(AdapterError::Spawn)?;

    let reader_writer = Arc::clone(&writer);
    thread::Builder::new()
        .name("host-commands".to_owned())
        .spawn(move || {
            read_host_commands(&sender, &reader_writer);
            // Our stdin closed, so the host is gone or shutting down. Close the
            // queue and let the worker finish the action it may still hold —
            // that much shuts down deterministically. The event stream cannot:
            // niri-ipc's blocking read has no cancellation point, so once the
            // worker is joined the adapter leaves on purpose instead of
            // outliving the shell that owns it.
            drop(sender);
            if worker.join().is_err() {
                eprintln!("celestina-niri-adapter: the request worker panicked");
            }
            journal::record(
                Record::new(Level::Critical, "helper.stop")
                    .with_text("helper", "niri-adapter")
                    .with_text("reason", "host-input-closed")
                    .with("ok", Field::Bool(true)),
            );
            // `process::exit` deliberately bypasses Rust destructors. Close
            // the process journal first so the ordinary host shutdown keeps
            // its final correlated event instead of abandoning the writer.
            journal::close_process_journal();
            process::exit(0);
        })
        .map_err(AdapterError::Spawn)?;

    stream_forever(&writer)
}

fn main() -> ExitCode {
    journal::install(Journal::for_component(
        "niri-adapter",
        u64::from(process::id()),
    ));
    journal::record(
        Record::new(Level::Critical, "helper.start")
            .with_text("helper", "niri-adapter")
            .with_text("version", env!("CARGO_PKG_VERSION"))
            .with(
                "argument_count",
                Field::Uint(std::env::args().skip(1).count() as u64),
            ),
    );

    let outcome = run();
    journal::record(
        Record::new(Level::Critical, "helper.stop")
            .with_text("helper", "niri-adapter")
            .with("ok", Field::Bool(outcome.is_ok()))
            .with_text(
                "error",
                &outcome
                    .as_ref()
                    .err()
                    .map_or(String::new(), ToString::to_string),
            ),
    );
    journal::close_process_journal();

    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("celestina-niri-adapter: fatal: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Cursor;

    fn apply_json(state: &mut EventStreamState, json: &str) {
        let event = serde_json::from_str::<Event>(json).expect("valid Niri event fixture");
        state.apply(event);
    }

    #[test]
    fn compositor_native_minimization_does_not_break_shell_state_stream() {
        assert!(
            parse_shell_event(r#"{"MinimizedWindowsChanged":{"windows":[{"id":42}]}}"#)
                .expect("recognized extension")
                .is_none()
        );
        assert!(matches!(
            parse_shell_event(r#"{"WindowFocusChanged":{"id":42}}"#),
            Ok(Some(Event::WindowFocusChanged { id: Some(42) }))
        ));
        // A compositor newer than this adapter: foreign events are skipped,
        // never a stream-ending error (the 26.04 `BindingModeChanged` loop).
        assert!(
            parse_shell_event(r#"{"BindingModeChanged":{"name":"resize"}}"#)
                .expect("foreign event tolerated")
                .is_none()
        );
        // A known event that fails to decode is still real corruption.
        assert!(parse_shell_event(r#"{"WindowFocusChanged":"not-an-object"}"#).is_err());
    }

    /// A snapshot taken against a memory that knows nothing, which is what every
    /// case below except the grouping ones is about. A workspace with no known
    /// home reports the output it is on, so these keep asserting the contract
    /// they asserted before homes existed.
    fn snapshot_of(state: &EventStreamState) -> ShellSnapshot {
        shell_snapshot(state, &mut Homes::new(), &HashMap::new()).0
    }

    /// The author's centre monitor: 3840x2160 at scale 1.5.
    fn dp1_sized() -> HashMap<String, (f64, f64)> {
        HashMap::from([("DP-1".to_owned(), (2560.0, 1440.0))])
    }

    #[test]
    fn a_fullscreen_sized_tile_names_its_output() {
        let state = workspace_holding(&window_json(1, "game", "null", "[2560.0,1440.0]", false));

        assert_eq!(
            fullscreen_outputs(&state, &dp1_sized()),
            vec!["DP-1".to_owned()]
        );
        let snapshot = shell_snapshot(&state, &mut Homes::new(), &dp1_sized()).0;
        assert_eq!(snapshot.fullscreen_outputs, vec!["DP-1".to_owned()]);
    }

    #[test]
    fn a_maximized_tile_is_not_fullscreen() {
        // The full width, but not the full height: a maximized tile excludes
        // the panel's exclusive zone, which is exactly what tells it apart.
        let state = workspace_holding(&window_json(1, "editor", "[1,1]", "[2560.0,1400.0]", false));

        assert!(fullscreen_outputs(&state, &dp1_sized()).is_empty());
    }

    #[test]
    fn an_output_without_a_known_size_reports_nothing() {
        let state = workspace_holding(&window_json(1, "game", "null", "[2560.0,1440.0]", false));

        assert!(fullscreen_outputs(&state, &HashMap::new()).is_empty());
        assert!(snapshot_of(&state).fullscreen_outputs.is_empty());
    }

    #[test]
    fn only_the_active_workspace_can_hold_the_fullscreen_tenant() {
        let mut state = EventStreamState::default();
        apply_json(
            &mut state,
            r#"{"WorkspacesChanged":{"workspaces":[{"id":3,"idx":1,"name":"one","output":"DP-1","is_urgent":false,"is_active":false,"is_focused":false,"active_window_id":null},{"id":4,"idx":2,"name":"two","output":"DP-1","is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":null}]}}"#,
        );
        // The fullscreen-sized window sits on the inactive workspace: nothing
        // is scanning it out, so nothing needs yielding.
        apply_json(
            &mut state,
            &format!(
                r#"{{"WindowsChanged":{{"windows":[{}]}}}}"#,
                window_json(1, "game", "null", "[2560.0,1440.0]", false)
            ),
        );

        assert!(fullscreen_outputs(&state, &dp1_sized()).is_empty());
    }

    /// Two monitors, one named workspace each, and no windows anywhere.
    fn two_outputs(state: &mut EventStreamState) {
        apply_json(
            state,
            r#"{"WorkspacesChanged":{"workspaces":[{"id":3,"idx":1,"name":"left","output":"HDMI-A-1","is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":null},{"id":4,"idx":1,"name":"right","output":"DP-1","is_urgent":false,"is_active":true,"is_focused":false,"active_window_id":null}]}}"#,
        );
        apply_json(state, r#"{"WindowsChanged":{"windows":[]}}"#);
    }

    /// The same two workspaces after `DP-1` was switched off: the compositor has
    /// moved `right` onto the survivor and no longer says where it came from.
    fn displaced_onto_one(state: &mut EventStreamState) {
        apply_json(
            state,
            r#"{"WorkspacesChanged":{"workspaces":[{"id":3,"idx":1,"name":"left","output":"HDMI-A-1","is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":null},{"id":4,"idx":2,"name":"right","output":"HDMI-A-1","is_urgent":false,"is_active":false,"is_focused":false,"active_window_id":null}]}}"#,
        );
        apply_json(state, r#"{"WindowsChanged":{"windows":[]}}"#);
    }

    fn home_of<'a>(snapshot: &'a ShellSnapshot, label: &str) -> &'a str {
        &snapshot
            .workspaces
            .iter()
            .find(|workspace| workspace.label == label)
            .expect("the fixture publishes this workspace")
            .home
    }

    /// One workspace, and whatever windows the caller describes on it.
    fn workspace_holding(windows: &str) -> EventStreamState {
        let mut state = EventStreamState::default();
        apply_json(
            &mut state,
            r#"{"WorkspacesChanged":{"workspaces":[{"id":3,"idx":1,"name":"one","output":"DP-1","is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":null}]}}"#,
        );
        apply_json(
            &mut state,
            &format!(r#"{{"WindowsChanged":{{"windows":[{windows}]}}}}"#),
        );
        state
    }

    /// One window's JSON, with the layout fields the map is drawn from.
    fn window_json(id: u64, title: &str, place: &str, size: &str, floating: bool) -> String {
        format!(
            r#"{{"id":{id},"title":"{title}","app_id":"app","pid":100,"workspace_id":3,"is_focused":false,"is_floating":{floating},"is_urgent":false,"layout":{{"pos_in_scrolling_layout":{place},"tile_size":{size},"window_size":[800,600],"tile_pos_in_workspace_view":null,"window_offset_in_tile":[0.0,0.0]}},"focus_timestamp":null}}"#
        )
    }

    #[test]
    fn a_workspace_publishes_the_columns_its_windows_are_in() {
        let state = workspace_holding(
            &[
                window_json(1, "second column", "[2,1]", "[800.0,600.0]", false),
                window_json(2, "first column lower", "[1,2]", "[800.0,300.0]", false),
                window_json(3, "first column upper", "[1,1]", "[800.0,300.0]", false),
            ]
            .join(","),
        );

        let snapshot = snapshot_of(&state);
        let map = &snapshot.workspaces[0].map;

        // The arrangement itself is the core's rule; what is proved here is that
        // the adapter carries the compositor's windows to it and its answer out.
        assert_eq!(map.columns.len(), 2);
        assert_eq!(map.columns[0].windows.len(), 2);
        assert_eq!(map.columns[0].windows[0].title, "first column upper");
        assert_eq!(map.columns[1].windows[0].title, "second column");
    }

    #[test]
    fn a_floating_window_is_published_outside_the_layout() {
        let state = workspace_holding(
            &[
                window_json(1, "floating", "null", "[400.0,300.0]", true),
                window_json(2, "tiled", "[1,1]", "[800.0,600.0]", false),
            ]
            .join(","),
        );

        let snapshot = snapshot_of(&state);
        let map = &snapshot.workspaces[0].map;

        assert_eq!(map.columns.len(), 1);
        assert_eq!(map.columns[0].windows[0].title, "tiled");
        assert_eq!(map.floating.len(), 1);
        assert_eq!(map.floating[0].title, "floating");
    }

    #[test]
    fn an_impossible_measure_never_reaches_the_wire_as_a_share() {
        let state = workspace_holding(&window_json(
            1,
            "impossible",
            "[1,1]",
            "[-5.0,600.0]",
            false,
        ));

        let snapshot = snapshot_of(&state);
        let map = &snapshot.workspaces[0].map;

        // A share that is not finite and positive reaches a layout as a surface
        // that silently fails to draw, so no input may produce one.
        assert!(map.columns[0].width_share.is_finite());
        assert!(map.columns[0].width_share > 0.0);
        assert!(map.columns[0].windows[0].height_share.is_finite());
        assert!(map.columns[0].windows[0].height_share > 0.0);
    }

    #[test]
    fn a_hostile_window_title_is_bounded_before_it_is_published() {
        let title = "T".repeat(MAX_TITLE_UNITS + 200);
        let state = workspace_holding(&window_json(1, &title, "[1,1]", "[800.0,600.0]", false));

        let snapshot = snapshot_of(&state);

        assert_eq!(
            snapshot.workspaces[0].map.columns[0].windows[0]
                .title
                .chars()
                .count(),
            MAX_TITLE_UNITS
        );
    }

    #[test]
    fn a_workspace_past_the_bounds_says_how_much_it_is_hiding() {
        let windows: Vec<String> = (1..=workspace_map::MAX_WINDOWS + 4)
            .map(|index| {
                window_json(
                    index as u64,
                    &format!("window {index}"),
                    &format!("[{index},1]"),
                    "[800.0,600.0]",
                    false,
                )
            })
            .collect();
        let state = workspace_holding(&windows.join(","));

        let snapshot = snapshot_of(&state);
        let map = &snapshot.workspaces[0].map;

        // Not silently four of nine: whatever the bounds dropped is counted, so
        // the surface can say it is not showing everything.
        assert!(map.hidden > 0);
        assert_eq!(
            map.columns.len() + map.hidden,
            workspace_map::MAX_WINDOWS + 4
        );
    }

    #[test]
    fn a_workspace_holding_nothing_publishes_an_empty_map() {
        let state = workspace_holding("");

        let snapshot = snapshot_of(&state);
        let map = &snapshot.workspaces[0].map;

        assert!(map.columns.is_empty());
        assert!(map.floating.is_empty());
        assert_eq!(map.hidden, 0);
    }

    #[test]
    fn a_workspace_with_no_known_home_reports_the_output_it_is_on() {
        let mut state = EventStreamState::default();
        displaced_onto_one(&mut state);

        // Nothing has ever seen two outputs, so there is nothing to know and the
        // field says exactly what the strip already knew.
        let snapshot = snapshot_of(&state);

        assert_eq!(home_of(&snapshot, "left"), "HDMI-A-1");
        assert_eq!(home_of(&snapshot, "right"), "HDMI-A-1");
    }

    /// The session that found this: five named workspaces per monitor plus
    /// niri's unnamed spare at each monitor's sixth position, and a *named*
    /// workspace "6" on one of them. The spare's display label is its index —
    /// also "6" — and looking that label up in the homes memory dressed every
    /// other monitor's spare in the named workspace's home, which the strip
    /// then drew as a foreign monitor group.
    #[test]
    fn an_unnamed_spare_does_not_borrow_a_named_workspaces_home() {
        let mut state = EventStreamState::default();
        apply_json(
            &mut state,
            r#"{"WorkspacesChanged":{"workspaces":[
                {"id":10,"idx":1,"name":"6","output":"DP-1","is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":null},
                {"id":11,"idx":6,"name":null,"output":"DP-1","is_urgent":false,"is_active":false,"is_focused":false,"active_window_id":null},
                {"id":20,"idx":1,"name":"11","output":"DP-2","is_urgent":false,"is_active":true,"is_focused":false,"active_window_id":null},
                {"id":21,"idx":6,"name":null,"output":"DP-2","is_urgent":false,"is_active":false,"is_focused":false,"active_window_id":null}
            ]}}"#,
        );
        apply_json(&mut state, r#"{"WindowsChanged":{"windows":[]}}"#);

        let mut homes = Homes::new();
        // The person's own declaration, the strongest claim a home can have.
        homes.declare("6", "DP-1");
        let (snapshot, _) = shell_snapshot(&state, &mut homes, &HashMap::new());

        // The named "6" answers to its declaration.
        let named = snapshot
            .workspaces
            .iter()
            .find(|workspace| workspace.label == "6" && workspace.output == "DP-1")
            .expect("the named workspace is published");
        assert_eq!(named.home, "DP-1");

        // The spare wearing "6" as its display label belongs where it is.
        let spare = snapshot
            .workspaces
            .iter()
            .find(|workspace| workspace.label == "6" && workspace.output == "DP-2")
            .expect("the spare is published");
        assert_eq!(
            spare.home, "DP-2",
            "an index fallback is not an identity the memory may answer for"
        );

        // And it taught the memory nothing: "6" still means the declaration,
        // and no spare's sighting was recorded under that label.
        assert_eq!(homes.home_of("6"), Some("DP-1"));
    }

    #[test]
    fn a_two_output_frame_learns_and_publishes_both_homes() {
        let mut state = EventStreamState::default();
        two_outputs(&mut state);
        let mut homes = Homes::new();

        let (snapshot, learned) = shell_snapshot(&state, &mut homes, &HashMap::new());

        assert!(learned);
        assert_eq!(home_of(&snapshot, "left"), "HDMI-A-1");
        assert_eq!(home_of(&snapshot, "right"), "DP-1");
    }

    #[test]
    fn a_displaced_workspace_keeps_the_home_it_was_taught() {
        let mut state = EventStreamState::default();
        two_outputs(&mut state);
        let mut homes = Homes::new();
        shell_snapshot(&state, &mut homes, &HashMap::new());

        // The monitor goes away. This is the frame that would overwrite the
        // memory if the memory let it, and the whole feature rests on it not.
        let mut displaced = EventStreamState::default();
        displaced_onto_one(&mut displaced);
        let (snapshot, learned) = shell_snapshot(&displaced, &mut homes, &HashMap::new());

        assert!(!learned);
        assert_eq!(home_of(&snapshot, "right"), "DP-1");
        assert_eq!(home_of(&snapshot, "left"), "HDMI-A-1");
    }

    #[test]
    fn a_single_output_frame_teaches_the_memory_nothing() {
        let mut state = EventStreamState::default();
        displaced_onto_one(&mut state);
        let mut homes = Homes::new();

        let (_, learned) = shell_snapshot(&state, &mut homes, &HashMap::new());

        assert!(!learned);
        assert!(homes.is_empty());
    }

    /// The author's own displaced case, small enough to assert on: two monitors
    /// taught, then one switched off so both workspaces arrive on the survivor.
    fn taught_then_displaced() -> ShellSnapshot {
        let mut learning = EventStreamState::default();
        two_outputs(&mut learning);
        let mut homes = Homes::new();
        shell_snapshot(&learning, &mut homes, &HashMap::new());

        let mut state = EventStreamState::default();
        displaced_onto_one(&mut state);
        shell_snapshot(&state, &mut homes, &HashMap::new()).0
    }

    fn workspace<'a>(snapshot: &'a ShellSnapshot, label: &str) -> &'a WorkspaceSnapshot {
        snapshot
            .workspaces
            .iter()
            .find(|workspace| workspace.label == label)
            .expect("the fixture publishes this workspace")
    }

    #[test]
    fn a_strip_of_one_monitor_publishes_every_workspace_expanded() {
        let mut state = EventStreamState::default();
        two_outputs(&mut state);

        // Two outputs, one workspace each: each strip has a single group, so
        // there is nothing to collapse and the surface draws what it always
        // drew.
        let snapshot = snapshot_of(&state);

        assert!(snapshot
            .workspaces
            .iter()
            .all(|workspace| workspace.group == GroupState::Expanded));
    }

    #[test]
    fn a_displaced_strip_opens_only_the_group_holding_the_focus() {
        let snapshot = taught_then_displaced();

        // `left` is the active one in the displaced fixture, so its group opens
        // and the monitor that went away arrives closed rather than as five more
        // equal pills.
        assert_eq!(workspace(&snapshot, "left").group, GroupState::Expanded);
        assert_ne!(workspace(&snapshot, "right").group, GroupState::Expanded);
    }

    #[test]
    fn a_closed_group_names_the_workspace_its_capsule_asks_for() {
        let snapshot = taught_then_displaced();

        // Nothing on `DP-1` is active any more, so the capsule asks for that
        // group's first workspace rather than for nothing.
        assert_eq!(
            workspace(&snapshot, "right").group,
            GroupState::CollapsedTarget
        );
    }

    #[test]
    fn a_declared_home_regroups_a_strip_that_had_learned_otherwise() {
        let mut learning = EventStreamState::default();
        two_outputs(&mut learning);
        let mut homes = Homes::new();
        shell_snapshot(&learning, &mut homes, &HashMap::new());

        // The person moved that workspace in their Niri configuration and says
        // so in the shell's settings; the observation underneath is overruled
        // without anybody having to find and delete it.
        homes.set_declarations([("right", "HDMI-A-1")]);
        let mut state = EventStreamState::default();
        displaced_onto_one(&mut state);
        let snapshot = shell_snapshot(&state, &mut homes, &HashMap::new()).0;

        assert_eq!(home_of(&snapshot, "right"), "HDMI-A-1");
        // One group now, so the strip is flat again and nothing is collapsed.
        assert_eq!(workspace(&snapshot, "right").group, GroupState::Expanded);
        assert_eq!(workspace(&snapshot, "left").group, GroupState::Expanded);
    }

    #[test]
    fn snapshot_is_sorted_and_joins_the_active_window() {
        let mut state = EventStreamState::default();
        apply_json(
            &mut state,
            r#"{"WorkspacesChanged":{"workspaces":[{"id":8,"idx":2,"name":null,"output":"DP-1","is_urgent":false,"is_active":false,"is_focused":false,"active_window_id":null},{"id":3,"idx":1,"name":"web","output":"DP-1","is_urgent":true,"is_active":true,"is_focused":true,"active_window_id":42}]}}"#,
        );
        apply_json(
            &mut state,
            r#"{"WindowsChanged":{"windows":[{"id":42,"title":"Niri docs","app_id":"zen","pid":100,"workspace_id":3,"is_focused":true,"is_floating":false,"is_urgent":false,"layout":{"pos_in_scrolling_layout":[1,1],"tile_size":[800.0,600.0],"window_size":[800,600],"tile_pos_in_workspace_view":null,"window_offset_in_tile":[0.0,0.0]},"focus_timestamp":null}]}}"#,
        );

        let snapshot = snapshot_of(&state);
        assert_eq!(snapshot.workspaces.len(), 2);
        assert_eq!(snapshot.workspaces[0].label, "web");
        assert_eq!(
            snapshot.workspaces[0].active_window_title.as_deref(),
            Some("Niri docs")
        );
        assert_eq!(snapshot.workspaces[1].label, "2");
    }

    #[test]
    fn snapshot_changes_after_incremental_window_event() {
        let mut state = EventStreamState::default();
        apply_json(
            &mut state,
            r#"{"WorkspacesChanged":{"workspaces":[{"id":3,"idx":1,"name":"one","output":"DP-1","is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":42}]}}"#,
        );
        apply_json(
            &mut state,
            r#"{"WindowsChanged":{"windows":[{"id":42,"title":"Terminal","app_id":"kitty","pid":100,"workspace_id":3,"is_focused":true,"is_floating":false,"is_urgent":false,"layout":{"pos_in_scrolling_layout":[1,1],"tile_size":[800.0,600.0],"window_size":[800,600],"tile_pos_in_workspace_view":null,"window_offset_in_tile":[0.0,0.0]},"focus_timestamp":null}]}}"#,
        );
        let before = snapshot_of(&state);

        apply_json(
            &mut state,
            r#"{"WindowOpenedOrChanged":{"window":{"id":42,"title":"Editor","app_id":"kitty","pid":100,"workspace_id":3,"is_focused":true,"is_floating":false,"is_urgent":false,"layout":{"pos_in_scrolling_layout":[1,1],"tile_size":[800.0,600.0],"window_size":[800,600],"tile_pos_in_workspace_view":null,"window_offset_in_tile":[0.0,0.0]},"focus_timestamp":null}}}"#,
        );
        let after = snapshot_of(&state);

        assert_ne!(before, after);
        assert_eq!(
            after.workspaces[0].active_window_title.as_deref(),
            Some("Editor")
        );
    }

    #[test]
    fn snapshot_omits_workspaces_without_an_output() {
        let mut state = EventStreamState::default();
        apply_json(
            &mut state,
            r#"{"WorkspacesChanged":{"workspaces":[{"id":3,"idx":1,"name":"detached","output":null,"is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":null}]}}"#,
        );

        assert!(snapshot_of(&state).workspaces.is_empty());
    }

    #[test]
    fn snapshot_carries_the_workspace_id_as_a_string() {
        let mut state = EventStreamState::default();
        apply_json(
            &mut state,
            r#"{"WorkspacesChanged":{"workspaces":[{"id":18446744073709551615,"idx":1,"name":"big","output":"DP-1","is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":null}]}}"#,
        );

        let snapshot = snapshot_of(&state);
        assert_eq!(snapshot.workspaces[0].id, "18446744073709551615");
        let encoded = serde_json::to_string(&snapshot).expect("the snapshot serializes");
        // A JSON number would reach the host's double-typed parser rounded.
        assert!(encoded.contains(r#""id":"18446744073709551615""#));
    }

    #[test]
    fn snapshot_bounds_compositor_text_the_host_would_reject() {
        // A window title is client-controlled, and the workspace name and
        // output name are compositor state this helper does not own. Published
        // unbounded, one of them pushes the line past the host's framing limit,
        // which discards the whole line and blanks the strip for as long as the
        // window lives.
        let mut state = EventStreamState::default();
        let name = "n".repeat(MAX_LABEL_UNITS + 40);
        let output = "O".repeat(MAX_LABEL_UNITS + 40);
        apply_json(
            &mut state,
            &format!(
                r#"{{"WorkspacesChanged":{{"workspaces":[{{"id":3,"idx":1,"name":"{name}","output":"{output}","is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":42}}]}}}}"#
            ),
        );
        let title = "t".repeat(MAX_TITLE_UNITS * 4);
        apply_json(
            &mut state,
            &format!(
                r#"{{"WindowsChanged":{{"windows":[{{"id":42,"title":"{title}","app_id":"hostile","pid":100,"workspace_id":3,"is_focused":true,"is_floating":false,"is_urgent":false,"layout":{{"pos_in_scrolling_layout":[1,1],"tile_size":[800.0,600.0],"window_size":[800,600],"tile_pos_in_workspace_view":null,"window_offset_in_tile":[0.0,0.0]}},"focus_timestamp":null}}]}}}}"#
            ),
        );

        let snapshot = snapshot_of(&state);
        let workspace = &snapshot.workspaces[0];
        assert_eq!(workspace.label.chars().count(), MAX_LABEL_UNITS);
        assert_eq!(workspace.output.chars().count(), MAX_LABEL_UNITS);
        assert_eq!(
            workspace
                .active_window_title
                .as_ref()
                .map(|title| title.chars().count()),
            Some(MAX_TITLE_UNITS)
        );
    }

    #[test]
    fn snapshot_caps_the_number_of_workspaces() {
        // The host rejects a workspace list longer than its own cap outright,
        // so publishing one would cost the whole snapshot rather than its tail.
        let mut state = EventStreamState::default();
        let workspaces = (0..3)
            .flat_map(|output| (1..=250).map(move |index| (output, index)))
            .map(|(output, index): (u8, u8)| {
                let id = u64::from(output) * 1000 + u64::from(index);
                format!(
                    r#"{{"id":{id},"idx":{index},"name":null,"output":"DP-{output}","is_urgent":false,"is_active":false,"is_focused":false,"active_window_id":null}}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        apply_json(
            &mut state,
            &format!(r#"{{"WorkspacesChanged":{{"workspaces":[{workspaces}]}}}}"#),
        );

        let snapshot = snapshot_of(&state);
        assert_eq!(snapshot.workspaces.len(), MAX_WORKSPACES);
        // The kept prefix is the sorted one, so the same workspaces survive
        // every event instead of a set that depends on map iteration order.
        assert_eq!(snapshot.workspaces[0].output, "DP-0");
        assert_eq!(snapshot.workspaces[0].index, 1);
    }

    #[test]
    fn a_focus_command_parses_its_request_and_workspace_ids() {
        let command = parse_command(br#"{"kind":"focus-workspace","id":"7","workspace":"12"}"#)
            .expect("a well-formed command");

        assert_eq!(
            command,
            HostCommand::FocusWorkspace {
                id: "7".to_owned(),
                workspace: "12".to_owned(),
            }
        );
        assert_eq!(command.id(), "7");
    }

    #[test]
    fn a_screenshot_command_carries_only_its_request_id() {
        // The compositor decides everything else — where it saves, whether the
        // pointer shows — so the host has nothing else to send.
        let command =
            parse_command(br#"{"kind":"screenshot","id":"7"}"#).expect("a well-formed command");

        assert_eq!(command, HostCommand::Screenshot { id: "7".to_owned() });
        assert_eq!(command.id(), "7");
    }

    #[test]
    fn an_unreadable_command_is_rejected_against_its_own_request_id() {
        let rejection = parse_command(br#"{"kind":"launch-rocket","id":"9"}"#)
            .expect_err("an unknown command kind is refused");

        assert_eq!(rejection.id.as_deref(), Some("9"));
        assert!(!rejection.reason.is_empty());
    }

    #[test]
    fn a_command_without_a_usable_id_is_never_echoed_back() {
        let long = "x".repeat(MAX_ID_CHARS + 1);
        let line = format!(r#"{{"kind":"focus-workspace","id":"{long}","workspace":"12"}}"#);

        let rejection = parse_command(line.as_bytes()).expect_err("an oversized id is refused");
        assert_eq!(rejection.id, None);
    }

    #[test]
    fn a_command_survives_the_line_the_framing_discarded() {
        // The framing itself belongs to the shared crate now; what belongs here
        // is that this adapter's own commands still parse on the far side of a
        // discarded line.
        let mut input = Vec::new();
        input.extend(std::iter::repeat_n(
            b'x',
            celestina_shell_core::lines::MAX_LINE_BYTES + 64,
        ));
        input.push(b'\n');
        input.extend_from_slice(br#"{"kind":"focus-workspace","id":"7","workspace":"12"}"#);
        input.push(b'\n');
        let mut reader = Cursor::new(input);

        assert_eq!(
            read_bounded_line(&mut reader).expect("reading does not fail"),
            HostLine::Oversized
        );
        let HostLine::Complete(line) =
            read_bounded_line(&mut reader).expect("reading does not fail")
        else {
            panic!("the command after an oversized line is lost");
        };
        assert_eq!(
            parse_command(&line).expect("a well-formed command").id(),
            "7"
        );
    }

    #[test]
    fn a_failed_request_frame_carries_a_bounded_reason() {
        let frame = RequestFrame::failed("7", &"z".repeat(MAX_REASON_CHARS + 50));

        assert_eq!(frame.state, "failed");
        assert_eq!(
            frame.reason.as_ref().map(|reason| reason.chars().count()),
            Some(MAX_REASON_CHARS)
        );
    }
}
