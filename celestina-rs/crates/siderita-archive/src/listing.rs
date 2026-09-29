//! The pre-flight check of a delegated archive: the tool's own listing, read
//! before the tool is allowed to write anything.
//!
//! A RAR or 7z is written by an installed tool, so the member-by-member guard of
//! [`crate::contain`] cannot run while it writes, and the check of its finished
//! tree only *detects* a write that already left. Old tools do not refuse a
//! dangerous link themselves (p7zip 16.02 predates 7-Zip's `-snld`), so the
//! archive is first listed — `7z l -slt`, `unrar lt` — and refused when its
//! index names:
//!
//! - a member whose stored name is absolute or holds `..`;
//! - a symlink whose target, read as text, leaves the root;
//! - a hard link whose target is not an ordinary relative name;
//! - a member whose path runs *under* a symlink member's name, or that has the
//!   same name as one: the two shapes that let a tool write through a link it
//!   has just created.
//!
//! The listing is read line by line as the tool prints it and never held
//! whole. What is remembered to decide the last rule, which does not depend on
//! order, is one 64-bit hash per folder, name and link, up to
//! [`REMEMBERED_MAX`] of them; a hash collision can only refuse an honest
//! archive, never admit a hostile one. A link's name is also kept, to say
//! which member was refused, but only up to [`LINK_NAMES_MAX`] bytes in all;
//! past that a link is remembered by its hash alone.
//!
//! A listing that does not look like the tool's is not trusted: 7z's must
//! reach its `----------` rule, unrar's must carry its `Details:` header or
//! at least one `Name:` record. Anything else — an older layout, a localised
//! build, another program answering to the name — refuses the archive rather
//! than letting it through unchecked.

use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use crate::error::ArchiveError;
use crate::member::{safe_relative, target_stays_inside};

/// How many folders, names and links the check remembers before it gives up
/// and refuses the archive as too large to check.
const REMEMBERED_MAX: usize = 2_000_000;

/// How many bytes of link names are kept to report a refusal by name.
const LINK_NAMES_MAX: usize = 1024 * 1024;

/// Which tool printed the listing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Style {
    /// `7z l -slt`: `Key = value` blocks after a `----------` rule.
    SevenZip,
    /// `unrar lt`: a `Details:` header, then indented `Key: value` blocks.
    Unrar,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum Kind {
    #[default]
    Other,
    Symlink,
    HardLink,
}

/// One member as the listing describes it, while its block is being read.
#[derive(Default)]
struct Record {
    name: Option<String>,
    kind: Kind,
    target: Option<String>,
}

/// The running check of one listing.
pub(crate) struct Listing {
    archive: PathBuf,
    style: Style,
    /// Whether the output was recognised as the tool's listing: 7z's rule
    /// (members start after it), or unrar's `Details:` header or first
    /// `Name:` record.
    started: bool,
    current: Record,
    folders: HashSet<u64>,
    names: HashSet<u64>,
    /// Each link by hash, with its name while [`LINK_NAMES_MAX`] allows.
    links: HashMap<u64, Option<PathBuf>>,
    link_bytes: usize,
    refusal: Option<ArchiveError>,
}

impl Listing {
    pub(crate) fn new(style: Style, archive: &Path) -> Self {
        Self {
            archive: archive.to_path_buf(),
            style,
            started: false,
            current: Record::default(),
            folders: HashSet::new(),
            names: HashSet::new(),
            links: HashMap::new(),
            link_bytes: 0,
            refusal: None,
        }
    }

    /// Reads one line of the listing.
    pub(crate) fn line(&mut self, line: &str) {
        if self.refusal.is_some() {
            return;
        }
        match self.style {
            Style::SevenZip => self.seven_zip_line(line),
            Style::Unrar => self.unrar_line(line),
        }
    }

    /// A line was longer than the reader keeps whole. A name cut short could
    /// hide the `..` that follows it, so the listing cannot be trusted.
    pub(crate) fn cut(&mut self) {
        self.refuse(ArchiveError::malformed(
            &self.archive,
            "the tool's listing holds a line too long to check",
        ));
    }

    /// Ends the listing: the archive may be extracted, or here is why not.
    pub(crate) fn finish(mut self) -> Result<(), ArchiveError> {
        self.flush();
        if let Some(refusal) = self.refusal {
            return Err(refusal);
        }
        if !self.started {
            return Err(ArchiveError::malformed(
                &self.archive,
                "the tool's listing could not be read",
            ));
        }
        for (hash, name) in &self.links {
            if self.folders.contains(hash) || self.names.contains(hash) {
                return Err(unsafe_member(
                    name.as_deref().unwrap_or(Path::new("(a symbolic link)")),
                ));
            }
        }
        Ok(())
    }

