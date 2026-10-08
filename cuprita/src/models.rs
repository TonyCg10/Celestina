//! The list models the pages read, one row per network, device, endpoint or
//! stream. Each is a `QAbstractListModel` whose rows are domain values. The
//! bridges live in `controller/` (cxx-qt-build takes the bridge files of one
//! QML module from a single directory, QTBUG-93443); this
//! module owns the one recipe they share: bringing the rows in line with a
//! new snapshot by key, so a row that did not change is not touched and a
//! row that did is updated in place instead of being rebuilt (no flicker, no
//! lost focus).

/// A row with a key that stays the same across snapshots.
pub trait Keyed: Clone + PartialEq {
    fn key(&self) -> String;
}

/// What a model must do around each change, in Qt's begin/end order.
pub trait RowSink<R> {
    fn rows(&mut self) -> &mut Vec<R>;
    fn begin_insert(&mut self, at: usize);
    fn end_insert(&mut self);
    fn begin_remove(&mut self, at: usize);
    fn end_remove(&mut self);
    /// One row moves up from `from` to `to` (`to < from`).
    fn begin_move(&mut self, from: usize, to: usize);
    fn end_move(&mut self);
    fn changed(&mut self, at: usize);
}

/// Brings `sink`'s rows to `next`: removes the rows whose key left, then walks
/// `next` in order, updating a row in place when its key is already there,
/// moving it up (a real move, so its delegate survives) when it sits further
/// down, and inserting it otherwise. A key repeated in `next` keeps its first
/// row only, and anything left past the end is removed.
pub fn reconcile<R: Keyed, S: RowSink<R>>(sink: &mut S, next: Vec<R>) {
    let mut seen = std::collections::HashSet::new();
    let next: Vec<R> = next.into_iter().filter(|r| seen.insert(r.key())).collect();

    let mut at = sink.rows().len();
    while at > 0 {
        at -= 1;
        if !seen.contains(&sink.rows()[at].key()) {
            sink.begin_remove(at);
            sink.rows().remove(at);
            sink.end_remove();
        }
    }

    let wanted = next.len();
    for (index, row) in next.into_iter().enumerate() {
        let key = row.key();
        let found = sink.rows()[index..].iter().position(|r| r.key() == key);
        match found {
            Some(offset) => {
                if offset > 0 {
                    let from = index + offset;
                    sink.begin_move(from, index);
                    let moved = sink.rows().remove(from);
                    sink.rows().insert(index, moved);
                    sink.end_move();
                }
                if sink.rows()[index] != row {
                    sink.rows()[index] = row;
                    sink.changed(index);
                }
            }
            None => {
                sink.begin_insert(index);
                sink.rows().insert(index, row);
                sink.end_insert();
            }
        }
    }

    while sink.rows().len() > wanted {
        let last = sink.rows().len() - 1;
        sink.begin_remove(last);
        sink.rows().pop();
        sink.end_remove();
    }
}

/// The numeric role a model's first named role takes (`Qt::UserRole`).
pub const FIRST_ROLE: i32 = 0x0100;

/// The `QHash<int, QByteArray>` a model's `roleNames()` returns.
pub fn role_names(names: &[&str]) -> cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray> {
    let mut hash = cxx_qt_lib::QHash::<cxx_qt_lib::QHashPair_i32_QByteArray>::default();
    for (offset, name) in (0_i32..).zip(names) {
        hash.insert(FIRST_ROLE + offset, cxx_qt_lib::QByteArray::from(*name));
    }
    hash
}

/// The row a `QModelIndex` names, when it is in range.
pub fn row_of<'a, R>(rows: &'a [R], index: &cxx_qt_lib::QModelIndex) -> Option<&'a R> {
    usize::try_from(index.row())
        .ok()
        .and_then(|row| rows.get(row))
}

/// A count as Qt wants it.
pub fn count(len: usize) -> i32 {
    i32::try_from(len).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, PartialEq, Debug)]
    struct Item(&'static str, u8);

    impl Keyed for Item {
        fn key(&self) -> String {
            self.0.to_owned()
        }
    }

    #[derive(Default)]
    struct Log {
        rows: Vec<Item>,
        events: Vec<String>,
    }

    impl RowSink<Item> for Log {
        fn rows(&mut self) -> &mut Vec<Item> {
            &mut self.rows
        }
        fn begin_insert(&mut self, at: usize) {
            self.events.push(format!("insert {at}"));
        }
        fn end_insert(&mut self) {}
        fn begin_remove(&mut self, at: usize) {
            self.events.push(format!("remove {at}"));
        }
        fn end_remove(&mut self) {}
        fn begin_move(&mut self, from: usize, to: usize) {
            self.events.push(format!("move {from} {to}"));
        }
        fn end_move(&mut self) {}
        fn changed(&mut self, at: usize) {
            self.events.push(format!("change {at}"));
        }
    }

    #[test]
    fn unchanged_rows_are_not_touched() {
        let mut log = Log {
            rows: vec![Item("a", 1), Item("b", 2)],
            ..Log::default()
        };
        reconcile(&mut log, vec![Item("a", 1), Item("b", 3)]);
        assert_eq!(log.events, ["change 1"]);
        assert_eq!(log.rows, [Item("a", 1), Item("b", 3)]);
    }

    #[test]
    fn rows_leave_arrive_and_move() {
        let mut log = Log {
            rows: vec![Item("a", 1), Item("b", 2), Item("c", 3)],
            ..Log::default()
        };
        reconcile(&mut log, vec![Item("c", 3), Item("d", 4), Item("a", 1)]);
        assert_eq!(log.rows, [Item("c", 3), Item("d", 4), Item("a", 1)]);
        assert_eq!(log.events, ["remove 1", "move 1 0", "insert 1"]);
    }

    #[test]
    fn a_key_moving_up_is_a_move() {
        let mut log = Log {
            rows: vec![Item("a", 1), Item("b", 2), Item("c", 3)],
            ..Log::default()
        };
        reconcile(&mut log, vec![Item("c", 3), Item("a", 1), Item("b", 2)]);
        assert_eq!(log.rows, [Item("c", 3), Item("a", 1), Item("b", 2)]);
        assert_eq!(log.events, ["move 2 0"]);
    }

    #[test]
    fn a_duplicate_key_does_not_leave_rows_behind() {
        let mut log = Log::default();
        reconcile(&mut log, vec![Item("x", 1), Item("x", 2), Item("y", 3)]);
        assert_eq!(log.rows, [Item("x", 1), Item("y", 3)]);
        reconcile(&mut log, vec![Item("x", 5)]);
        assert_eq!(log.rows, [Item("x", 5)]);
        // Old behaviour without trimming: two rows from a doubled snapshot.
        let mut stale = Log {
            rows: vec![Item("x", 1), Item("x", 2)],
            ..Log::default()
        };
        reconcile(&mut stale, vec![Item("x", 1)]);
        assert_eq!(stale.rows, [Item("x", 1)]);
    }
}
