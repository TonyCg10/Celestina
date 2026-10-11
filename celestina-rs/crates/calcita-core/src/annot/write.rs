//! Writing edits into a PDF as one incremental update.
//!
//! Nothing of the original is rewritten: `lopdf`'s `IncrementalDocument`
//! copies every original byte first and appends only the objects that
//! change, a cross-reference section and a trailer pointing back at the
//! previous one (ISO 32000-1, 7.5.6), so the original bytes are a prefix of
//! the result and an earlier revision stays recoverable. A second save
//! appends again.
//!
//! What changes: a page's `/Annots` (an annotation added or removed, an
//! inline one rewritten), each annotation added or rewritten, and its
//! appearance stream. A text box only moved or resized keeps the
//! appearance its program drew, its `/DA`, `/Contents` and `/RC`; one whose
//! text, size or colour changed is laid out again by Calcita
//! ([`lays_out_again`]), and so is every highlight and drawing that changed.
//! Every edit the edit set accepted can be written: what is laid out again
//! was checked when it was pushed.
//!
//! The same machinery makes the copy the page provider shows while editing
//! ([`hidden_copy`]): the editable annotations marked hidden, so QtPdf
//! draws the rest and Calcita draws those itself.

use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use lopdf::{dictionary, Dictionary, IncrementalDocument, Object, ObjectId, Stream, StringFormat};

use super::appearance::{
    self, free_text_ap, highlight_ap, ink_ap, Appearance, Resources, FONT_RESOURCE,
    MULTIPLY_RESOURCE,
};
use super::coords::PageBox;
use super::edit::{lays_out_again, Base, EditSet};
use super::model::{AnnotId, Annotation, Kind, Rect, Rgb};
use super::read::{
    load_for_edit, locations, model_of, page_annotations, pages, AnnotError, Loaded, Location,
};

/// The annotation flag that hides an annotation (ISO 32000-1, table 165).
const FLAG_HIDDEN: i64 = 1 << 1;

/// The annotation flag that prints it, set on every annotation Calcita adds.
const FLAG_PRINT: i64 = 1 << 2;

/// Writes `original` with `edits` applied as one incremental update to
/// `out`, a new file (an existing one is never replaced).
///
/// # Errors
///
/// As [`apply_to_bytes`], or [`AnnotError::Write`] when `out` cannot be
/// created or written.
pub fn apply(original: &Path, edits: &EditSet, out: &Path) -> Result<(), AnnotError> {
    let bytes = apply_to_bytes(original, edits)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(out)
        .map_err(AnnotError::Write)?;
    file.write_all(&bytes).map_err(AnnotError::Write)?;
    file.sync_all().map_err(AnnotError::Write)
}

/// The bytes of `original` with `edits` applied as one incremental update:
/// the original's bytes, then the update.
///
/// # Errors
///
/// The reader's [`AnnotError`] (unreadable, too large, encrypted,
/// unparseable), [`AnnotError::NotEditable`] when the document's structure
/// does not hold what an edit names, [`AnnotError::Unencodable`] for text
/// Helvetica in WinAnsi cannot show, [`AnnotError::Write`] when `lopdf`
/// cannot write the update.
pub fn apply_to_bytes(original: &Path, edits: &EditSet) -> Result<Vec<u8>, AnnotError> {
    let loaded = load_for_edit(original)?;
    let (annotations, boxes) = model_of(&loaded.document);
    let base = Base { annotations, boxes };
    let resolved = edits.resolve(&base);
    let places = locations(&loaded.document);
    let page_ids: Vec<ObjectId> = pages(&loaded.document)
        .into_iter()
        .map(|(_, id)| id)
        .collect();
    let mut update = Update::new(loaded);
    for annotation in &resolved.changed {
        let original = base
            .annotations
            .iter()
            .find(|original| original.id == annotation.id)
            .ok_or(AnnotError::NotEditable)?;
        let place = places.get(&annotation.id).ok_or(AnnotError::NotEditable)?;
        let page = base
            .page_box(annotation.page)
            .ok_or(AnnotError::NotEditable)?;
        update.change(original, annotation, *place, page)?;
    }
    update.delete(&resolved.deleted, &places)?;
    for annotation in &resolved.added {
        let page_id = usize::try_from(annotation.page)
            .ok()
            .and_then(|page| page_ids.get(page))
            .copied()
            .ok_or(AnnotError::NotEditable)?;
        let page = base
            .page_box(annotation.page)
            .ok_or(AnnotError::NotEditable)?;
        update.add(annotation, page_id, page)?;
    }
    update.finish()
}

