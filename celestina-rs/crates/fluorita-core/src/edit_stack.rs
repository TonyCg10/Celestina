//! The composable edit: what was done to a picture, in what order, and how to
//! undo it.
//!
//! A document is a base canvas plus a history of steps. Two rules give it its
//! shape, and both exist because of a failure they prevent:
//!
//! - **Canvas transformations come first, annotations live in the resulting
//!   canvas.** Applying a transformation therefore carries every existing
//!   object through it, so a word written on a face stays on that face when
//!   the picture is later cropped or turned. A model that kept annotations in
//!   the coordinates they were drawn in would move them off their subject the
//!   moment the canvas changed.
//! - **Coordinates are image coordinates**, in pixels of the current canvas,
//!   never in pixels of a window. On a scaled display a stroke stored in
//!   window pixels lands where it was not drawn, and no test that runs
//!   offscreen would ever see it.
//!
//! Nothing here rasterises. [`EditDocument::composition`] hands the engine the
//! canvas and the objects to draw; whether that becomes new bytes or a
//! rewritten orientation header is decided with
//! [`EditDocument::orientation_only`] and the capability matrix in
//! [`crate::edit`].

use crate::edit::{EditCapabilities, EditClass, Operation};

/// The dimensions of a canvas, in pixels. Never zero in either axis.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Canvas {
    width: u32,
    height: u32,
}

impl Canvas {
    /// Builds a canvas, refusing an empty one.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Option<Self> {
        if width == 0 || height == 0 {
            None
        } else {
            Some(Self { width, height })
        }
    }

    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }

    /// The allocation this canvas implies, as a count that cannot overflow.
    #[must_use]
    pub const fn pixels(self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

/// A point in canvas pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

/// An axis-aligned rectangle in canvas pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub origin: Point,
    pub width: f32,
    pub height: f32,
}

impl Area {
    #[must_use]
    pub const fn new(origin: Point, width: f32, height: f32) -> Self {
        Self {
            origin,
            width,
            height,
        }
    }

    fn is_valid(self) -> bool {
        self.origin.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }

    fn far(self) -> Point {
        Point::new(self.origin.x + self.width, self.origin.y + self.height)
    }

    fn from_corners(first: Point, second: Point) -> Self {
        let x = first.x.min(second.x);
        let y = first.y.min(second.y);
        Self {
            origin: Point::new(x, y),
            width: (first.x - second.x).abs(),
            height: (first.y - second.y).abs(),
        }
    }

    fn intersects(self, canvas: Canvas) -> bool {
        let far = self.far();
        far.x > 0.0
            && far.y > 0.0
            && self.origin.x < canvas.width() as f32
            && self.origin.y < canvas.height() as f32
    }

    fn within(self, canvas: Canvas) -> bool {
        let far = self.far();
        self.origin.x >= 0.0
            && self.origin.y >= 0.0
            && far.x <= canvas.width() as f32
            && far.y <= canvas.height() as f32
    }
}

/// A quarter turn of the whole canvas.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Quarter {
    Clockwise,
    Half,
    CounterClockwise,
}

impl Quarter {
    const fn quarters(self) -> u8 {
        match self {
            Self::Clockwise => 1,
            Self::Half => 2,
            Self::CounterClockwise => 3,
        }
    }
}

/// The axis a mirror is taken across.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Axis {
    /// Left becomes right.
    Horizontal,
    /// Top becomes bottom.
    Vertical,
}

/// A change to the canvas itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transform {
    Rotate(Quarter),
    Flip(Axis),
    /// Keep this area and discard the rest.
    Crop(Area),
    /// Rescale the whole canvas to these dimensions.
    Resize(Canvas),
}

impl Transform {
    /// The operation a surface would name this transform, so the capability
    /// matrix can be asked about it.
    #[must_use]
    pub const fn operation(self) -> Operation {
        match self {
            Self::Rotate(_) => Operation::Rotate,
            Self::Flip(_) => Operation::Flip,
            Self::Crop(_) => Operation::Crop,
            Self::Resize(_) => Operation::Resize,
        }
    }

    /// The canvas this transform produces from `from`.
    fn canvas(self, from: Canvas) -> Option<Canvas> {
        match self {
            Self::Rotate(Quarter::Clockwise | Quarter::CounterClockwise) => {
                Canvas::new(from.height(), from.width())
            }
            Self::Rotate(Quarter::Half) | Self::Flip(_) => Some(from),
            Self::Crop(area) => {
                Canvas::new(round_to_pixels(area.width), round_to_pixels(area.height))
            }
            Self::Resize(canvas) => Some(canvas),
        }
    }

    /// Carries one point from `from`'s coordinates into the new canvas.
    fn map(self, from: Canvas, point: Point) -> Point {
        let width = from.width() as f32;
        let height = from.height() as f32;
        match self {
            Self::Rotate(Quarter::Clockwise) => Point::new(height - point.y, point.x),
            Self::Rotate(Quarter::Half) => Point::new(width - point.x, height - point.y),
            Self::Rotate(Quarter::CounterClockwise) => Point::new(point.y, width - point.x),
            Self::Flip(Axis::Horizontal) => Point::new(width - point.x, point.y),
            Self::Flip(Axis::Vertical) => Point::new(point.x, height - point.y),
            Self::Crop(area) => Point::new(point.x - area.origin.x, point.y - area.origin.y),
            Self::Resize(to) => Point::new(
                point.x * to.width() as f32 / width,
                point.y * to.height() as f32 / height,
            ),
        }
    }

    /// Carries one point back out of the new canvas into `from`'s
    /// coordinates. Undo is exactly this, which is why no step needs to store
    /// the geometry it moved.
    fn unmap(self, from: Canvas, point: Point) -> Point {
        let width = from.width() as f32;
        let height = from.height() as f32;
        match self {
            Self::Rotate(Quarter::Clockwise) => Point::new(point.y, height - point.x),
            Self::Rotate(Quarter::Half) => Point::new(width - point.x, height - point.y),
            Self::Rotate(Quarter::CounterClockwise) => Point::new(width - point.y, point.x),
            Self::Flip(Axis::Horizontal) => Point::new(width - point.x, point.y),
            Self::Flip(Axis::Vertical) => Point::new(point.x, height - point.y),
            Self::Crop(area) => Point::new(point.x + area.origin.x, point.y + area.origin.y),
            Self::Resize(to) => Point::new(
                point.x * width / to.width() as f32,
                point.y * height / to.height() as f32,
            ),
        }
    }

    /// How lengths — a stroke's width, a text's size — change under this
    /// transform. Only a resize changes them.
    fn length_factor(self, from: Canvas) -> f32 {
        match self {
            Self::Resize(to) => {
                let horizontal = to.width() as f32 / from.width() as f32;
                let vertical = to.height() as f32 / from.height() as f32;
                (horizontal * vertical).sqrt()
            }
            Self::Rotate(_) | Self::Flip(_) | Self::Crop(_) => 1.0,
        }
    }

