//! The `org.kde.StatusNotifierItem` client: reads one registered item's
//! properties into a [`TrayItem`], sends it the activations and scrolls, and
//! recognises the signals that mean an item changed (design §5.5, §5.6).
//!
//! Every peer is hostile. Each property is read on its own with the
//! connection's short call timeout, and one item's read is given up past
//! [`ITEM_READ_BUDGET`]; a text reply is refused by its wire size and
//! signature before it is decoded (a variant is never decoded as a generic
//! value, which would let a byte array grow into one value per byte); the
//! pixmap list is refused by its wire size before any of it is copied, and
//! the pixmaps that survive go through the model's bounds. A command is sent
//! without expecting a reply, so an item that never answers cannot hold the
//! worker.

use std::time::{Duration, Instant};

use zbus::{
    blocking::Connection,
    message::{Flags, Message, Type},
    zvariant::{ObjectPath, Str},
};

use crate::tray::model::{choose_pixmap, Orientation, Status, TrayEvent, TrayItem};
use crate::tray::ItemId;

/// The interfaces an item may serve, tried in this order.
pub const ITEM_INTERFACES: [&str; 2] = [
    "org.kde.StatusNotifierItem",
    "org.freedesktop.StatusNotifierItem",
];
/// The most bytes an `IconPixmap` reply may carry before it is refused
/// unread: sixteen pixmaps of the largest kept size.
pub const MAX_PIXMAP_BODY_BYTES: usize = 16 * 256 * 256 * 4;
/// The size, in pixels, the Tray page draws an icon at; the pixmap chosen is
/// the smallest at least this large.
pub const ICON_SIZE: u32 = 22;
/// The most bytes a text property's reply may carry before it is refused
/// unread: the model keeps 128 characters of a title or icon name, and a
/// bus name or object path is at most 255 bytes.
pub const MAX_TEXT_BODY_BYTES: usize = 4096;
/// How long reading one item's properties may take in all before the item
/// counts as unresponsive: a peer that answers each call just inside the
/// call timeout must not hold the host for the whole sequence. The budget
/// is looked at between calls, so a read takes at most this plus one call's
/// timeout (1 s): under the two seconds the shell's exit waits for the
/// host. An honest item has six calls to answer within it.
pub const ITEM_READ_BUDGET: Duration = Duration::from_millis(900);
/// The wire signature of `IconPixmap`.
const PIXMAPS_SIGNATURE: &[u8] = b"a(iiay)";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
/// The bus driver, the only sender of `NameOwnerChanged` that is trusted.
const BUS_DRIVER: &str = "org.freedesktop.DBus";

/// Where an item is served, once the watcher's registration was resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemAddress {
    /// The item as registered.
    pub id: ItemId,
    /// The unique name serving it now (the registered name itself when that
    /// is unique).
    pub owner: String,
    /// The interface it answered on, one of [`ITEM_INTERFACES`].
    pub interface: &'static str,
    /// The object path of its `com.canonical.dbusmenu`, when it has one.
    pub menu: Option<String>,
}

/// An item that answered: where it is served and what it shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Served {
    /// Where it is served.
    pub address: ItemAddress,
    /// What the page shows.
    pub item: TrayItem,
}

/// What reading an item produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Read {
    /// The item answered (boxed: the pixmap and strings outweigh the other
    /// outcomes).
    Item(Box<Served>),
    /// Nothing answers at the item's path on either interface; the
    /// registration is kept and the item hidden until it signals.
    NotServed,
    /// The owner left the bus.
    Gone,
    /// The owner did not answer within the call timeout.
    Unresponsive,
}

/// How a failed call is handled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    /// The property or interface is not there; a default stands in.
    Missing,
    Gone,
    Unresponsive,
    /// Anything else: the call is treated as a missing property.
    Other,
}

fn classify(err: &zbus::Error) -> Failure {
    if let zbus::Error::InputOutput(io) = err {
        if io.kind() == std::io::ErrorKind::TimedOut {
            return Failure::Unresponsive;
        }
    }
    use zbus::fdo::Error as Fdo;
    match Fdo::from(err.clone()) {
        Fdo::ServiceUnknown(_) | Fdo::NameHasNoOwner(_) => Failure::Gone,
        Fdo::UnknownObject(_)
        | Fdo::UnknownInterface(_)
        | Fdo::UnknownMethod(_)
        | Fdo::UnknownProperty(_)
        | Fdo::InvalidArgs(_) => Failure::Missing,
        Fdo::NoReply(_) => Failure::Unresponsive,
        _ => Failure::Other,
    }
}