/// The bytes of `original` with each annotation of `ids` marked hidden, as
/// one incremental update kept in memory: what the page provider renders
/// while editing, so QtPdf draws every other annotation and Calcita draws
/// these over the page. Without an id to hide, the original's bytes.
///
/// # Errors
///
/// As [`apply_to_bytes`].
pub fn hidden_copy(original: &Path, ids: &[AnnotId]) -> Result<Vec<u8>, AnnotError> {
    let loaded = load_for_edit(original)?;
    if ids.is_empty() {
        return Ok(loaded.bytes);
    }
    let places = locations(&loaded.document);
    let mut update = Update::new(loaded);
    for id in ids {
        let Some(place) = places.get(id) else {
            continue;
        };
        update.rewrite(*place, |dictionary| {
            let flags = dictionary
                .get(b"F")
                .ok()
                .and_then(|flags| flags.as_i64().ok())
                .unwrap_or(0);
            dictionary.set("F", flags | FLAG_HIDDEN);
        })?;
    }
    update.finish()
}

/// One incremental update being built.
struct Update {
    document: IncrementalDocument,
    /// The Helvetica font object every text appearance of this update
    /// shares, once one needs it.
    font: Option<ObjectId>,
}

/// `lopdf`'s failure to find what the document's own structure names.
fn structure(error: lopdf::Error) -> AnnotError {
    AnnotError::Unparseable(error)
}

/// A PDF number: a real rounded to three decimals.
fn real(value: f64) -> Object {
    let rounded = (value * 1000.0).round() / 1000.0;
    // PDF readers take reals as single precision anyway; page coordinates
    // stay well inside its exact range at a thousandth of a point.
    #[allow(clippy::cast_possible_truncation)]
    Object::Real(rounded as f32)
}

fn rect_object(rect: Rect) -> Object {
    Object::Array(vec![
        real(rect.x0),
        real(rect.y0),
        real(rect.x1),
        real(rect.y1),
    ])
}

fn color_object(color: Rgb) -> Object {
    Object::Array(
        [color.r, color.g, color.b]
            .into_iter()
            .map(|channel| real(f64::from(channel) / 255.0))
            .collect(),
    )
}

/// A text string (ISO 32000-1, 7.9.2.2): ASCII as it is, anything else as
/// UTF-16 big-endian after its byte-order mark.
fn text_string(text: &str) -> Object {
    if text.is_ascii() {
        return Object::String(text.as_bytes().to_vec(), StringFormat::Literal);
    }
    let mut bytes = vec![0xFE, 0xFF];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_be_bytes());
    }
    Object::String(bytes, StringFormat::Hexadecimal)
}

/// A text box's default appearance string: Helvetica at `size`, in `color`.
fn default_appearance(size: f64, color: Rgb) -> Object {
    let unit = |channel: u8| appearance::number(f64::from(channel) / 255.0);
    Object::string_literal(format!(
        "/{FONT_RESOURCE} {} Tf {} {} {} rg",
        appearance::number(size),
        unit(color.r),
        unit(color.g),
        unit(color.b)
    ))
}

impl Update {
    fn new(loaded: Loaded) -> Self {
        Self {
            document: IncrementalDocument::create_from(loaded.bytes, loaded.document),
            font: None,
        }
    }

    fn finish(mut self) -> Result<Vec<u8>, AnnotError> {
        let mut bytes = Vec::new();
        self.document
            .save_to(&mut bytes)
            .map_err(AnnotError::Write)?;
        Ok(bytes)
    }

