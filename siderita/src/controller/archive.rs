//! language-contract: product-copy
//!
//! Compressing and extracting from the file manager.
//!
//! The rules — what a container is, which member is safe to write, which name
//! the extraction takes, what is never overwritten — belong to
//! [`siderita_archive`]. What lives here is only the Qt half: deciding on the
//! Qt thread, running the long write on a worker, and reporting it through the
//! very same progress, cancellation and failure surface a paste already uses,
//! so there is one Cancel button and one meaning of "an operation is running".
//!
//! The marker above declares what the Spanish literals here are: the status
//! lines, the failure sentences and the word that frees a taken name are the
//! words a person reads. The domain answers *why* in its own English terms —
//! it has no language — and this module is the one place that becomes Spanish.

use core::pin::Pin;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::localzone::LocalZone;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};
use siderita_archive::{ArchiveError, Format, SkipReason, Skipped};
use siderita_ops::Progress;

use super::display_name;
use super::qobject;

/// An extraction batch that is not finished: what is left to extract, where, and
/// what has already been reported.
///
/// It exists because an encrypted archive turns one operation into two halves
/// with a person in between. Rather than hold the worker thread waiting on a
/// dialog, the worker ends and hands its remaining work here; answering starts a
/// new worker from exactly this state.
pub(crate) struct Pending {
    /// The job this batch belongs to, once it has one: answering a password
    /// resumes the same job rather than starting a second one.
    job: Option<u64>,
    /// Still to extract; the first is the one the question is about.
    archives: Vec<PathBuf>,
    into: PathBuf,
    failures: Vec<String>,
    skipped: Vec<Skipped>,
    done: usize,
    total: usize,
}

impl qobject::SideritaController {
    /// Whether the entry `key` names is an archive this domain can extract,
    /// decided by its bytes and never by its name — the same rule content
    /// activation follows for text and media.
    pub fn is_archive(&self, key: &QString) -> bool {
        crate::pathkey::decode(key)
            .ok()
            .as_deref()
            .and_then(siderita_archive::sniff)
            // A RAR or 7z is only offered when the tool that reads it is
            // installed: a verb that is always refused is worse than no verb.
            .is_some_and(siderita_archive::can_read)
    }

    /// Whether every entry in `keys` is an extractable archive, so a menu can
    /// offer «Extraer» for a whole selection or not at all.
    pub fn are_archives(&self, keys: &QStringList) -> bool {
        let mut any = false;
        for key in keys.iter() {
            if key.is_empty() {
                continue;
            }
            any = true;
            if !self.is_archive(key) {
                return false;
            }
        }
        any
    }

    /// The file name the compress dialog starts with for this selection and
    /// format: one entry keeps its own stem, several take the folder's name, and
    /// a name already taken is stepped past — so the suggested name never asks a
    /// person to accept overwriting something.
    ///
    /// Composed here rather than in QML because it is a *name on disk*: it is
    /// answered from the bytes of the entries and of the folder, which QML never
    /// takes apart.
    pub fn archive_suggested_name(&self, keys: &QStringList, format: &QString) -> QString {
        let Some(format) = Format::from_token(&format.to_string()) else {
            return QString::default();
        };
        let Ok(paths) = crate::pathkey::decode_list(keys) else {
            return QString::default();
        };
        let Some(folder) = self.rust().history.current() else {
            return QString::default();
        };
        let fallback = folder
            .file_name()
            .map(OsStr::to_os_string)
            .unwrap_or_else(|| OsString::from("archivos"));
        let stem = siderita_archive::default_stem(&paths, &fallback);

        let mut name = stem.to_os_string();
        name.push(".");
        name.push(format.extension());
        if std::fs::symlink_metadata(folder.join(&name)).is_ok() {
            // The suggestion names an archive file, so the marker keeps its
            // `.zip` / `.tar.gz` where a person expects to read it.
            let freed = siderita_ops::next_available(
                folder,
                &name,
                "nuevo",
                siderita_ops::NameShape::Extension(format.extension()),
            );
            name = freed.file_name().map(OsStr::to_os_string).unwrap_or(name);
        }
        QString::from(name.to_string_lossy().as_ref())
    }

    /// Extracts every archive in `keys` into the folder being shown.
    pub fn extract_keys(mut self: Pin<&mut Self>, keys: &QStringList) {
        self.as_mut().set_op_error(QString::default());
        let Some(paths) = self.as_mut().accept_keys(keys) else {
            return;
        };
        if paths.is_empty() {
            return;
        }
        let Some(into) = self.rust().history.current().map(Path::to_path_buf) else {
            return;
        };
        let total = paths.len();
        self.spawn_extract(
            Pending {
                job: None,
                archives: paths,
                into,
                failures: Vec::new(),
                skipped: Vec::new(),
                done: 0,
                total,
            },
            None,
        );
    }

