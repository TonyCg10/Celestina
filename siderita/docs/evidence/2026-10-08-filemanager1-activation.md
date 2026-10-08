# Evidence: 2026-10-08 cold D-Bus activation of FileManager1

- **Date:** 2026-10-08
- **Scope:** `SID-H1-R` — `siderita`
- **Environment:** the author's CachyOS, Qt 6.11.2, Niri, the author's live session bus
- **Artifact:** the session's release build at `/home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/siderita`; the landing builds, verifies and deploys the registered binary

## Procedure

```sh
(cd siderita && cargo fmt --all --check)
(cd siderita && cargo clippy --all-targets --locked -- -D warnings)
(cd siderita && cargo test)
(cd siderita && cargo build --release --locked)
sh siderita/scripts/qml-tests.sh
sh siderita/scripts/smoke.sh --binary /home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/siderita
bash scripts/qmllint-cxxqt.sh siderita
bash scripts/check-architecture-contract.sh
python3 scripts/check-language-contract.py
bash scripts/check-documentation-contract.sh
```

Cold activation on the live session bus, with no Siderita running
(`pgrep -a siderita` empty, no `org.freedesktop.FileManager1` owner):

```sh
B=/home/toni/CODIGO/CELESTINA.worktrees/.cargo-target/release/siderita
S=~/.local/share/dbus-1/services/org.freedesktop.FileManager1.service
sed "s|@BIN@|$B|" siderita/portal/org.freedesktop.FileManager1.service > "$S"
busctl --user call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus ReloadConfig
busctl --user call org.freedesktop.FileManager1 /org/freedesktop/FileManager1 \
    org.freedesktop.FileManager1 ShowFolders ass 1 file:///home/toni ""
pgrep -a siderita
busctl --user list | grep FileManager1
niri msg windows
busctl --user call org.freedesktop.FileManager1 /org/freedesktop/FileManager1 \
    org.freedesktop.FileManager1 ShowFolders ass 1 file:///tmp ""
pgrep -a siderita
kill <pid>; rm "$S"
busctl --user call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus ReloadConfig
```

## Result

- **Exit:** formatting, clippy, the unit tests (180 passed, 0 failed, 1
  ignored), the release build, the QML tests (195 passed, 0 failed), the smoke
  run, the architecture contract, the language contract and the documentation
  contract exited 0. qmllint exited 0 at the 241 baseline once the unit was
  rebased onto AUD-1-N: the earlier 239/241 readings followed the directory
  order `find` handed qmllint, which that suite unit sorts (see
  `docs/evidence/2026-10-08-qmllint-order.md`); this unit adds no warning.
- **Cold activation:** both `ReloadConfig` calls and both `ShowFolders` calls
  exited 0. After the first call `pgrep -a siderita` printed
  `2654737 …/release/siderita --file-manager`; `busctl --user list` showed
  `org.freedesktop.FileManager1 2654737 siderita toni :1.34050 user@1000.service`;
  `niri msg windows` listed window 249, title `Siderita`, app id
  `org.celestina.Siderita`, PID 2654737. After the second call `pgrep` still
  printed only PID 2654737 and Niri still listed one `org.celestina.Siderita`
  window. After the kill `pgrep` printed nothing, the temporary service file
  was removed, and after the second `ReloadConfig` the bus listed no
  `FileManager1` owner.

- **Portal process:** with no Siderita running, `siderita --portal` started
  directly (PID 2660383) owned `org.freedesktop.impl.portal.desktop.celestina`
  after 5 s while `busctl --user list | grep -c FileManager1` printed `0`; the
  process was then killed and `pgrep -a siderita` printed nothing.

- **Request naming no local folder:** `siderita/src/dbus.rs`
  `a_request_naming_no_local_folder_resolves_to_none` covers `folders_of` with
  `ShowFolders ["smb://host/x"]`, a `file://` URI with a query and
  `ShowItems ["file:///"]`. Live, with the temporary service file installed and
  no Siderita running, `ShowFolders ass 1 smb://host/x ""` exited 0; 4 s later
  `pgrep -a siderita` printed nothing and Niri listed no
  `org.celestina.Siderita` window: the activated process received the request,
  found no folder and quit (`requestWithoutFolder`), leaving the name
  activatable. The service file was removed and the bus reloaded.

## Limits

- The tab strip of the shown window was not inspected in the session, and no
  QML test hosts `Main.qml` (it needs the CXX-Qt types). The window never
  restores saved tabs: it always starts with one start tab, which the first
  request in `--file-manager` mode closes after appending the folder in the
  foreground (`closeTab(0)` moves the current index from 1 to 0). The author
  sees the result.
- The lost-race path (an activated process quitting when a Siderita already
  owns the name) was not exercised live: the bus does not activate while the
  name has an owner.

## Follow-up

- None.

## Landing

- **Base revision:** `2f06c10c740da563f198657aa840c2dd5da370f4`
- **Check:** `production_artifact.py check siderita --require-verified` exit 1: production-artifact: production inputs changed; run build-production.sh; tests or rules changed; run verify-production.sh again
- **Build:** siderita build: build-production.sh exit 0, verify-production.sh exit 0, manifest source_fingerprint sha256:11fa0f5100a3a94dd90275f77ec4c63de1385e7c0fa84eb947a36e1be58f5d78, verification_fingerprint sha256:a861e84bcb235069d25632492dcd8b480458ccd466bd7d4beb327331fdc21892
- **Deploy:** after the push: siderita: deploy-production.sh, status-production.sh
