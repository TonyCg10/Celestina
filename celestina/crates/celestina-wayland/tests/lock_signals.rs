//! The lock profile does not stop on `SIGINT`, `SIGTERM`, `SIGHUP`,
//! `SIGQUIT`, `SIGABRT`, `SIGALRM`, `SIGUSR1` or `SIGUSR2`
//! (CEL-2-H design §4.13, F27): it logs them. A plain `main`, not libtest:
//! the loop blocks the signals in the thread that creates the signal source,
//! and libtest runs tests on other threads, where a delivered `SIGTERM` would
//! terminate the test process instead of reaching the source.

use std::{cell::RefCell, rc::Rc, time::Duration};

use calloop::{signals::Signal, EventLoop};
use celestina_wayland::lock::{insert_signal_policy, IGNORED_SIGNALS};
use rustix::process::{getpid, kill_process, Signal as Posix};

fn posix(signal: Signal) -> Posix {
    match signal {
        Signal::SIGINT => Posix::INT,
        Signal::SIGTERM => Posix::TERM,
        Signal::SIGHUP => Posix::HUP,
        Signal::SIGQUIT => Posix::QUIT,
        Signal::SIGABRT => Posix::ABORT,
        Signal::SIGALRM => Posix::ALARM,
        Signal::SIGUSR1 => Posix::USR1,
        Signal::SIGUSR2 => Posix::USR2,
        other => panic!("the policy covers {other:?}, which this test does not send"),
    }
}

fn main() {
    assert_eq!(
        IGNORED_SIGNALS,
        &[
            Signal::SIGINT,
            Signal::SIGTERM,
            Signal::SIGHUP,
            Signal::SIGQUIT,
            Signal::SIGABRT,
            Signal::SIGALRM,
            Signal::SIGUSR1,
            Signal::SIGUSR2,
        ],
        "the policy names every catchable signal whose default action ends the process"
    );
    let mut event_loop: EventLoop<'static, ()> = EventLoop::try_new().expect("an event loop");
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&seen);
    insert_signal_policy(&event_loop.handle(), move |signal| {
        sink.borrow_mut().push(signal)
    })
    .expect("the policy installs");

    for signal in IGNORED_SIGNALS {
        kill_process(getpid(), posix(*signal)).expect("a signal to ourselves");
    }
    for _ in 0..20 {
        event_loop
            .dispatch(Some(Duration::from_millis(100)), &mut ())
            .expect("the loop keeps running");
        if seen.borrow().len() == IGNORED_SIGNALS.len() {
            break;
        }
    }
    // Reaching this line is the first half of the test: a default-action
    // `SIGTERM` or `SIGHUP` would have ended the process above.
    let mut got = seen.borrow().clone();
    got.sort_by_key(|s| *s as i32);
    let mut want = IGNORED_SIGNALS.to_vec();
    want.sort_by_key(|s| *s as i32);
    assert_eq!(
        got, want,
        "each signal was seen and none stopped the process"
    );
    println!("test lock_profile_does_not_stop_on_the_ignored_signals ... ok");
}
