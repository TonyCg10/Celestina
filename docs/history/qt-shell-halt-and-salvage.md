# The Qt/QML shell: halt and salvage record

Moved verbatim from the root `AGENTS.md`, section "Halted projects", on 2026-11-24.

**History: the Qt/QML shell.** The author halted the Qt/QML Celestina shell
on 2026-09-27: the `celestina-qt/` project (moved from `celestina/` on
2026-10-14; id and prefix `celestina-qt`) and the crate registered only for it,
`celestina-rs/crates/celestina-shell-core` (prefix `celestina-shell-core`).
The unit CEL-QT-REMOVE removed both from the tree by the author's decision
of 2026-10-23: no tag, branch or copy keeps them, their bytes remain only in
the Git history before that commit, and `docs/projects.toml` lists the shell
as a retired project.

**Salvage record.** Before the removal, the Rust shell at `celestina/` copied
or read these files of the halted tree, never editing them; each copy records
in its head comment the source path and the commit it came from. The paths no
longer exist in the tree; the list stays as the provenance those head comments
cite:
`celestina-rs/crates/celestina-shell-core/src/{notifications,launcher,coalesce,workspace_groups,workspace_map,settings,calendar}.rs`
and the `bounded` helper of that crate's `src/lib.rs` (there is no
`bounded.rs`) into `celestina-rs/crates/celestina-shell`;
`celestina-qt/src/provider_adapter/{worker,notifications,launcher}.rs`
and `celestina-qt/src/niri_adapter.rs` as reference for
`celestina/src/providers`; `celestina-qt/scripts/dev-session.kdl` and
`dev-session.sh` for `celestina/scripts`. The CEL-2 parity program extended
the list in the same way, each time before the first commit of the unit it
served:

- CEL-2-B: `celestina-qt/src/provider_adapter/{nightlight,session,sessionholds}.rs`
  as reference for `celestina/src/providers`, and
  `celestina-rs/crates/celestina-shell-core/src/{nightlight,session,pending}.rs`
  into `celestina-rs/crates/celestina-shell`.
- CEL-2-E: from `celestina-qt/src/provider_adapter/wallpaper.rs` only the image
  selection, as reference for `celestina/src/providers`, and
  `celestina-rs/crates/celestina-shell-core/src/wallpaper.rs` into
  `celestina-rs/crates/celestina-shell`.
- CEL-2-C: `celestina-qt/src/provider_adapter/{audio,brightness}.rs` as
  reference for `celestina/src/providers`, and
  `celestina-rs/crates/celestina-shell-core/src/{audio,brightness}.rs` into
  `celestina-rs/crates/celestina-shell`.
- CEL-2-D: `celestina-qt/src/provider_adapter/media.rs` as reference for
  `celestina/src/providers`, and
  `celestina-rs/crates/celestina-shell-core/src/media.rs` into
  `celestina-rs/crates/celestina-shell`.
- CEL-2-F: `celestina-rs/crates/celestina-shell-core/src/{connectivity,network,bluetooth,power}.rs`
  into `celestina-rs/crates/celestina-shell`, as far as read-only state needs.
- CEL-2-G: `celestina-qt/src/provider_adapter/clipboard.rs` as reference for
  `celestina/src/providers`, and
  `celestina-rs/crates/celestina-shell-core/src/clipboard.rs` into
  `celestina-rs/crates/celestina-shell`.
- CEL-2-H (CEL-2-H-a and CEL-2-H-b): as reference only, nothing copied:
  `celestina-qt/docs/decisions/0004-first-party-session-lock.md`,
  `celestina-qt/src/lockverify/main.cpp`, `celestina-qt/src/lockauthenticator.cpp`,
  `celestina-qt/src/lockcontroller.cpp` and `celestina-qt/src/lock/`, for
  `celestina/crates/celestina-lock`, `celestina/crates/celestina-lock-verify`
  and `celestina/src/lock`. ADR 0004 of the halted shell is re-decided in
  `celestina/docs/decisions/`, adapted and re-reviewed; its file is not
  copied.
- CEL-2-I: nothing is copied. ADR 0005 of the halted shell is re-decided in
  `celestina/docs/decisions/`, adapted and re-reviewed; its file is not copied.
  `celestina-qt/src/polkit{agent,conversation,promptcontroller}.cpp` and
  `celestina-qt/src/polkitconverse/main.cpp` are read-only reference for
  `celestina/src/providers/polkit`.
- CEL-2-J: `celestina-qt/src/provider_adapter/melibea.rs` as reference for
  `celestina/src/providers`, and
  `celestina-rs/crates/celestina-shell-core/src/melibea.rs` into
  `celestina-rs/crates/celestina-shell`.
- CEL-2-K: `celestina-qt/src/provider_adapter/{sysmon,weather}.rs` as
  reference for `celestina/src/providers`, and
  `celestina-rs/crates/celestina-shell-core/src/{sysmon,weather}.rs` into
  `celestina-rs/crates/celestina-shell`; `calendar.rs` is already listed.
- CEL-2-L: nothing beyond `workspace_map.rs`, which is already listed.
- CEL-QT-RESCUE: `celestina-qt/packaging/niri/{README.md,per-layer-blur-strength.patch}`
  and `celestina-qt/scripts/build-patched-niri.sh` into `celestina/packaging/niri`
  (the script beside the patch it applies), and
  `celestina-rs/crates/celestina-shell-core/src/niri_colours.rs` into
  `celestina-rs/crates/celestina-tokens/src`, the last salvage before the
  removal.
