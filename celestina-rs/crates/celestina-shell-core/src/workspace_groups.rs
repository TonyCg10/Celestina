//! Which monitor a workspace belongs to, once that monitor is gone.
//!
//! Niri publishes the output a workspace **is on**, never the one it was
//! configured for. Those are the same thing right up until a monitor is turned
//! off: then every workspace that lived there moves to a surviving output and
//! the compositor stops mentioning where it came from. A session configured with
//! five workspaces per monitor across three monitors therefore arrives at the
//! panel as fifteen workspaces on one output, indistinguishable from a session
//! that really has fifteen there.
//!
//! So the grouping cannot be read; it has to be remembered or declared. This
//! module owns both, and the one rule that keeps the memory honest: **an
//! observation may only teach what it is in a position to know.**
//!
//! A frame carrying a single output cannot distinguish "this workspace belongs
//! here" from "this workspace was displaced here", so it teaches nothing at all.
//! Only a frame that sees more than one output may record a home, and even then
//! only for a workspace that has none yet — a home already known is never
//! rewritten by an observation, because the observation that would rewrite it is
//! exactly the displaced one. A declaration is the only thing that changes a
//! known home, which is what makes the declared route the repair for a memory
//! that learned the wrong thing.
//!
//! Nothing here reads the compositor's configuration. Niri's own `open-on-output`
//! is not available over IPC and this shell does not parse another program's
//! files; the declaration below is the shell's own, in the shell's own settings.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::bounded;

/// The schema version of the persisted memory. A file from the future is not
/// read: guessing at a newer schema would invent a layout the person never had.
pub const SCHEMA_VERSION: u32 = 1;
/// How large the persisted memory may be on disk. The file is whatever last
/// wrote it rather than whatever this shell last saved, so it is read bounded:
/// generous for [`MAX_HOMES`] short names, finite against a tampered path.
pub const MAX_FILE_BYTES: usize = 64 * 1024;
/// How many homes are remembered. Well past three monitors of fifteen
/// workspaces, and finite against a state file that grew or was tampered with.
pub const MAX_HOMES: usize = 128;
/// How many groups a strip will render. A session with more distinct monitors
/// than this has a bigger problem than a long strip.
pub const MAX_GROUPS: usize = 8;
/// A workspace or output name, in characters. Matches the adapter's own bound so
/// a name that survived the snapshot survives this too.
pub const MAX_NAME_CHARS: usize = 64;

/// One workspace, as the strip sees it.
///
/// This is deliberately not the adapter's full row: grouping needs identity,
/// placement and the three states that must survive being collapsed behind a
/// capsule. Anything else is presentation and stays with the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Workspace {
    /// The compositor's name for it. An unnamed workspace has an empty label and
    /// is never remembered — niri keeps a trailing empty one that is created and
    /// destroyed as you work, and recording it would fill the memory with noise.
    pub label: String,
    /// The output it is on right now.
    pub output: String,
    /// Whether it is the active workspace of its output.
    pub active: bool,
    /// Whether something on it is asking for attention.
    pub urgent: bool,
    /// Whether it holds any window.
    pub occupied: bool,
}

impl Workspace {
    /// A workspace with both names bounded, as everything crossing this boundary
    /// must be.
    #[must_use]
    pub fn new(label: &str, output: &str, active: bool, urgent: bool, occupied: bool) -> Self {
        Self {
            label: bounded(label, MAX_NAME_CHARS),
            output: bounded(output, MAX_NAME_CHARS),
            active,
            urgent,
            occupied,
        }
    }

    /// Whether this workspace can be remembered at all. The compositor's
    /// trailing empty workspace has no name and no permanent identity.
    #[must_use]
    pub fn is_named(&self) -> bool {
        !self.label.is_empty()
    }
}

/// Where each named workspace belongs: what was observed, and what was declared.
///
/// The two are kept apart rather than merged on write, because they answer to
/// different authorities. A declaration is the person's choice and outlives any
/// number of observations; an observation is a fact about one moment that a
/// later declaration must be able to overrule without having been erased first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Homes {
    /// Written so a later shell can refuse a file it does not understand.
    schema: u32,
    /// Learned from frames that were in a position to teach. Workspace label to
    /// output name.
    learned: BTreeMap<String, String>,
    /// Declared by the person, in the shell's own settings. Wins outright.
    declared: BTreeMap<String, String>,
}

