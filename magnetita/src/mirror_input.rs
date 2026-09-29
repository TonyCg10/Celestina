//! The mirror window's input on its way to the daemon: one queue the GUI
//! thread fills and one worker thread that empties it over one session-bus
//! connection, opened once and opened again only after a failure.
//!
//! A drag produces a move per pointer event, faster than a bus round trip
//! when the daemon is busy. The queue keeps at most one pending move: a new
//! move replaces a move still waiting at its tail, so the phone always gets
//! the latest position and never a backlog of stale ones. Presses, releases,
//! keys and navigation are never merged; the queue is bounded, and past the
//! bound new input is dropped rather than grown, except a release and the
//! stop: a dropped touch-up or key-up would leave the phone pressed, so
//! they may use a reserve past the bound.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

/// The most operations that may wait for the worker.
const QUEUE: usize = 256;
/// Room past [`QUEUE`] kept for releases and the stop.
const RELEASE_RESERVE: usize = 64;
/// How many times a release is tried, each on a fresh connection after a
/// failure, and how long between tries.
const RELEASE_ATTEMPTS: u32 = 3;
const RELEASE_RETRY: std::time::Duration = std::time::Duration::from_millis(100);

/// The wire's touch phases for a move and a lift.
const MOVE: u8 = 1;
const UP: u8 = 2;

/// What the input worker sends the daemon, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outbound {
    Touch { action: u8, x: u16, y: u16 },
    Key { keycode: u16, pressed: bool },
    Global(String),
    Stop,
}

impl Outbound {
    fn is_move(&self) -> bool {
        matches!(self, Outbound::Touch { action: MOVE, .. })
    }

    /// A touch-up, a key-up or the stop: never worth dropping.
    fn releases(&self) -> bool {
        matches!(
            self,
            Outbound::Touch { action: UP, .. }
                | Outbound::Key { pressed: false, .. }
                | Outbound::Stop
        )
    }
}

/// Where the worker sends: `Mirror1` in the app, a recorder in the tests.
pub trait MirrorSink {
    fn send(&mut self, op: &Outbound) -> Result<(), String>;
}

#[derive(Default)]
struct State {
    ops: VecDeque<Outbound>,
    closed: bool,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    ready: Condvar,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The GUI thread's end of the queue. Closing it lets the worker send what
/// is queued and end.
pub struct InputQueue {
    shared: Arc<Shared>,
}

impl InputQueue {
    /// Queues `op`, merging a move into a move still waiting; false when the
    /// queue is full and `op` was dropped.
    pub fn push(&self, op: Outbound) -> bool {
        let mut state = self.shared.lock();
        if state.closed {
            return false;
        }
        if op.is_move() && state.ops.back().is_some_and(Outbound::is_move) {
            if let Some(last) = state.ops.back_mut() {
                *last = op;
            }
        } else if state.ops.len() >= QUEUE
            && !(op.releases() && state.ops.len() < QUEUE + RELEASE_RESERVE)
        {
            return false;
        } else {
            state.ops.push_back(op);
        }
        self.shared.ready.notify_one();
        true
    }