    /// The quarter turns text accumulates, so a word rotates with the picture
    /// it was written on.
    const fn text_quarters(self) -> u8 {
        match self {
            Self::Rotate(quarter) => quarter.quarters(),
            Self::Flip(_) | Self::Crop(_) | Self::Resize(_) => 0,
        }
    }
}

fn round_to_pixels(value: f32) -> u32 {
    if value.is_finite() && value > 0.0 {
        // `as` saturates at u32::MAX for anything larger, which the pixel
        // budget then refuses; it never wraps.
        value.round() as u32
    } else {
        0
    }
}

/// A colour, as the surface resolved it.
///
/// The values come from `CelestinaTheme` tokens; this crate never names a
/// colour of its own, it only carries the one the surface chose so the
/// renderer draws what the person picked.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Ink {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Ink {
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

/// How a redaction hides what is under it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Redaction {
    /// Replace the area with large blocks of its own average colour.
    Pixelate,
    /// Blur the area beyond recognition.
    Blur,
}

/// The outline of a closed shape.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
}

/// One thing placed on the canvas.
///
/// Every variant is geometry plus appearance, and every variant is carried
/// through a canvas transformation rather than re-anchored afterwards.
#[derive(Clone, Debug, PartialEq)]
pub enum Annotation {
    Text {
        area: Area,
        text: String,
        size: f32,
        ink: Ink,
        /// A plate behind the glyphs, for light photographs.
        backdrop: Option<Ink>,
        /// Quarter turns accumulated from canvas rotations, so the word turns
        /// with the picture.
        quarters: u8,
    },
    /// A freehand stroke.
    Stroke {
        points: Vec<Point>,
        width: f32,
        ink: Ink,
    },
    /// A straight line, optionally with an arrow head at `to`.
    Line {
        from: Point,
        to: Point,
        width: f32,
        ink: Ink,
        arrow: bool,
    },
    Shape {
        kind: ShapeKind,
        area: Area,
        width: f32,
        ink: Ink,
        fill: Option<Ink>,
    },
    /// A translucent wash, drawn under nothing and over everything it covers.
    Highlight { area: Area, ink: Ink },
    /// The one annotation whose purpose is that what is underneath cannot be
    /// recovered from the result.
    Redact { area: Area, style: Redaction },
}

impl Annotation {
    fn map(&mut self, transform: Transform, from: Canvas) {
        let factor = transform.length_factor(from);
        match self {
            Self::Text {
                area,
                size,
                quarters,
                ..
            } => {
                map_area(area, transform, from);
                *size *= factor;
                *quarters = (*quarters + transform.text_quarters()) % 4;
            }
            Self::Stroke { points, width, .. } => {
                for point in points.iter_mut() {
                    *point = transform.map(from, *point);
                }
                *width *= factor;
            }
            Self::Line {
                from: start,
                to,
                width,
                ..
            } => {
                *start = transform.map(from, *start);
                *to = transform.map(from, *to);
                *width *= factor;
            }
            Self::Shape { area, width, .. } => {
                map_area(area, transform, from);
                *width *= factor;
            }
            Self::Highlight { area, .. } | Self::Redact { area, .. } => {
                map_area(area, transform, from);
            }
        }
    }

    fn unmap(&mut self, transform: Transform, from: Canvas) {
        let factor = transform.length_factor(from);
        match self {
            Self::Text {
                area,
                size,
                quarters,
                ..
            } => {
                unmap_area(area, transform, from);
                *size /= factor;
                *quarters = (*quarters + 4 - transform.text_quarters()) % 4;
            }
            Self::Stroke { points, width, .. } => {
                for point in points.iter_mut() {
                    *point = transform.unmap(from, *point);
                }
                *width /= factor;
            }
            Self::Line {
                from: start,
                to,
                width,
                ..
            } => {
                *start = transform.unmap(from, *start);
                *to = transform.unmap(from, *to);
                *width /= factor;
            }
            Self::Shape { area, width, .. } => {
                unmap_area(area, transform, from);
                *width /= factor;
            }
            Self::Highlight { area, .. } | Self::Redact { area, .. } => {
                unmap_area(area, transform, from);
            }
        }
    }

    fn bounds(&self) -> Area {
        match self {
            Self::Text { area, .. }
            | Self::Shape { area, .. }
            | Self::Highlight { area, .. }
            | Self::Redact { area, .. } => *area,
            Self::Line { from, to, .. } => Area::from_corners(*from, *to),
            Self::Stroke { points, .. } => {
                let mut area = Area::new(Point::new(0.0, 0.0), 0.0, 0.0);
                if let Some(first) = points.first() {
                    let mut low = *first;
                    let mut high = *first;
                    for point in points {
                        low = Point::new(low.x.min(point.x), low.y.min(point.y));
                        high = Point::new(high.x.max(point.x), high.y.max(point.y));
                    }
                    area = Area::from_corners(low, high);
                }
                area
            }
        }
    }

    fn validate(&self, limits: &EditLimits, canvas: Canvas) -> Result<(), EditRejected> {
        match self {
            Self::Text { text, size, .. } => {
                if text.chars().count() > limits.max_text_characters {
                    return Err(EditRejected::TextTooLong);
                }
                if !size.is_finite() || *size <= 0.0 {
                    return Err(EditRejected::InvalidGeometry);
                }
            }
            Self::Stroke { points, width, .. } => {
                if points.len() < 2 {
                    return Err(EditRejected::InvalidGeometry);
                }
                if points.len() > limits.max_points_per_stroke {
                    return Err(EditRejected::StrokeTooLong);
                }
                if !points.iter().all(|point| point.is_finite()) || !width.is_finite() {
                    return Err(EditRejected::InvalidGeometry);
                }
            }
            Self::Line {
                from, to, width, ..
            } => {
                if !from.is_finite() || !to.is_finite() || !width.is_finite() {
                    return Err(EditRejected::InvalidGeometry);
                }
            }
            Self::Shape { area, width, .. } => {
                if !area.is_valid() || !width.is_finite() {
                    return Err(EditRejected::InvalidGeometry);
                }
            }
            Self::Highlight { area, .. } | Self::Redact { area, .. } => {
                if !area.is_valid() {
                    return Err(EditRejected::InvalidGeometry);
                }
            }
        }
        let bounds = self.bounds();
        if !bounds.is_valid() && !matches!(self, Self::Line { .. }) {
            return Err(EditRejected::InvalidGeometry);
        }
        if !bounds.intersects(canvas) {
            return Err(EditRejected::OutsideCanvas);
        }
        Ok(())
    }
}

fn map_area(area: &mut Area, transform: Transform, from: Canvas) {
    let near = transform.map(from, area.origin);
    let far = transform.map(from, area.far());
    *area = Area::from_corners(near, far);
}

fn unmap_area(area: &mut Area, transform: Transform, from: Canvas) {
    let near = transform.unmap(from, area.origin);
    let far = transform.unmap(from, area.far());
    *area = Area::from_corners(near, far);
}

/// A stable handle for one placed annotation, so a surface can move, resize,
/// replace or delete it without depending on its position in a list.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObjectId(u64);

