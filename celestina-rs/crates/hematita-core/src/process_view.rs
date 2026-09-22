//! What the table shows, decided without Qt: which rows survive the search,
//! in what order, and how they gather under their applications.

use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq)]
pub struct ProcessRow {
    pub pid: u32,
    pub name: String,
    pub uid: u32,
    pub cpu_percent: f32,
    pub memory_kib: u64,
    pub read_rate: f64,
    pub write_rate: f64,
    pub application: Option<String>,
    pub actionable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortField {
    Cpu,
    Memory,
    Name,
    Pid,
    Read,
    Write,
}

impl SortField {
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "cpu" => Some(Self::Cpu),
            "memory" => Some(Self::Memory),
            "name" => Some(Self::Name),
            "pid" => Some(Self::Pid),
            "read" => Some(Self::Read),
            "write" => Some(Self::Write),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Memory => "memory",
            Self::Name => "name",
            Self::Pid => "pid",
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

/// Indices of the rows that match `filter` (case-insensitive substring of the
/// name, the PID as text or the application id; empty matches all), sorted by
/// `field`; ties break by PID ascending so the order is stable across ticks.
#[must_use]
pub fn project(rows: &[ProcessRow], filter: &str, field: SortField, ascending: bool) -> Vec<usize> {
    let needle = filter.trim().to_lowercase();
    let mut order: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| {
            needle.is_empty()
                || row.name.to_lowercase().contains(&needle)
                || row.pid.to_string().contains(&needle)
                || row
                    .application
                    .as_deref()
                    .is_some_and(|app| app.to_lowercase().contains(&needle))
        })
        .map(|(index, _)| index)
        .collect();
    order.sort_by(|&a, &b| {
        let (left, right) = (&rows[a], &rows[b]);
        let primary = match field {
            SortField::Cpu => left
                .cpu_percent
                .partial_cmp(&right.cpu_percent)
                .unwrap_or(Ordering::Equal),
            SortField::Memory => left.memory_kib.cmp(&right.memory_kib),
            SortField::Name => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
            SortField::Pid => left.pid.cmp(&right.pid),
            SortField::Read => left
                .read_rate
                .partial_cmp(&right.read_rate)
                .unwrap_or(Ordering::Equal),
            SortField::Write => left
                .write_rate
                .partial_cmp(&right.write_rate)
                .unwrap_or(Ordering::Equal),
        };
        let primary = if ascending {
            primary
        } else {
            primary.reverse()
        };
        primary.then_with(|| left.pid.cmp(&right.pid))
    });
    order
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub id: String,
    pub member_indices: Vec<usize>,
    pub cpu_percent: f32,
    pub memory_kib: u64,
}

/// Gathers the rows in `order` under their application, groups in the order
/// their first member appears. Rows with no application form no group.
#[must_use]
pub fn group(rows: &[ProcessRow], order: &[usize]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for &index in order {
        let Some(application) = rows[index].application.as_deref() else {
            continue;
        };
        let row = &rows[index];
        match groups.iter_mut().find(|group| group.id == application) {
            Some(group) => {
                group.member_indices.push(index);
                group.cpu_percent += row.cpu_percent;
                group.memory_kib = group.memory_kib.saturating_add(row.memory_kib);
            }
            None => groups.push(Group {
                id: application.to_owned(),
                member_indices: vec![index],
                cpu_percent: row.cpu_percent,
                memory_kib: row.memory_kib,
            }),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pid: u32, name: &str, cpu: f32, memory: u64, app: Option<&str>) -> ProcessRow {
        ProcessRow {
            pid,
            name: name.to_owned(),
            uid: 1000,
            cpu_percent: cpu,
            memory_kib: memory,
            read_rate: 0.0,
            write_rate: f64::from(pid),
            application: app.map(str::to_owned),
            actionable: true,
        }
    }

    fn rows() -> Vec<ProcessRow> {
        vec![
            row(10, "kitty", 1.0, 500, Some("kitty")),
            row(20, "Firefox", 30.0, 9000, Some("firefox")),
            row(30, "cargo", 30.0, 3000, None),
            row(40, "Web Content", 5.0, 4000, Some("firefox")),
        ]
    }

    #[test]
    fn the_filter_matches_name_pid_and_application_case_insensitively() {
        let rows = rows();
        assert_eq!(project(&rows, "fire", SortField::Pid, true), vec![1, 3]);
        assert_eq!(project(&rows, "30", SortField::Pid, true), vec![2]);
        assert_eq!(project(&rows, "WEB", SortField::Pid, true), vec![3]);
        assert_eq!(project(&rows, "", SortField::Pid, true), vec![0, 1, 2, 3]);
        assert_eq!(
            project(&rows, "nothing", SortField::Pid, true),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn sorting_honours_the_field_and_direction_and_breaks_ties_by_pid() {
        let rows = rows();
        assert_eq!(project(&rows, "", SortField::Cpu, false), vec![1, 2, 3, 0]);
        assert_eq!(project(&rows, "", SortField::Cpu, true), vec![0, 3, 1, 2]);
        assert_eq!(project(&rows, "", SortField::Name, true), vec![2, 1, 0, 3]);
        assert_eq!(
            project(&rows, "", SortField::Memory, false),
            vec![1, 3, 2, 0]
        );
        assert_eq!(
            project(&rows, "", SortField::Write, false),
            vec![3, 2, 1, 0]
        );
    }

    #[test]
    fn groups_follow_first_appearance_and_sum_their_members() {
        let rows = rows();
        let order = project(&rows, "", SortField::Cpu, false);
        let groups = group(&rows, &order);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].id, "firefox");
        assert_eq!(groups[0].member_indices, vec![1, 3]);
        assert_eq!(groups[0].cpu_percent, 35.0);
        assert_eq!(groups[0].memory_kib, 13000);
        assert_eq!(groups[1].id, "kitty");
    }

    #[test]
    fn sort_tokens_round_trip() {
        for field in [
            SortField::Cpu,
            SortField::Memory,
            SortField::Name,
            SortField::Pid,
            SortField::Read,
            SortField::Write,
        ] {
            assert_eq!(SortField::from_token(field.as_str()), Some(field));
        }
        assert_eq!(SortField::from_token("bogus"), None);
    }
}