    fn font(&mut self) -> ObjectId {
        if let Some(font) = self.font {
            return font;
        }
        let font = self.document.new_document.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
            "Encoding" => "WinAnsiEncoding",
        });
        self.font = Some(font);
        font
    }

    /// Adds `appearance` as a form XObject and gives its id.
    fn appearance(&mut self, appearance: Appearance) -> ObjectId {
        let resources = match appearance.resources {
            Resources::None => Dictionary::new(),
            Resources::Helvetica => {
                let font = self.font();
                dictionary! { "Font" => dictionary! { FONT_RESOURCE => Object::Reference(font) } }
            }
            Resources::Multiply => dictionary! {
                "ExtGState" => dictionary! {
                    MULTIPLY_RESOURCE => dictionary! { "Type" => "ExtGState", "BM" => "Multiply" },
                },
            },
        };
        let stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Form",
                "FormType" => 1,
                "BBox" => rect_object(appearance.bbox),
                "Matrix" => vec![1.into(), 0.into(), 0.into(), 1.into(), 0.into(), 0.into()],
                "Resources" => resources,
            },
            appearance.content,
        );
        self.document.new_document.add_object(stream)
    }

    /// The appearance `annotation` is drawn with, as Calcita lays it out.
    fn appearance_of(
        &mut self,
        annotation: &Annotation,
        page: PageBox,
    ) -> Result<ObjectId, AnnotError> {
        let appearance = match &annotation.kind {
            Kind::Text {
                contents, size_pt, ..
            } => free_text_ap(page, annotation.rect, contents, *size_pt, annotation.color)?,
            Kind::Highlight { quads } => highlight_ap(annotation.rect, quads, annotation.color),
            Kind::Ink { strokes, width } => {
                ink_ap(annotation.rect, strokes, *width, annotation.color)
            }
            Kind::Other { .. } => return Err(AnnotError::NotEditable),
        };
        Ok(self.appearance(appearance))
    }

    /// The page dictionary as this update has it.
    fn page_annots_entry(&self, page: ObjectId) -> Result<Option<Object>, AnnotError> {
        let dictionary = if self.document.new_document.has_object(page) {
            self.document.new_document.get_dictionary(page)
        } else {
            self.document.get_prev_documents().get_dictionary(page)
        }
        .map_err(structure)?;
        Ok(dictionary.get(b"Annots").ok().cloned())
    }

    /// Runs `change` on the page's `/Annots` array as this update writes it:
    /// the array object copied into the update when the page names one,
    /// else the page itself with its inline array (created when missing).
    fn with_annots<R>(
        &mut self,
        page: ObjectId,
        change: impl FnOnce(&mut Vec<Object>) -> R,
    ) -> Result<R, AnnotError> {
        if let Some(Object::Reference(id)) = self.page_annots_entry(page)? {
            let is_array = if self.document.new_document.has_object(id) {
                self.document.new_document.get_object(id)
            } else {
                self.document.get_prev_documents().get_object(id)
            }
            .is_ok_and(|object| object.as_array().is_ok());
            if is_array {
                self.document
                    .opt_clone_object_to_new_document(id)
                    .map_err(structure)?;
                let array = self
                    .document
                    .new_document
                    .get_object_mut(id)
                    .and_then(Object::as_array_mut)
                    .map_err(structure)?;
                return Ok(change(array));
            }
        }
        self.document
            .opt_clone_object_to_new_document(page)
            .map_err(structure)?;
        let dictionary = self
            .document
            .new_document
            .get_dictionary_mut(page)
            .map_err(structure)?;
        if !matches!(dictionary.get(b"Annots"), Ok(Object::Array(_))) {
            dictionary.set("Annots", Object::Array(Vec::new()));
        }
        let array = dictionary
            .get_mut(b"Annots")
            .and_then(Object::as_array_mut)
            .map_err(structure)?;
        Ok(change(array))
    }

    /// Runs `change` on the annotation at `place` as this update writes it:
    /// its own object copied into the update, or its inline dictionary in
    /// the page's `/Annots`.
    fn rewrite(
        &mut self,
        place: Location,
        change: impl FnOnce(&mut Dictionary),
    ) -> Result<(), AnnotError> {
        if let Some(object) = place.object {
            self.document
                .opt_clone_object_to_new_document(object)
                .map_err(structure)?;
            let dictionary = self
                .document
                .new_document
                .get_dictionary_mut(object)
                .map_err(structure)?;
            change(dictionary);
            return Ok(());
        }
        let index = usize::try_from(place.index).map_err(|_| AnnotError::NotEditable)?;
        self.with_annots(place.page, |array| {
            match array
                .get_mut(index)
                .and_then(|entry| entry.as_dict_mut().ok())
            {
                Some(dictionary) => {
                    change(dictionary);
                    Ok(())
                }
                None => Err(AnnotError::NotEditable),
            }
        })?
    }

    /// Rewrites an annotation another program, or an earlier save, wrote, so
    /// it ends as `edited` (`original` is how the file has it).
    fn change(
        &mut self,
        original: &Annotation,
        edited: &Annotation,
        place: Location,
        page: PageBox,
    ) -> Result<(), AnnotError> {
        let rect = rect_object(edited.rect);
        // A text box only moved or resized keeps its own appearance (the
        // viewer maps its box onto the new rectangle), `/DA`, `/Contents`
        // and `/RC`: only `/Rect` changes.
        let relaid = lays_out_again(Some(original), edited);
        let appearance = if relaid {
            Some(self.appearance_of(edited, page)?)
        } else {
            None
        };
        let kind = edited.kind.clone();
        let color = edited.color;
        self.rewrite(place, move |dictionary| {
            dictionary.set("Rect", rect);
            match kind {
                Kind::Text {
                    contents, size_pt, ..
                } => {
                    if appearance.is_some() {
                        dictionary.set("Contents", text_string(&contents));
                        dictionary.set("DA", default_appearance(size_pt, color));
                    }
                }
                Kind::Highlight { quads } => {
                    dictionary.set("QuadPoints", quad_points(&quads));
                    dictionary.set("C", color_object(color));
                }
                Kind::Ink { strokes, .. } => {
                    dictionary.set("InkList", ink_list(&strokes));
                    dictionary.set("C", color_object(color));
                }
                Kind::Other { .. } => {}
            }
            if let Some(appearance) = appearance {
                dictionary.set("AP", dictionary! { "N" => Object::Reference(appearance) });
            }
        })
    }

    /// Takes each of `ids` out of its page's `/Annots`, with every reply to
    /// it (an annotation whose `/IRT` names it, and the replies to those,
    /// on any page) and the pop-up each of them names; the objects
    /// themselves stay in the file, unreferenced.
    fn delete(
        &mut self,
        ids: &[AnnotId],
        places: &HashMap<AnnotId, Location>,
    ) -> Result<(), AnnotError> {
        let mut doomed: HashSet<(ObjectId, u32)> = HashSet::new();
        let mut doomed_objects: HashSet<ObjectId> = HashSet::new();
        for id in ids {
            let place = places.get(id).ok_or(AnnotError::NotEditable)?;
            doomed.insert((place.page, place.index));
            doomed_objects.extend(place.object);
        }
        let entries = annots_entries(self.document.get_prev_documents());
        // Replies to what goes, until no more are found (each pass takes at
        // least one entry, so it ends).
        loop {
            let replies: Vec<&AnnotsEntry> = entries
                .iter()
                .filter(|entry| !doomed.contains(&(entry.page, entry.index)))
                .filter(|entry| {
                    entry
                        .in_reply_to
                        .is_some_and(|parent| doomed_objects.contains(&parent))
                })
                .collect();
            if replies.is_empty() {
                break;
            }
            for reply in replies {
                doomed.insert((reply.page, reply.index));
                doomed_objects.extend(reply.object);
            }
        }
        let popups: HashSet<ObjectId> = entries
            .iter()
            .filter(|entry| doomed.contains(&(entry.page, entry.index)))
            .filter_map(|entry| entry.popup)
            .collect();
        let mut by_page: HashMap<ObjectId, Vec<u32>> = HashMap::new();
        for (page, index) in &doomed {
            by_page.entry(*page).or_default().push(*index);
        }
        let pages_with_popups: HashSet<ObjectId> = entries
            .iter()
            .filter(|entry| entry.object.is_some_and(|object| popups.contains(&object)))
            .map(|entry| entry.page)
            .collect();
        for page in pages_with_popups {
            by_page.entry(page).or_default();
        }
        for (page, mut indexes) in by_page {
            // Removed from the end, so the earlier places stay where they are.
            indexes.sort_by_key(|index| std::cmp::Reverse(*index));
            self.with_annots(page, |array| {
                for index in &indexes {
                    if let Ok(index) = usize::try_from(*index) {
                        if index < array.len() {
                            array.remove(index);
                        }
                    }
                }
                array.retain(|entry| {
                    entry
                        .as_reference()
                        .map_or(true, |reference| !popups.contains(&reference))
                });
            })?;
        }
        Ok(())
    }

    /// Adds `annotation`, one Calcita made, to the page `page_id`.
    fn add(
        &mut self,
        annotation: &Annotation,
        page_id: ObjectId,
        page: PageBox,
    ) -> Result<(), AnnotError> {
        let appearance = self.appearance_of(annotation, page)?;
        let mut dictionary = dictionary! {
            "Type" => "Annot",
            "Rect" => rect_object(annotation.rect),
            "F" => FLAG_PRINT,
            "P" => Object::Reference(page_id),
            "AP" => dictionary! { "N" => Object::Reference(appearance) },
        };
        if let AnnotId::Name(name) = &annotation.id {
            dictionary.set("NM", text_string(name));
        }
        match &annotation.kind {
            Kind::Text {
                contents, size_pt, ..
            } => {
                dictionary.set("Subtype", "FreeText");
                dictionary.set("Contents", text_string(contents));
                dictionary.set("DA", default_appearance(*size_pt, annotation.color));
                // No border: a text box is its text.
                dictionary.set("BS", dictionary! { "W" => 0 });
            }
            Kind::Highlight { quads } => {
                dictionary.set("Subtype", "Highlight");
                dictionary.set("QuadPoints", quad_points(quads));
                dictionary.set("C", color_object(annotation.color));
            }
            Kind::Ink { strokes, width } => {
                dictionary.set("Subtype", "Ink");
                dictionary.set("InkList", ink_list(strokes));
                dictionary.set("C", color_object(annotation.color));
                dictionary.set("BS", dictionary! { "W" => real(*width) });
            }
            Kind::Other { .. } => return Err(AnnotError::NotEditable),
        }
        let id = self.document.new_document.add_object(dictionary);
        self.with_annots(page_id, |array| array.push(Object::Reference(id)))
    }
}

