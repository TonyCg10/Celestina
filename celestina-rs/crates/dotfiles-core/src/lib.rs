#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesiredState {
    Directory,
    Symlink { source: PathBuf },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dotfile {
    destination: PathBuf,
    desired: DesiredState,
}

impl Dotfile {
    #[must_use]
    pub fn directory(destination: impl Into<PathBuf>) -> Self {
        Self {
            destination: destination.into(),
            desired: DesiredState::Directory,
        }
    }

    #[must_use]
    pub fn symlink(destination: impl Into<PathBuf>, source: impl Into<PathBuf>) -> Self {
        Self {
            destination: destination.into(),
            desired: DesiredState::Symlink {
                source: source.into(),
            },
        }
    }

    #[must_use]
    pub fn destination(&self) -> &Path {
        &self.destination
    }

    #[must_use]
    pub const fn desired(&self) -> &DesiredState {
        &self.desired
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlannedAction {
    CreateDirectory {
        destination: PathBuf,
    },
    CreateSymlink {
        source: PathBuf,
        destination: PathBuf,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Conflict {
    DuplicateDestination {
        destination: PathBuf,
    },
    ExistingEntry {
        destination: PathBuf,
        expected: DesiredState,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Plan {
    actions: Vec<PlannedAction>,
    conflicts: Vec<Conflict>,
}

impl Plan {
    #[must_use]
    pub fn actions(&self) -> &[PlannedAction] {
        &self.actions
    }

    #[must_use]
    pub fn conflicts(&self) -> &[Conflict] {
        &self.conflicts
    }

    #[must_use]
    pub fn is_applicable(&self) -> bool {
        self.conflicts.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanError {
    path: PathBuf,
    kind: io::ErrorKind,
    message: String,
}

impl PlanError {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn kind(&self) -> io::ErrorKind {
        self.kind
    }
}

impl fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot inspect '{}': {} ({:?})",
            self.path.display(),
            self.message,
            self.kind
        )
    }
}

impl Error for PlanError {}

/// Produces a read-only plan. It never creates, replaces or removes entries.
pub fn plan(entries: &[Dotfile]) -> Result<Plan, PlanError> {
    let mut result = Plan::default();
    let mut destinations = HashSet::new();

    for entry in entries {
        if !destinations.insert(entry.destination.clone()) {
            result.conflicts.push(Conflict::DuplicateDestination {
                destination: entry.destination.clone(),
            });
            continue;
        }

        match fs::symlink_metadata(&entry.destination) {
            Ok(metadata) => inspect_existing(entry, &metadata, &mut result)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                result.actions.push(action_for(entry));
            }
            Err(error) => return Err(plan_error(entry.destination.clone(), error)),
        }
    }

    Ok(result)
}

fn inspect_existing(
    entry: &Dotfile,
    metadata: &fs::Metadata,
    result: &mut Plan,
) -> Result<(), PlanError> {
    let already_matches = match &entry.desired {
        DesiredState::Directory => metadata.is_dir(),
        DesiredState::Symlink { source } if metadata.file_type().is_symlink() => {
            fs::read_link(&entry.destination)
                .map_err(|error| plan_error(entry.destination.clone(), error))?
                == *source
        }
        DesiredState::Symlink { .. } => false,
    };

    if !already_matches {
        result.conflicts.push(Conflict::ExistingEntry {
            destination: entry.destination.clone(),
            expected: entry.desired.clone(),
        });
    }

    Ok(())
}

fn action_for(entry: &Dotfile) -> PlannedAction {
    match &entry.desired {
        DesiredState::Directory => PlannedAction::CreateDirectory {
            destination: entry.destination.clone(),
        },
        DesiredState::Symlink { source } => PlannedAction::CreateSymlink {
            source: source.clone(),
            destination: entry.destination.clone(),
        },
    }
}

fn plan_error(path: PathBuf, error: io::Error) -> PlanError {
    PlanError {
        path,
        kind: error.kind(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use super::{plan, Conflict, Dotfile, PlannedAction};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "celestina-dotfiles-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_entries_become_preview_actions_without_mutation() {
        let fixture = TestDirectory::new("preview");
        let destination = fixture.path().join("config");
        let result = plan(&[Dotfile::directory(&destination)]).expect("build plan");

        assert_eq!(
            result.actions(),
            &[PlannedAction::CreateDirectory {
                destination: destination.clone()
            }]
        );
        assert!(result.is_applicable());
        assert!(!destination.exists());
    }

    #[test]
    fn existing_mismatch_is_a_conflict_not_a_replace_action() {
        let fixture = TestDirectory::new("conflict");
        let destination = fixture.path().join("config");
        fs::write(&destination, b"user data").expect("write existing file");

        let result = plan(&[Dotfile::directory(&destination)]).expect("build plan");

        assert!(result.actions().is_empty());
        assert!(matches!(
            result.conflicts(),
            [Conflict::ExistingEntry { .. }]
        ));
        assert_eq!(
            fs::read(&destination).expect("read existing file"),
            b"user data"
        );
    }

    #[test]
    fn duplicate_destination_blocks_application() {
        let fixture = TestDirectory::new("duplicate");
        let destination = fixture.path().join("config");

        let result = plan(&[
            Dotfile::directory(&destination),
            Dotfile::directory(&destination),
        ])
        .expect("build plan");

        assert!(matches!(
            result.conflicts(),
            [Conflict::DuplicateDestination { .. }]
        ));
        assert!(!result.is_applicable());
    }

    #[cfg(unix)]
    #[test]
    fn matching_symlink_is_already_satisfied() {
        let fixture = TestDirectory::new("symlink");
        let source = fixture.path().join("source");
        let destination = fixture.path().join("destination");
        fs::write(&source, b"tracked").expect("write source");
        symlink(&source, &destination).expect("create symlink");

        let result = plan(&[Dotfile::symlink(&destination, &source)]).expect("build plan");

        assert!(result.actions().is_empty());
        assert!(result.conflicts().is_empty());
    }
}
