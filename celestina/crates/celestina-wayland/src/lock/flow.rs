//! The lock profile's bookkeeping, pure: which outputs exist, which of them
//! has a lock surface, when the cover is committed and what the product is
//! told, in what order. The runtime performs the protocol work through
//! [`Effects`]; this module decides it, so every rule below is tested without
//! a compositor.
//!
//! The rules (design §4.6):
//!
//! * a lock surface exists for an output as soon as a lock is requested, and
//!   for an output that appears later at once, locked or not, and **exactly
//!   one** per `wl_output` (a second would be a `duplicate_output` protocol
//!   error);
//! * the cover is committed on every `configure`, before the product hears
//!   of it, whether `locked` has arrived or not (on a replacement niri sends
//!   `locked` before the new surface's `configure`);
//! * a `configure` after `finished`, for an unknown output or of size zero is
//!   tolerated and does nothing (a refused client still receives one);
//! * `locked` is announced once.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use crate::output::{OutputId, OutputInfo};

/// What the flow tells the product, in the order it must hear it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Emit {
    /// The compositor confirmed the lock (the capability is minted by the
    /// runtime, only for this event).
    Locked,
    Finished,
    SurfaceConfigured {
        output: OutputId,
        size: (u32, u32),
    },
    OutputAdded(OutputInfo),
    OutputRemoved(OutputId),
}

/// The protocol work the flow asks for. `Handle` is the lock surface; dropping
/// it destroys it.
pub(crate) trait Effects {
    type Handle;
    /// Creates the lock surface of `output`; `None` when it cannot be created
    /// (the output is gone, or the lock global is).
    fn create_surface(&mut self, output: OutputId) -> Option<Self::Handle>;
    /// Attaches and commits the black cover of `size` on `surface`.
    fn commit_cover(&mut self, surface: &Self::Handle, size: (u32, u32));
    fn emit(&mut self, event: Emit);
}

/// Where the lock request stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    /// No lock was requested yet; outputs are only recorded.
    Idle,
    /// `lock()` was sent; `locked` or `finished` has not arrived.
    Requested,
    Locked,
    /// The compositor sent `finished`: no more surfaces, no more events.
    Finished,
}

struct Slot<H> {
    surface: Option<H>,
}

pub(crate) struct Flow<H> {
    phase: Phase,
    slots: BTreeMap<OutputId, Slot<H>>,
}