impl Default for Homes {
    fn default() -> Self {
        Self {
            schema: SCHEMA_VERSION,
            learned: BTreeMap::new(),
            declared: BTreeMap::new(),
        }
    }
}

impl Homes {
    /// An empty memory: nothing observed and nothing declared.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Brings the memory inside its bounds, on the way in and on the way out.
    ///
    /// A hand-edited or tampered file is a fact about the disk, not about this
    /// shell: an over-long name is truncated exactly as a compositor name would
    /// be, an empty one is dropped, and a memory past [`MAX_HOMES`] keeps its
    /// first entries rather than being refused whole. Nothing here can make the
    /// strip worse than it was without the file, which is the property that
    /// lets a failure degrade instead of fail.
    fn clamped(mut self) -> Self {
        self.schema = SCHEMA_VERSION;
        self.learned = bounded_map(self.learned);
        self.declared = bounded_map(self.declared);
        self
    }

    /// The exact bytes to write.
    ///
    /// # Errors
    ///
    /// Returns the serializer's own error, which cannot happen for this shape
    /// but is not worth an `unwrap` at a call site that can report it.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        let mut bytes = serde_json::to_vec(&self.clone().clamped())?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Reads a persisted memory, or nothing.
    ///
    /// Returns `None` for anything this shell did not write and cannot safely
    /// interpret: a file past [`MAX_FILE_BYTES`], invalid JSON, or a schema from
    /// a version that knew things this one does not. The caller then starts from
    /// an empty memory, which groups the strip by the output each workspace is
    /// on — exactly how it behaved before homes existed — and must not overwrite
    /// the file it could not read.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_FILE_BYTES {
            return None;
        }
        let parsed: Self = serde_json::from_slice(bytes).ok()?;
        if parsed.schema > SCHEMA_VERSION {
            return None;
        }
        Some(parsed.clamped())
    }

    /// Replaces every declaration with the person's current ones.
    ///
    /// Wholesale rather than merged, because this is the settings file speaking
    /// and a declaration the person deleted there has to disappear here. What
    /// was *learned* is untouched: removing a declaration is meant to reveal the
    /// observation underneath it, not to erase it.
    ///
    /// Returns whether anything changed, so a caller can tell a settings edit
    /// that matters from one that does not.
    pub fn set_declarations<'a>(
        &mut self,
        declarations: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> bool {
        let mut replacement = BTreeMap::new();
        for (label, output) in declarations {
            let label = bounded(label, MAX_NAME_CHARS);
            let output = bounded(output, MAX_NAME_CHARS);
            if label.is_empty() || output.is_empty() {
                continue;
            }
            if replacement.len() >= MAX_HOMES {
                break;
            }
            replacement.insert(label, output);
        }

        let changed = replacement != self.declared;
        self.declared = replacement;
        changed
    }

    /// Where a workspace belongs, if anything knows.
    ///
    /// A declaration answers first. This is the whole point of the split: a
    /// memory that learned a displaced placement is repaired by declaring the
    /// right one, not by finding and deleting the wrong one.
    #[must_use]
    pub fn home_of(&self, label: &str) -> Option<&str> {
        self.declared
            .get(label)
            .or_else(|| self.learned.get(label))
            .map(String::as_str)
    }

    /// Records the person's own answer, which no observation may overwrite.
    ///
    /// An empty output clears the declaration and lets whatever was learned show
    /// through again, so declaring is reversible without a second verb.
    pub fn declare(&mut self, label: &str, output: &str) {
        let label = bounded(label, MAX_NAME_CHARS);
        if label.is_empty() {
            return;
        }

        let output = bounded(output, MAX_NAME_CHARS);
        if output.is_empty() {
            self.declared.remove(&label);
            return;
        }

        if self.declared.len() >= MAX_HOMES && !self.declared.contains_key(&label) {
            return;
        }

        self.declared.insert(label, output);
    }

    /// Learns from one frame, and refuses to learn from a frame that cannot know.
    ///
    /// Returns whether anything was recorded, so a caller can persist only when
    /// there is something new to persist rather than rewriting the file on every
    /// compositor event.
    ///
    /// Two refusals, and both matter more than what is accepted:
    ///
    /// - A frame showing one output teaches nothing. Every workspace in it is on
    ///   that output whether it belongs there or was displaced there, and those
    ///   are the two cases this module exists to tell apart.
    /// - A workspace whose home is already known is left alone. The frame that
    ///   would rewrite it is the frame where its monitor went away, which is
    ///   precisely the frame that is wrong about it.
    pub fn learn(&mut self, workspaces: &[Workspace]) -> bool {
        if distinct_outputs(workspaces) < 2 {
            return false;
        }

        let mut learned_something = false;
        for workspace in workspaces {
            if !workspace.is_named() || workspace.output.is_empty() {
                continue;
            }
            if self.learned.contains_key(&workspace.label) {
                continue;
            }
            if self.learned.len() >= MAX_HOMES {
                break;
            }

            self.learned
                .insert(workspace.label.clone(), workspace.output.clone());
            learned_something = true;
        }

        learned_something
    }

    /// The declarations exactly as they are held, already bounded.
    #[must_use]
    pub fn declarations(&self) -> &BTreeMap<String, String> {
        &self.declared
    }

    /// Drops everything observed, keeping every declaration.
    ///
    /// The way out of a memory that learned a layout the person has since
    /// changed: forget what was seen, keep what was said, and let the next
    /// multi-output frame teach it again.
    pub fn forget_learned(&mut self) {
        self.learned.clear();
    }

    /// How many homes are remembered, observed and declared together. For
    /// diagnostics and tests; the panel never counts them.
    #[must_use]
    pub fn len(&self) -> usize {
        self.learned.len() + self.declared.len()
    }

    /// Whether nothing is known at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.learned.is_empty() && self.declared.is_empty()
    }
}

