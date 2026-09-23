//! language-contract: product-copy
//!
//! The transient announcements, and the one column that shows them.
//!
//! `status_text` used to carry four unrelated kinds of message through one
//! property: a running job (which the ring above it already drew), an activity
//! with no knowable end, an outcome that had already happened, and two standing
//! facts about the folder. That is why the strip across the bottom bar could
//! never simply be deleted, and why the unmount line stayed on screen until
//! something else happened to overwrite it.
//!
//! What replaces it is a queue whose entries have owners. A notice pushed while
//! something runs is settled or dropped by the same completion path that ends
//! the work; a notice pushed after the fact is settled from birth. The surface
//! decides when a settled notice has been read and asks for it to be dropped.
//!
//! The queue belongs to the controller, not to the process: a job is global (it
//! outlives the tab that started it) but an announcement is addressed to the
//! person looking at this folder right now.
//!
//! The marker above declares the Spanish here: a notice's text is the line a
//! person reads.

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use super::qobject;

/// Which of the two tones a notice wears. Anything a person must be able to
/// read on a long path is `Danger`: it wraps, it stays longer, and it is the
/// top of the column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoticeTone {
    Info,
    Danger,
}

impl NoticeTone {
    fn token(self) -> &'static str {
        match self {
            NoticeTone::Info => "info",
            NoticeTone::Danger => "danger",
        }
    }
}

/// One announcement.
pub(crate) struct Notice {
    id: u64,
    text: String,
    icon: String,
    tone: NoticeTone,
    /// True while the action it names is still in flight. The surface draws a
    /// running notice only once it has lasted past its appearance threshold,
    /// which is what keeps a 300 ms unmount from flashing.
    running: bool,
}

/// At most this many at once. A fourth would push the column into the rows, and
/// three unread announcements already means nobody is reading them.
const VISIBLE: usize = 3;

/// The queue itself, with no Qt in it.
///
/// Separated from the controller on purpose: the rule worth testing is that
/// every notice pushed while something runs is eventually settled or dropped,
/// and that rule is about this data structure rather than about a QObject. A
/// test that needed a controller instance could not run at all, and one that
/// grepped the sources for call pairs would be a grep pretending to be a test.
#[derive(Default)]
pub(crate) struct Notices {
    entries: Vec<Notice>,
    next_id: u64,
    /// The notice announcing the read in flight, or zero. A controller reads
    /// one location at a time, so one handle is the whole bookkeeping.
    read: u64,
}

impl Notices {
    /// Adds a notice and hands back the caller's handle for settling or
    /// dropping it later.
    fn push(&mut self, text: &str, icon: &str, tone: NoticeTone, running: bool) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.entries.push(Notice {
            id,
            text: text.to_owned(),
            icon: icon.to_owned(),
            tone,
            running,
        });
        while self.entries.len() > VISIBLE {
            self.entries.remove(0);
        }
        id
    }

    /// Marks a notice finished and gives it its settled wording. Answers
    /// whether it was still there: a notice evicted by the cap is not an error,
    /// and its owner must not treat the miss as one.
    fn settle(&mut self, id: u64, text: &str) -> bool {
        let Some(notice) = self.entries.iter_mut().find(|notice| notice.id == id) else {
            return false;
        };
        notice.running = false;
        notice.text = text.to_owned();
        true
    }

    /// Removes a notice outright: a read that simply finished, or a settled
    /// notice whose time on screen is up.
    fn remove(&mut self, id: u64) -> bool {
        let before = self.entries.len();
        self.entries.retain(|notice| notice.id != id);
        before != self.entries.len()
    }

    /// One line per notice, as the column consumes them:
    /// `id\ticon\ttone\trunning\ttext`. The text is last and takes the whole
    /// remainder, because a file name may legally contain a tab and the four
    /// fields before it never can.
    fn rows(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|notice| {
                format!(
                    "{}\t{}\t{}\t{}\t{}",
                    notice.id,
                    notice.icon,
                    notice.tone.token(),
                    if notice.running { "1" } else { "0" },
                    notice.text
                )
            })
            .collect()
    }

    /// The notices still claiming that something is happening. The invariant
    /// this module exists to keep is that this is empty once every started
    /// action has finished.
    #[cfg(test)]
    fn running_ids(&self) -> Vec<u64> {
        self.entries
            .iter()
            .filter(|notice| notice.running)
            .map(|notice| notice.id)
            .collect()
    }
}

impl qobject::SideritaController {
    /// Adds a notice and publishes the queue. A caller that pushes with
    /// `running` and never settles or drops it is the defect this module
    /// exists to remove.
    pub(crate) fn push_notice(
        mut self: Pin<&mut Self>,
        text: &str,
        icon: &str,
        tone: NoticeTone,
        running: bool,
    ) -> u64 {
        let id = self
            .as_mut()
            .rust_mut()
            .get_mut()
            .notices
            .push(text, icon, tone, running);
        self.publish_notices();
        id
    }

    /// Marks a notice as finished and gives it its settled wording. A notice
    /// the surface has already drawn mutates in place; one it has not drawn yet
    /// is dropped by the surface without ever appearing.
    pub(crate) fn settle_notice(mut self: Pin<&mut Self>, id: u64, text: &str) {
        if self.as_mut().rust_mut().get_mut().notices.settle(id, text) {
            self.publish_notices();
        }
    }

    /// Removes a notice outright.
    pub(crate) fn drop_notice(mut self: Pin<&mut Self>, id: u64) {
        if self.as_mut().rust_mut().get_mut().notices.remove(id) {
            self.publish_notices();
        }
    }

