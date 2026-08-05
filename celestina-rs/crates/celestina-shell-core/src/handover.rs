//! What Noctalia still does for this session, and what it would take to stop
//! needing it.
//!
//! Every other module here decides something small and immediate. This one
//! decides something that happens once: whether the session is ready to depend
//! on this shell alone. That decision is easy to get wrong in one direction —
//! everything *looks* built, so the old shell gets turned off, and the gap
//! shows up on a Monday morning with no way back that anybody wrote down.
//!
//! So the model is deliberately pessimistic. A responsibility is covered only
//! when something implements it **and** the author has recorded that they
//! watched it work on a real session; code alone is never enough, because
//! everything in this repository compiles and most of it has never been seen on
//! a screen. Anything not on this list is not handled: the list is the claim.

/// One thing the session needs somebody to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Responsibility {
    /// What it is, in the author's terms rather than the protocol's.
    pub name: &'static str,
    /// What this shell built to take it over, or `None` when nothing has.
    pub implemented_by: Option<&'static str>,
    /// The validation entry that would prove it works on a real session, or
    /// `None` when this responsibility needs no author check.
    pub validated_by: Option<&'static str>,
}

impl Responsibility {
    /// Whether this shell can take it over at all yet.
    #[must_use]
    pub fn is_implemented(&self) -> bool {
        self.implemented_by.is_some()
    }
}

/// Everything Noctalia supplies today.
///
/// Adding a row here is how a new responsibility is admitted; leaving one out
/// is how the handover quietly becomes wrong, which is why the tests check this
/// list against the checkpoints rather than trusting it.
pub const RESPONSIBILITIES: [Responsibility; 8] = [
    Responsibility {
        name: "panel and workspaces",
        implemented_by: Some("R0-R1"),
        validated_by: Some("VAL-R1-01"),
    },
    Responsibility {
        name: "launcher and clipboard history",
        implemented_by: Some("R2"),
        validated_by: Some("VAL-R2-01"),
    },
    Responsibility {
        name: "session verbs, OSD and night light",
        implemented_by: Some("R3"),
        validated_by: Some("VAL-R3"),
    },
    Responsibility {
        name: "notifications",
        implemented_by: Some("R4"),
        validated_by: Some("VAL-R4"),
    },
    Responsibility {
        name: "control centre and session menu",
        implemented_by: Some("R5"),
        validated_by: Some("VAL-R5"),
    },
    Responsibility {
        name: "wallpaper and session look",
        implemented_by: Some("R7"),
        validated_by: Some("VAL-R7"),
    },
    // The two that are deliberately not built. They are on this list precisely
    // because leaving them off would make the handover look complete.
    Responsibility {
        name: "screen lock",
        implemented_by: None,
        validated_by: Some("VAL-SHELL-LOCK"),
    },
    Responsibility {
        name: "polkit authentication agent",
        implemented_by: None,
        validated_by: None,
    },
];

/// Why the handover cannot proceed, in the order a person would want to fix
/// them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blocker {
    /// Nothing in this shell does it yet.
    NotImplemented(&'static str),
    /// Something does it, but nobody has watched it work.
    NotValidated {
        responsibility: &'static str,
        validation: &'static str,
    },
}

impl Blocker {
    /// The sentence to show. Written for the person deciding, not for a log.
    #[must_use]
    pub fn sentence(&self) -> String {
        match self {
            Self::NotImplemented(name) => {
                format!("nothing in this shell provides {name} yet")
            }
            Self::NotValidated {
                responsibility,
                validation,
            } => format!("{responsibility} is built but {validation} has not been recorded"),
        }
    }
}

/// What still stands between this session and doing without Noctalia.
///
/// `passed` is the set of validation ids the author has recorded as passing.
/// Nothing here reads `VALIDATION.md`: what counts as recorded is the caller's
/// business, and keeping it out means this rule is testable without a file.
#[must_use]
pub fn blockers(passed: &[&str]) -> Vec<Blocker> {
    let mut found = Vec::new();
    for responsibility in &RESPONSIBILITIES {
        let Some(_) = responsibility.implemented_by else {
            found.push(Blocker::NotImplemented(responsibility.name));
            continue;
        };
        if let Some(validation) = responsibility.validated_by {
            if !passed.contains(&validation) {
                found.push(Blocker::NotValidated {
                    responsibility: responsibility.name,
                    validation,
                });
            }
        }
    }
    found
}

/// Whether removing Noctalia may be offered at all.
///
/// This is the one question the tool asks, and the answer today is no: it stays
/// no until every responsibility is both built and seen working. A shell that
/// helped remove what it has not been proven to replace would be trading a
/// working session for a tidy one.
#[must_use]
pub fn may_remove(passed: &[&str]) -> bool {
    blockers(passed).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every validation this model names, as if the author had passed them all.
    fn everything() -> Vec<&'static str> {
        RESPONSIBILITIES
            .iter()
            .filter_map(|responsibility| responsibility.validated_by)
            .collect()
    }

    #[test]
    fn removal_is_refused_while_anything_is_unbuilt_or_unseen() {
        // The state this session is actually in today.
        assert!(!may_remove(&[]));
        // And with every validation recorded, the two unbuilt responsibilities
        // still refuse it.
        assert!(!may_remove(&everything()));
    }

    #[test]
    fn a_built_but_unwatched_responsibility_blocks_with_its_validation_named() {
        let found = blockers(&[]);
        let notifications = found
            .iter()
            .find(|blocker| {
                matches!(blocker, Blocker::NotValidated { responsibility, .. }
                                     if *responsibility == "notifications")
            })
            .expect("notifications blocks the handover");

        // The sentence has to say what to do, not merely that something is
        // wrong.
        assert!(notifications.sentence().contains("VAL-R4"));
    }

    #[test]
    fn what_nobody_built_is_named_as_such() {
        let found = blockers(&everything());
        assert!(found.contains(&Blocker::NotImplemented("screen lock")));
        assert!(found.contains(&Blocker::NotImplemented("polkit authentication agent")));
        // Nothing else is left: those two are the whole remaining gap.
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn recording_a_validation_removes_exactly_its_own_blocker() {
        let before = blockers(&[]).len();
        let after = blockers(&["VAL-R4"]).len();
        assert_eq!(after, before - 1);
    }

    #[test]
    fn every_responsibility_says_how_it_would_be_proved_or_why_it_need_not_be() {
        for responsibility in &RESPONSIBILITIES {
            // A responsibility that is implemented must name the check that
            // would prove it; otherwise it could pass the handover having
            // never been seen working.
            if responsibility.is_implemented() {
                assert!(
                    responsibility.validated_by.is_some(),
                    "{} is implemented but names no validation",
                    responsibility.name
                );
            }
        }
    }

    #[test]
    fn the_list_names_what_is_missing_rather_than_leaving_it_out() {
        // The temptation is to list only what was built, which would make the
        // handover look finished. Both known gaps are on the list.
        assert!(RESPONSIBILITIES
            .iter()
            .any(|responsibility| !responsibility.is_implemented()));
    }
}