    /// Compresses every entry in `keys` into `name`, a plain file name inside
    /// the folder being shown, using the container `format` names.
    pub fn compress_keys(
        mut self: Pin<&mut Self>,
        keys: &QStringList,
        name: &QString,
        format: &QString,
    ) {
        self.as_mut().set_op_error(QString::default());
        let Some(paths) = self.as_mut().accept_keys(keys) else {
            return;
        };
        if paths.is_empty() {
            return;
        }
        let Some(format) = Format::from_token(&format.to_string()) else {
            self.as_mut()
                .set_op_error(QString::from("Formato de archivo desconocido"));
            return;
        };
        let Some(folder) = self.rust().history.current().map(Path::to_path_buf) else {
            return;
        };
        // A name, not a path: the dialog collects one field, and a person typing
        // `../fuera.zip` is not asking to write into another folder.
        let name = name.to_string();
        if name.is_empty() || name.contains('/') || name.starts_with('.') && name.len() == 1 {
            self.as_mut()
                .set_op_error(QString::from("Nombre de archivo no válido"));
            return;
        }
        let destination = folder.join(&name);
        self.spawn_compress(paths, destination, format);
    }

    /// Runs the extraction on a worker thread, on the shared progress surface.
    ///
    /// Refused while another operation runs or a conflict is being answered,
    /// exactly like a paste: the two would otherwise share one Cancel button and
    /// one progress state between two writers.
    ///
    /// `password` is the answer to the question that stopped this very batch, so
    /// it opens the *first* archive left in `state`. From there it is carried to
    /// the rest of the batch as a second attempt and never as the first one:
    /// every following archive is opened with no password at all, and the
    /// carried one is offered only to an archive that answered it needs one. So
    /// a person who protected several archives with the same key is asked once,
    /// and an archive that is not encrypted never sees their password.
    fn spawn_extract(mut self: Pin<&mut Self>, state: Pending, password: Option<String>) {
        if state.archives.is_empty() {
            let Pending {
                failures,
                skipped,
                total,
                job,
                ..
            } = state;
            if let Some(job) = job {
                self.finish_archive_op(job, total, failures, skipped, false);
            }
            return;
        }
        // A resumed batch keeps its own job; a fresh one registers a new one.
        let (job, token) = match state.job {
            Some(job) => (job, self.as_mut().job_token(job)),
            None => {
                self.as_mut()
                    .start_job("Extrayendo…", super::jobs::JobKind::Extract, state.total)
            }
        };
        let qt = self.qt_thread();

        std::thread::spawn(move || {
            let Pending {
                job: _,
                archives,
                into,
                mut failures,
                mut skipped,
                mut done,
                total,
            } = state;
            // The key a person has already given in this batch. The first
            // archive is the one they were asked about, so it is tried on that
            // one outright; from then on it is only a fallback for an archive
            // that says it needs a password.
            let carried = password;
            let mut answered = carried.is_some();

            for (index, archive) in archives.iter().enumerate() {
                if token.is_cancelled() {
                    break;
                }
                announce(&qt, job, done.min(i32::MAX as usize) as i32, archive);
                // First attempt: the answer for the archive that asked, and
                // nothing at all for every other one.
                let opening = if answered { carried.as_deref() } else { None };
                answered = false;
                let mut tried = opening.is_some();
                let mut outcome = attempt(&qt, job, archive, &into, opening, &token);
                // Second and last attempt, silent: the key that already opened an
                // archive of this batch, offered only because this one asked for
                // a password. Nothing of the refused attempt survives — `extract`
                // clears its destination when it refuses — so this starts clean.
                if !tried && outcome.as_ref().is_err_and(ArchiveError::needs_password) {
                    if let Some(secret) = carried.as_deref() {
                        tried = true;
                        outcome = attempt(&qt, job, archive, &into, Some(secret), &token);
                    }
                }
                match outcome {
                    Ok(extracted) => skipped.extend(extracted.skipped),
                    Err(error) if error.is_cancelled() => break,
                    Err(error) if error.needs_password() => {
                        // Stop here and hand the question to the Qt thread: what
                        // is left of the batch travels with it, so answering
                        // resumes exactly where this stopped. `tried` is what the
                        // dialog says: a password was offered and refused, rather
                        // than none having been given yet.
                        let waiting = Pending {
                            job: Some(job),
                            archives: archives[index..].to_vec(),
                            into,
                            failures,
                            skipped,
                            done,
                            total,
                        };
                        let name = display_name(archive);
                        let _ = qt.queue(move |controller| {
                            controller.ask_for_password(waiting, name, tried);
                        });
                        return;
                    }
                    Err(error) => failures.push(report(archive, &error)),
                }
                done += 1;
            }

            let cancelled = token.is_cancelled();
            let _ = qt.queue(move |controller| {
                controller.finish_archive_op(job, total, failures, skipped, cancelled);
            });
        });
    }

