use std::path::{Path, PathBuf};

/// Pure navigation history. It never touches the filesystem or assumes HOME.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NavigationHistory {
    current: Option<PathBuf>,
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
}

impl NavigationHistory {
    #[must_use]
    pub fn new(initial: impl Into<PathBuf>) -> Self {
        Self {
            current: Some(initial.into()),
            back: Vec::new(),
            forward: Vec::new(),
        }
    }

    #[must_use]
    pub fn current(&self) -> Option<&Path> {
        self.current.as_deref()
    }

    #[must_use]
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    #[must_use]
    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// Records an explicit navigation and clears the forward branch.
    pub fn navigate_to(&mut self, destination: impl Into<PathBuf>) -> bool {
        let destination = destination.into();
        if self.current.as_ref() == Some(&destination) {
            return false;
        }

        if let Some(current) = self.current.replace(destination) {
            self.back.push(current);
        }
        self.forward.clear();
        true
    }

    pub fn go_back(&mut self) -> Option<PathBuf> {
        let destination = self.back.pop()?;
        if let Some(current) = self.current.replace(destination.clone()) {
            self.forward.push(current);
        }
        Some(destination)
    }

    pub fn go_forward(&mut self) -> Option<PathBuf> {
        let destination = self.forward.pop()?;
        if let Some(current) = self.current.replace(destination.clone()) {
            self.back.push(current);
        }
        Some(destination)
    }

    pub fn go_up(&mut self) -> Option<PathBuf> {
        let parent = self.current()?.parent()?.to_path_buf();
        self.navigate_to(&parent).then_some(parent)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::NavigationHistory;

    #[test]
    fn a_new_branch_clears_forward_history() {
        let mut history = NavigationHistory::new("/a");
        assert!(history.navigate_to("/b"));
        assert!(history.navigate_to("/c"));
        assert_eq!(history.go_back().as_deref(), Some(Path::new("/b")));

        assert!(history.navigate_to("/d"));

        assert!(!history.can_go_forward());
        assert_eq!(history.current(), Some(Path::new("/d")));
    }

    #[test]
    fn back_and_forward_are_reversible() {
        let mut history = NavigationHistory::new("/a");
        history.navigate_to("/b");

        assert_eq!(history.go_back().as_deref(), Some(Path::new("/a")));
        assert_eq!(history.go_forward().as_deref(), Some(Path::new("/b")));
    }

    #[test]
    fn up_records_normal_history() {
        let mut history = NavigationHistory::new("/a/b");

        assert_eq!(history.go_up().as_deref(), Some(Path::new("/a")));
        assert!(history.can_go_back());
        assert_eq!(history.go_back().as_deref(), Some(Path::new("/a/b")));
    }

    #[test]
    fn navigating_to_current_location_is_a_noop() {
        let mut history = NavigationHistory::new("/a");

        assert!(!history.navigate_to("/a"));
        assert!(!history.can_go_back());
    }
}