/// One monitor's worth of workspaces, as the strip will draw it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The output this group belongs to. Also the label a collapsed capsule
    /// carries, so a person can tell which monitor is behind it.
    pub key: String,
    /// Whether the strip shows this group's workspaces or a single capsule.
    /// Exactly one group in a strip is expanded.
    pub expanded: bool,
    /// The group's workspaces, in the order the compositor gave them.
    pub workspaces: Vec<Workspace>,
    /// Whether anything inside is asking for attention. A capsule that hid an
    /// urgent workspace would be the collapse telling a lie.
    pub urgent: bool,
    /// Whether anything inside holds a window.
    pub occupied: bool,
}

impl Group {
    /// How many workspaces are behind a collapsed capsule.
    #[must_use]
    pub fn len(&self) -> usize {
        self.workspaces.len()
    }

    /// Whether the group carries nothing. Grouping never produces one of these;
    /// the accessor exists so callers do not have to reach for `len`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.workspaces.is_empty()
    }

    /// Which workspace a click on the collapsed capsule should ask for: the one
    /// that was active on that monitor, or its first if the memory has nothing.
    ///
    /// The capsule does not expand itself. It asks for a focus like every other
    /// control in this shell, and the group expands because the focus arrived —
    /// so there is one rule for expansion rather than two.
    #[must_use]
    pub fn focus_target(&self) -> Option<&Workspace> {
        self.workspaces
            .iter()
            .find(|workspace| workspace.active)
            .or_else(|| self.workspaces.first())
    }
}

/// Splits a strip's workspaces into monitor groups, with one of them expanded.
///
/// The order of the groups follows the order their first workspace appeared in,
/// which is the compositor's own ordering, so a strip never rearranges itself
/// between frames for a reason the person cannot see.
///
/// A single group is returned expanded and alone. That is the ordinary case —
/// every monitor connected, each panel showing its own five — and the caller is
/// meant to render it exactly as it rendered before this module existed. There
/// is no capsule, no chrome and nothing to explain when there is nothing to
/// collapse.
#[must_use]
pub fn group(workspaces: &[Workspace], homes: &Homes) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();

    for workspace in workspaces {
        let key = homes
            .home_of(&workspace.label)
            .unwrap_or(&workspace.output)
            .to_owned();

        let position = groups.iter().position(|group| group.key == key);
        let index = match position {
            Some(index) => index,
            None => {
                // A session with more monitors than the bound leaves the excess
                // with the group of the output they are actually on rather than
                // dropping workspaces the person can still switch to.
                if groups.len() >= MAX_GROUPS {
                    groups
                        .iter()
                        .position(|group| group.key == workspace.output)
                        .unwrap_or_default()
                } else {
                    groups.push(Group {
                        key,
                        expanded: false,
                        workspaces: Vec::new(),
                        urgent: false,
                        occupied: false,
                    });
                    groups.len() - 1
                }
            }
        };

        groups[index].urgent |= workspace.urgent;
        groups[index].occupied |= workspace.occupied;
        groups[index].workspaces.push(workspace.clone());
    }

    // Expansion follows the focus. A strip with nothing active — every window
    // closed on a monitor that is off, say — expands its first group rather than
    // presenting a row of capsules with no way in.
    let expanded = groups
        .iter()
        .position(|group| group.workspaces.iter().any(|workspace| workspace.active));
    if let Some(index) = expanded.or(if groups.is_empty() { None } else { Some(0) }) {
        groups[index].expanded = true;
    }

    groups
}