    fn seven_zip_line(&mut self, line: &str) {
        if !self.started {
            self.started = line.trim_end() == "----------";
            return;
        }
        if let Some(name) = line.strip_prefix("Path = ") {
            self.flush();
            self.current.name = Some(name.to_owned());
        } else if let Some(target) = line
            .strip_prefix("Symbolic Link = ")
            .or_else(|| line.strip_prefix("Link = "))
        {
            if !target.is_empty() {
                self.current.kind = Kind::Symlink;
                self.current.target = Some(target.to_owned());
            }
        } else if let Some(target) = line
            .strip_prefix("Hard Link = ")
            .or_else(|| line.strip_prefix("Copy Link = "))
        {
            if !target.is_empty() {
                self.current.kind = Kind::HardLink;
                self.current.target = Some(target.to_owned());
            }
        } else if let Some(attributes) = line.strip_prefix("Attributes = ") {
            // A unix mode string such as `lrwxrwxrwx` marks a symlink whose
            // target the 7z container keeps as data, not as a property.
            let unix_link = attributes.split_whitespace().any(|word| {
                word.len() == 10
                    && word.starts_with('l')
                    && word[1..].chars().all(|c| "rwxsStT-".contains(c))
            });
            if unix_link && self.current.kind == Kind::Other {
                self.current.kind = Kind::Symlink;
            }
        }
    }

    fn unrar_line(&mut self, line: &str) {
        let line = line.trim_start();
        if line.starts_with("Details: ") {
            self.started = true;
        } else if let Some(name) = line.strip_prefix("Name: ") {
            self.started = true;
            self.flush();
            self.current.name = Some(name.to_owned());
        } else if let Some(kind) = line.strip_prefix("Type: ") {
            let kind = kind.to_lowercase();
            if kind.contains("symbolic link") || kind.contains("junction") {
                self.current.kind = Kind::Symlink;
            } else if kind.contains("hard link") || kind.contains("file reference") {
                self.current.kind = Kind::HardLink;
            }
        } else if let Some(target) = line.strip_prefix("Target: ") {
            self.current.target = Some(target.to_owned());
        }
    }

    /// Judges the member whose block just ended.
    fn flush(&mut self) {
        let record = std::mem::take(&mut self.current);
        let Some(stored) = record.name else { return };
        if self.refusal.is_some() {
            return;
        }
        let Some(name) = safe_relative(Path::new(&stored)) else {
            return self.refuse(unsafe_member(Path::new(&stored)));
        };
        let mut ancestor = name.parent();
        while let Some(folder) = ancestor.filter(|folder| !folder.as_os_str().is_empty()) {
            self.folders.insert(hash(folder));
            ancestor = folder.parent();
        }
        match record.kind {
            Kind::Symlink => {
                if record
                    .target
                    .as_deref()
                    .is_some_and(|target| !target_stays_inside(&name, Path::new(target)))
                {
                    return self.refuse(unsafe_member(&name));
                }
                let size = name.as_os_str().len();
                let kept = (self.link_bytes + size <= LINK_NAMES_MAX).then(|| {
                    self.link_bytes += size;
                    name.clone()
                });
                if self.links.insert(hash(&name), kept).is_some() {
                    return self.refuse(unsafe_member(&name));
                }
            }
            Kind::HardLink => {
                if record
                    .target
                    .as_deref()
                    .is_some_and(|target| safe_relative(Path::new(target)).is_none())
                {
                    return self.refuse(unsafe_member(&name));
                }
                self.names.insert(hash(&name));
            }
            Kind::Other => {
                self.names.insert(hash(&name));
            }
        }
        if self.folders.len() + self.names.len() + self.links.len() > REMEMBERED_MAX {
            self.cut_short();
        }
    }

    fn cut_short(&mut self) {
        self.refuse(ArchiveError::malformed(
            &self.archive,
            "the archive holds too many entries to check before extracting",
        ));
    }

    fn refuse(&mut self, refusal: ArchiveError) {
        if self.refusal.is_none() {
            self.refusal = Some(refusal);
        }
    }
}

fn hash(path: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.as_os_str().hash(&mut hasher);
    hasher.finish()
}

