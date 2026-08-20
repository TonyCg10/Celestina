//! The Qt half of what a file says about itself.
//!
//! Correcting a track's tags and removing what a photograph carries are the
//! same shape of work — replace a container's metadata block, copy its stream
//! across untouched — so they share one adapter rather than growing two. What
//! may be changed lives in `fluorita-core`'s `metadata`; the rewriting lives in
//! `fluorita-engine`'s. This file moves values between them and QML under the
//! rules the rest of the application already follows:
//!
//! - **The GUI thread never rewrites a file.** Reading the current values is a
//!   bounded read; writing runs on an owned worker and the result arrives
//!   through the queue.
//! - **Nothing is published that the engine did not confirm.** The panel keeps
//!   showing what the file said until the write lands.
//! - **A path is a key, not text** (ADR 0008).
//! - **A container this suite cannot write says so** rather than offering a
//!   correction that would be refused after the person typed it.

use std::path::PathBuf;
use std::thread::JoinHandle;

use celestina_core::{pathkey, CancellationToken};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QStringList};

use fluorita_core::{
    MediaKind, MetadataCapabilities, PrivateFact, SaveChoice, TagChange, TagField,
};
use fluorita_engine::{metadata as engine, DesktopTrash, MetadataRequest};

mod copy;

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
        /// The metadata surface QML binds to.
        #[qobject]
        #[qml_element]
        /// True while a file's metadata is open for reading or changing.
        #[qproperty(bool, open)]
        /// The item this is about, as its path key.
        #[qproperty(QString, key)]
        /// Its name, for the panel's heading. Lossy, like every label.
        #[qproperty(QString, name)]
        /// True when this container's tags can be corrected. False for one that
        /// can only be read, which the surface says out loud instead of
        /// offering a correction it would have to refuse.
        #[qproperty(bool, correctable)]
        /// Why not, when it is not. Empty when it is.
        #[qproperty(QString, read_only_reason)]
        /// What the file says right now — read from the file itself, not from
        /// what the catalogue remembered.
        #[qproperty(QString, title)]
        #[qproperty(QString, artist)]
        #[qproperty(QString, album)]
        #[qproperty(QString, album_artist)]
        /// What a photograph is carrying, as the words a person reads. Empty
        /// when it carries none of them, which is itself worth showing.
        #[qproperty(QStringList, private_facts)]
        /// True when those can be removed.
        #[qproperty(bool, strippable)]
        /// True when this container can carry a cover this suite can write.
        #[qproperty(bool, coverable)]
        /// True while a write is in flight. Every verb is refused meanwhile.
        #[qproperty(bool, busy)]
        /// What happened to the last write, or empty.
        #[qproperty(QString, notice)]
        type FluoritaMetadata = super::MetadataRust;

        /// Reads what one item says about itself and opens the panel. Takes a
        /// row's path key; anything else is refused with a stated reason.
        #[qinvokable]
        fn open_item(self: Pin<&mut FluoritaMetadata>, key: &QString);

        /// Whether this item says anything this panel can show or change.
        /// Asked before the action is offered, so nothing is put in front of a
        /// person that would answer with a refusal.
        #[qinvokable]
        fn admits(self: &FluoritaMetadata, key: &QString) -> bool;

        /// Closes the panel, discarding whatever was typed.
        #[qinvokable]
        fn close(self: Pin<&mut FluoritaMetadata>);

        /// Writes the four fields as given. An empty value removes that tag; a
        /// value equal to what the file already says is not written at all.
        /// `replace` writes in place and sends the original to the Trash;
        /// otherwise a copy lands beside it.
        #[qinvokable]
        fn correct(
            self: Pin<&mut FluoritaMetadata>,
            title: &QString,
            artist: &QString,
            album: &QString,
            album_artist: &QString,
            replace: bool,
        );

        /// Removes everything the photograph was reported to be carrying, on
        /// the same two terms.
        #[qinvokable]
        fn strip_private(self: Pin<&mut FluoritaMetadata>, replace: bool);

        /// Asks the desktop for a picture and embeds it as the track's front
        /// cover, on the same two terms. Returns at once: the chooser lasts as
        /// long as the person takes to decide, so it runs on a worker.
        #[qinvokable]
        fn choose_cover(self: Pin<&mut FluoritaMetadata>, replace: bool);
    }

    impl cxx_qt::Threading for FluoritaMetadata {}
}

pub struct MetadataRust {
    open: bool,
    key: QString,
    name: QString,
    correctable: bool,
    read_only_reason: QString,
    title: QString,
    artist: QString,
    album: QString,
    album_artist: QString,
    private_facts: QStringList,
    strippable: bool,
    coverable: bool,
    busy: bool,
    notice: QString,