    /// Announces the read this controller has just started, and remembers it.
    ///
    /// A read is the one activity whose lifetime is already published: it lasts
    /// exactly as long as `loading`. Tying the notice to that single handle is
    /// what makes the owner impossible to forget — `end_read_notice` sits
    /// beside every `set_loading(false)`, and a second read replaces the first
    /// rather than stacking on it, because a controller reads one location at a
    /// time.
    pub(crate) fn begin_read_notice(mut self: Pin<&mut Self>, text: &str, icon: &str) {
        self.as_mut().end_read_notice();
        let id = self
            .as_mut()
            .push_notice(text, icon, NoticeTone::Info, true);
        self.rust_mut().get_mut().notices.read = id;
    }

    /// Ends it. A read that simply finished has no outcome worth a word, so the
    /// notice is dropped rather than settled — and if the threshold never
    /// elapsed, it was never drawn at all.
    pub(crate) fn end_read_notice(mut self: Pin<&mut Self>) {
        let id = self.rust().notices.read;
        if id == 0 {
            return;
        }
        self.as_mut().rust_mut().get_mut().notices.read = 0;
        self.drop_notice(id);
    }

    /// The surface asking for an entry to go: its dwell expired, or a person
    /// pressed it. The two error properties are not queue entries — other
    /// surfaces read them too — so they answer to their own reserved ids.
    pub fn dismiss_notice(mut self: Pin<&mut Self>, id: &QString) {
        let id = id.to_string();
        match id.as_str() {
            "error" => self.as_mut().set_error_text(QString::default()),
            "op-error" => self.as_mut().set_op_error(QString::default()),
            _ => {
                if let Ok(id) = id.parse::<u64>() {
                    self.drop_notice(id);
                }
            }
        }
    }

    /// Publishes the queue as the rows the column consumes.
    pub(crate) fn publish_notices(mut self: Pin<&mut Self>) {
        let mut rows = cxx_qt_lib::QStringList::default();
        for row in self.rust().notices.rows() {
            rows.append(QString::from(row.as_str()));
        }
        self.as_mut().set_notice_rows(rows);
    }
}

#[cfg(test)]
mod tests {
    use super::{NoticeTone, Notices, VISIBLE};

    #[test]
    fn a_fourth_notice_evicts_the_oldest() {
        let mut queue = Notices::default();
        for index in 0..4 {
            queue.push(&format!("aviso {index}"), "info", NoticeTone::Info, false);
        }
        assert_eq!(queue.entries.len(), VISIBLE);
        assert_eq!(queue.entries[0].text, "aviso 1");
    }

    #[test]
    fn settling_stops_the_running_claim_and_changes_the_wording() {
        let mut queue = Notices::default();
        let id = queue.push("Desmontando…", "unplug", NoticeTone::Info, true);
        assert_eq!(queue.running_ids(), vec![id]);
        assert!(queue.settle(id, "Disco desmontado"));
        assert!(queue.running_ids().is_empty());
        assert_eq!(queue.entries[0].text, "Disco desmontado");
    }

    #[test]
    fn settling_or_removing_an_evicted_notice_is_not_an_error() {
        let mut queue = Notices::default();
        let first = queue.push("Leyendo carpeta…", "folder", NoticeTone::Info, true);
        for index in 0..3 {
            queue.push(&format!("aviso {index}"), "info", NoticeTone::Info, false);
        }
        assert!(!queue.settle(first, "Carpeta leída"));
        assert!(!queue.remove(first));
    }

    /// The invariant the whole module exists for. The unmount line used to have
    /// no completion path at all and stayed on screen until something else
    /// happened to overwrite it.
    #[test]
    fn nothing_still_claims_to_be_running_once_every_action_has_ended() {
        let mut queue = Notices::default();
        let mount = queue.push("Montando…", "hard-drive", NoticeTone::Info, true);
        let read = queue.push("Leyendo la papelera…", "user-trash", NoticeTone::Info, true);
        queue.push("Pegado cancelado", "circle-stop", NoticeTone::Info, false);
        assert_eq!(queue.running_ids(), vec![mount, read]);

        // The mount finishes with something worth saying; the read does not.
        queue.settle(mount, "Disco montado");
        queue.remove(read);
        assert!(
            queue.running_ids().is_empty(),
            "a running notice outlived the action that pushed it"
        );
    }

    /// The text is the remainder after four tabs, so a name carrying one
    /// survives. A row cut field-by-field would lose everything after it.
    #[test]
    fn a_row_puts_the_text_last_so_a_tab_in_a_name_survives() {
        let mut queue = Notices::default();
        let id = queue.push(
            "No se pudo mover «un\tnombre»",
            "circle-alert",
            NoticeTone::Danger,
            false,
        );
        let rows = queue.rows();
        assert_eq!(rows.len(), 1);
        let mut parts = rows[0].splitn(5, '\t');
        assert_eq!(parts.next(), Some(id.to_string().as_str()));
        assert_eq!(parts.next(), Some("circle-alert"));
        assert_eq!(parts.next(), Some("danger"));
        assert_eq!(parts.next(), Some("0"));
        assert_eq!(parts.next(), Some("No se pudo mover «un\tnombre»"));
    }

    #[test]
    fn a_danger_notice_keeps_its_tone_through_the_queue() {
        let mut queue = Notices::default();
        queue.push(
            "No se pudo leer la carpeta",
            "circle-alert",
            NoticeTone::Danger,
            false,
        );
        assert_eq!(queue.entries[0].tone, NoticeTone::Danger);
    }
}