/// One `Get` of `property` on `interface` at the item, as the raw reply.
fn get(
    connection: &Connection,
    owner: &str,
    path: &str,
    interface: &str,
    property: &str,
) -> zbus::Result<Message> {
    connection.call_method(
        Some(owner),
        path,
        Some(PROPERTIES),
        "Get",
        &(interface, property),
    )
}

/// The offset of the value inside a `Properties.Get` reply whose variant
/// carries `signature`, or `None` when the reply carries another type. A
/// variant on the wire is the signature's length, its bytes, a NUL, then the
/// value aligned as its own type requires; checking the three leading bytes
/// is what lets the value be decoded typed, never as a generic value.
fn variant_value_at(bytes: &[u8], signature: &[u8]) -> Option<usize> {
    let &len = bytes.first()?;
    let len = usize::from(len);
    let value_at = len + 2;
    (bytes.len() >= value_at && &bytes[1..=len] == signature && bytes[len + 1] == 0)
        .then_some(value_at)
}

/// The text of a `Get` reply holding a string or an object path, `None` for
/// any other type or a body past [`MAX_TEXT_BODY_BYTES`].
#[must_use]
pub fn text_of(reply: &Message) -> Option<String> {
    let body = reply.body();
    if body.len() > MAX_TEXT_BODY_BYTES {
        tracing::warn!(
            bytes = body.len(),
            "tray: a text property past the bound was refused unread"
        );
        return None;
    }
    let data = body.data();
    let bytes = data.bytes();
    if let Some(at) = variant_value_at(bytes, b"s") {
        return data
            .slice(at..)
            .deserialize_for_signature::<_, Str<'_>>("s")
            .ok()
            .map(|(text, _)| text.as_str().to_owned());
    }
    if let Some(at) = variant_value_at(bytes, b"o") {
        return data
            .slice(at..)
            .deserialize_for_signature::<_, ObjectPath<'_>>("o")
            .ok()
            .map(|(path, _)| path.as_str().to_owned());
    }
    None
}

/// A string property, `None` when it is missing or not a string; `Err` when
/// the owner is gone or silent.
fn get_text(
    connection: &Connection,
    owner: &str,
    path: &str,
    interface: &str,
    property: &str,
) -> Result<Option<String>, Failure> {
    match get(connection, owner, path, interface, property) {
        Ok(reply) => Ok(text_of(&reply)),
        Err(err) => match classify(&err) {
            Failure::Gone => Err(Failure::Gone),
            Failure::Unresponsive => Err(Failure::Unresponsive),
            Failure::Missing | Failure::Other => Ok(None),
        },
    }
}

/// The pixmaps of an `IconPixmap` reply, refused whole when the body is
/// larger than [`MAX_PIXMAP_BODY_BYTES`] or not an `a(iiay)`.
#[must_use]
pub fn pixmaps_of(reply: &Message) -> Vec<(i32, i32, Vec<u8>)> {
    let body = reply.body();
    if body.len() > MAX_PIXMAP_BODY_BYTES {
        tracing::warn!(
            bytes = body.len(),
            "tray: an IconPixmap reply past the bound was refused unread"
        );
        return Vec::new();
    }
    let data = body.data();
    let Some(value_at) = variant_value_at(data.bytes(), PIXMAPS_SIGNATURE) else {
        return Vec::new();
    };
    match data
        .slice(value_at..)
        .deserialize_for_signature::<_, Vec<(i32, i32, Vec<u8>)>>("a(iiay)")
    {
        Ok((pixmaps, _)) => pixmaps,
        Err(err) => {
            tracing::debug!(%err, "tray: an IconPixmap reply does not decode");
            Vec::new()
        }
    }
}

fn get_pixmaps(
    connection: &Connection,
    owner: &str,
    path: &str,
    interface: &str,
) -> Result<Vec<(i32, i32, Vec<u8>)>, Failure> {
    match get(connection, owner, path, interface, "IconPixmap") {
        Ok(reply) => Ok(pixmaps_of(&reply)),
        Err(err) => match classify(&err) {
            Failure::Gone => Err(Failure::Gone),
            Failure::Unresponsive => Err(Failure::Unresponsive),
            Failure::Missing | Failure::Other => Ok(Vec::new()),
        },
    }
}