    /// Asks the person for the password the archive is waiting on, and parks the
    /// rest of the batch until they answer.
    ///
    /// The operation surface stays claimed: the batch is not finished, it is
    /// waiting, and a second writer starting meanwhile would take the Cancel
    /// button out from under it.
    pub(crate) fn ask_for_password(
        mut self: Pin<&mut Self>,
        waiting: Pending,
        name: String,
        retry: bool,
    ) {
        self.as_mut().rust_mut().get_mut().pending_password = Some(waiting);
        self.as_mut()
            .set_password_archive(QString::from(name.as_str()));
        self.as_mut().set_password_retry(retry);
        self.as_mut().set_password_pending(true);
        self.as_mut()
            .set_status_text(QString::from("Esperando la contraseña…"));
    }

    /// Retries the parked extraction with the password a person typed.
    pub fn answer_password(mut self: Pin<&mut Self>, password: &QString) {
        let Some(waiting) = self.as_mut().rust_mut().get_mut().pending_password.take() else {
            return;
        };
        self.as_mut().clear_password_question();
        self.as_mut().set_status_text(QString::from("Extrayendo…"));
        self.spawn_extract(waiting, Some(password.to_string()));
    }

    /// Gives up on the archive that asked, and carries on with the rest.
    ///
    /// Skipping one encrypted archive is not a reason to abandon the others a
    /// person selected; the one skipped is reported like any other failure, so
    /// the batch never claims to have extracted it.
    pub fn cancel_password(mut self: Pin<&mut Self>) {
        let Some(mut waiting) = self.as_mut().rust_mut().get_mut().pending_password.take() else {
            return;
        };
        self.as_mut().clear_password_question();
        if !waiting.archives.is_empty() {
            let skipped_archive = waiting.archives.remove(0);
            waiting.failures.push(format!(
                "{}: hace falta la contraseña",
                display_name(&skipped_archive)
            ));
            waiting.done += 1;
        }
        self.spawn_extract(waiting, None);
    }

    /// Clears the question itself, leaving the batch state alone.
    fn clear_password_question(mut self: Pin<&mut Self>) {
        self.as_mut().set_password_pending(false);
        self.as_mut().set_password_retry(false);
        self.as_mut().set_password_archive(QString::default());
    }

    /// Runs the compression on a worker thread, on the shared progress surface.
    fn spawn_compress(
        mut self: Pin<&mut Self>,
        sources: Vec<PathBuf>,
        destination: PathBuf,
        format: Format,
    ) {
        if *self.conflict_pending() {
            return;
        }
        let (job, token) = self.as_mut().start_job(
            "Comprimiendo…",
            super::jobs::JobKind::Compress,
            sources.len(),
        );
        let qt = self.qt_thread();

        std::thread::spawn(move || {
            let total = sources.len();
            announce(&qt, job, 0, &destination);
            let mut on_progress = throttled(&qt, job, "comprimidos", 0);
            let failures = match siderita_archive::create(
                &sources,
                &destination,
                format,
                &LocalZone,
                &token,
                &mut on_progress,
            ) {
                Ok(()) | Err(ArchiveError::Op(siderita_ops::OpError::Cancelled)) => Vec::new(),
                Err(error) => vec![report(&destination, &error)],
            };

            let cancelled = token.is_cancelled();
            let _ = qt.queue(move |controller| {
                controller.finish_archive_op(job, total, failures, Vec::new(), cancelled);
            });
        });
    }

    /// Ends the job on the Qt thread and reports the truth: what failed, what
    /// the archive carried but no filesystem entry could hold, and whether the
    /// person stopped it part-way.
    pub(crate) fn finish_archive_op(
        mut self: Pin<&mut Self>,
        job: u64,
        total: usize,
        failures: Vec<String>,
        skipped: Vec<Skipped>,
        cancelled: bool,
    ) {
        self.as_mut().end_job(job);

        // A compression or extraction is not undoable by the domain: the undo
        // record still describes the last reversible write, so it is left alone
        // only when nothing landed, and cleared when something did.
        if failures.is_empty() {
            self.as_mut().set_undo(None);
        }

        let mut reported = failures;
        if !skipped.is_empty() {
            let lines: Vec<String> = skipped.iter().map(left_out).collect();
            reported.push(format!(
                "{} entrada(s) del archivo no se extrajeron:\n{}",
                lines.len(),
                lines.join("\n")
            ));
        }
        let total = total.max(reported.len());
        self.as_mut().finish_batch(total, &reported);
        if reported.is_empty() && cancelled {
            self.as_mut()
                .set_status_text(QString::from("Operación cancelada"));
        }
    }
}