    /// The file being described, byte-exact. Never published: the key is.
    source: Option<PathBuf>,
    capabilities: Option<MetadataCapabilities>,
    /// What the file was found to be carrying, kept so a removal asks for
    /// exactly what was reported rather than for a list QML rebuilt.
    carried: Vec<PrivateFact>,
    worker: Option<JoinHandle<()>>,
    cancellation: CancellationToken,
}

impl Default for MetadataRust {
    fn default() -> Self {
        Self {
            open: false,
            key: QString::default(),
            name: QString::default(),
            correctable: false,
            read_only_reason: QString::default(),
            title: QString::default(),
            artist: QString::default(),
            album: QString::default(),
            album_artist: QString::default(),
            private_facts: QStringList::default(),
            strippable: false,
            coverable: false,
            busy: false,
            notice: QString::default(),
            source: None,
            capabilities: None,
            carried: Vec::new(),
            worker: None,
            cancellation: CancellationToken::new(),
        }
    }
}

impl Drop for MetadataRust {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl qobject::FluoritaMetadata {
    pub fn open_item(mut self: std::pin::Pin<&mut Self>, key: &QString) {
        if *self.busy() {
            return;
        }
        let Ok(path) = pathkey::decode(&key.to_string()) else {
            self.as_mut()
                .set_notice(QString::from(copy::UNREADABLE_KEY));
            return;
        };
        let Some(kind) = MediaKind::classify_path(&path) else {
            self.as_mut().set_notice(QString::from(copy::NO_METADATA));
            return;
        };
        let capabilities = MetadataCapabilities::of(kind, &path);
        if capabilities.format().is_none() {
            self.as_mut().set_notice(QString::from(copy::NO_METADATA));
            return;
        }

        // Reading it is worker work. The prefix is bounded, but a mapped
        // folder can be a phone over sshfs — the suite mounts them — and four
        // megabytes across a link like that is a frozen window, not a header
        // read.
        let label = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        {
            let mut object = self.as_mut().rust_mut();
            object.source = Some(path.clone());
            object.capabilities = Some(capabilities);
            object.carried.clear();
        }
        self.as_mut().set_key(key.clone());
        self.as_mut().set_name(QString::from(&label));
        self.as_mut().set_title(QString::default());
        self.as_mut().set_artist(QString::default());
        self.as_mut().set_album(QString::default());
        self.as_mut().set_album_artist(QString::default());
        self.as_mut().set_private_facts(QStringList::default());
        self.as_mut()
            .set_strippable(capabilities.strips_private_facts());
        self.as_mut().set_coverable(capabilities.embeds_cover());
        self.as_mut().set_correctable(capabilities.corrects_tags());
        self.as_mut().set_read_only_reason(QString::from(
            if capabilities.shows_tags() && !capabilities.corrects_tags() {
                copy::READ_ONLY_CONTAINER
            } else {
                ""
            },
        ));
        self.as_mut().set_notice(QString::default());
        self.as_mut().set_open(true);
        self.as_mut().set_busy(true);

        // One reader at a time. A second file opened while the first is still
        // being read would otherwise leave a detached thread behind, and this
        // object's shutdown would have nothing to join.
        self.as_mut().cancel_worker();

        let qt_thread = self.qt_thread();
        let worker = std::thread::spawn(move || {
            let bytes = read_prefix(&path);
            let tags = if capabilities.shows_tags() {
                engine::read_flac_tags(&bytes).unwrap_or_default()
            } else {
                Vec::new()
            };
            let carried = if capabilities.strips_private_facts() {
                engine::private_facts(&bytes)
            } else {
                Vec::new()
            };
            let value = |field: TagField| {
                tags.iter()
                    .find(|(candidate, _)| *candidate == field)
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default()
            };
            let read = (
                value(TagField::Title),
                value(TagField::Artist),
                value(TagField::Album),
                value(TagField::AlbumArtist),
            );
            let _ = qt_thread.queue(move |mut object| {
                // The panel may have moved on to another file, or closed,
                // while this was reading. An answer to a question nobody is
                // asking any more is dropped rather than published.
                if !*object.open() || object.rust().source.as_deref() != Some(path.as_path()) {
                    return;
                }
                let mut facts = QStringList::default();
                for fact in &carried {
                    facts.append(QString::from(copy::private_fact(*fact)));
                }
                object.as_mut().rust_mut().carried = carried;
                object.as_mut().set_title(QString::from(&read.0));
                object.as_mut().set_artist(QString::from(&read.1));
                object.as_mut().set_album(QString::from(&read.2));
                object.as_mut().set_album_artist(QString::from(&read.3));
                object.as_mut().set_private_facts(facts);
                object.as_mut().set_busy(false);
            });
        });
        self.as_mut().rust_mut().worker = Some(worker);
    }