/// An item's `Status` property.
#[must_use]
pub fn status_of(text: &str) -> Status {
    match text {
        "Passive" => Status::Passive,
        "NeedsAttention" => Status::NeedsAttention,
        _ => Status::Active,
    }
}

/// A `Menu` property that names a menu: a path other than `/`.
#[must_use]
pub fn menu_path(text: Option<&str>) -> Option<String> {
    match text {
        Some(path) if path.starts_with('/') && path != "/" => Some(path.to_owned()),
        _ => None,
    }
}

/// Reads the item `id`, served by `owner`, into a [`TrayItem`] drawn at
/// [`ICON_SIZE`]: `Id` decides the interface, then `Title`, `Status`,
/// `IconName`, `Menu` and `IconPixmap`. The whole read has
/// [`ITEM_READ_BUDGET`]; past it the item is unresponsive.
#[must_use]
pub fn read_item(connection: &Connection, id: &ItemId, owner: &str) -> Read {
    let started = Instant::now();
    let path = id.path.as_str();
    let mut interface = None;
    for candidate in ITEM_INTERFACES {
        match get_text(connection, owner, path, candidate, "Id") {
            Ok(Some(_)) => {
                interface = Some(candidate);
                break;
            }
            Ok(None) => {}
            Err(Failure::Gone) => return Read::Gone,
            Err(_) => return Read::Unresponsive,
        }
        if started.elapsed() > ITEM_READ_BUDGET {
            return Read::Unresponsive;
        }
    }
    let Some(interface) = interface else {
        return Read::NotServed;
    };
    let text = |property: &str| {
        if started.elapsed() > ITEM_READ_BUDGET {
            return Err(Failure::Unresponsive);
        }
        get_text(connection, owner, path, interface, property)
    };
    let failed = |failure: Failure| match failure {
        Failure::Gone => Read::Gone,
        _ => Read::Unresponsive,
    };
    let title = match text("Title") {
        Ok(title) => title.unwrap_or_default(),
        Err(failure) => return failed(failure),
    };
    let status = match text("Status") {
        Ok(status) => status_of(status.as_deref().unwrap_or("Active")),
        Err(failure) => return failed(failure),
    };
    let icon_name = match text("IconName") {
        Ok(name) => name.unwrap_or_default(),
        Err(failure) => return failed(failure),
    };
    let menu = match text("Menu") {
        Ok(menu) => menu_path(menu.as_deref()),
        Err(failure) => return failed(failure),
    };
    if started.elapsed() > ITEM_READ_BUDGET {
        return Read::Unresponsive;
    }
    let pixmap = match get_pixmaps(connection, owner, path, interface) {
        Ok(pixmaps) => choose_pixmap(&pixmaps, ICON_SIZE),
        Err(failure) => return failed(failure),
    };
    let item = TrayItem {
        id: id.clone(),
        title,
        status,
        icon_name,
        pixmap,
        has_menu: menu.is_some(),
    };
    Read::Item(Box::new(Served {
        address: ItemAddress {
            id: id.clone(),
            owner: owner.to_owned(),
            interface,
            menu,
        },
        item,
    }))
}

/// What the loop asks of an item directly: the method and its body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemCall {
    /// `Activate(x, y)`.
    Activate,
    /// `SecondaryActivate(x, y)`.
    SecondaryActivate,
    /// `Scroll(delta, orientation)`.
    Scroll(i32, Orientation),
}

impl ItemCall {
    fn method(&self) -> &'static str {
        match self {
            Self::Activate => "Activate",
            Self::SecondaryActivate => "SecondaryActivate",
            Self::Scroll(..) => "Scroll",
        }
    }
}

/// The wire name of a scroll axis.
#[must_use]
pub fn orientation_name(orientation: Orientation) -> &'static str {
    match orientation {
        Orientation::Vertical => "vertical",
        Orientation::Horizontal => "horizontal",
    }
}