impl ObjectId {
    /// The raw value, for a surface that has to carry it across a seam.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// The handle a surface just sent back.
    ///
    /// Rebuilding one is safe because it is not authority: every verb that
    /// takes an `ObjectId` looks it up on the canvas and answers
    /// [`EditRejected::UnknownObject`] when it is not there. A host cannot
    /// reach an object by guessing a number.
    #[must_use]
    pub const fn from_value(value: u64) -> Self {
        Self(value)
    }
}

/// The ceilings an edit may not cross.
///
/// The canvas budget is passed in rather than declared here: the application
/// already owns the pixel ceiling it refuses to *view* a picture at, and the
/// edit path must refuse at exactly the same number instead of keeping a
/// second opinion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EditLimits {
    pub max_canvas_pixels: u64,
    pub max_objects: usize,
    pub max_points_per_stroke: usize,
    pub max_text_characters: usize,
    pub max_steps: usize,
}

impl EditLimits {
    /// Limits for a host whose viewing budget is `max_canvas_pixels`.
    #[must_use]
    pub const fn new(max_canvas_pixels: u64) -> Self {
        Self {
            max_canvas_pixels,
            max_objects: 512,
            max_points_per_stroke: 4096,
            max_text_characters: 2048,
            max_steps: 4096,
        }
    }
}

/// Why an edit was refused. Every variant is a refusal the surface can state;
/// none of them is a failure it should retry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditRejected {
    /// The item's kind or format admits no editing at all.
    NotEditable,
    /// The item admits editing, but not this operation.
    OperationNotAdmitted(Operation),
    /// A coordinate, length or size that is negative, zero where it may not
    /// be, or not a finite number.
    InvalidGeometry,
    /// A crop that leaves the canvas, or an annotation entirely off it.
    OutsideCanvas,
    /// The resulting canvas would cross the host's pixel budget.
    CanvasTooLarge { pixels: u64 },
    /// The canvas would have no area left.
    EmptyCanvas,
    /// More objects than the document may hold.
    TooManyObjects,
    /// A single stroke with more points than the document may hold.
    StrokeTooLong,
    /// Text longer than the document may hold.
    TextTooLong,
    /// More steps than the history may hold.
    HistoryFull,
    /// No object with that handle is on the canvas.
    UnknownObject,
}

/// One entry in the history. Each carries what undo needs, which is why undo
/// is exact rather than a re-run of everything that came before.
#[derive(Clone, Debug, PartialEq)]
enum Step {
    Transform {
        transform: Transform,
        previous_canvas: Canvas,
    },
    Add {
        id: ObjectId,
        index: usize,
        annotation: Box<Annotation>,
    },
    Update {
        id: ObjectId,
        before: Box<Annotation>,
        after: Box<Annotation>,
    },
    Remove {
        id: ObjectId,
        index: usize,
        annotation: Box<Annotation>,
    },
}

/// A picture and everything that has been done to it.
#[derive(Clone, Debug)]
pub struct EditDocument {
    base: Canvas,
    canvas: Canvas,
    objects: Vec<(ObjectId, Annotation)>,
    history: Vec<Step>,
    undone: Vec<Step>,
    next_id: u64,
    limits: EditLimits,
}

impl EditDocument {
    /// Opens a document over a picture of these dimensions.
    #[must_use]
    pub fn new(base: Canvas, limits: EditLimits) -> Self {
        Self {
            base,
            canvas: base,
            objects: Vec::new(),
            history: Vec::new(),
            undone: Vec::new(),
            next_id: 1,
            limits,
        }
    }

    /// The dimensions the picture arrived with.
    #[must_use]
    pub const fn base(&self) -> Canvas {
        self.base
    }

    /// The dimensions the picture currently has.
    #[must_use]
    pub const fn canvas(&self) -> Canvas {
        self.canvas
    }

    /// Whether anything has been done that a save would write.
    #[must_use]
    pub fn is_edited(&self) -> bool {
        !self.history.is_empty()
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.history.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.undone.is_empty()
    }

    /// The objects on the canvas, in the order they are drawn.
    #[must_use]
    pub fn objects(&self) -> &[(ObjectId, Annotation)] {
        &self.objects
    }

    /// Applies a canvas transformation, carrying every object through it.
    ///
    /// # Errors
    ///
    /// Refuses a crop that leaves the canvas, a canvas with no area, one past
    /// the host's pixel budget, an operation the item does not admit, and a
    /// history that is already full.
    pub fn transform(
        &mut self,
        transform: Transform,
        capabilities: &EditCapabilities,
    ) -> Result<(), EditRejected> {
        self.admit(transform.operation(), capabilities)?;
        self.room_for_one_more_step()?;

        if let Transform::Crop(area) = transform {
            if !area.is_valid() {
                return Err(EditRejected::InvalidGeometry);
            }
            if !area.within(self.canvas) {
                return Err(EditRejected::OutsideCanvas);
            }
        }
        let next = transform
            .canvas(self.canvas)
            .ok_or(EditRejected::EmptyCanvas)?;
        if next.pixels() > self.limits.max_canvas_pixels {
            return Err(EditRejected::CanvasTooLarge {
                pixels: next.pixels(),
            });
        }

        let previous_canvas = self.canvas;
        for (_, annotation) in &mut self.objects {
            annotation.map(transform, previous_canvas);
        }
        self.canvas = next;
        self.history.push(Step::Transform {
            transform,
            previous_canvas,
        });
        self.undone.clear();
        Ok(())
    }

    /// Places an annotation on the canvas.
    ///
    /// # Errors
    ///
    /// Refuses invalid geometry, an object entirely off the canvas, one past
    /// the object or stroke or text ceilings, an item that does not admit
    /// annotation, and a history that is already full.
    pub fn annotate(
        &mut self,
        annotation: Annotation,
        capabilities: &EditCapabilities,
    ) -> Result<ObjectId, EditRejected> {
        self.admit(Operation::Annotate, capabilities)?;
        self.room_for_one_more_step()?;
        if self.objects.len() >= self.limits.max_objects {
            return Err(EditRejected::TooManyObjects);
        }
        annotation.validate(&self.limits, self.canvas)?;

        let id = ObjectId(self.next_id);
        self.next_id += 1;
        let index = self.objects.len();
        self.objects.push((id, annotation.clone()));
        self.history.push(Step::Add {
            id,
            index,
            annotation: Box::new(annotation),
        });
        self.undone.clear();
        Ok(id)
    }

    /// Replaces one object with a new version of itself — the model behind
    /// moving, resizing and retyping.
    ///
    /// # Errors
    ///
    /// Refuses an unknown handle, invalid geometry and an object moved
    /// entirely off the canvas.
    pub fn update(&mut self, id: ObjectId, annotation: Annotation) -> Result<(), EditRejected> {
        self.room_for_one_more_step()?;
        annotation.validate(&self.limits, self.canvas)?;
        let slot = self
            .objects
            .iter_mut()
            .find(|(candidate, _)| *candidate == id)
            .ok_or(EditRejected::UnknownObject)?;
        let before = std::mem::replace(&mut slot.1, annotation.clone());
        self.history.push(Step::Update {
            id,
            before: Box::new(before),
            after: Box::new(annotation),
        });
        self.undone.clear();
        Ok(())
    }

