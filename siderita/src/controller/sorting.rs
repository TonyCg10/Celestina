//! The sort field a stored index names.
//!
//! Kept apart from the coordinator because it is a mapping with one job and its
//! own reason to change: the indices are what QML and the settings file speak,
//! so they are stable in a way the enum behind them need not be.

use siderita_core::SortField;

pub(crate) const fn sort_field_from_index(index: i32) -> Option<SortField> {
    match index {
        0 => Some(SortField::Name),
        1 => Some(SortField::Size),
        2 => Some(SortField::Modified),
        3 => Some(SortField::Kind),
        _ => None,
    }
}

pub(crate) const RECENT_LIMIT: usize = 100;

#[cfg(test)]
mod tests {
    use super::sort_field_from_index;
    use siderita_core::SortField;

    #[test]
    fn sort_field_indices_are_stable_for_qml() {
        assert_eq!(sort_field_from_index(0), Some(SortField::Name));
        assert_eq!(sort_field_from_index(1), Some(SortField::Size));
        assert_eq!(sort_field_from_index(2), Some(SortField::Modified));
        assert_eq!(sort_field_from_index(3), Some(SortField::Kind));
        assert_eq!(sort_field_from_index(4), None);
    }
}