/// Sends `call` to the item without expecting a reply. The pointer position
/// is not known to the page, so activations carry `(0, 0)`.
///
/// # Errors
///
/// The bus's failure to send.
pub fn send(connection: &Connection, item: &ItemAddress, call: &ItemCall) -> zbus::Result<()> {
    let builder = Message::method_call(item.id.path.as_str(), call.method())?
        .destination(item.owner.as_str())?
        .interface(item.interface)?
        .with_flags(Flags::NoReplyExpected)?;
    let message = match call {
        ItemCall::Activate | ItemCall::SecondaryActivate => builder.build(&(0i32, 0i32))?,
        ItemCall::Scroll(delta, orientation) => {
            builder.build(&(*delta, orientation_name(*orientation)))?
        }
    };
    connection.send(&message)
}

/// The signals of items (`NewIcon`, `NewTitle`, ...) that mean a re-read.
const CHANGE_SIGNALS: [&str; 8] = [
    "NewTitle",
    "NewIcon",
    "NewAttentionIcon",
    "NewOverlayIcon",
    "NewToolTip",
    "NewStatus",
    "NewIconThemePath",
    "NewMenu",
];

/// A bus message the tray acts on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BusSignal {
    /// The menu at `path`, served by the unique name `sender`, changed.
    MenuChanged {
        /// The unique name of the sender.
        sender: String,
        /// The object path the signal came from.
        path: String,
    },
    /// An item at `path`, served by the unique name `sender`, changed.
    ItemChanged {
        /// The unique name of the sender.
        sender: String,
        /// The object path the signal came from.
        path: String,
    },
    /// `name` changed hands: `new_owner` is the unique name holding it now,
    /// or empty when it was released.
    NameOwnerChanged {
        /// The name.
        name: String,
        /// Its new owner, or empty.
        new_owner: String,
    },
}

/// Recognises `message` as one of the signals the tray acts on. A
/// `NameOwnerChanged` counts only from the bus driver itself and only as a
/// broadcast: the connection's stream also carries signals a peer addresses
/// to the shell, and a forged one would drop other items or redirect their
/// commands to the forger.
#[must_use]
pub fn recognise(message: &Message) -> Option<BusSignal> {
    let header = message.header();
    if header.message_type() != Type::Signal {
        return None;
    }
    let interface = header.interface()?.as_str();
    let member = header.member()?.as_str();
    if interface == BUS_DRIVER && member == "NameOwnerChanged" {
        let from_driver = header.sender().is_some_and(|s| s.as_str() == BUS_DRIVER);
        if !from_driver || header.destination().is_some() {
            tracing::debug!("tray: a NameOwnerChanged not broadcast by the bus is ignored");
            return None;
        }
        let (name, _old, new_owner): (String, String, String) =
            message.body().deserialize().ok()?;
        return Some(BusSignal::NameOwnerChanged { name, new_owner });
    }
    if interface == "com.canonical.dbusmenu"
        && (member == "LayoutUpdated" || member == "ItemsPropertiesUpdated")
    {
        return Some(BusSignal::MenuChanged {
            sender: header.sender()?.as_str().to_owned(),
            path: header.path()?.as_str().to_owned(),
        });
    }
    if ITEM_INTERFACES.contains(&interface) && CHANGE_SIGNALS.contains(&member) {
        return Some(BusSignal::ItemChanged {
            sender: header.sender()?.as_str().to_owned(),
            path: header.path()?.as_str().to_owned(),
        });
    }
    None
}

/// What the DBusMenu client plugs into the tray worker: the two commands
/// that concern an item's menu. Served by `NoMenu` where there is no
/// client, and by `DbusMenu` in the shell.
pub trait MenuClient: Send {
    /// Fetches the menu of `item` and publishes it as [`TrayEvent::Menu`].
    fn open_menu(
        &mut self,
        connection: &Connection,
        item: &ItemAddress,
        publish: &mut dyn FnMut(TrayEvent) -> bool,
    );

    /// Sends a click on the entry `entry` of `item`'s menu.
    fn menu_event(&mut self, connection: &Connection, item: &ItemAddress, entry: i32);

    /// A submenu `entry` of `item`'s menu is about to be shown; a changed
    /// layout is published again. Nothing by default.
    fn about_to_show(
        &mut self,
        _connection: &Connection,
        _item: &ItemAddress,
        _entry: i32,
        _publish: &mut dyn FnMut(TrayEvent) -> bool,
    ) {
    }

