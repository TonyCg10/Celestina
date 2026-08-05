# Host hygiene and external application containment

## Purpose

This operator policy governs explicit, read-only audits of the machine that
hosts the Celestina ecosystem. It separates observed system state from later
proposals to keep, remove, contain, or replace software.

Celestina is one possible destination for a replacement, not the scope of this
policy. A finding does not enter a Celestina roadmap automatically. This file
is not mandatory context for ordinary product work and grants no authority to
install, remove, reconfigure, enable, or disable anything.

## Priority order

Evaluate findings in this order:

1. Prevent unnecessary foreign dependency and library closures from becoming
   part of the native host.
2. Prevent external applications, services, portals, and agents from claiming
   desktop responsibilities that belong to Niri and Celestina.
3. Consider disk usage only after host ownership and containment are sound.

A large GNOME or KDE runtime contained inside Flatpak is preferable to a much
smaller native package that expands the host platform or competes for desktop
authority. Visual uniformity is not an audit objective.

## Host boundary

The native host may contain:

- the CachyOS/Arch base and hardware support required by the machine;
- Niri and the explicitly selected Wayland/session foundation;
- Celestina, its Qt/Rust stack, and its registered applications;
- explicitly selected system services with a demonstrated consumer;
- lean native applications whose complete closure fits this boundary; and
- named retained exceptions whose cost and owner are understood.

Flatpak and equivalent sandboxed distributions are external territory. Their
private runtimes may contain other desktop stacks without making those stacks
part of the native host. Audit their permissions and exported integration, but
do not treat their contained library size as native dependency noise.

## Classification

| Class | Meaning |
|---|---|
| Host foundation | Required operating-system, hardware, or session substrate |
| Celestina platform | First-party ecosystem component or its owned runtime |
| Chosen service | External service deliberately retained for a named consumer |
| Lean native application | Native application with a proportional, compatible closure |
| Contained external application | Sandboxed application whose foreign stack stays private |
| Foreign desktop closure | Native GNOME/KDE or other desktop stack pulled by limited needs |
| Competing desktop authority | Service, portal, agent, or handler overlapping an owned role |
| Retained exception | Known cost accepted because its value currently wins |
| Unowned residue | Package, service, or configuration with no demonstrated consumer |

A project name, toolkit, or package group is not evidence by itself. Classify
an item only after identifying why it is installed, its reverse consumers, its
runtime behavior, and the responsibility it claims.

## Read-only audit protocol

Record the date, installed revisions, and command results before proposing any
change. Empty output is an observation, not an error to hide. Do not use
`sudo`, mutate package reasons, refresh databases, or update packages during an
audit.

### 1. Native packages and dependency ownership

```sh
pacman -Qqe
pacman -Qqd
pacman -Qdtq
pacman -Qqm
pacman -Qi PACKAGE
pactree PACKAGE
pactree -r PACKAGE
```

Use explicit packages as the starting intent, dependency-installed packages as
supporting state, and orphan reports only as leads. For every suspected foreign
closure, trace the retained application or service that requires it.

### 2. Running and enabled authority

```sh
systemctl list-unit-files --state=enabled
systemctl --user list-unit-files --state=enabled
systemctl list-units --type=service --state=running
systemctl --user list-units --type=service --state=running
busctl --user list
```

Identify the owner and consumer of enabled services, session agents, D-Bus
names, notification hosts, keyrings, settings daemons, and background helpers.
Installed but inert libraries and active desktop authority are different
findings.

### 3. Desktop integration

Inspect `/etc/xdg/autostart`, `~/.config/autostart`, the effective
`xdg-desktop-portal` user units and configuration, exported desktop entries,
and relevant defaults reported by `xdg-mime query default MIME_TYPE`.

Flag competing portal backends, automatic startup, or unsolicited default
handlers separately from the application that introduced them. A candidate
handler is not a changed default.

### 4. Flatpak containment