    #[must_use]
    pub fn admits(&self, key: &QString) -> bool {
        let Ok(path) = pathkey::decode(&key.to_string()) else {
            return false;
        };
        MediaKind::classify_path(&path)
            .is_some_and(|kind| MetadataCapabilities::of(kind, &path).format().is_some())
    }

    pub fn close(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().cancel_worker();
        {
            let mut object = self.as_mut().rust_mut();
            object.source = None;
            object.capabilities = None;
            object.carried.clear();
        }
        self.as_mut().set_open(false);
        self.as_mut().set_busy(false);
        self.as_mut().set_key(QString::default());
    }

    pub fn correct(
        mut self: std::pin::Pin<&mut Self>,
        title: &QString,
        artist: &QString,
        album: &QString,
        album_artist: &QString,
        replace: bool,
    ) {
        if *self.busy() || !*self.correctable() {
            return;
        }
        let mut change = TagChange::new();
        for (field, value) in [
            (TagField::Title, title),
            (TagField::Artist, artist),
            (TagField::Album, album),
            (TagField::AlbumArtist, album_artist),
        ] {
            if let Err(rejected) = change.set(field, &value.to_string()) {
                self.as_mut()
                    .set_notice(QString::from(copy::rejected(rejected)));
                return;
            }
        }
        if let Err(rejected) =
            engine::judge(self.rust().capabilities.and_then(|it| it.format()), &change)
        {
            self.as_mut()
                .set_notice(QString::from(copy::rejected(rejected)));
            return;
        }
        self.write(change, Vec::new(), replace);
    }

    pub fn strip_private(mut self: std::pin::Pin<&mut Self>, replace: bool) {
        if *self.busy() || !*self.strippable() {
            return;
        }
        let carried = self.rust().carried.clone();
        if carried.is_empty() {
            self.as_mut()
                .set_notice(QString::from(copy::NOTHING_CARRIED));
            return;
        }
        self.write(TagChange::new(), carried, replace);
    }
}

impl qobject::FluoritaMetadata {
    pub fn choose_cover(mut self: std::pin::Pin<&mut Self>, replace: bool) {
        if *self.busy() || !*self.coverable() {
            return;
        }
        let Some(source) = self.rust().source.clone() else {
            return;
        };
        self.as_mut().cancel_worker();
        self.as_mut().set_busy(true);
        self.as_mut().set_notice(QString::default());

        let choice = if replace {
            SaveChoice::Replace
        } else {
            SaveChoice::Copy
        };
        let cancellation = self.rust().cancellation.clone();
        let qt_thread = self.qt_thread();
        let worker = std::thread::spawn(move || {
            let message = match chosen_cover() {
                Ok(Some(chosen)) => {
                    let request = MetadataRequest {
                        source: &source,
                        tags: &TagChange::new(),
                        strip: &[],
                        cover: Some(fluorita_engine::Cover {
                            bytes: &chosen.bytes,
                            mime: chosen.mime,
                            width: chosen.width,
                            height: chosen.height,
                        }),
                        choice,
                        copy_marker: copy::COPY_MARKER,
                    };
                    match engine::write(&request, &DesktopTrash, &cancellation) {
                        Ok(written) => Some(copy::written(&written)),
                        Err(error) => Some(error.user_message()),
                    }
                }
                // A dismissed dialog is not a failure and says nothing.
                Ok(None) => None,
                Err(message) => Some(message),
            };
            let _ = qt_thread.queue(move |mut object| {
                object.as_mut().set_busy(false);
                if let Some(message) = message {
                    object.as_mut().set_notice(QString::from(&message));
                    object.as_mut().set_open(false);
                }
            });
        });
        self.as_mut().rust_mut().worker = Some(worker);
    }

    fn write(
        self: std::pin::Pin<&mut Self>,
        change: TagChange,
        strip: Vec<PrivateFact>,
        replace: bool,
    ) {
        self.write_with(change, strip, None, replace);
    }

    fn write_with(
        mut self: std::pin::Pin<&mut Self>,
        change: TagChange,
        strip: Vec<PrivateFact>,
        chosen: Option<ChosenCover>,
        replace: bool,
    ) {
        let Some(source) = self.rust().source.clone() else {
            return;
        };
        self.as_mut().cancel_worker();
        self.as_mut().set_busy(true);
        self.as_mut().set_notice(QString::default());

        let choice = if replace {
            SaveChoice::Replace
        } else {
            SaveChoice::Copy
        };
        let cancellation = self.rust().cancellation.clone();
        let qt_thread = self.qt_thread();
        let worker = std::thread::spawn(move || {
            let cover = chosen.as_ref().map(|chosen| fluorita_engine::Cover {
                bytes: &chosen.bytes,
                mime: chosen.mime,
                width: chosen.width,
                height: chosen.height,
            });
            let request = MetadataRequest {
                source: &source,
                tags: &change,
                strip: &strip,
                cover,
                choice,
                copy_marker: copy::COPY_MARKER,
            };
            let outcome = engine::write(&request, &DesktopTrash, &cancellation);
            let message = match &outcome {
                Ok(written) => copy::written(written),
                Err(error) => error.user_message(),
            };
            let landed = outcome.is_ok();
            let _ = qt_thread.queue(move |mut object| {
                object.as_mut().set_busy(false);
                object.as_mut().set_notice(QString::from(&message));
                if landed {
                    // The file on disk is no longer the file this panel read,
                    // so it is closed rather than left showing stale values.
                    object.as_mut().set_open(false);
                }
            });
        });
        self.as_mut().rust_mut().worker = Some(worker);
    }