    /// Takes one object off the canvas.
    ///
    /// # Errors
    ///
    /// Refuses an unknown handle and a history that is already full.
    pub fn remove(&mut self, id: ObjectId) -> Result<(), EditRejected> {
        self.room_for_one_more_step()?;
        let index = self
            .objects
            .iter()
            .position(|(candidate, _)| *candidate == id)
            .ok_or(EditRejected::UnknownObject)?;
        let (_, annotation) = self.objects.remove(index);
        self.history.push(Step::Remove {
            id,
            index,
            annotation: Box::new(annotation),
        });
        self.undone.clear();
        Ok(())
    }

    /// Reverses the last step. Returns whether there was one.
    pub fn undo(&mut self) -> bool {
        let Some(step) = self.history.pop() else {
            return false;
        };
        self.reverse(&step);
        self.undone.push(step);
        true
    }

    /// Reapplies the last undone step. Returns whether there was one.
    pub fn redo(&mut self) -> bool {
        let Some(step) = self.undone.pop() else {
            return false;
        };
        self.forward(&step);
        self.history.push(step);
        true
    }

    /// The orientation change this document amounts to, when it amounts to
    /// nothing else.
    ///
    /// `Some` means every step is a rotation or a mirror and no object was
    /// placed, so a format that records orientation as metadata can carry this
    /// edit without a pixel being rewritten. Any crop, resize or annotation
    /// makes it `None`, because those cannot be described in a header.
    #[must_use]
    pub fn orientation_only(&self) -> Option<Orientation> {
        if !self.objects.is_empty() || self.history.is_empty() {
            return None;
        }
        let mut orientation = Orientation {
            quarters: 0,
            mirrored: false,
        };
        for step in &self.history {
            match step {
                Step::Transform { transform, .. } => match transform {
                    Transform::Rotate(quarter) => {
                        orientation = orientation.then(Orientation {
                            quarters: quarter.quarters(),
                            mirrored: false,
                        });
                    }
                    // A vertical mirror is a horizontal one with the picture
                    // turned half way round; the tag has no separate value for
                    // it, and neither does this.
                    Transform::Flip(axis) => {
                        orientation = orientation.then(Orientation {
                            quarters: match axis {
                                Axis::Horizontal => 0,
                                Axis::Vertical => 2,
                            },
                            mirrored: true,
                        });
                    }
                    Transform::Crop(_) | Transform::Resize(_) => return None,
                },
                Step::Add { .. } | Step::Update { .. } | Step::Remove { .. } => return None,
            }
        }
        Some(orientation)
    }

    /// Which side of the contract saving this document falls on.
    ///
    /// [`EditClass::Lossless`] only when the whole document is an orientation
    /// change *and* the file's format records orientation. Everything else
    /// produces a new image, and the surface must say so.
    #[must_use]
    pub fn class(&self, capabilities: &EditCapabilities) -> EditClass {
        let orientation_is_free = capabilities
            .admits(Operation::Rotate)
            .is_some_and(EditClass::preserves_original_bytes);
        match self.orientation_only() {
            Some(orientation) if orientation_is_free && !orientation.is_identity() => {
                EditClass::Lossless
            }
            _ => EditClass::Raster,
        }
    }

    /// What the engine has to draw, from the picture on disk: the canvas
    /// transformations in the order they were applied, the canvas they end at,
    /// and the objects to place on it, already in that canvas's coordinates.
    ///
    /// The transformations travel with it because the renderer starts from the
    /// original file every time. A composition that carried only the final
    /// canvas would tell it the size of a picture it has no way to produce.
    #[must_use]
    pub fn composition(&self) -> Composition {
        Composition {
            canvas: self.canvas,
            transforms: self
                .history
                .iter()
                .filter_map(|step| match step {
                    Step::Transform { transform, .. } => Some(*transform),
                    Step::Add { .. } | Step::Update { .. } | Step::Remove { .. } => None,
                })
                .collect(),
            objects: self
                .objects
                .iter()
                .map(|(_, object)| object)
                .cloned()
                .collect(),
        }
    }

    /// What a live preview has to draw: which part of the picture on disk is
    /// still visible, and how it is turned.
    ///
    /// A surface cannot rebuild this from the transformation list without
    /// reimplementing the mapping, and two implementations of that mapping is
    /// exactly how a preview comes to disagree with the file that gets
    /// written. Walking the stack backwards from the current canvas is the
    /// same arithmetic undo uses.
    #[must_use]
    pub fn preview(&self) -> Preview {
        let mut near = Point::new(0.0, 0.0);
        let mut far = Point::new(self.canvas.width() as f32, self.canvas.height() as f32);
        let mut orientation = Orientation {
            quarters: 0,
            mirrored: false,
        };
        for step in self.history.iter().rev() {
            if let Step::Transform {
                transform,
                previous_canvas,
            } = step
            {
                near = transform.unmap(*previous_canvas, near);
                far = transform.unmap(*previous_canvas, far);
            }
        }
        for step in &self.history {
            if let Step::Transform { transform, .. } = step {
                match transform {
                    Transform::Rotate(quarter) => {
                        orientation = orientation.then(Orientation {
                            quarters: quarter.quarters(),
                            mirrored: false,
                        });
                    }
                    Transform::Flip(axis) => {
                        orientation = orientation.then(Orientation {
                            quarters: match axis {
                                Axis::Horizontal => 0,
                                Axis::Vertical => 2,
                            },
                            mirrored: true,
                        });
                    }
                    Transform::Crop(_) | Transform::Resize(_) => {}
                }
            }
        }
        Preview {
            source: Area::from_corners(near, far),
            orientation,
        }
    }

    /// The canvas transformations in force, in application order.
    #[must_use]
    pub fn transforms(&self) -> Vec<Transform> {
        self.composition().transforms
    }

    fn admit(
        &self,
        operation: Operation,
        capabilities: &EditCapabilities,
    ) -> Result<(), EditRejected> {
        if !capabilities.is_editable() {
            return Err(EditRejected::NotEditable);
        }
        capabilities
            .admits(operation)
            .map(|_| ())
            .ok_or(EditRejected::OperationNotAdmitted(operation))
    }

    fn room_for_one_more_step(&self) -> Result<(), EditRejected> {
        if self.history.len() >= self.limits.max_steps {
            Err(EditRejected::HistoryFull)
        } else {
            Ok(())
        }
    }

    fn reverse(&mut self, step: &Step) {
        match step {
            Step::Transform {
                transform,
                previous_canvas,
            } => {
                for (_, annotation) in &mut self.objects {
                    annotation.unmap(*transform, *previous_canvas);
                }
                self.canvas = *previous_canvas;
            }
            Step::Add { id, .. } => {
                self.objects.retain(|(candidate, _)| candidate != id);
            }
            Step::Update { id, before, .. } => {
                if let Some(slot) = self.objects.iter_mut().find(|(c, _)| c == id) {
                    slot.1 = (**before).clone();
                }
            }
            Step::Remove {
                id,
                index,
                annotation,
            } => {
                let index = (*index).min(self.objects.len());
                self.objects.insert(index, (*id, (**annotation).clone()));
            }
        }
    }