```sh
flatpak list --user --app --columns=application,name,runtime,installation
flatpak list --system --app --columns=application,name,runtime,installation
flatpak list --user --runtime --columns=ref,installation,size
flatpak list --system --runtime --columns=ref,installation,size
flatpak info --show-permissions APP_ID
```

Review filesystem, device, socket, D-Bus, background, and portal access. Note
exported launchers, MIME candidates, and services. Accept a large private
runtime when containment is correct; disk size remains informational.

### 5. Residual configuration

Treat package state, configuration, caches, and user data as separate owners.
An absent package does not prove its former configuration is safe to delete,
and an unfamiliar directory does not prove residue. Record provenance and
value before proposing any cleanup.

## Proposal outcomes

Every finding ends in exactly one proposed outcome:

- keep it as host foundation, Celestina platform, or a chosen service;
- retain it as a named exception;
- remove it after proving that no retained consumer exists;
- replace a native application with its contained Flatpak distribution;
- select a leaner native alternative with a verified closure;
- propose a Celestina capability only for a demonstrated basic or daily need;
- defer the decision until named evidence is available.

Prefer the smallest reversible change. A proposal must state impact, affected
consumers, expected authority removed or contained, and rollback. Replacing an
application must preserve the user's data and required workflows.

## Safety and authorization

An audit never runs package removal, Flatpak uninstall, service disablement,
default-handler changes, or configuration deletion. Those are separate actions
requiring explicit authorization after the author reviews the proposal.

Execute accepted changes one coherent slice at a time, re-audit the affected
closure afterward, and distinguish an unsuccessful removal from evidence that
the original classification was wrong.

## Finding record

Use this shape for each observed item:

```text
Component:
Observed role:
Installed because:
Native dependency closure:
Session authority:
Flatpak containment:
Current consumers:
Classification:
Proposal:
Risk:
Rollback:
Missing evidence:
Decision: pending
```

When the author requests a durable audit, store the dated observations under
`docs/evidence/`. Keep volatile results out of this policy and do not turn
unaccepted proposals into project status or roadmap work.

## Host audit record — 2026-08-04

The author explicitly requested that this first durable snapshot remain in this
file rather than under `docs/evidence/`. It is an observation and proposal
record only. No package, service, Flatpak, handler, portal, configuration, or
live process was changed. Host-bus and journal access used read-only sandbox
approval, not `sudo`.

### Snapshot and audit limits

- **Captured:** 2026-08-04 19:26-19:36 EDT.
- **Host:** CachyOS/Arch, Linux `7.1.6-1-cachyos`, x86-64.
- **Native package state:** 1,510 installed packages; 284 explicitly installed;
  1,226 dependency-installed; 2 dependency orphans; 9 foreign/AUR packages.
- **Flatpak state:** no user-installed applications; 13 system applications;
  17 GB under `/var/lib/flatpak`; 46 GB of per-application data under
  `~/.var/app`.
- **Queries:** local `pacman` state and reverse trees; enabled/running system and
  user units; session D-Bus names; user processes; portal and autostart files;
  effective MIME defaults; Flatpak applications, runtimes, permissions and
  overrides; targeted residual configuration.
- **Not done:** no package database refresh, removal simulation, service
  restart, portal probe that could alter the session, handler invocation,
  permission override, configuration write, or destructive residue test.

The 13 Flatpak data directories all belong to currently installed
applications. Their largest observed owners were Bottles (20 GB), Steam
(7.3 GB), Twintail (6.0 GB), Android Studio (4.4 GB), VSCodium (3.3 GB), and
Modrinth (3.2 GB). These sizes are informational; ownership and containment
remain the decision criteria.

### Findings

#### Native dependency orphans

Component: `litehtml0.9` 0.9-2.1 and `lua51-lgi` 0.9.2-14

Observed role: Dependency-installed libraries with no package reverse consumer.

Installed because: Their former consumers are no longer installed.