    /// The menu at `path` of the connection `sender` signalled a change;
    /// the open menus among them are published again. Nothing by default.
    fn menu_changed(
        &mut self,
        _connection: &Connection,
        _sender: &str,
        _path: &str,
        _publish: &mut dyn FnMut(TrayEvent) -> bool,
    ) {
    }

    /// The menu of `item` closed on the page: its changes are not followed
    /// any more. Nothing by default.
    fn close_menu(&mut self, _item: &ItemAddress) {}
}

/// The menu client while there is none: a menu request is logged and
/// nothing is published.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoMenu;

impl MenuClient for NoMenu {
    fn open_menu(
        &mut self,
        _: &Connection,
        item: &ItemAddress,
        _: &mut dyn FnMut(TrayEvent) -> bool,
    ) {
        tracing::debug!(item = %item.id.bus_name, "tray: menus are not served yet");
    }

    fn menu_event(&mut self, _: &Connection, item: &ItemAddress, entry: i32) {
        tracing::debug!(item = %item.id.bus_name, entry, "tray: menus are not served yet");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tray::model::Pixmap;
    use zbus::zvariant::{Array, Signature, Structure, Value};

    fn pixmap_reply(pixmaps: &[(i32, i32, Vec<u8>)]) -> Message {
        let signature = Signature::try_from("(iiay)").expect("a signature");
        let mut array = Array::new(&signature);
        for (w, h, bytes) in pixmaps {
            let structure = Structure::from((*w, *h, Array::from(bytes.as_slice())));
            array.append(Value::Structure(structure)).expect("appended");
        }
        Message::signal("/x", "org.x.Y", "Z")
            .expect("a builder")
            .build(&Value::Array(array))
            .expect("a message")
    }

    #[test]
    fn pixmaps_are_read_from_the_variant_and_bounded_by_the_model() {
        let small = (2, 2, vec![7u8; 16]);
        let huge = (4096, 4096, vec![0u8; 16]);
        let reply = pixmap_reply(&[huge.clone(), small.clone()]);
        let pixmaps = pixmaps_of(&reply);
        assert_eq!(pixmaps, vec![huge, small.clone()]);
        assert_eq!(
            choose_pixmap(&pixmaps, ICON_SIZE),
            Some(Pixmap {
                w: 2,
                h: 2,
                argb: vec![7u8; 16]
            })
        );
        assert_eq!(
            choose_pixmap(&pixmaps_of(&pixmap_reply(&[(4096, 4096, vec![0; 16])])), 22),
            None
        );
    }

    fn reply_of(value: Value<'_>) -> Message {
        Message::signal("/x", "org.x.Y", "Z")
            .expect("a builder")
            .build(&value)
            .expect("a message")
    }

    #[test]
    fn text_properties_are_decoded_typed_and_refused_by_type_and_size() {
        assert_eq!(
            text_of(&reply_of(Value::from("Red"))).as_deref(),
            Some("Red")
        );
        let path = ObjectPath::try_from("/Menu").expect("a path");
        assert_eq!(
            text_of(&reply_of(Value::ObjectPath(path))).as_deref(),
            Some("/Menu")
        );
        // Another type in the variant is not a text, however small...
        assert_eq!(text_of(&reply_of(Value::from(7i32))), None);
        assert_eq!(text_of(&reply_of(Value::from(vec![1u8, 2, 3]))), None);
        // ...and a large array is refused by its size before any decoding.
        let bytes = vec![0u8; MAX_TEXT_BODY_BYTES + 1];
        assert_eq!(text_of(&reply_of(Value::from(bytes))), None);
        let long = "x".repeat(MAX_TEXT_BODY_BYTES + 1);
        assert_eq!(text_of(&reply_of(Value::from(long))), None);
        // A body that is not a variant at all.
        let bare = Message::signal("/x", "org.x.Y", "Z")
            .expect("a builder")
            .build(&("text",))
            .expect("a message");
        assert_eq!(text_of(&bare), None);
    }

    #[test]
    fn a_reply_that_is_not_a_pixmap_list_or_too_large_yields_nothing() {
        let wrong = Message::signal("/x", "org.x.Y", "Z")
            .expect("a builder")
            .build(&Value::from("not pixmaps"))
            .expect("a message");
        assert!(pixmaps_of(&wrong).is_empty());
        let empty = Message::signal("/x", "org.x.Y", "Z")
            .expect("a builder")
            .build(&())
            .expect("a message");
        assert!(pixmaps_of(&empty).is_empty());
        let oversized = pixmap_reply(&[(1, 1, vec![0u8; MAX_PIXMAP_BODY_BYTES + 1])]);
        assert!(pixmaps_of(&oversized).is_empty());
    }

    #[test]
    fn status_and_menu_properties_map_to_the_model() {
        assert_eq!(status_of("Passive"), Status::Passive);
        assert_eq!(status_of("NeedsAttention"), Status::NeedsAttention);
        assert_eq!(status_of("Active"), Status::Active);
        assert_eq!(status_of("weird"), Status::Active);
        assert_eq!(menu_path(Some("/MenuBar")), Some("/MenuBar".to_owned()));
        assert_eq!(menu_path(Some("/")), None);
        assert_eq!(menu_path(Some("")), None);
        assert_eq!(menu_path(None), None);
    }

    #[test]
    fn item_signals_and_name_changes_are_recognised() {
        let changed = Message::signal(
            "/StatusNotifierItem",
            "org.kde.StatusNotifierItem",
            "NewIcon",
        )
        .expect("a builder")
        .sender(":1.9")
        .expect("a sender")
        .build(&())
        .expect("a message");
        assert_eq!(
            recognise(&changed),
            Some(BusSignal::ItemChanged {
                sender: ":1.9".into(),
                path: "/StatusNotifierItem".into()
            })
        );
        let other = Message::signal("/StatusNotifierItem", "org.kde.StatusNotifierItem", "Other")
            .expect("a builder")
            .sender(":1.9")
            .expect("a sender")
            .build(&())
            .expect("a message");
        assert_eq!(recognise(&other), None);
        let owner = Message::signal(
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "NameOwnerChanged",
        )
        .expect("a builder")
        .sender("org.freedesktop.DBus")
        .expect("a sender")
        .build(&("org.kde.foo", ":1.4", ""))
        .expect("a message");
        assert_eq!(
            recognise(&owner),
            Some(BusSignal::NameOwnerChanged {
                name: "org.kde.foo".into(),
                new_owner: String::new()
            })
        );
        // A peer forging the driver's signal, and one the driver addresses
        // to the shell alone, are both ignored.
        let forged = Message::signal(
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "NameOwnerChanged",
        )
        .expect("a builder")
        .sender(":1.666")
        .expect("a sender")
        .build(&(":1.42", ":1.42", ""))
        .expect("a message");
        assert_eq!(recognise(&forged), None);
        let directed = Message::signal(
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "NameOwnerChanged",
        )
        .expect("a builder")
        .sender("org.freedesktop.DBus")
        .expect("a sender")
        .destination(":1.7")
        .expect("a destination")
        .build(&("org.kde.foo", ":1.4", ""))
        .expect("a message");
        assert_eq!(recognise(&directed), None);
        let call = Message::method_call("/x", "M")
            .expect("a builder")
            .interface("org.kde.StatusNotifierItem")
            .expect("an interface")
            .build(&())
            .expect("a message");
        assert_eq!(recognise(&call), None);
    }

    #[test]
    fn menu_update_signals_are_recognised() {
        let signal = |member: &str| {
            Message::signal("/Menu", "com.canonical.dbusmenu", member)
                .expect("a builder")
                .sender(":1.9")
                .expect("a sender")
                .build(&())
                .expect("a message")
        };
        let changed = Some(BusSignal::MenuChanged {
            sender: ":1.9".into(),
            path: "/Menu".into(),
        });
        assert_eq!(recognise(&signal("LayoutUpdated")), changed);
        assert_eq!(recognise(&signal("ItemsPropertiesUpdated")), changed);
        assert_eq!(recognise(&signal("ItemActivationRequested")), None);
    }

    #[test]
    fn scroll_axes_have_their_wire_names() {
        assert_eq!(orientation_name(Orientation::Vertical), "vertical");
        assert_eq!(orientation_name(Orientation::Horizontal), "horizontal");
        assert_eq!(
            ItemCall::Scroll(1, Orientation::Vertical).method(),
            "Scroll"
        );
    }
}
