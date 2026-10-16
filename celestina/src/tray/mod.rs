//! The status notifier tray (design §5.2, §5.5): the domain the tray host,
//! the Tray page and the DBusMenu popups share, and the bus clients that
//! fill it. `model` and `menu` touch no bus and no surface; `watcher`,
//! `item` and `dbusmenu` speak to peers on the bus (hostile input), and the
//! model's bounds apply to everything they bring in.

pub mod dbusmenu;
pub mod item;
pub mod menu;
pub mod model;
#[cfg(test)]
pub(crate) mod test_bus;
pub mod watcher;

pub use model::{
    bound_items, bound_menu, bound_text, choose_pixmap, scroll_steps, ItemId, ItemIdError,
    ItemsCoalescer, MenuEntry, MenuTree, NameHolder, Orientation, Pixmap, Status, TrayCommand,
    TrayEvent, TrayItem,
};