Native dependency closure: 928 KiB and 664 KiB respectively, plus dependencies
that have other consumers.

Session authority: None observed.

Flatpak containment: Not applicable.

Current consumers: `pacman -Qdtq` and both reverse trees report none; targeted
searches found no reference under user configuration or user-local executables.

Classification: Unowned residue.

Proposal: Remove them after the author confirms that no unpackaged script or
manually built program loads either library.

Risk: An untracked manual consumer would stop loading.

Rollback: Reinstall the packages from a current repository or package cache.

Missing evidence: Author confirmation about manually installed consumers.

Decision: pending

#### Dormant alternative network stacks

Component: `iwd` 3.12-1.1 and `netctl` 1.29-2

Observed role: Alternative wireless and network-profile managers installed
explicitly beside NetworkManager.

Installed because: Historical explicit selection; no current package requires
either one.

Native dependency closure: Small and isolated; NetworkManager instead requires
`wpa_supplicant`, whose service is active.

Session authority: Both units are disabled and inactive. No `/etc/iwd`
configuration and no non-example `/etc/netctl` profile was found.

Flatpak containment: Not applicable.

Current consumers: None demonstrated by packages, enabled units, or inspected
configuration.

Classification: Unowned residue.

Proposal: Remove both after confirming that they are not retained as deliberate
recovery tools.

Risk: A manual recovery procedure could rely on their commands.

Rollback: Reinstall the packages; no active configuration was observed to
restore.

Missing evidence: Author decision on offline network-recovery tooling.

Decision: pending

#### Dormant mobile-broadband manager

Component: `modemmanager` 1.24.2-1.1

Observed role: Optional NetworkManager mobile-broadband backend.

Installed because: Explicit installation; no package requires the daemon.

Native dependency closure: About 5 MiB for the package plus MBIM/QMI and mobile
provider libraries.

Session authority: Its unit is disabled and inactive.

Flatpak containment: Not applicable.

Current consumers: No live consumer was observed, but hardware and travel use
were not tested.

Classification: Retained exception.

Proposal: Defer until the author confirms whether cellular modems or tethering
workflows are still required.

Risk: Removing it could silently remove future mobile-broadband support.

Rollback: Reinstall it before reconnecting such hardware.

Missing evidence: Intended cellular-hardware workflow.

Decision: pending

#### GNOME portal and Nautilus closure

Component: `xdg-desktop-portal-gnome` 50.0-1.1 and its Nautilus/GNOME closure

Observed role: A GNOME portal backend starts alongside the selected GTK, wlr,
and Celestina backends. Its journal repeatedly reports Niri monitor
configuration warnings.

Installed because: It provides `xdg-desktop-portal-impl` to `niri` and pulls
`nautilus`, `gnome-desktop-4`, GTK 4, libadwaita, and the GTK portal. The user
Niri portal configuration selects wlr for Screenshot/ScreenCast, Celestina for
FileChooser, and GTK as the default.

Native dependency closure: Nautilus is 13.7 MiB directly and brings GVFS,
LocalSearch, GNOME desktop libraries, and related adapters. Removing this portal
would not remove GTK 4 or libadwaita by itself: explicit Baobab, File Roller,
Mission Center, Zenity, Pavucontrol, Shelly, and other retained consumers also
use that stack.

Session authority: `org.freedesktop.impl.portal.desktop.gnome` is active. The
audit did not mutate the portal to expose an authoritative per-interface routing
trace.

Flatpak containment: This is native host authority, not a private Flatpak
runtime.

Current consumers: Potential fallback for portal interfaces not implemented by
GTK, wlr, or Celestina. Exact live interface use was not proven.

Classification: Foreign desktop closure.

Proposal: Remove the GNOME backend and its newly unneeded closure only after a
controlled read/write/ScreenCast/RemoteDesktop portal matrix proves that no
retained application needs a GNOME-only interface and the package transaction
still leaves a valid Niri portal provider.