fn unsafe_member(name: &Path) -> ArchiveError {
    ArchiveError::UnsafeMember {
        name: name.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{Listing, Style};
    use crate::error::ArchiveError;
    use std::path::Path;

    fn check(style: Style, listing: &str) -> Result<(), ArchiveError> {
        let mut check = Listing::new(style, Path::new("sample"));
        for line in listing.lines() {
            check.line(line);
        }
        check.finish()
    }

    fn seven(members: &[&[&str]]) -> String {
        let mut text = String::from("--\nPath = sample.7z\nType = 7z\n\n----------\n");
        for member in members {
            for line in *member {
                text.push_str(line);
                text.push('\n');
            }
            text.push('\n');
        }
        text
    }

    fn refused(outcome: Result<(), ArchiveError>) -> bool {
        matches!(outcome, Err(ArchiveError::UnsafeMember { .. }))
    }

    /// The auditor's chain as 7-Zip lists a tar: every hop looks inside, and
    /// `esc/pwned.txt` runs under the symlink `esc`.
    #[test]
    fn the_chain_is_refused_from_the_listing() {
        let listing = seven(&[
            &["Path = d", "Folder = +"],
            &["Path = d/up", "Symbolic Link = .."],
            &["Path = esc", "Symbolic Link = d/up/.."],
            &["Path = esc/pwned.txt", "Size = 5"],
        ]);
        assert!(refused(check(Style::SevenZip, &listing)));
    }

    #[test]
    fn a_member_under_a_link_is_refused_whatever_the_order() {
        let listing = seven(&[
            &["Path = sub/note.txt"],
            &["Path = sub", "Attributes = A_ lrwxrwxrwx"],
        ]);
        assert!(refused(check(Style::SevenZip, &listing)));
        // A file with the same name as a link is written through it too.
        let listing = seven(&[&["Path = a", "Symbolic Link = b"], &["Path = a"]]);
        assert!(refused(check(Style::SevenZip, &listing)));
    }

    #[test]
    fn absolute_and_parent_names_and_escaping_targets_are_refused() {
        for members in [
            seven(&[&["Path = /etc/passwd"]]),
            seven(&[&["Path = a/../../outside"]]),
            seven(&[&["Path = link", "Symbolic Link = /etc"]]),
            seven(&[&["Path = a/link", "Symbolic Link = ../../outside"]]),
            seven(&[&["Path = hard", "Hard Link = ../secret"]]),
        ] {
            assert!(refused(check(Style::SevenZip, &members)), "{members}");
        }
    }

    #[test]
    fn an_honest_listing_passes() {
        let listing = seven(&[
            &["Path = data", "Folder = +"],
            &["Path = data/one.txt", "Attributes = A_ -rw-r--r--"],
            &["Path = data/soft", "Symbolic Link = one.txt"],
            &["Path = data/hard", "Hard Link = data/one.txt"],
        ]);
        check(Style::SevenZip, &listing).expect("honest");
    }

    #[test]
    fn a_listing_without_its_rule_is_not_trusted() {
        assert!(matches!(
            check(Style::SevenZip, "Path = a\n"),
            Err(ArchiveError::Malformed { .. })
        ));
    }

    #[test]
    fn a_line_cut_short_is_not_trusted() {
        let mut listing = Listing::new(Style::SevenZip, Path::new("sample"));
        listing.line("----------");
        listing.cut();
        assert!(matches!(
            listing.finish(),
            Err(ArchiveError::Malformed { .. })
        ));
    }

    /// Round 2 of the review (N2): an unrar listing that carries neither its
    /// header nor a single record is not unrar's, and is not trusted; an empty
    /// archive with its header is.
    #[test]
    fn an_unrar_listing_it_does_not_recognise_is_not_trusted() {
        let foreign = "UNRAR 4.20 freeware\n\nPathname/Comment\n   evil/../../x\n";
        assert!(matches!(
            check(Style::Unrar, foreign),
            Err(ArchiveError::Malformed { .. })
        ));
        assert!(matches!(
            check(Style::Unrar, ""),
            Err(ArchiveError::Malformed { .. })
        ));
        let empty = "\nArchive: sample.rar\nDetails: RAR 5\n\n";
        check(Style::Unrar, empty).expect("an empty archive with its header");
    }

    #[test]
    fn a_copy_link_is_read_as_a_link_to_a_member() {
        let listing = seven(&[&["Path = copy", "Copy Link = ../outside"]]);
        assert!(refused(check(Style::SevenZip, &listing)));
    }

    #[test]
    fn unrar_technical_listing_is_read_the_same_way() {
        let honest = "\nArchive: sample.rar\nDetails: RAR 5\n\n        Name: data/one.txt\n        Type: File\n        Size: 3\n\n        Name: data/soft\n        Type: Unix symbolic link\n      Target: one.txt\n";
        check(Style::Unrar, honest).expect("honest");
        let chain = "        Name: d/up\n        Type: Unix symbolic link\n      Target: ..\n\n        Name: esc\n        Type: Unix symbolic link\n      Target: d/up/..\n\n        Name: esc/pwned.txt\n        Type: File\n";
        assert!(refused(check(Style::Unrar, chain)));
        let absolute = "        Name: /etc/passwd\n        Type: File\n";
        assert!(refused(check(Style::Unrar, absolute)));
    }
}