    fn forward(&mut self, step: &Step) {
        match step {
            Step::Transform {
                transform,
                previous_canvas,
            } => {
                for (_, annotation) in &mut self.objects {
                    annotation.map(*transform, *previous_canvas);
                }
                if let Some(next) = transform.canvas(*previous_canvas) {
                    self.canvas = next;
                }
            }
            Step::Add {
                id,
                index,
                annotation,
            } => {
                let index = (*index).min(self.objects.len());
                self.objects.insert(index, (*id, (**annotation).clone()));
            }
            Step::Update { id, after, .. } => {
                if let Some(slot) = self.objects.iter_mut().find(|(c, _)| c == id) {
                    slot.1 = (**after).clone();
                }
            }
            Step::Remove { id, .. } => {
                self.objects.retain(|(candidate, _)| candidate != id);
            }
        }
    }
}

/// An orientation change that a format's header can carry on its own.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Orientation {
    /// Clockwise quarter turns, 0 to 3.
    pub quarters: u8,
    /// Whether the picture is mirrored, applied before the turn.
    pub mirrored: bool,
}

impl Orientation {
    /// Whether this describes no change at all.
    #[must_use]
    pub const fn is_identity(self) -> bool {
        self.quarters == 0 && !self.mirrored
    }

    /// The EXIF orientation value that describes this, 1 to 8.
    ///
    /// The tag encodes "mirror, then turn clockwise", which is the same order
    /// this type stores, so the mapping is a table rather than a calculation.
    #[must_use]
    pub const fn to_exif(self) -> u16 {
        match (self.mirrored, self.quarters % 4) {
            (false, 0) => 1,
            (false, 1) => 6,
            (false, 2) => 3,
            (false, _) => 8,
            (true, 0) => 2,
            (true, 1) => 7,
            (true, 2) => 4,
            (true, _) => 5,
        }
    }

    /// Reads an EXIF orientation value. An unknown or absent value is
    /// [`Orientation::is_identity`], because a file that does not say is a
    /// file to be shown as it is.
    #[must_use]
    pub const fn from_exif(value: u16) -> Self {
        match value {
            2 => Self {
                quarters: 0,
                mirrored: true,
            },
            3 => Self {
                quarters: 2,
                mirrored: false,
            },
            4 => Self {
                quarters: 2,
                mirrored: true,
            },
            5 => Self {
                quarters: 3,
                mirrored: true,
            },
            6 => Self {
                quarters: 1,
                mirrored: false,
            },
            7 => Self {
                quarters: 1,
                mirrored: true,
            },
            8 => Self {
                quarters: 3,
                mirrored: false,
            },
            _ => Self {
                quarters: 0,
                mirrored: false,
            },
        }
    }

    /// The orientation of a picture that already carried `self` and is then
    /// turned by `next`.
    ///
    /// Mirroring reverses the direction a later turn goes in — a mirrored
    /// picture turned clockwise on screen is turned anticlockwise in its own
    /// frame — which is the whole reason this is a method and not an addition.
    #[must_use]
    pub const fn then(self, next: Self) -> Self {
        let carried = if next.mirrored {
            4 - self.quarters % 4
        } else {
            self.quarters % 4
        };
        Self {
            quarters: (next.quarters + carried) % 4,
            mirrored: self.mirrored != next.mirrored,
        }
    }
}

/// What a live preview draws: the part of the picture on disk that survives
/// the current stack, and the orientation to draw it in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preview {
    /// In the coordinates of the picture as it is read from disk.
    pub source: Area,
    /// Applied after the crop, around the centre.
    pub orientation: Orientation,
}

/// Everything the engine needs to draw the result, and nothing else.
#[derive(Clone, Debug, PartialEq)]
pub struct Composition {
    /// The dimensions the result has.
    pub canvas: Canvas,
    /// Applied to the picture as read from disk, in this order, before
    /// anything is drawn on it.
    pub transforms: Vec<Transform>,
    /// Drawn in this order, in the coordinates of `canvas`.
    pub objects: Vec<Annotation>,
}

#[cfg(test)]
mod tests {
    use super::{
        Annotation, Area, Axis, Canvas, EditDocument, EditLimits, EditRejected, Ink, ObjectId,
        Point, Quarter, Redaction, ShapeKind, Transform,
    };
    use crate::edit::{EditCapabilities, EditClass, Operation};
    use crate::media::MediaKind;
    use std::path::Path;

    const BUDGET: u64 = 100_000_000;

    fn photograph() -> EditCapabilities {
        EditCapabilities::of(MediaKind::Image, Path::new("/m/DSC_0001.jpg"))
    }

    fn screenshot() -> EditCapabilities {
        EditCapabilities::of(MediaKind::Image, Path::new("/m/captura.png"))
    }

    fn canvas(width: u32, height: u32) -> Canvas {
        Canvas::new(width, height).expect("a canvas with area")
    }

    fn document() -> EditDocument {
        EditDocument::new(canvas(4000, 3000), EditLimits::new(BUDGET))
    }

    fn ink() -> Ink {
        Ink::new(255, 255, 255, 255)
    }

    fn box_at(x: f32, y: f32, width: f32, height: f32) -> Area {
        Area::new(Point::new(x, y), width, height)
    }

    fn redaction(area: Area) -> Annotation {
        Annotation::Redact {
            area,
            style: Redaction::Pixelate,
        }
    }

    fn text(area: Area) -> Annotation {
        Annotation::Text {
            area,
            text: "Note".to_owned(),
            size: 48.0,
            ink: ink(),
            backdrop: None,
            quarters: 0,
        }
    }

    fn stroke(points: Vec<Point>) -> Annotation {
        Annotation::Stroke {
            points,
            width: 8.0,
            ink: ink(),
        }
    }

    fn area_of(document: &EditDocument, id: ObjectId) -> Area {
        let (_, annotation) = document
            .objects()
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .expect("the object is on the canvas");
        match annotation {
            Annotation::Text { area, .. }
            | Annotation::Shape { area, .. }
            | Annotation::Highlight { area, .. }
            | Annotation::Redact { area, .. } => *area,
            other => panic!("not a boxed annotation: {other:?}"),
        }
    }

    fn area_of_ids(document: &EditDocument) -> Vec<ObjectId> {
        document.objects().iter().map(|(id, _)| *id).collect()
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.01
    }

    fn same_area(left: Area, right: Area) -> bool {
        close(left.origin.x, right.origin.x)
            && close(left.origin.y, right.origin.y)
            && close(left.width, right.width)
            && close(left.height, right.height)
    }

