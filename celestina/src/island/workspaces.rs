//! Pure mapping from the niri model to the workspaces one Island shows.
//! Ported from the spike's `src/island/workspaces.rs`; the labels live in
//! [`crate::copy`].

use crate::{copy, providers::niri::NiriModel};

/// One workspace of the Island's output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// niri's workspace id: the target of a focus request.
    pub id: u64,
    /// The workspace's name, or its numbered label.
    pub text: String,
    /// The workspace shown on the output.
    pub active: bool,
}

/// The workspaces of `output`, in niri's order (by index).
#[must_use]
pub fn rows_for(model: &NiriModel, output: &str) -> Vec<Row> {
    model
        .workspaces
        .iter()
        .filter(|w| w.output.as_deref() == Some(output))
        .map(|w| Row {
            id: w.id,
            text: w
                .name
                .clone()
                .unwrap_or_else(|| copy::workspace_number(w.idx)),
            active: w.is_active,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::niri::Workspace;

    fn w(id: u64, idx: u8, name: Option<&str>, out: &str, active: bool) -> Workspace {
        Workspace {
            id,
            idx,
            name: name.map(str::to_owned),
            output: Some(out.into()),
            is_active: active,
            is_focused: false,
            columns: 0,
        }
    }

    #[test]
    fn rows_keep_only_the_islands_output_and_name_or_number_them() {
        let model = NiriModel {
            workspaces: vec![
                w(1, 1, None, "DP-1", false),
                w(2, 2, Some("web"), "DP-1", true),
                w(3, 1, None, "HDMI-A-1", true),
            ],
            ..NiriModel::default()
        };
        let rows = rows_for(&model, "DP-1");
        let shown: Vec<(&str, bool)> = rows.iter().map(|r| (r.text.as_str(), r.active)).collect();
        let first = copy::workspace_number(1);
        assert_eq!(shown, [(first.as_str(), false), ("web", true)]);
        assert_eq!(rows[1].id, 2);
        assert!(rows_for(&model, "DP-9").is_empty());
        assert!(rows_for(&NiriModel::default(), "DP-1").is_empty());
    }
}