Risk: Premature removal can break screen sharing, remote desktop, global
shortcuts, background requests, or another less visible portal interface.

Rollback: Reinstall the exact backend closure and restart the user portal only
in an explicitly authorized maintenance window.

Missing evidence: Effective per-interface routing and real application portal
tests.

Decision: pending

#### File-manager D-Bus activation

Component: `org.freedesktop.FileManager1`

Observed role: `inode/directory` correctly defaults to
`org.celestina.Siderita.desktop`, and Siderita implements `FileManager1`, but
the session had no current owner. The only activatable service file points to
`/usr/bin/nautilus --gapplication-service`.

Installed because: Nautilus installs both `org.freedesktop.FileManager1` and
`org.gnome.Nautilus` D-Bus activation files.

Native dependency closure: Same Nautilus closure described above.

Session authority: A foreign application calling “show in file manager” can
activate Nautilus even though Siderita owns the directory MIME default. The
running Siderita portal process does not claim this separate name.

Flatpak containment: Several Flatpaks are allowed to talk to
`org.freedesktop.FileManager1`, so the activation crosses the sandbox boundary.

Current consumers: Applications that reveal downloaded, generated, or selected
files through the freedesktop interface.

Classification: Competing desktop authority.

Proposal: Propose a bounded Siderita/Celestina capability that installs a real
user D-Bus activation path for the already implemented interface before
Nautilus is removed. This finding does not enter a roadmap automatically.

Risk: A broken replacement would make “show in folder” calls fail or open the
wrong location.

Rollback: Restore the Nautilus activation file/package and the former handler
state.

Missing evidence: Packaging design, cold activation test, multi-call behavior,
and live rollback validation.

Decision: pending

#### Piri Niri extension

Component: `piri-bin` 0.1.8-1

Observed role: Niri starts `piri daemon`; two key bindings call the local
`piri-window` wrapper; `piri.toml` configures plugins and scratchpad behavior.

Installed because: It is an explicit, currently configured scratchpad
extension.

Native dependency closure: A 6 MiB binary over the already retained Niri
compositor.

Session authority: The daemon is active. One defunct `notify-send` child had
remained under it for the lifetime of the session at audit time.

Flatpak containment: Not applicable.

Current consumers: The live Niri configuration and Mod+N/Mod+M workflow.

Classification: Retained exception.

Proposal: Retain it as a named exception until the scratchpad workflow is
explicitly abandoned or a compositor-level replacement can satisfy the same
hidden-window contract. Investigate the unreaped notification child separately
before treating the daemon as healthy.

Risk: Removing it now breaks configured shortcuts without providing the
required behavior.

Rollback: Reinstall/re-enable Piri and restore the existing Niri bindings.

Missing evidence: Author decision on the current scratchpad workflow and a
replacement with genuine hiding semantics.

Decision: pending

#### Transitional shell authorities

Component: Noctalia 5.0.0 development package, Blueman, and NetworkManager Applet

Observed role: Noctalia owns `org.freedesktop.Notifications` and
`org.kde.StatusNotifierWatcher`; Blueman and `nm-applet` run from autostart and
provide current Bluetooth/network control surfaces.

Installed because: Noctalia remains the explicit rollback and current owner for
shell responsibilities not yet handed to Celestina. Celestina's network and
Bluetooth write surface is planned but not implemented.

Native dependency closure: Noctalia is a 23.9 MiB native shell; Blueman and
`nm-applet` reuse the retained GTK/NetworkManager/BlueZ stack.

Session authority: All three are active and deliberately visible on the live
session. Celestina shell was not active and did not compete for their names.

Flatpak containment: Not applicable.

Current consumers: The current Niri session, notification history, tray,
network management, Bluetooth management, lock/suspend, night light, and other
not-yet-handed-over shell workflows.

Classification: Retained exception.

Proposal: Keep them until the registered Celestina replacement phases and
author validation explicitly hand over each responsibility.