    #[test]
    fn a_redaction_stays_on_what_it_covers_when_the_picture_is_cropped() {
        let mut document = document();
        let plate = box_at(1200.0, 700.0, 400.0, 120.0);
        let id = document
            .annotate(redaction(plate), &photograph())
            .expect("the redaction is placed");

        document
            .transform(
                Transform::Crop(box_at(1000.0, 500.0, 2000.0, 1500.0)),
                &photograph(),
            )
            .expect("the crop is inside the canvas");

        assert_eq!(document.canvas(), canvas(2000, 1500));
        assert!(
            same_area(area_of(&document, id), box_at(200.0, 200.0, 400.0, 120.0)),
            "the covered thing moved with the crop, not away from it"
        );
    }

    #[test]
    fn turning_the_canvas_turns_what_was_written_on_it_and_undo_puts_it_back() {
        let mut document = document();
        let written = box_at(100.0, 200.0, 300.0, 60.0);
        let id = document
            .annotate(text(written), &photograph())
            .expect("placed");

        document
            .transform(Transform::Rotate(Quarter::Clockwise), &photograph())
            .expect("a quarter turn");

        assert_eq!(document.canvas(), canvas(3000, 4000));
        assert!(
            same_area(
                area_of(&document, id),
                box_at(3000.0 - 260.0, 100.0, 60.0, 300.0)
            ),
            "the box turned with the canvas"
        );
        match document.objects().first().expect("one object") {
            (_, Annotation::Text { quarters, .. }) => {
                assert_eq!(*quarters, 1, "the word turns with the picture");
            }
            other => panic!("unexpected object: {other:?}"),
        }

        assert!(document.undo());
        assert_eq!(document.canvas(), canvas(4000, 3000));
        assert!(
            same_area(area_of(&document, id), written),
            "undo restores the coordinates it moved, exactly"
        );
        match document.objects().first().expect("one object") {
            (_, Annotation::Text { quarters, .. }) => assert_eq!(*quarters, 0),
            other => panic!("unexpected object: {other:?}"),
        }
    }

    #[test]
    fn resizing_scales_the_marks_and_undo_unscales_them() {
        let mut document = document();
        document
            .annotate(
                stroke(vec![Point::new(1000.0, 600.0), Point::new(2000.0, 1200.0)]),
                &photograph(),
            )
            .expect("placed");

        document
            .transform(Transform::Resize(canvas(2000, 1500)), &photograph())
            .expect("half size");

        match document.objects().first().expect("one object") {
            (_, Annotation::Stroke { points, width, .. }) => {
                assert!(close(points[0].x, 500.0) && close(points[0].y, 300.0));
                assert!(close(points[1].x, 1000.0) && close(points[1].y, 600.0));
                assert!(close(*width, 4.0), "a thinner picture gets a thinner line");
            }
            other => panic!("unexpected object: {other:?}"),
        }

        assert!(document.undo());
        match document.objects().first().expect("one object") {
            (_, Annotation::Stroke { points, width, .. }) => {
                assert!(close(points[0].x, 1000.0) && close(points[0].y, 600.0));
                assert!(close(*width, 8.0));
            }
            other => panic!("unexpected object: {other:?}"),
        }
        assert_eq!(area_of_ids(&document).len(), 1);
    }

    #[test]
    fn a_crop_that_leaves_the_canvas_is_refused_and_changes_nothing() {
        let mut document = document();
        let refused = document.transform(
            Transform::Crop(box_at(3800.0, 200.0, 400.0, 400.0)),
            &photograph(),
        );
        assert_eq!(refused, Err(EditRejected::OutsideCanvas));
        assert_eq!(document.canvas(), canvas(4000, 3000));
        assert!(!document.is_edited());
        assert!(!document.can_undo());
    }

    #[test]
    fn a_resize_past_the_hosts_viewing_budget_is_refused_before_anything_allocates() {
        let mut document = document();
        let refused = document.transform(Transform::Resize(canvas(20_000, 20_000)), &photograph());
        assert_eq!(
            refused,
            Err(EditRejected::CanvasTooLarge {
                pixels: 400_000_000
            })
        );
        assert_eq!(document.canvas(), canvas(4000, 3000));
    }

    #[test]
    fn a_crop_with_no_area_left_is_refused() {
        let mut document = document();
        assert_eq!(
            document.transform(
                Transform::Crop(box_at(10.0, 10.0, 0.0, 100.0)),
                &photograph()
            ),
            Err(EditRejected::InvalidGeometry)
        );
        assert_eq!(
            document.transform(
                Transform::Crop(box_at(10.0, 10.0, 0.4, 100.0)),
                &photograph()
            ),
            Err(EditRejected::EmptyCanvas),
            "an area thinner than a pixel is not a picture"
        );
    }

    #[test]
    fn an_object_entirely_off_the_canvas_is_refused_and_one_hanging_over_the_edge_is_not() {
        let mut document = document();
        assert_eq!(
            document.annotate(
                redaction(box_at(5000.0, 100.0, 200.0, 200.0)),
                &photograph()
            ),
            Err(EditRejected::OutsideCanvas)
        );
        assert!(document
            .annotate(
                redaction(box_at(3900.0, 100.0, 300.0, 200.0)),
                &photograph()
            )
            .is_ok());
    }

    #[test]
    fn geometry_that_is_not_a_number_is_refused() {
        let mut document = document();
        assert_eq!(
            document.annotate(redaction(box_at(f32::NAN, 0.0, 10.0, 10.0)), &photograph()),
            Err(EditRejected::InvalidGeometry)
        );
        assert_eq!(
            document.annotate(
                stroke(vec![Point::new(0.0, 0.0), Point::new(f32::INFINITY, 1.0)]),
                &photograph()
            ),
            Err(EditRejected::InvalidGeometry)
        );
    }

    #[test]
    fn the_ceilings_hold() {
        let mut limits = EditLimits::new(BUDGET);
        limits.max_objects = 2;
        limits.max_points_per_stroke = 3;
        limits.max_text_characters = 4;
        let mut document = EditDocument::new(canvas(4000, 3000), limits);

        assert_eq!(
            document.annotate(
                stroke(vec![
                    Point::new(0.0, 0.0),
                    Point::new(1.0, 1.0),
                    Point::new(2.0, 2.0),
                    Point::new(3.0, 3.0),
                ]),
                &photograph()
            ),
            Err(EditRejected::StrokeTooLong)
        );
        assert_eq!(
            document.annotate(
                Annotation::Text {
                    area: box_at(0.0, 0.0, 100.0, 50.0),
                    text: "demasiado".to_owned(),
                    size: 20.0,
                    ink: ink(),
                    backdrop: None,
                    quarters: 0,
                },
                &photograph()
            ),
            Err(EditRejected::TextTooLong)
        );

        for _ in 0..2 {
            document
                .annotate(redaction(box_at(0.0, 0.0, 10.0, 10.0)), &photograph())
                .expect("under the ceiling");
        }
        assert_eq!(
            document.annotate(redaction(box_at(0.0, 0.0, 10.0, 10.0)), &photograph()),
            Err(EditRejected::TooManyObjects)
        );
    }