/// One entry of a page's `/Annots`, as `delete` follows replies.
struct AnnotsEntry {
    page: ObjectId,
    index: u32,
    object: Option<ObjectId>,
    /// `/IRT`: the annotation this one replies to.
    in_reply_to: Option<ObjectId>,
    popup: Option<ObjectId>,
}

/// Every entry of every page's `/Annots` that is a dictionary.
fn annots_entries(document: &lopdf::Document) -> Vec<AnnotsEntry> {
    let reference = |dictionary: &Dictionary, key: &[u8]| {
        dictionary
            .get(key)
            .ok()
            .and_then(|value| value.as_reference().ok())
    };
    pages(document)
        .into_iter()
        .flat_map(|(_, page)| {
            page_annotations(document, page)
                .into_iter()
                .map(move |(index, object, dictionary)| AnnotsEntry {
                    page,
                    index,
                    object,
                    in_reply_to: reference(dictionary, b"IRT"),
                    popup: reference(dictionary, b"Popup"),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn quad_points(quads: &[super::model::Quad]) -> Object {
    Object::Array(
        quads
            .iter()
            .flat_map(|quad| quad.points.iter())
            .flat_map(|point| [real(point.x), real(point.y)])
            .collect(),
    )
}

fn ink_list(strokes: &[Vec<super::model::Point>]) -> Object {
    Object::Array(
        strokes
            .iter()
            .map(|stroke| {
                Object::Array(
                    stroke
                        .iter()
                        .flat_map(|point| [real(point.x), real(point.y)])
                        .collect(),
                )
            })
            .collect(),
    )
}