Risk: Early removal loses live session functions and the known rollback.

Rollback: Current packages and Niri startup lines are already the rollback.

Missing evidence: The corresponding Celestina live validation and explicit
handover decisions.

Decision: pending

#### Chosen native integration services

Component: GNOME Keyring, GVFS, NetworkManager, BlueZ, WirePlumber,
power-profiles-daemon, UPower, UDisks2, Magnetita, input-remapper, and
CoolerControl

Observed role: Secrets, remote/device filesystems, networking, Bluetooth,
media, power, storage, phone integration, input mapping, and cooling support.

Installed because: Each has an observed first-party or explicit host consumer.
GNOME Keyring owns `org.freedesktop.secrets` and is required by QGIS/Seahorse;
the explicit GVFS AFC/MTP/SMB packages own the active GVFS path; Celestina and
Magnetita consume several of the remaining system services, while
input-remapper and CoolerControl are explicit hardware services.

Native dependency closure: Mixed GTK, system, Qt, Rust, and hardware adapters;
no second active KDE secrets daemon or full KDE desktop service stack was
observed.

Session authority: Active, but aligned with named host or Celestina roles.

Flatpak containment: Sandboxed applications selectively talk to several of
these host services through declared bus permissions.

Current consumers: Live session and hardware/application workflows.

Classification: Chosen service.

Proposal: Keep them. Revisit one service only when its named consumer is
removed or replaced.

Risk: Removing by toolkit association would break real workflows.

Rollback: Not applicable because no change is proposed.

Missing evidence: None for the keep decision; CoolerControl's exact UI consumer
was not enumerated but its explicitly enabled daemon is treated as deliberate.

Decision: pending

#### Waydroid container at idle

Component: `waydroid-container.service`

Observed role: Enabled system container service for the explicitly installed
Waydroid application.

Installed because: Chosen Android compatibility environment.

Native dependency closure: LXC, binder, networking, GTK, Python, and Android
image/data state.

Session authority: The container service was active at about 25 MiB RSS/28 MiB
peak while `waydroid status` reported `Session: STOPPED`.

Flatpak containment: Not applicable; this is a privileged native container
boundary.

Current consumers: No Android user session at audit time.

Classification: Chosen service.

Proposal: Defer the boot-policy decision until the author states whether
zero-step Waydroid startup is worth an always-running idle container. If not,
evaluate on-demand startup in a separate authorized slice.

Risk: Disabling the boot service may add a manual prerequisite or break the
current launcher workflow.

Rollback: Re-enable and start the existing unit.

Missing evidence: Expected Waydroid launch workflow and startup-latency test.

Decision: pending

#### Flatpak runtime containment

Component: 13 system Flatpak applications and their runtimes

Observed role: External browsers, communication, development, translation, and
gaming applications.

Installed because: Every base runtime branch observed (Freedesktop 25.08,
GNOME 49/50, and KDE 6.10/6.11) has at least one installed application
consumer. Wine and Vulkan extensions correspond to the installed gaming stack.

Native dependency closure: Flatpak itself and host integration only; the large
foreign desktop runtimes remain private under `/var/lib/flatpak`.

Session authority: Exported launchers and selected D-Bus permissions exist, but
the runtimes do not become native GNOME/KDE platform dependencies.

Flatpak containment: Correct at the distribution boundary. Per-application
exceptions are recorded separately below.

Current consumers: All 13 installed applications; Slack and Bottles were live
at audit time.

Classification: Contained external application.

Proposal: Keep the runtime model. Do not replace contained stacks with smaller
native closures merely to reduce disk usage.

Risk: Removing a runtime branch breaks its named applications.

Rollback: Not applicable because no change is proposed.

Missing evidence: Extension-level unused-runtime analysis was not performed.

Decision: pending

#### Broad Flatpak permission exceptions

Component: VSCodium and the broad game/browser/communication sandboxes