    pub fn close(&self) {
        self.shared.lock().closed = true;
        self.shared.ready.notify_one();
    }
}

impl Drop for InputQueue {
    fn drop(&mut self) {
        self.close();
    }
}

/// Starts the worker. `open` makes the sink: called before the first
/// operation and again after a send fails, never per operation.
pub fn spawn<S, F>(mut open: F) -> std::io::Result<(InputQueue, JoinHandle<()>)>
where
    S: MirrorSink,
    F: FnMut() -> Result<S, String> + Send + 'static,
{
    let shared = Arc::new(Shared::default());
    let worker = Arc::clone(&shared);
    let join = std::thread::Builder::new()
        .name("magnetita-mirror-input".into())
        .spawn(move || {
            let mut sink: Option<S> = None;
            while let Some(op) = next(&worker) {
                // A press or a move lost with the bus is past; a release is
                // tried again on a fresh connection, a few times, so the
                // phone is not left pressed by one bad moment of the bus.
                let attempts = if op.releases() { RELEASE_ATTEMPTS } else { 1 };
                for attempt in 1..=attempts {
                    if attempt > 1 {
                        std::thread::sleep(RELEASE_RETRY);
                    }
                    if sink.is_none() {
                        match open() {
                            Ok(opened) => sink = Some(opened),
                            Err(error) => {
                                eprintln!("magnetita: mirror input: {error}");
                                continue;
                            }
                        }
                    }
                    let Some(connected) = sink.as_mut() else {
                        continue;
                    };
                    match connected.send(&op) {
                        Ok(()) => break,
                        Err(error) => {
                            eprintln!("magnetita: mirror input: {error}");
                            sink = None;
                        }
                    }
                }
            }
        })?;
    Ok((InputQueue { shared }, join))
}

/// The next operation, waiting for one; `None` once closed and drained.
fn next(shared: &Shared) -> Option<Outbound> {
    let mut state = shared.lock();
    loop {
        if let Some(op) = state.ops.pop_front() {
            return Some(op);
        }
        if state.closed {
            return None;
        }
        state = shared.ready.wait(state).unwrap_or_else(|e| e.into_inner());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn touch(action: u8, x: u16) -> Outbound {
        Outbound::Touch { action, x, y: 0 }
    }

    #[test]
    fn a_waiting_move_is_replaced_and_nothing_else_is_merged() {
        let queue = InputQueue {
            shared: Arc::new(Shared::default()),
        };
        assert!(queue.push(touch(0, 1)));
        for x in 2..50 {
            assert!(queue.push(touch(MOVE, x)));
        }
        assert!(queue.push(touch(2, 50)));
        assert!(queue.push(touch(MOVE, 51)));
        assert!(queue.push(Outbound::Key {
            keycode: 4,
            pressed: true
        }));
        let ops: Vec<_> = queue.shared.lock().ops.drain(..).collect();
        assert_eq!(
            ops,
            [
                touch(0, 1),
                touch(MOVE, 49),
                touch(2, 50),
                touch(MOVE, 51),
                Outbound::Key {
                    keycode: 4,
                    pressed: true
                }
            ]
        );
    }

    #[test]
    fn the_queue_is_bounded() {
        let queue = InputQueue {
            shared: Arc::new(Shared::default()),
        };
        for n in 0..QUEUE {
            assert!(queue.push(Outbound::Key {
                keycode: n as u16,
                pressed: true
            }));
        }
        assert!(!queue.push(Outbound::Key {
            keycode: 1,
            pressed: true
        }));
        assert!(!queue.push(touch(0, 1)), "a new press is dropped");
        assert!(queue.push(touch(UP, 1)), "a lift is never dropped");
        assert!(queue.push(Outbound::Key {
            keycode: 1,
            pressed: false
        }));
        assert!(queue.push(Outbound::Stop));
        assert_eq!(queue.shared.lock().ops.len(), QUEUE + 3);
    }

    struct Recorder {
        sent: mpsc::Sender<Outbound>,
        fail_first: bool,
    }

    impl MirrorSink for Recorder {
        fn send(&mut self, op: &Outbound) -> Result<(), String> {
            if std::mem::take(&mut self.fail_first) {
                return Err("bus gone".into());
            }
            self.sent.send(op.clone()).map_err(|e| e.to_string())
        }
    }

    /// A release that meets a failing bus is sent again on a new connection.
    #[test]
    fn a_release_survives_one_failed_send() {
        let (sent, got) = mpsc::channel();
        let opened = Arc::new(Mutex::new(0));
        let count = Arc::clone(&opened);
        let (queue, join) = spawn(move || {
            let mut opened = count.lock().unwrap();
            *opened += 1;
            Ok(Recorder {
                sent: sent.clone(),
                fail_first: *opened == 1,
            })
        })
        .unwrap();
        queue.push(touch(UP, 5));
        drop(queue);
        join.join().unwrap();
        assert_eq!(got.try_iter().collect::<Vec<_>>(), [touch(UP, 5)]);
        assert_eq!(*opened.lock().unwrap(), 2);
    }

    /// One connection for every operation, opened again only after a send
    /// failed, and everything queued is sent before the worker ends.
    #[test]
    fn one_connection_carries_the_input_and_a_failure_reopens_it() {
        let (sent, got) = mpsc::channel();
        let opened = Arc::new(Mutex::new(0));
        let count = Arc::clone(&opened);
        let (queue, join) = spawn(move || {
            let mut opened = count.lock().unwrap();
            *opened += 1;
            Ok(Recorder {
                sent: sent.clone(),
                fail_first: *opened == 1,
            })
        })
        .unwrap();
        for keycode in 0..10 {
            queue.push(Outbound::Key {
                keycode,
                pressed: true,
            });
        }
        queue.push(Outbound::Stop);
        drop(queue);
        join.join().unwrap();
        let received: Vec<_> = got.try_iter().collect();
        assert_eq!(received.len(), 10, "the first key was lost with the bus");
        assert_eq!(received.last(), Some(&Outbound::Stop));
        assert_eq!(*opened.lock().unwrap(), 2);
    }
}