    #[test]
    fn a_full_history_refuses_the_next_step_instead_of_forgetting_the_first() {
        let mut limits = EditLimits::new(BUDGET);
        limits.max_steps = 1;
        let mut document = EditDocument::new(canvas(400, 300), limits);
        document
            .annotate(redaction(box_at(0.0, 0.0, 10.0, 10.0)), &photograph())
            .expect("the first step fits");
        assert_eq!(
            document.transform(Transform::Rotate(Quarter::Half), &photograph()),
            Err(EditRejected::HistoryFull)
        );
        assert!(document.can_undo());
    }

    #[test]
    fn video_and_audio_refuse_every_edit() {
        let clip = EditCapabilities::of(MediaKind::Video, Path::new("/m/clip.mkv"));
        let mut document = document();
        assert_eq!(
            document.transform(Transform::Rotate(Quarter::Half), &clip),
            Err(EditRejected::NotEditable)
        );
        assert_eq!(
            document.annotate(redaction(box_at(0.0, 0.0, 10.0, 10.0)), &clip),
            Err(EditRejected::NotEditable)
        );
    }

    #[test]
    fn moving_an_object_is_undone_and_redone_as_one_step() {
        let mut document = document();
        let id = document
            .annotate(redaction(box_at(100.0, 100.0, 200.0, 200.0)), &photograph())
            .expect("placed");

        document
            .update(id, redaction(box_at(900.0, 900.0, 200.0, 200.0)))
            .expect("moved");
        assert!(same_area(
            area_of(&document, id),
            box_at(900.0, 900.0, 200.0, 200.0)
        ));

        assert!(document.undo());
        assert!(same_area(
            area_of(&document, id),
            box_at(100.0, 100.0, 200.0, 200.0)
        ));
        assert!(document.redo());
        assert!(same_area(
            area_of(&document, id),
            box_at(900.0, 900.0, 200.0, 200.0)
        ));
    }

    #[test]
    fn deleting_an_object_and_undoing_puts_it_back_where_it_was_drawn() {
        let mut document = document();
        let first = document
            .annotate(redaction(box_at(0.0, 0.0, 10.0, 10.0)), &photograph())
            .expect("placed");
        let second = document
            .annotate(text(box_at(50.0, 50.0, 200.0, 60.0)), &photograph())
            .expect("placed");
        let third = document
            .annotate(
                Annotation::Shape {
                    kind: ShapeKind::Ellipse,
                    area: box_at(300.0, 300.0, 100.0, 100.0),
                    width: 4.0,
                    ink: ink(),
                    fill: None,
                },
                &photograph(),
            )
            .expect("placed");

        document.remove(second).expect("deleted");
        assert_eq!(area_of_ids(&document), vec![first, third]);

        assert!(document.undo());
        assert_eq!(
            area_of_ids(&document),
            vec![first, second, third],
            "it comes back in its drawing order, not on top"
        );
        assert!(document.redo());
        assert_eq!(area_of_ids(&document), vec![first, third]);
    }

    #[test]
    fn adding_and_undoing_and_redoing_restores_the_object_itself() {
        let mut document = document();
        let id = document
            .annotate(text(box_at(10.0, 10.0, 100.0, 40.0)), &photograph())
            .expect("placed");
        assert!(document.undo());
        assert!(document.objects().is_empty());
        assert!(document.redo());
        assert!(same_area(
            area_of(&document, id),
            box_at(10.0, 10.0, 100.0, 40.0)
        ));
    }

    #[test]
    fn a_new_step_drops_what_was_undone() {
        let mut document = document();
        document
            .annotate(redaction(box_at(0.0, 0.0, 10.0, 10.0)), &photograph())
            .expect("placed");
        assert!(document.undo());
        assert!(document.can_redo());
        document
            .annotate(redaction(box_at(20.0, 20.0, 10.0, 10.0)), &photograph())
            .expect("placed");
        assert!(
            !document.can_redo(),
            "a branch that was abandoned does not come back"
        );
    }

    #[test]
    fn a_turn_on_a_photograph_costs_no_pixels_and_the_same_turn_on_a_screenshot_does() {
        let mut turned = document();
        turned
            .transform(Transform::Rotate(Quarter::Clockwise), &photograph())
            .expect("a quarter turn");

        assert_eq!(
            turned.orientation_only().map(|it| it.quarters),
            Some(1),
            "nothing but orientation happened"
        );
        assert_eq!(turned.class(&photograph()), EditClass::Lossless);
        assert_eq!(
            turned.class(&screenshot()),
            EditClass::Raster,
            "a PNG has no orientation header, so the turn is a re-render"
        );
    }

    #[test]
    fn anything_beyond_orientation_makes_the_save_a_new_image() {
        let mut cropped = document();
        cropped
            .transform(Transform::Rotate(Quarter::Clockwise), &photograph())
            .expect("turned");
        cropped
            .transform(
                Transform::Crop(box_at(0.0, 0.0, 1000.0, 1000.0)),
                &photograph(),
            )
            .expect("cropped");
        assert_eq!(cropped.orientation_only(), None);
        assert_eq!(cropped.class(&photograph()), EditClass::Raster);

        let mut annotated = document();
        annotated
            .transform(Transform::Rotate(Quarter::Clockwise), &photograph())
            .expect("turned");
        annotated
            .annotate(redaction(box_at(0.0, 0.0, 10.0, 10.0)), &photograph())
            .expect("placed");
        assert_eq!(annotated.orientation_only(), None);
        assert_eq!(annotated.class(&photograph()), EditClass::Raster);
    }

    #[test]
    fn turning_a_picture_all_the_way_round_is_not_an_edit_to_write() {
        let mut document = document();
        for _ in 0..2 {
            document
                .transform(Transform::Rotate(Quarter::Half), &photograph())
                .expect("turned");
        }
        let orientation = document.orientation_only().expect("orientation only");
        assert!(orientation.is_identity());
        assert_eq!(
            document.class(&photograph()),
            EditClass::Raster,
            "there is nothing lossless to write when nothing moved"
        );
    }

    #[test]
    fn a_mirror_is_carried_as_orientation_too() {
        let mut document = document();
        document
            .transform(Transform::Flip(Axis::Horizontal), &photograph())
            .expect("mirrored");
        let orientation = document.orientation_only().expect("orientation only");
        assert!(orientation.mirrored);
        assert_eq!(orientation.quarters, 0);
        assert_eq!(document.class(&photograph()), EditClass::Lossless);

        document
            .transform(Transform::Flip(Axis::Vertical), &photograph())
            .expect("mirrored again");
        let orientation = document.orientation_only().expect("orientation only");
        assert!(!orientation.mirrored, "two mirrors are a half turn");
        assert_eq!(orientation.quarters, 2);
    }