/// Brings one persisted map inside the bounds every name crossing this module
/// obeys. Truncation rather than refusal: a name that came back too long is
/// still the best answer anyone has about that workspace.
fn bounded_map(map: BTreeMap<String, String>) -> BTreeMap<String, String> {
    map.into_iter()
        .map(|(label, output)| {
            (
                bounded(&label, MAX_NAME_CHARS),
                bounded(&output, MAX_NAME_CHARS),
            )
        })
        .filter(|(label, output)| !label.is_empty() && !output.is_empty())
        .take(MAX_HOMES)
        .collect()
}

/// How many different outputs a frame mentions.
fn distinct_outputs(workspaces: &[Workspace]) -> usize {
    let mut seen: Vec<&str> = Vec::new();
    for workspace in workspaces {
        if workspace.output.is_empty() {
            continue;
        }
        if !seen.contains(&workspace.output.as_str()) {
            seen.push(&workspace.output);
        }
    }
    seen.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The author's layout: five workspaces on each of three monitors.
    fn three_monitors() -> Vec<Workspace> {
        let mut workspaces = Vec::new();
        for index in 1..=5 {
            workspaces.push(Workspace::new(
                &index.to_string(),
                "HDMI-A-1",
                index == 1,
                false,
                false,
            ));
        }
        for index in 6..=10 {
            workspaces.push(Workspace::new(
                &index.to_string(),
                "DP-1",
                index == 6,
                false,
                false,
            ));
        }
        for index in 11..=15 {
            workspaces.push(Workspace::new(
                &index.to_string(),
                "DP-2",
                index == 11,
                false,
                false,
            ));
        }
        workspaces
    }

    /// The same session with two monitors switched off: everything has moved to
    /// the survivor and the compositor no longer says where any of it came from.
    fn collapsed_onto_one(active: &str) -> Vec<Workspace> {
        (1..=15)
            .map(|index| {
                let label = index.to_string();
                let is_active = label == active;
                Workspace::new(&label, "HDMI-A-1", is_active, false, false)
            })
            .collect()
    }

    #[test]
    fn a_single_output_frame_teaches_nothing() {
        let mut homes = Homes::new();

        assert!(!homes.learn(&collapsed_onto_one("1")));
        assert!(homes.is_empty());
        // This is the whole point: on a machine whose other two monitors are off
        // today, learning must not record fifteen workspaces as living on the
        // one that survived.
        assert_eq!(homes.home_of("7"), None);
    }

    #[test]
    fn a_multi_output_frame_records_every_named_workspace() {
        let mut homes = Homes::new();

        assert!(homes.learn(&three_monitors()));
        assert_eq!(homes.home_of("3"), Some("HDMI-A-1"));
        assert_eq!(homes.home_of("7"), Some("DP-1"));
        assert_eq!(homes.home_of("13"), Some("DP-2"));
    }

    #[test]
    fn a_known_home_survives_its_monitor_being_turned_off() {
        let mut homes = Homes::new();
        homes.learn(&three_monitors());

        // The displaced frame is exactly the one that would rewrite the memory,
        // so it is refused even though it changes nothing else.
        assert!(!homes.learn(&collapsed_onto_one("1")));
        assert_eq!(homes.home_of("7"), Some("DP-1"));
    }

    #[test]
    fn a_declaration_overrules_what_was_learned() {
        let mut homes = Homes::new();
        homes.learn(&three_monitors());

        homes.declare("7", "DP-2");

        assert_eq!(homes.home_of("7"), Some("DP-2"));
    }

    #[test]
    fn clearing_a_declaration_reveals_what_was_learned() {
        let mut homes = Homes::new();
        homes.learn(&three_monitors());
        homes.declare("7", "DP-2");

        homes.declare("7", "");

        assert_eq!(homes.home_of("7"), Some("DP-1"));
    }

    #[test]
    fn forgetting_keeps_every_declaration() {
        let mut homes = Homes::new();
        homes.learn(&three_monitors());
        homes.declare("7", "DP-2");

        homes.forget_learned();

        assert_eq!(homes.home_of("7"), Some("DP-2"));
        assert_eq!(homes.home_of("3"), None);
    }

    #[test]
    fn an_unnamed_workspace_is_never_remembered() {
        let mut homes = Homes::new();
        let mut workspaces = three_monitors();
        workspaces.push(Workspace::new("", "DP-2", false, false, false));

        homes.learn(&workspaces);

        assert_eq!(homes.len(), 15);
    }

    #[test]
    fn the_memory_is_bounded() {
        let mut homes = Homes::new();
        let workspaces: Vec<Workspace> = (0..MAX_HOMES + 40)
            .map(|index| {
                let output = if index % 2 == 0 { "A" } else { "B" };
                Workspace::new(&index.to_string(), output, false, false, false)
            })
            .collect();

        homes.learn(&workspaces);

        assert_eq!(homes.len(), MAX_HOMES);
    }

    #[test]
    fn one_monitor_of_workspaces_is_one_expanded_group() {
        let homes = Homes::new();
        let workspaces: Vec<Workspace> = three_monitors()
            .into_iter()
            .filter(|workspace| workspace.output == "HDMI-A-1")
            .collect();

        let groups = group(&workspaces, &homes);

        // The ordinary case: nothing to collapse, so the caller draws the strip
        // it always drew.
        assert_eq!(groups.len(), 1);
        assert!(groups[0].expanded);
        assert_eq!(groups[0].len(), 5);
    }

    #[test]
    fn fifteen_displaced_workspaces_become_three_groups() {
        let mut homes = Homes::new();
        homes.learn(&three_monitors());

        let groups = group(&collapsed_onto_one("1"), &homes);

        assert_eq!(groups.len(), 3);
        assert_eq!(
            groups.iter().map(|g| g.key.as_str()).collect::<Vec<_>>(),
            ["HDMI-A-1", "DP-1", "DP-2"]
        );
        assert_eq!(groups.iter().map(Group::len).collect::<Vec<_>>(), [5, 5, 5]);
    }

    #[test]
    fn the_group_holding_the_active_workspace_is_the_expanded_one() {
        let mut homes = Homes::new();
        homes.learn(&three_monitors());

        let groups = group(&collapsed_onto_one("13"), &homes);

        let expanded: Vec<&str> = groups
            .iter()
            .filter(|group| group.expanded)
            .map(|group| group.key.as_str())
            .collect();
        assert_eq!(expanded, ["DP-2"]);
    }

    #[test]
    fn a_collapsed_group_still_reports_urgency() {
        let mut homes = Homes::new();
        homes.learn(&three_monitors());
        let mut workspaces = collapsed_onto_one("1");
        workspaces[8].urgent = true; // workspace "9", which lives on DP-1

        let groups = group(&workspaces, &homes);

        let dp1 = groups.iter().find(|group| group.key == "DP-1").unwrap();
        assert!(!dp1.expanded);
        assert!(dp1.urgent);
    }

    #[test]
    fn a_capsule_asks_for_the_workspace_that_was_active_on_its_monitor() {
        let mut homes = Homes::new();
        homes.learn(&three_monitors());
        let mut workspaces = collapsed_onto_one("1");
        // Niri keeps one active workspace per output; a displaced group can
        // still carry the one it had.
        workspaces[7].active = true; // workspace "8"

        let groups = group(&workspaces, &homes);

        let dp1 = groups.iter().find(|group| group.key == "DP-1").unwrap();
        assert_eq!(dp1.focus_target().map(|w| w.label.as_str()), Some("8"));
    }

    #[test]
    fn a_capsule_with_nothing_active_asks_for_its_first_workspace() {
        let mut homes = Homes::new();
        homes.learn(&three_monitors());

        let groups = group(&collapsed_onto_one("1"), &homes);

        let dp2 = groups.iter().find(|group| group.key == "DP-2").unwrap();
        assert_eq!(dp2.focus_target().map(|w| w.label.as_str()), Some("11"));
    }

    #[test]
    fn a_workspace_with_no_known_home_groups_by_where_it_is() {
        let homes = Homes::new();

        let groups = group(&three_monitors(), &homes);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].key, "HDMI-A-1");
    }

    #[test]
    fn a_strip_with_nothing_active_still_opens_one_group() {
        let mut homes = Homes::new();
        homes.learn(&three_monitors());
        let workspaces: Vec<Workspace> = collapsed_onto_one("none");

        let groups = group(&workspaces, &homes);

        assert_eq!(groups.iter().filter(|group| group.expanded).count(), 1);
        assert!(groups[0].expanded);
    }

    #[test]
    fn a_written_memory_reads_back_identical() {
        let mut homes = Homes::new();
        homes.learn(&three_monitors());
        homes.declare("7", "DP-2");

        let bytes = homes.to_bytes().expect("the memory serializes");

        assert_eq!(Homes::from_bytes(&bytes), Some(homes));
    }

    #[test]
    fn a_truncated_or_corrupt_file_is_refused_rather_than_guessed_at() {
        let mut homes = Homes::new();
        homes.learn(&three_monitors());
        let bytes = homes.to_bytes().expect("the memory serializes");

        // The half-written file a physical reset leaves behind.
        assert_eq!(Homes::from_bytes(&bytes[..bytes.len() / 2]), None);
        assert_eq!(Homes::from_bytes(b"not json at all"), None);
        assert_eq!(Homes::from_bytes(&[]), None);
    }

    #[test]
    fn a_file_from_a_later_schema_is_not_read() {
        let future = format!(
            r#"{{"schema":{},"learned":{{"7":"DP-1"}}}}"#,
            SCHEMA_VERSION + 1
        );

        // Reading it would mean inventing a layout from fields this version
        // does not know it is missing.
        assert_eq!(Homes::from_bytes(future.as_bytes()), None);
    }

    #[test]
    fn an_oversized_file_is_refused_before_it_is_parsed() {
        let bytes = vec![b'{'; MAX_FILE_BYTES + 1];

        assert_eq!(Homes::from_bytes(&bytes), None);
    }

    #[test]
    fn a_hand_edited_file_is_bounded_rather_than_rejected() {
        let long = "x".repeat(MAX_NAME_CHARS * 3);
        let file = format!(r#"{{"schema":1,"learned":{{"{long}":"{long}","":"DP-1","7":""}}}}"#);

        let homes = Homes::from_bytes(file.as_bytes()).expect("a readable file");

        // The over-long pair survives truncated; the two nameless halves do not
        // survive at all, because neither names anything.
        assert_eq!(homes.len(), 1);
        assert_eq!(
            homes.home_of(&"x".repeat(MAX_NAME_CHARS)),
            Some("x".repeat(MAX_NAME_CHARS).as_str())
        );
    }

    #[test]
    fn declarations_are_replaced_wholesale_and_never_touch_what_was_learned() {
        let mut homes = Homes::new();
        homes.learn(&three_monitors());

        assert!(homes.set_declarations([("7", "DP-2"), ("3", "DP-2")]));
        assert_eq!(homes.home_of("7"), Some("DP-2"));

        // The person removed one of the two from their settings file. It has to
        // disappear here, revealing the observation it was covering.
        assert!(homes.set_declarations([("3", "DP-2")]));
        assert_eq!(homes.home_of("7"), Some("DP-1"));
        assert_eq!(homes.home_of("3"), Some("DP-2"));

        // The same settings twice is not a change worth acting on.
        assert!(!homes.set_declarations([("3", "DP-2")]));
    }

    #[test]
    fn a_declaration_with_a_nameless_half_declares_nothing() {
        let mut homes = Homes::new();
        homes.learn(&three_monitors());

        homes.set_declarations([("", "DP-2"), ("7", "")]);

        assert_eq!(homes.home_of("7"), Some("DP-1"));
    }

    #[test]
    fn an_empty_strip_produces_no_groups() {
        let homes = Homes::new();

        assert!(group(&[], &homes).is_empty());
    }
}
