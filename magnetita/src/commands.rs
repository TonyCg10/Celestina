//! The author's registered commands on the settings page: name, program
//! and arguments, kept by the daemon and shown to the phone as names.
//! Arguments are typed as one line and split on spaces; a program that
//! needs more is a script.

use core::pin::Pin;

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
        #[qproperty(QStringList, command_ids)]
        #[qproperty(QStringList, command_names)]
        #[qproperty(QStringList, command_lines)]
        #[qproperty(bool, available)]
        type CommandsModel = super::CommandsModelRust;

        /// Re-read the registered commands from the daemon.
        #[qinvokable]
        fn refresh(self: Pin<&mut CommandsModel>);

        /// Register `name` running `program` with `args` split on spaces.
        #[qinvokable]
        fn add(self: Pin<&mut CommandsModel>, name: QString, program: QString, args: QString);

        /// Remove the command at `index`.
        #[qinvokable]
        fn remove(self: Pin<&mut CommandsModel>, index: i32);
    }

    impl cxx_qt::Threading for CommandsModel {}
}

#[derive(Default)]
pub struct CommandsModelRust {
    command_ids: QStringList,
    command_names: QStringList,
    command_lines: QStringList,
    available: bool,
    ids: Vec<u32>,
    /// The refresh and edit threads, joined on drop; a late list is dropped.
    owned: crate::lifecycle::Owned,
}

impl qobject::CommandsModel {
    pub fn refresh(mut self: Pin<&mut Self>) {
        let qt = self.as_mut().qt_thread();
        self.rust().owned.spawn(move |guard| {
            let list = crate::devices::list_commands();
            if !guard.open() {
                return;
            }
            let _ = qt.queue(
                move |mut model: Pin<&mut qobject::CommandsModel>| match list {
                    Ok(list) => {
                        let ids: QStringList = list
                            .iter()
                            .map(|c| QString::from(c.id.to_string().as_str()))
                            .collect();
                        let names: QStringList = list
                            .iter()
                            .map(|c| QString::from(c.name.as_str()))
                            .collect();
                        let lines: QStringList = list
                            .iter()
                            .map(|c| {
                                let mut line = c.program.clone();
                                for a in &c.args {
                                    line.push(' ');
                                    line.push_str(a);
                                }
                                QString::from(line.as_str())
                            })
                            .collect();
                        model.as_mut().rust_mut().ids = list.iter().map(|c| c.id).collect();
                        model.as_mut().set_command_ids(ids);
                        model.as_mut().set_command_names(names);
                        model.as_mut().set_command_lines(lines);
                        model.as_mut().set_available(true);
                    }
                    Err(error) => {
                        eprintln!("magnetita: commands unavailable: {error}");
                        model.as_mut().set_available(false);
                    }
                },
            );
        });
    }

    pub fn add(mut self: Pin<&mut Self>, name: QString, program: QString, args: QString) {
        let (name, program, args) = (name.to_string(), program.to_string(), args.to_string());
        let args: Vec<String> = args.split_whitespace().map(str::to_owned).collect();
        let qt = self.as_mut().qt_thread();
        self.rust().owned.spawn(move |guard| {
            if let Err(error) = crate::devices::set_command(0, &name, &program, &args) {
                eprintln!("magnetita: command not registered: {error}");
            }
            if guard.open() {
                let _ = qt.queue(|model: Pin<&mut qobject::CommandsModel>| model.refresh());
            }
        });
    }

    pub fn remove(mut self: Pin<&mut Self>, index: i32) {
        let Some(id) = usize::try_from(index)
            .ok()
            .and_then(|i| self.rust().ids.get(i).copied())
        else {
            return;
        };
        let qt = self.as_mut().qt_thread();
        self.rust().owned.spawn(move |guard| {
            if let Err(error) = crate::devices::remove_command(id) {
                eprintln!("magnetita: command not removed: {error}");
            }
            if guard.open() {
                let _ = qt.queue(|model: Pin<&mut qobject::CommandsModel>| model.refresh());
            }
        });
    }
}