    #[test]
    fn the_composition_hands_over_the_final_canvas_and_the_objects_on_it() {
        let mut document = document();
        document
            .annotate(
                redaction(box_at(1200.0, 700.0, 400.0, 120.0)),
                &photograph(),
            )
            .expect("placed");
        document
            .transform(
                Transform::Crop(box_at(1000.0, 500.0, 2000.0, 1500.0)),
                &photograph(),
            )
            .expect("cropped");

        let composition = document.composition();
        assert_eq!(composition.canvas, canvas(2000, 1500));
        assert_eq!(
            composition.transforms,
            vec![Transform::Crop(box_at(1000.0, 500.0, 2000.0, 1500.0))],
            "the renderer starts from the file on disk, so it is told what to do to it"
        );
        assert_eq!(composition.objects.len(), 1);
        match &composition.objects[0] {
            Annotation::Redact { area, .. } => {
                assert!(same_area(*area, box_at(200.0, 200.0, 400.0, 120.0)));
            }
            other => panic!("unexpected object: {other:?}"),
        }
    }

    #[test]
    fn an_object_that_is_no_longer_there_is_refused_rather_than_ignored() {
        let mut document = document();
        let ghost = document
            .annotate(redaction(box_at(0.0, 0.0, 10.0, 10.0)), &photograph())
            .expect("placed");
        document.remove(ghost).expect("deleted");
        assert_eq!(
            document.update(ghost, redaction(box_at(0.0, 0.0, 10.0, 10.0))),
            Err(EditRejected::UnknownObject)
        );
        assert_eq!(document.remove(ghost), Err(EditRejected::UnknownObject));
    }

    #[test]
    fn every_operation_the_matrix_offers_has_a_transform_or_is_annotation() {
        for operation in Operation::ALL {
            let covered = match operation {
                Operation::Rotate => {
                    Transform::Rotate(Quarter::Half).operation() == Operation::Rotate
                }
                Operation::Flip => Transform::Flip(Axis::Horizontal).operation() == Operation::Flip,
                Operation::Crop => {
                    Transform::Crop(box_at(0.0, 0.0, 1.0, 1.0)).operation() == Operation::Crop
                }
                Operation::Resize => {
                    Transform::Resize(canvas(10, 10)).operation() == Operation::Resize
                }
                Operation::Annotate => true,
            };
            assert!(covered, "{operation:?} has no way to be requested");
        }
    }
}

#[cfg(test)]
mod orientation_tests {
    use super::Orientation;

    fn orientation(quarters: u8, mirrored: bool) -> Orientation {
        Orientation { quarters, mirrored }
    }

    #[test]
    fn every_exif_value_round_trips() {
        for value in 1..=8u16 {
            assert_eq!(
                Orientation::from_exif(value).to_exif(),
                value,
                "EXIF {value} did not survive being read and written"
            );
        }
    }

    #[test]
    fn an_absent_or_nonsense_value_means_show_it_as_it_is() {
        assert!(Orientation::from_exif(0).is_identity());
        assert!(Orientation::from_exif(9).is_identity());
        assert_eq!(Orientation::from_exif(1).to_exif(), 1);
    }

    #[test]
    fn turning_a_picture_that_was_already_turned_adds_up() {
        let already = Orientation::from_exif(6); // a quarter turn clockwise
        let again = already.then(orientation(1, false));
        assert_eq!(again.to_exif(), 3, "two quarter turns are a half turn");
        assert_eq!(
            already.then(orientation(3, false)).to_exif(),
            1,
            "the rest of the way round is where it started"
        );
    }

    #[test]
    fn mirroring_and_turning_do_not_commute() {
        let mirror = orientation(0, true);
        let turn = orientation(1, false);

        let turned_after_mirroring = mirror.then(turn);
        assert!(turned_after_mirroring.mirrored);
        assert_eq!(turned_after_mirroring.quarters, 1);
        assert_eq!(turned_after_mirroring.to_exif(), 7);

        let mirrored_after_turning = turn.then(mirror);
        assert!(mirrored_after_turning.mirrored);
        assert_eq!(
            mirrored_after_turning.quarters, 3,
            "mirroring a picture that was already turned turns it the other way"
        );
        assert_eq!(mirrored_after_turning.to_exif(), 5);
    }

    #[test]
    fn two_mirrors_cancel() {
        let once = orientation(0, false).then(orientation(0, true));
        let twice = once.then(orientation(0, true));
        assert!(twice.is_identity());
    }
}

#[cfg(test)]
mod preview_tests {
    use super::{Area, Canvas, EditDocument, EditLimits, Point, Quarter, Transform};
    use crate::edit::EditCapabilities;
    use crate::media::MediaKind;
    use std::path::Path;

    fn photograph() -> EditCapabilities {
        EditCapabilities::of(MediaKind::Image, Path::new("/m/foto.jpg"))
    }

    fn document() -> EditDocument {
        EditDocument::new(
            Canvas::new(4000, 3000).expect("a canvas"),
            EditLimits::new(100_000_000),
        )
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.01
    }

    #[test]
    fn an_untouched_picture_previews_as_the_whole_of_itself() {
        let preview = document().preview();
        assert!(close(preview.source.origin.x, 0.0));
        assert!(close(preview.source.width, 4000.0));
        assert!(close(preview.source.height, 3000.0));
        assert!(preview.orientation.is_identity());
    }

    #[test]
    fn a_crop_previews_as_the_part_of_the_file_that_is_left() {
        let mut document = document();
        document
            .transform(
                Transform::Crop(Area::new(Point::new(1000.0, 500.0), 2000.0, 1500.0)),
                &photograph(),
            )
            .expect("cropped");

        let preview = document.preview();
        assert!(close(preview.source.origin.x, 1000.0));
        assert!(close(preview.source.origin.y, 500.0));
        assert!(close(preview.source.width, 2000.0));
        assert!(close(preview.source.height, 1500.0));
    }

    #[test]
    fn a_turn_then_a_crop_still_names_an_area_of_the_file_on_disk() {
        let mut document = document();
        document
            .transform(Transform::Rotate(Quarter::Clockwise), &photograph())
            .expect("turned");
        // In the turned canvas (3000×4000), keep the top-left quarter.
        document
            .transform(
                Transform::Crop(Area::new(Point::new(0.0, 0.0), 1500.0, 2000.0)),
                &photograph(),
            )
            .expect("cropped");

        let preview = document.preview();
        assert_eq!(preview.orientation.quarters, 1);
        assert!(!preview.orientation.mirrored);
        // A clockwise turn puts the file's bottom-left corner at the canvas's
        // top-left, so that is the part the crop kept.
        assert!(close(preview.source.origin.x, 0.0), "{:?}", preview.source);
        assert!(
            close(preview.source.origin.y, 1500.0),
            "{:?}",
            preview.source
        );
        assert!(close(preview.source.width, 2000.0));
        assert!(close(preview.source.height, 1500.0));
    }

    #[test]
    fn a_resize_does_not_change_which_part_of_the_file_is_shown() {
        let mut document = document();
        document
            .transform(
                Transform::Resize(Canvas::new(400, 300).expect("a canvas")),
                &photograph(),
            )
            .expect("resized");
        let preview = document.preview();
        assert!(close(preview.source.width, 4000.0));
        assert!(close(preview.source.height, 3000.0));
    }
}