impl<H> Flow<H> {
    pub(crate) fn new() -> Self {
        Self {
            phase: Phase::Idle,
            slots: BTreeMap::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn phase(&self) -> Phase {
        self.phase
    }

    /// Outputs that currently own a lock surface, in id order.
    #[cfg(test)]
    pub(crate) fn covered(&self) -> impl Iterator<Item = OutputId> + '_ {
        self.slots
            .iter()
            .filter(|(_, slot)| slot.surface.is_some())
            .map(|(id, _)| *id)
    }

    /// The output whose lock surface satisfies `is`.
    pub(crate) fn output_where(&self, is: impl Fn(&H) -> bool) -> Option<OutputId> {
        self.slots
            .iter()
            .find(|(_, slot)| slot.surface.as_ref().is_some_and(&is))
            .map(|(id, _)| *id)
    }

    fn wants_surfaces(&self) -> bool {
        matches!(self.phase, Phase::Requested | Phase::Locked)
    }

    /// An output appeared (or is described again): recorded, announced the
    /// first time, and given its lock surface when a lock is on the way.
    pub(crate) fn output_added<E: Effects<Handle = H>>(&mut self, fx: &mut E, info: OutputInfo) {
        let id = info.id;
        let known = self.slots.contains_key(&id);
        let slot = self.slots.entry(id).or_insert(Slot { surface: None });
        if slot.surface.is_none() && matches!(self.phase, Phase::Requested | Phase::Locked) {
            slot.surface = fx.create_surface(id);
        }
        if !known && self.phase != Phase::Finished {
            fx.emit(Emit::OutputAdded(info));
        }
    }

    /// An output went away: its surface is destroyed (dropped) and the product
    /// told, once.
    pub(crate) fn output_removed<E: Effects<Handle = H>>(&mut self, fx: &mut E, id: OutputId) {
        if self.slots.remove(&id).is_some() && self.phase != Phase::Finished {
            fx.emit(Emit::OutputRemoved(id));
        }
    }

    /// `lock()` was sent: every known output gets its surface.
    pub(crate) fn requested<E: Effects<Handle = H>>(&mut self, fx: &mut E) {
        if self.phase != Phase::Idle {
            return;
        }
        self.phase = Phase::Requested;
        for (id, slot) in &mut self.slots {
            if slot.surface.is_none() {
                slot.surface = fx.create_surface(*id);
            }
        }
    }

    /// The compositor sent `locked`; announced once.
    pub(crate) fn locked<E: Effects<Handle = H>>(&mut self, fx: &mut E) {
        if self.phase != Phase::Requested {
            return;
        }
        self.phase = Phase::Locked;
        fx.emit(Emit::Locked);
    }

    /// The compositor sent `finished`: the surfaces are destroyed and the
    /// product told, once. Surfaces that still receive a `configure` are
    /// ignored from here on.
    pub(crate) fn finished<E: Effects<Handle = H>>(&mut self, fx: &mut E) {
        if self.phase == Phase::Finished {
            return;
        }
        self.phase = Phase::Finished;
        for slot in self.slots.values_mut() {
            slot.surface = None;
        }
        fx.emit(Emit::Finished);
    }

    /// The compositor configured `id`'s lock surface at `size`: the cover is
    /// committed first, the product is told after.
    pub(crate) fn configured<E: Effects<Handle = H>>(
        &mut self,
        fx: &mut E,
        id: OutputId,
        size: (u32, u32),
    ) {
        if !self.wants_surfaces() || size.0 == 0 || size.1 == 0 {
            return;
        }
        let Some(surface) = self.slots.get(&id).and_then(|slot| slot.surface.as_ref()) else {
            return;
        };
        fx.commit_cover(surface, size);
        fx.emit(Emit::SurfaceConfigured { output: id, size });
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    enum Step {
        Create(OutputId),
        Destroyed(OutputId),
        Cover(OutputId, (u32, u32)),
        Emit(Emit),
    }

    struct Handle {
        output: OutputId,
        log: Rc<RefCell<Vec<Step>>>,
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            self.log.borrow_mut().push(Step::Destroyed(self.output));
        }
    }

    #[derive(Default)]
    struct Fake {
        log: Rc<RefCell<Vec<Step>>>,
        /// Outputs whose surface cannot be created.
        refuse: Vec<OutputId>,
    }

    impl Effects for Fake {
        type Handle = Handle;

        fn create_surface(&mut self, output: OutputId) -> Option<Handle> {
            if self.refuse.contains(&output) {
                return None;
            }
            self.log.borrow_mut().push(Step::Create(output));
            Some(Handle {
                output,
                log: self.log.clone(),
            })
        }

        fn commit_cover(&mut self, surface: &Handle, size: (u32, u32)) {
            self.log
                .borrow_mut()
                .push(Step::Cover(surface.output, size));
        }

        fn emit(&mut self, event: Emit) {
            self.log.borrow_mut().push(Step::Emit(event));
        }
    }

    impl Fake {
        fn steps(&self) -> Vec<Step> {
            self.log.borrow().clone()
        }

        fn take(&self) -> Vec<Step> {
            std::mem::take(&mut *self.log.borrow_mut())
        }

        fn created(&self, output: OutputId) -> usize {
            self.steps()
                .iter()
                .filter(|s| **s == Step::Create(output))
                .count()
        }
    }

    fn out(id: u32) -> OutputId {
        OutputId(id)
    }

    fn info(id: u32) -> OutputInfo {
        OutputInfo {
            id: out(id),
            name: format!("DP-{id}"),
            logical_size: (1920, 1080),
            scale: 1.0,
            refresh_mhz: 60_000,
        }
    }

    fn started(outputs: &[u32]) -> (Flow<Handle>, Fake) {
        let mut flow = Flow::new();
        let mut fx = Fake::default();
        for id in outputs {
            flow.output_added(&mut fx, info(*id));
        }
        flow.requested(&mut fx);
        fx.take();
        (flow, fx)
    }

    #[test]
    fn t_w1_first_commit_of_a_lock_surface_is_the_cover() {
        let (mut flow, mut fx) = started(&[1]);
        flow.configured(&mut fx, out(1), (1920, 1080));
        assert_eq!(
            fx.steps(),
            vec![
                Step::Cover(out(1), (1920, 1080)),
                Step::Emit(Emit::SurfaceConfigured {
                    output: out(1),
                    size: (1920, 1080)
                }),
            ]
        );
    }

    #[test]
    fn the_cover_is_committed_on_every_configure_before_and_after_locked() {
        let (mut flow, mut fx) = started(&[1]);
        flow.configured(&mut fx, out(1), (800, 600));
        flow.locked(&mut fx);
        flow.configured(&mut fx, out(1), (1024, 768));
        let covers: Vec<_> = fx
            .steps()
            .into_iter()
            .filter(|s| matches!(s, Step::Cover(..)))
            .collect();
        assert_eq!(
            covers,
            vec![
                Step::Cover(out(1), (800, 600)),
                Step::Cover(out(1), (1024, 768))
            ]
        );
    }

    #[test]
    fn a_configure_that_arrives_after_locked_is_covered_q1_replacement() {
        // On a replacement niri sends `locked` first and the new surface's
        // `configure` after it.
        let (mut flow, mut fx) = started(&[1]);
        flow.locked(&mut fx);
        flow.configured(&mut fx, out(1), (3840, 2160));
        assert_eq!(
            fx.take(),
            vec![
                Step::Emit(Emit::Locked),
                Step::Cover(out(1), (3840, 2160)),
                Step::Emit(Emit::SurfaceConfigured {
                    output: out(1),
                    size: (3840, 2160)
                }),
            ]
        );
    }

    #[test]
    fn a_configure_after_finished_is_ignored_without_error_q1_refused_client() {
        let (mut flow, mut fx) = started(&[1]);
        flow.finished(&mut fx);
        fx.take();
        flow.configured(&mut fx, out(1), (1920, 1080));
        assert_eq!(fx.steps(), vec![], "no cover, no event after finished");
        assert_eq!(flow.phase(), Phase::Finished);
    }

    #[test]
    fn a_configure_for_an_unknown_output_or_of_size_zero_does_nothing() {
        let (mut flow, mut fx) = started(&[1]);
        flow.configured(&mut fx, out(9), (100, 100));
        flow.configured(&mut fx, out(1), (0, 100));
        flow.configured(&mut fx, out(1), (100, 0));
        assert_eq!(fx.steps(), vec![]);
    }

    #[test]
    fn outputs_known_before_the_request_get_exactly_one_surface_at_the_request() {
        let mut flow = Flow::new();
        let mut fx = Fake::default();
        flow.output_added(&mut fx, info(1));
        flow.output_added(&mut fx, info(2));
        assert_eq!(fx.created(out(1)) + fx.created(out(2)), 0, "no lock yet");
        flow.requested(&mut fx);
        flow.requested(&mut fx);
        assert_eq!((fx.created(out(1)), fx.created(out(2))), (1, 1));
        assert_eq!(flow.covered().collect::<Vec<_>>(), vec![out(1), out(2)]);
    }

    #[test]
    fn an_output_added_before_and_after_locked_gets_exactly_one_surface() {
        let (mut flow, mut fx) = started(&[1]);
        flow.output_added(&mut fx, info(2));
        flow.locked(&mut fx);
        flow.output_added(&mut fx, info(3));
        // The compositor describing an output again must not create a second
        // surface (`duplicate_output`).
        flow.output_added(&mut fx, info(2));
        flow.output_added(&mut fx, info(3));
        assert_eq!(
            (fx.created(out(2)), fx.created(out(3))),
            (1, 1),
            "one surface per wl_output"
        );
        assert_eq!(
            flow.covered().collect::<Vec<_>>(),
            vec![out(1), out(2), out(3)]
        );
    }

    #[test]
    fn an_output_is_announced_once() {
        let (mut flow, mut fx) = started(&[]);
        flow.output_added(&mut fx, info(1));
        flow.output_added(&mut fx, info(1));
        let announced = fx
            .steps()
            .iter()
            .filter(|s| matches!(s, Step::Emit(Emit::OutputAdded(_))))
            .count();
        assert_eq!(announced, 1);
    }

    #[test]
    fn a_removed_output_destroys_its_surface() {
        let (mut flow, mut fx) = started(&[1, 2]);
        flow.locked(&mut fx);
        fx.take();
        flow.output_removed(&mut fx, out(2));
        assert_eq!(
            fx.take(),
            vec![
                Step::Destroyed(out(2)),
                Step::Emit(Emit::OutputRemoved(out(2)))
            ]
        );
        assert_eq!(flow.covered().collect::<Vec<_>>(), vec![out(1)]);
        flow.output_removed(&mut fx, out(2));
        assert_eq!(fx.steps(), vec![], "removing it again says nothing");
    }

    #[test]
    fn the_output_comes_back_with_a_new_surface() {
        let (mut flow, mut fx) = started(&[1]);
        flow.locked(&mut fx);
        flow.output_removed(&mut fx, out(1));
        flow.output_added(&mut fx, info(1));
        // `started` cleared the log after the first surface: this is the
        // second one, and the first was destroyed.
        assert_eq!(fx.created(out(1)), 1);
        assert!(fx.steps().contains(&Step::Destroyed(out(1))));
        assert_eq!(flow.covered().count(), 1);
    }

    #[test]
    fn zero_outputs_at_locked_then_one_appears_and_is_covered_f39() {
        let (mut flow, mut fx) = started(&[]);
        flow.locked(&mut fx);
        assert_eq!(fx.take(), vec![Step::Emit(Emit::Locked)]);
        flow.output_added(&mut fx, info(5));
        flow.configured(&mut fx, out(5), (1280, 720));
        assert_eq!(
            fx.take(),
            vec![
                Step::Create(out(5)),
                Step::Emit(Emit::OutputAdded(info(5))),
                Step::Cover(out(5), (1280, 720)),
                Step::Emit(Emit::SurfaceConfigured {
                    output: out(5),
                    size: (1280, 720)
                }),
            ]
        );
    }

    #[test]
    fn locked_is_announced_once_and_only_after_a_request() {
        let mut flow: Flow<Handle> = Flow::new();
        let mut fx = Fake::default();
        flow.locked(&mut fx);
        assert_eq!(fx.steps(), vec![], "locked without a request is ignored");
        flow.requested(&mut fx);
        flow.locked(&mut fx);
        flow.locked(&mut fx);
        let locked = fx
            .steps()
            .iter()
            .filter(|s| **s == Step::Emit(Emit::Locked))
            .count();
        assert_eq!(locked, 1);
        flow.finished(&mut fx);
        flow.locked(&mut fx);
        let locked = fx
            .steps()
            .iter()
            .filter(|s| **s == Step::Emit(Emit::Locked))
            .count();
        assert_eq!(locked, 1, "locked after finished is ignored");
    }

    #[test]
    fn finished_destroys_every_surface_announces_once_and_stops_surfaces() {
        let (mut flow, mut fx) = started(&[1, 2]);
        flow.finished(&mut fx);
        flow.finished(&mut fx);
        assert_eq!(
            fx.take(),
            vec![
                Step::Destroyed(out(1)),
                Step::Destroyed(out(2)),
                Step::Emit(Emit::Finished)
            ]
        );
        flow.output_added(&mut fx, info(3));
        assert_eq!(fx.created(out(3)), 0, "no surface on a finished lock");
        assert_eq!(flow.covered().count(), 0);
    }

    #[test]
    fn a_surface_that_cannot_be_created_is_retried_when_the_output_is_described_again() {
        let mut flow = Flow::new();
        let mut fx = Fake {
            refuse: vec![out(1)],
            ..Fake::default()
        };
        flow.output_added(&mut fx, info(1));
        flow.requested(&mut fx);
        assert_eq!(flow.covered().count(), 0);
        fx.refuse.clear();
        flow.output_added(&mut fx, info(1));
        assert_eq!(flow.covered().collect::<Vec<_>>(), vec![out(1)]);
    }

    #[test]
    fn output_where_finds_the_output_of_a_surface() {
        let (flow, _fx) = started(&[1, 2]);
        assert_eq!(flow.output_where(|h| h.output == out(2)), Some(out(2)));
        assert_eq!(flow.output_where(|h| h.output == out(7)), None);
    }

    /// A seeded xorshift, so the property test is reproducible.
    struct Xorshift(u64);

    impl Xorshift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    #[test]
    fn random_sequences_keep_one_live_surface_per_output_and_announce_locked_once() {
        for seed in 1..=200u64 {
            let mut rng = Xorshift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let mut flow: Flow<Handle> = Flow::new();
            let mut fx = Fake::default();
            for _ in 0..60 {
                let id = (rng.next() % 4) as u32 + 1;
                match rng.next() % 7 {
                    0 | 1 => flow.output_added(&mut fx, info(id)),
                    2 => flow.output_removed(&mut fx, out(id)),
                    3 => flow.requested(&mut fx),
                    4 => flow.locked(&mut fx),
                    5 => flow.configured(&mut fx, out(id), (640, 480)),
                    _ => {
                        if rng.next().is_multiple_of(11) {
                            flow.finished(&mut fx);
                        }
                    }
                }
                let steps = fx.steps();
                for output in 1..=4u32 {
                    let created = steps
                        .iter()
                        .filter(|s| **s == Step::Create(out(output)))
                        .count();
                    let destroyed = steps
                        .iter()
                        .filter(|s| **s == Step::Destroyed(out(output)))
                        .count();
                    let live = created - destroyed;
                    assert!(
                        live <= 1,
                        "seed {seed}: two live surfaces for output {output}"
                    );
                    assert_eq!(
                        live == 1,
                        flow.covered().any(|c| c == out(output)),
                        "seed {seed}: the flow and the protocol disagree on output {output}"
                    );
                }
                let locked = steps
                    .iter()
                    .filter(|s| **s == Step::Emit(Emit::Locked))
                    .count();
                assert!(locked <= 1, "seed {seed}: locked announced twice");
                for step in &steps {
                    if let Step::Cover(output, _) = step {
                        assert!(
                            steps.contains(&Step::Create(*output)),
                            "seed {seed}: a cover without a surface"
                        );
                    }
                }
            }
        }
    }
}