Observed role: VSCodium has effective `host` filesystem access even though a
user override also grants the narrower `/home/toni/CODIGO` path. Twintail,
Bottles, Steam, Modrinth, Floorp, ZapZap, and Slack have various combinations
of `devices=all`, development features, direct mounts, X11, application-data
paths, or launcher/icon writes.

Installed because: Development, gaming, browser, messaging, camera, and media
workflows can require wider access than a simple viewer.

Native dependency closure: Contained runtimes.

Session authority: Several applications may talk to notifications, tray,
secrets, power, storage, or file-manager interfaces.

Flatpak containment: Distribution containment is present, but these grants
materially widen host access. VSCodium's `host` access is the clearest candidate
for reduction because the explicit project path is already declared.

Current consumers: Active development and gaming workflows; Bottles was
running Victoria 3 and Slack was active during the audit.

Classification: Retained exception.

Proposal: Defer each permission reduction until a per-application test matrix
names required projects, removable media, cameras/controllers, downloads,
launchers, secrets, and inter-application paths. Test VSCodium first with
`host` removed and only named source/credential paths granted.

Risk: Blind tightening can break builds, extensions, game libraries,
controllers, calls, downloads, or cross-launcher integration.

Rollback: Restore the captured effective permissions for the affected app.

Missing evidence: Workflow-by-workflow permission tests.

Decision: pending

#### Flatpak override residue

Component: Override files for LMMS, Postman, Lutris, DBeaver, and Brave

Observed role: User Flatpak permission records for application IDs that are not
currently installed.

Installed because: Retained configuration from earlier installations.

Native dependency closure: None.

Session authority: None while those applications remain absent.

Flatpak containment: The files would affect future reinstallations of the same
IDs.

Current consumers: None installed.

Classification: Unowned residue.

Proposal: Remove the stale overrides after confirming that their preserved
restrictions are not wanted for an imminent reinstall.

Risk: A future reinstall would return to its manifest defaults rather than the
author's earlier restrictions.

Rollback: Recreate the small captured override entries.

Missing evidence: Reinstallation intent for the five application IDs.

Decision: pending

#### Image MIME handler

Component: `image/png`, `image/jpeg`, and related image defaults

Observed role: `xdg-mime` resolves PNG and JPEG to `gmic_qt.desktop`, whose
command is the G'MIC processing UI installed as a Krita plugin dependency.

Installed because: The G'MIC package exports image MIME associations; no
explicit image default exists in the inspected user `mimeapps.list`.

Native dependency closure: G'MIC and the retained Krita plugin stack.

Session authority: It is the effective default handler despite not being a
general-purpose image viewer.

Flatpak containment: Not applicable to the current handler.

Current consumers: Any application opening an image through the MIME default.

Classification: Competing desktop authority.

Proposal: Defer the handler change until the author names the intended
single-image workflow; then set one explicit default in a separate authorized
slice.

Risk: Choosing GIMP, Krita, a browser, or a future Celestina surface without a
workflow decision can replace one wrong default with another.

Rollback: Restore the previous MIME entry or remove the new explicit default.

Missing evidence: Author's preferred image-opening application and one live
open test.

Decision: pending

### Proposed decision order

1. Decide the two dependency orphans and the dormant `iwd`/`netctl` pair.
2. Decide the intended image handler; this is an observed wrong authority, not
   a disk-space preference.
3. Design and verify Siderita cold D-Bus activation before changing Nautilus or
   the GNOME portal closure.
4. Run the portal interface matrix, then decide whether the GNOME backend is a
   retained exception or removable closure.
5. Decide whether Waydroid needs an always-running container.
6. Test Flatpak permission reductions one application at a time, beginning with
   VSCodium.
7. Remove stale Flatpak overrides only after the reinstall-intent check.
8. Keep Noctalia, Piri, applets, and named services until their explicit
   workflow or handover conditions change.

Every item remains `pending`. This audit grants no authority to execute any of
the proposals.
