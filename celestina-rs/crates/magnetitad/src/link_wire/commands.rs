//! Registered commands: the author names a program and its arguments on
//! the desktop, the phone sees ids and names, and a run names an id. No
//! command line ever crosses the wire, and a run is a bounded, grouped
//! subprocess like every other tool the daemon starts.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use magnetita_proto::capability;
use magnetita_proto::control::commands::{CommandEntry, CommandList, CommandResult};
use magnetita_proto::Envelope;
use serde::{Deserialize, Serialize};

use crate::lock::LockOk;
use crate::subprocess::{self, GroupPolicy};

/// How long one run may take before its whole process group is terminated.
const RUN_BUDGET: Duration = Duration::from_secs(60);
/// The most `commands.json` may hold; a few hundred bytes per command.
const MAX_FILE_BYTES: u64 = 1024 * 1024;

/// One registered command, as the author wrote it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Registered {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) program: String,
    #[serde(default)]
    pub(crate) args: Vec<String>,
}

/// The registry, persisted as JSON next to the settings: owner-only, since
/// a command line may carry anything, and replaced whole through a synced
/// sibling, so a crash or a full disk mid-save leaves the previous file. A
/// change reaches memory only once the file holds it.
pub(crate) struct CommandStore {
    path: Option<PathBuf>,
    entries: Mutex<BTreeMap<u32, Registered>>,
}

impl CommandStore {
    /// Loads `commands.json` from the daemon's configuration directory; an
    /// absent or corrupt file is an empty registry.
    pub(crate) fn load() -> Self {
        Self::load_from(
            celestina_core::xdg::config_home().map(|d| d.join("magnetita").join("commands.json")),
        )
    }

    fn load_from(path: Option<PathBuf>) -> Self {
        let entries = path
            .as_ref()
            .and_then(|p| celestina_core::atomic_file::read_bounded(p, MAX_FILE_BYTES).ok())
            .flatten()
            .and_then(|bytes| serde_json::from_slice::<Vec<Registered>>(&bytes).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.id, r))
            .collect();
        Self {
            path,
            entries: Mutex::new(entries),
        }
    }

    #[cfg(test)]
    pub(crate) fn in_memory() -> Self {
        Self {
            path: None,
            entries: Mutex::new(BTreeMap::new()),
        }
    }

    pub(crate) fn list(&self) -> Vec<Registered> {
        self.entries.lock_ok().values().cloned().collect()
    }

    /// Adds (`id` 0) or replaces a command; returns its id.
    pub(crate) fn set(
        &self,
        id: u32,
        name: &str,
        program: &str,
        args: Vec<String>,
    ) -> Result<u32, String> {
        if name.trim().is_empty() || program.trim().is_empty() {
            return Err("name and program are required".into());
        }
        let mut entries = self.entries.lock_ok();
        let id = if id == 0 {
            entries.keys().max().copied().unwrap_or(0) + 1
        } else {
            id
        };
        let mut next = entries.clone();
        next.insert(
            id,
            Registered {
                id,
                name: name.trim().to_owned(),
                program: program.trim().to_owned(),
                args,
            },
        );
        self.persist(&next)?;
        *entries = next;
        Ok(id)
    }

    pub(crate) fn remove(&self, id: u32) -> Result<bool, String> {
        let mut entries = self.entries.lock_ok();
        let mut next = entries.clone();
        let removed = next.remove(&id).is_some();
        self.persist(&next)?;
        *entries = next;
        Ok(removed)
    }

    fn persist(&self, entries: &BTreeMap<u32, Registered>) -> Result<(), String> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let list: Vec<&Registered> = entries.values().collect();
        let bytes = serde_json::to_vec_pretty(&list).map_err(|e| e.to_string())?;
        celestina_core::atomic_file::replace_private(path, &bytes)
            .map(drop)
            .map_err(|e| e.to_string())
    }

    /// The list the phone sees: ids and names only.
    pub(crate) fn published(&self) -> Envelope {
        let commands = self
            .list()
            .into_iter()
            .map(|r| CommandEntry {
                id: r.id,
                name: r.name,
            })
            .take(256)
            .collect();
        Envelope {
            capability: capability::COMMANDS,
            kind: CommandList::KIND,
            id: 0,
            body: CommandList { commands }.encode(),
        }
    }

    /// Runs `id` under the daemon's subprocess discipline; the result the
    /// phone receives. Blocking: call it off the session's task.
    pub(crate) fn run(&self, id: u32, stopping: &AtomicBool) -> CommandResult {
        let Some(command) = self.entries.lock_ok().get(&id).cloned() else {
            return CommandResult { id, ok: false };
        };
        let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
        let ok = match subprocess::spawn_grouped(&command.program, &args, Stdio::null()) {
            Ok((mut child, group)) => {
                let deadline = Instant::now() + RUN_BUDGET;
                let status = subprocess::wait_bounded(&mut child, group, deadline, stopping);
                if status.is_none() {
                    subprocess::terminate_group_and_reap(&mut child, group);
                }
                let _ = GroupPolicy::Terminate;
                status.is_some_and(|s| s.success())
            }
            Err(_) => false,
        };
        CommandResult { id, ok }
    }
}

static STORE: std::sync::LazyLock<CommandStore> = std::sync::LazyLock::new(CommandStore::load);

/// The daemon's one registry.
pub(crate) fn store() -> &'static CommandStore {
    &STORE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_save_changes_nothing_and_a_save_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("mag-commands-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let blocker = root.join("blocker");
        std::fs::write(&blocker, b"not a directory").unwrap();

        let broken = CommandStore::load_from(Some(blocker.join("commands.json")));
        assert!(broken.set(0, "Say hi", "true", vec![]).is_err());
        assert!(
            broken.list().is_empty(),
            "memory changes only once the file has"
        );

        let path = root.join("magnetita").join("commands.json");
        let store = CommandStore::load_from(Some(path.clone()));
        let id = store.set(0, "Say hi", "true", vec!["-n".into()]).unwrap();
        let mode =
            |path: &std::path::Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode(&path),
            0o600,
            "the command lines are the owner's alone"
        );
        assert_eq!(mode(path.parent().unwrap()), 0o700);
        assert_eq!(
            CommandStore::load_from(Some(path.clone())).list(),
            store.list()
        );

        // A remove that cannot be saved leaves the command registered.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(store.remove(id).is_err());
        assert_eq!(store.list().len(), 1);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_registry_numbers_publishes_names_only_and_runs_by_id() {
        let store = CommandStore::in_memory();
        let a = store.set(0, "Say hi", "true", vec![]).unwrap();
        let b = store.set(0, "Fail", "false", vec![]).unwrap();
        assert_eq!((a, b), (1, 2));
        assert!(store.set(0, "", "true", vec![]).is_err());
        let list = CommandList::decode(&store.published().body).unwrap();
        assert_eq!(list.commands.len(), 2);
        assert_eq!(list.commands[0].name, "Say hi");
        let stopping = AtomicBool::new(false);
        assert!(store.run(a, &stopping).ok);
        assert!(!store.run(b, &stopping).ok);
        assert!(!store.run(9, &stopping).ok, "an unknown id runs nothing");
        assert!(store.remove(a).unwrap());
        assert_eq!(store.list().len(), 1);
    }
}
