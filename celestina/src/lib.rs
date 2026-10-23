//! The Celestina shell as a library: the app, its Islands and the launcher,
//! the command surface and its bus, the provider workers and the messages
//! they publish into the event loop. The binary (`src/main.rs`) parses the
//! command line and runs it.

#![forbid(unsafe_code)]

pub mod app;
pub mod bar;
pub mod bubbles;
pub mod bus;
pub mod clipboard;
pub mod command;
pub mod connectivity;
pub mod copy;
pub mod energy;
pub mod glance;
pub mod island;
pub mod launcher;
pub mod lock;
pub mod media;
pub mod message;
pub mod metrics;
pub mod notifications;
pub mod osd;
pub mod phone;
pub mod pieces;
pub mod providers;
pub mod tray;
pub mod wallpaper;

/// `err` and its sources, outermost first, joined by `: `: how the shell
/// reports a failure on stderr and in the journal.
#[must_use]
pub fn error_chain(err: &dyn std::error::Error) -> String {
    let mut text = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

#[cfg(test)]
mod tests {
    #[derive(Debug, thiserror::Error)]
    #[error("cannot create the painter")]
    struct Outer(#[source] std::io::Error);

    #[test]
    fn an_error_chain_names_every_cause_outermost_first() {
        let err = Outer(std::io::Error::other("no GL renderer"));
        assert_eq!(
            super::error_chain(&err),
            "cannot create the painter: no GL renderer"
        );
    }
}