    fn cancel_worker(mut self: std::pin::Pin<&mut Self>) {
        self.as_mut().rust_mut().cancellation.cancel();
        let worker = self.as_mut().rust_mut().worker.take();
        if let Some(worker) = worker {
            let _ = worker.join();
        }
        self.as_mut().rust_mut().cancellation = CancellationToken::new();
    }
}

/// A picture the person chose, read and measured, ready to embed.
struct ChosenCover {
    bytes: Vec<u8>,
    mime: &'static str,
    width: u32,
    height: u32,
}

/// Asks the desktop for a picture, then reads and judges it.
///
/// `Ok(None)` is a dismissed dialog, which is not a failure. Every refusal
/// carries the words for it, because a chooser that closes and does nothing is
/// the interaction this exists to avoid.
fn chosen_cover() -> Result<Option<ChosenCover>, String> {
    let path = match crate::folders::choose_picture(copy::COVER_TITLE, copy::COVER_FILTER) {
        crate::folders::FolderChoice::Chosen(path) => path,
        crate::folders::FolderChoice::Cancelled => return Ok(None),
        crate::folders::FolderChoice::Unavailable(reason) => return Err(reason),
    };

    let bytes = std::fs::metadata(&path).map(|it| it.len()).unwrap_or(0);
    // The same probe the viewer uses, so a cover is measured the way every
    // other picture in this application is.
    let key = QString::from(&pathkey::encode(&path));
    let measured = crate::player::qobject::probe_image(&key);
    let pixels = (measured.width() > 0 && measured.height() > 0).then(|| {
        u64::try_from(measured.width()).unwrap_or(0) * u64::try_from(measured.height()).unwrap_or(0)
    });
    fluorita_core::CoverBudget::DEFAULT
        .accepts(&path, bytes, pixels)
        .map_err(|rejected| copy::rejected(rejected).to_owned())?;

    let mime = match fluorita_core::ImageFormat::classify_path(&path) {
        Some(fluorita_core::ImageFormat::Png) => "image/png",
        Some(fluorita_core::ImageFormat::Jpeg) => "image/jpeg",
        Some(fluorita_core::ImageFormat::Webp) => "image/webp",
        // Every other format the library reads is one this cannot name to a
        // player, and an embedded picture nothing can identify is decoration
        // that never draws.
        _ => {
            return Err(copy::rejected(fluorita_core::MetadataRejected::CoverNotAnImage).to_owned())
        }
    };

    Ok(Some(ChosenCover {
        bytes: std::fs::read(&path).map_err(|error| error.to_string())?,
        mime,
        width: u32::try_from(measured.width()).unwrap_or(0),
        height: u32::try_from(measured.height()).unwrap_or(0),
    }))
}

/// How much of a file is read to find what it says about itself. Generous for
/// a comment block with a cover in it, bounded because this runs where the
/// window does.
const METADATA_PREFIX_BYTES: u64 = 4 * 1024 * 1024;

fn read_prefix(path: &std::path::Path) -> Vec<u8> {
    use std::io::Read;
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let mut bytes = Vec::new();
    let _ = file.take(METADATA_PREFIX_BYTES).read_to_end(&mut bytes);
    bytes
}

#[cfg(test)]
mod tests {
    use fluorita_core::{MediaKind, MetadataCapabilities, PrivateFact, TagChange, TagField};
    use fluorita_engine::metadata as engine;
    use std::path::Path;

    #[test]
    fn a_container_that_can_only_be_read_is_reported_and_not_offered() {
        let mp3 = MetadataCapabilities::of(MediaKind::Audio, Path::new("/m/pista.mp3"));
        assert!(mp3.shows_tags());
        assert!(!mp3.corrects_tags());
        assert!(!super::copy::READ_ONLY_CONTAINER.is_empty());

        let mut change = TagChange::new();
        change.set(TagField::Title, "Pavana").expect("valid");
        assert!(engine::judge(mp3.format(), &change).is_err());
    }

    #[test]
    fn every_private_fact_has_words_of_its_own() {
        for fact in PrivateFact::ALL {
            assert!(!super::copy::private_fact(fact).is_empty());
        }
    }
}
