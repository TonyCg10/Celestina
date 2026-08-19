use std::path::{Component, Path, PathBuf};

/// One entry stored inside an archive, as read from its index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Member {
    /// The member's stored name, already validated as a relative path that stays
    /// inside the extraction root.
    pub name: PathBuf,
    /// Uncompressed size in bytes; zero for a directory.
    pub size: u64,
    /// Whether the member is a directory.
    pub is_directory: bool,
}

/// The relative path a member may be written to, or `None` when the stored name
/// would escape the extraction root.
///
/// This is the zip-slip guard, and it is a whitelist rather than a blacklist: a
/// component is kept only when it is an ordinary name. Anything else — a root,
/// a drive prefix, `..`, an empty name — rejects the member outright, so no
/// amount of `a/../../b` spelling reaches the filesystem. `.` is dropped, being
/// a no-op the containers do emit.
pub(crate) fn safe_relative(raw: &Path) -> Option<PathBuf> {
    let mut safe = PathBuf::new();
    for component in raw.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!safe.as_os_str().is_empty()).then_some(safe)
}

/// Whether a symlink's `target`, resolved textually against the link's own
/// parent inside the archive, still lands inside the extraction root.
///
/// Purely lexical on purpose: it is answered before anything is written, and the
/// root does not exist yet to be resolved against.
pub(crate) fn target_stays_inside(link: &Path, target: &Path) -> bool {
    if target.is_absolute() {
        return false;
    }
    let mut depth: i64 = link
        .parent()
        .map(|parent| {
            parent
                .components()
                .filter(|c| matches!(c, Component::Normal(_)))
                .count()
        })
        .unwrap_or(0) as i64;
    for component in target.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{safe_relative, target_stays_inside};
    use std::path::{Path, PathBuf};

    #[test]
    fn only_ordinary_components_survive_the_guard() {
        assert_eq!(
            safe_relative(Path::new("./notas/uno.txt")),
            Some(PathBuf::from("notas/uno.txt"))
        );
        // Every spelling of an escape, including one that "comes back".
        assert_eq!(safe_relative(Path::new("../fuera")), None);
        assert_eq!(safe_relative(Path::new("a/../../fuera")), None);
        assert_eq!(safe_relative(Path::new("/etc/passwd")), None);
        assert_eq!(safe_relative(Path::new("")), None);
        assert_eq!(safe_relative(Path::new("./")), None);
    }

    #[test]
    fn a_symlink_may_point_within_the_archive_but_never_out_of_it() {
        assert!(target_stays_inside(
            Path::new("notas/enlace"),
            Path::new("uno.txt")
        ));
        assert!(target_stays_inside(
            Path::new("notas/dentro/enlace"),
            Path::new("../uno.txt")
        ));
        assert!(!target_stays_inside(
            Path::new("notas/enlace"),
            Path::new("../../fuera")
        ));
        assert!(!target_stays_inside(
            Path::new("notas/enlace"),
            Path::new("/etc/passwd")
        ));
    }
}