/// One extraction of one archive, weighed and reported like any other.
///
/// Split out because an archive is opened up to twice — once with no password,
/// once with the key the batch already carries — and both attempts have to
/// measure and report identically for the ring to mean the same thing.
fn attempt(
    qt: &cxx_qt::CxxQtThread<qobject::SideritaController>,
    job: u64,
    archive: &Path,
    into: &Path,
    password: Option<&str>,
    token: &celestina_core::CancellationToken,
) -> Result<siderita_archive::Extracted, ArchiveError> {
    let zone = LocalZone;
    let mut options = siderita_archive::ExtractOptions::new(&zone, "extraído");
    if let Some(secret) = password {
        options = options.with_password(secret);
    }
    // Asked before the work starts, from the archive's own index: one cheap pass
    // over headers, and the difference between a ring that turns and one that
    // fills.
    let expected = siderita_archive::measure(archive, &options).unwrap_or(0);
    let mut on_progress = throttled(qt, job, "extraídos", expected);
    siderita_archive::extract(archive, into, &options, token, &mut on_progress)
}

/// Publishes which entry the operation reached, on the Qt thread.
fn announce(
    qt: &cxx_qt::CxxQtThread<qobject::SideritaController>,
    job: u64,
    done: i32,
    path: &Path,
) {
    let announced = display_name(path);
    let _ = qt.queue(move |controller| {
        controller.job_reached(job, done, Some(announced), Some(String::new()));
    });
}

/// The same throttled byte read-out a paste publishes, at the same cadence.
fn throttled(
    qt: &cxx_qt::CxxQtThread<qobject::SideritaController>,
    job: u64,
    verb: &'static str,
    // What the archive weighs once extracted, when that could be measured. It
    // is what makes the read-out "so much of so much" and the ring fill.
    expected: u64,
) -> impl FnMut(Progress) {
    let qt = qt.clone();
    let mut last = std::time::Instant::now();
    move |progress: Progress| {
        if last.elapsed().as_millis() < 60 {
            return;
        }
        last = std::time::Instant::now();
        let moved = progress.bytes;
        let detail = if expected > 0 {
            format!(
                "{} de {} {verb}",
                crate::format::size(moved),
                crate::format::size(expected)
            )
        } else {
            format!("{} {verb}", crate::format::size(moved))
        };
        let _ = qt.queue(move |controller| {
            controller.job_weighed(job, moved, expected, detail);
        });
    }
}

/// One failure line: the entry a person recognises, then the reason in the
/// language the product speaks.
///
/// The domain answers *why* in its own (English) terms, as every pure crate
/// here does; the words a person reads belong to the application. This is the
/// one place that mapping happens for the archive verbs.
fn report(path: &Path, error: &ArchiveError) -> String {
    format!("{}: {}", display_name(path), spanish(error))
}

/// The Spanish line for a member the extraction left out.
fn left_out(skipped: &Skipped) -> String {
    let reason = match skipped.reason {
        SkipReason::UnsupportedKind => {
            "no es un fichero, una carpeta ni un enlace, y no se inventa"
        }
        SkipReason::SymlinkWithoutTarget => "es un enlace sin destino guardado",
    };
    format!("{}: {reason}", skipped.name.display())
}

/// The Spanish sentence for a domain refusal.
fn spanish(error: &ArchiveError) -> String {
    match error {
        ArchiveError::UnsupportedFormat { .. } => {
            "no es un archivo comprimido que Siderita sepa abrir".to_string()
        }
        ArchiveError::NotWritable { format } => {
            format!("Siderita no crea archivos {format}")
        }
        ArchiveError::Malformed { .. } => "está dañado o incompleto".to_string(),
        ArchiveError::UnsafeMember { name } => {
            let quoted = format!("«{name}»");
            format!("contiene una entrada que se escribiría fuera del destino {quoted}; no se ha extraído nada")
        }
        ArchiveError::NonUtf8Name { name } => format!(
            "«{}» tiene un nombre que un zip no puede guardar tal cual; \
             usa TAR.GZ para conservarlo",
            name.display()
        ),
        ArchiveError::NothingToCompress => "no hay nada que comprimir".to_string(),
        ArchiveError::PasswordRequired { .. } => {
            "está protegido con contraseña y no se ha dado ninguna".to_string()
        }
        ArchiveError::WrongPassword { .. } => "la contraseña no es correcta".to_string(),
        ArchiveError::ToolMissing { format, tool } => {
            format!("para abrir archivos {format} hace falta {tool}, que no está instalado")
        }
        ArchiveError::Op(error) => error.to_string(),
    }
}
