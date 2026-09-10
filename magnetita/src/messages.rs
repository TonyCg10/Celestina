//! The phone's SMS on the desktop: the conversation list, one open thread
//! and a send, all read from and written through `Devices1`. The daemon
//! holds the messages; this model only projects the page it is shown.

use core::pin::Pin;
use std::sync::mpsc::{sync_channel, SyncSender};
use std::thread::JoinHandle;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        /// The device whose messages are shown; set by the page.
        #[qproperty(QString, device_id)]
        #[qproperty(QStringList, conversation_threads)]
        #[qproperty(QStringList, conversation_labels)]
        #[qproperty(QStringList, conversation_snippets)]
        #[qproperty(QStringList, conversation_unread)]
        /// The open thread as text, empty on the list.
        #[qproperty(QString, open_thread)]
        #[qproperty(QString, open_label)]
        #[qproperty(QStringList, message_bodies)]
        #[qproperty(QStringList, message_from_me)]
        #[qproperty(QStringList, message_names)]
        #[qproperty(bool, available)]
        type MessagesModel = super::MessagesModelRust;

        /// Re-read the conversations (and the open thread) from the daemon.
        #[qinvokable]
        fn refresh(self: Pin<&mut MessagesModel>);

        /// Show one conversation.
        #[qinvokable]
        fn open_conversation(self: Pin<&mut MessagesModel>, thread: QString, label: QString);

        /// Back to the list.
        #[qinvokable]
        fn close_conversation(self: Pin<&mut MessagesModel>);

        /// Send `body` in the open conversation.
        #[qinvokable]
        fn send(self: Pin<&mut MessagesModel>, body: QString);
    }

    impl cxx_qt::Threading for MessagesModel {}
}

#[derive(Default)]
pub struct MessagesModelRust {
    device_id: QString,
    conversation_threads: QStringList,
    conversation_labels: QStringList,
    conversation_snippets: QStringList,
    conversation_unread: QStringList,
    open_thread: QString,
    open_label: QString,
    message_bodies: QStringList,
    message_from_me: QStringList,
    message_names: QStringList,
    available: bool,
    sender: Option<SyncSender<(String, u64, String)>>,
    worker: Option<JoinHandle<()>>,
    /// The refresh threads, joined on drop; a late snapshot is dropped.
    owned: crate::lifecycle::Owned,
}

impl Drop for MessagesModelRust {
    fn drop(&mut self) {
        self.owned.close();
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Snapshot {
    conversations: Vec<crate::devices::Conversation>,
    thread: Option<(u64, Vec<crate::devices::Message>)>,
}

impl qobject::MessagesModel {
    pub fn refresh(mut self: Pin<&mut Self>) {
        let device = self.rust().device_id.to_string();
        if device.is_empty() {
            return;
        }
        let open = self.rust().open_thread.to_string().parse::<u64>().ok();
        let qt = self.as_mut().qt_thread();
        self.rust().owned.spawn(move |guard| {
            let conversations = crate::devices::sms_conversations(&device);
            let thread = open.map(|t| {
                (
                    t,
                    crate::devices::sms_thread(&device, t).unwrap_or_default(),
                )
            });
            let snapshot = conversations.map(|conversations| Snapshot {
                conversations,
                thread,
            });
            if !guard.open() {
                return;
            }
            let _ = qt.queue(
                move |mut model: Pin<&mut qobject::MessagesModel>| match snapshot {
                    Ok(snapshot) => model.as_mut().apply(snapshot),
                    Err(error) => {
                        eprintln!("magnetita: messages unavailable: {error}");
                        model.as_mut().set_available(false);
                    }
                },
            );
        });
    }

    fn apply(mut self: Pin<&mut Self>, snapshot: Snapshot) {
        let threads: QStringList = snapshot
            .conversations
            .iter()
            .map(|c| QString::from(c.thread.to_string().as_str()))
            .collect();
        let labels: QStringList = snapshot
            .conversations
            .iter()
            .map(|c| QString::from(c.label.as_str()))
            .collect();
        let snippets: QStringList = snapshot
            .conversations
            .iter()
            .map(|c| QString::from(c.snippet.as_str()))
            .collect();
        let unread: QStringList = snapshot
            .conversations
            .iter()
            .map(|c| QString::from(c.unread.to_string().as_str()))
            .collect();
        self.as_mut().set_conversation_threads(threads);
        self.as_mut().set_conversation_labels(labels);
        self.as_mut().set_conversation_snippets(snippets);
        self.as_mut().set_conversation_unread(unread);
        if let Some((thread, messages)) = snapshot.thread {
            if self.rust().open_thread.to_string() == thread.to_string() {
                let bodies: QStringList = messages
                    .iter()
                    .map(|m| QString::from(m.body.as_str()))
                    .collect();
                let from_me: QStringList = messages
                    .iter()
                    .map(|m| QString::from(if m.from_me { "true" } else { "false" }))
                    .collect();
                let names: QStringList = messages
                    .iter()
                    .map(|m| QString::from(m.name.as_str()))
                    .collect();
                self.as_mut().set_message_bodies(bodies);
                self.as_mut().set_message_from_me(from_me);
                self.as_mut().set_message_names(names);
            }
        }
        self.as_mut().set_available(true);
    }

    pub fn open_conversation(mut self: Pin<&mut Self>, thread: QString, label: QString) {
        self.as_mut().set_open_thread(thread);
        self.as_mut().set_open_label(label);
        self.as_mut().set_message_bodies(QStringList::default());
        self.as_mut().set_message_from_me(QStringList::default());
        self.as_mut().set_message_names(QStringList::default());
        self.refresh();
    }

    pub fn close_conversation(mut self: Pin<&mut Self>) {
        self.as_mut().set_open_thread(QString::default());
        self.as_mut().set_open_label(QString::default());
    }

    /// Sends through one worker so the order of sends is the order typed.
    pub fn send(mut self: Pin<&mut Self>, body: QString) {
        let device = self.rust().device_id.to_string();
        let Ok(thread) = self.rust().open_thread.to_string().parse::<u64>() else {
            return;
        };
        let body = body.to_string();
        if body.trim().is_empty() {
            return;
        }
        if self.rust().sender.is_none() {
            let (sender, receiver) = sync_channel::<(String, u64, String)>(16);
            let worker = std::thread::spawn(move || {
                while let Ok((device, thread, body)) = receiver.recv() {
                    if let Err(error) = crate::devices::sms_send(&device, thread, &body) {
                        eprintln!("magnetita: sms send failed: {error}");
                    }
                }
            });
            let state = self.as_mut().rust_mut().get_mut();
            state.sender = Some(sender);
            state.worker = Some(worker);
        }
        if let Some(sender) = &self.rust().sender {
            let _ = sender.try_send((device, thread, body));
        }
    }
}
