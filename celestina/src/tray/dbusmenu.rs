//! The `com.canonical.dbusmenu` client: fetches an item's menu, turns the
//! recursive layout into a bounded [`MenuTree`], sends the clicks, asks
//! `AboutToShow` for a submenu the page opens, and follows the menu while
//! it is open, until the page says it closed (design §5.5).
//!
//! The layout comes from a hostile peer. A reply larger than
//! [`MAX_LAYOUT_BODY_BYTES`] is refused before it is decoded; the decoded
//! value is walked with a node budget, a depth limit and the per-level limit,
//! so a deep or huge layout is cut short instead of followed; and every
//! property is read tolerantly (a wrong type is a default).

use std::collections::HashMap;

use zbus::{
    blocking::Connection,
    message::{Flags, Message},
    zvariant::{OwnedValue, Value},
};

use crate::tray::{
    item::{ItemAddress, MenuClient},
    model::{
        bound_menu, MenuEntry, MenuTree, TrayEvent, MAX_MENU_DEPTH, MAX_MENU_ENTRIES,
        MAX_MENU_TOTAL,
    },
};

/// The interface of a menu.
pub const MENU_INTERFACE: &str = "com.canonical.dbusmenu";
/// The most bytes a `GetLayout` reply may carry before it is refused
/// unread: far above any honest menu of [`MAX_MENU_TOTAL`] entries.
pub const MAX_LAYOUT_BODY_BYTES: usize = 1 << 20;
/// How many menus are followed for updates at once; the oldest is dropped.
/// The page closes one menu before it opens another, so one is the normal
/// count; the bound covers a close the page never reported.
pub const MAX_FOLLOWED: usize = 8;
/// The properties asked of every node.
const PROPERTIES: [&str; 7] = [
    "label",
    "enabled",
    "visible",
    "type",
    "toggle-type",
    "toggle-state",
    "children-display",
];

/// A `dbusmenu` node as it comes on the wire: id, properties, children (each
/// child a variant holding another node).
pub type Layout = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

fn text(props: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    match &**props.get(key)? {
        Value::Str(s) => Some(s.as_str().to_owned()),
        _ => None,
    }
}

fn flag(props: &HashMap<String, OwnedValue>, key: &str, default: bool) -> bool {
    match props.get(key).map(|v| &**v) {
        Some(Value::Bool(b)) => *b,
        _ => default,
    }
}

/// A child variant read as a node; `None` when it is anything else.
fn node(child: &OwnedValue) -> Option<Layout> {
    let value = child.try_clone().ok().map(Value::from)?;
    let value = match value {
        Value::Value(inner) => *inner,
        other => other,
    };
    Layout::try_from(value).ok()
}

/// The entries of one level out of `children`. `budget` counts the nodes
/// still allowed in the whole tree; it is spent before a node is read, so
/// the walk is bounded however the peer shapes the layout.
fn level(children: &[OwnedValue], depth: usize, budget: &mut usize) -> Vec<MenuEntry> {
    let mut entries = Vec::new();
    for child in children {
        if entries.len() >= MAX_MENU_ENTRIES || *budget == 0 {
            break;
        }
        *budget -= 1;
        let Some((id, props, kids)) = node(child) else {
            continue;
        };
        if !flag(&props, "visible", true) {
            continue;
        }
        let kind = text(&props, "type");
        if kind.as_deref() == Some("separator") {
            entries.push(MenuEntry {
                id,
                separator: true,
                ..MenuEntry::default()
            });
            continue;
        }
        let toggle = match text(&props, "toggle-type").as_deref() {
            Some("checkmark" | "radio") => {
                let state = props.get("toggle-state").map(|v| &**v);
                Some(matches!(state, Some(Value::I32(1))))
            }
            _ => None,
        };
        let submenu = (text(&props, "children-display").as_deref() == Some("submenu")
            && depth < MAX_MENU_DEPTH)
            .then(|| MenuTree {
                id,
                entries: level(&kids, depth + 1, budget),
            });
        entries.push(MenuEntry {
            id,
            label: text(&props, "label").unwrap_or_default(),
            enabled: flag(&props, "enabled", true),
            toggle,
            submenu,
            separator: false,
        });
    }
    entries
}

/// The bounded menu of a whole `GetLayout` root: its children as the first
/// level, in order.
#[must_use]
pub fn menu_from_layout(root: &Layout) -> MenuTree {
    let mut budget = MAX_MENU_TOTAL;
    bound_menu(MenuTree {
        id: root.0,
        entries: level(&root.2, 1, &mut budget),
    })
}

/// The root of a `GetLayout` reply, refused unread past
/// [`MAX_LAYOUT_BODY_BYTES`].
fn layout_of(reply: &Message) -> Option<Layout> {
    let body = reply.body();
    if body.len() > MAX_LAYOUT_BODY_BYTES {
        tracing::warn!(
            bytes = body.len(),
            "tray: a menu layout past the bound was refused unread"
        );
        return None;
    }
    match body.deserialize::<(u32, Layout)>() {
        Ok((_revision, root)) => Some(root),
        Err(err) => {
            tracing::debug!(%err, "tray: a menu layout does not decode");
            None
        }
    }
}

fn call(
    connection: &Connection,
    item: &ItemAddress,
    method: &str,
    body: &(impl zbus::export::serde::Serialize + zbus::zvariant::DynamicType),
) -> zbus::Result<Message> {
    let Some(path) = item.menu.as_deref() else {
        return Err(zbus::Error::Failure("the item serves no menu".to_owned()));
    };
    connection.call_method(
        Some(item.owner.as_str()),
        path,
        Some(MENU_INTERFACE),
        method,
        body,
    )
}

/// `AboutToShow(id)`: whether the menu says its layout changed.
fn about_to_show(connection: &Connection, item: &ItemAddress, id: i32) -> bool {
    match call(connection, item, "AboutToShow", &(id,)) {
        Ok(reply) => reply.body().deserialize::<bool>().unwrap_or(false),
        Err(err) => {
            tracing::debug!(%err, "tray: AboutToShow failed");
            false
        }
    }
}

/// The whole menu of `item`, depth-limited and bounded.
fn fetch(connection: &Connection, item: &ItemAddress) -> Option<MenuTree> {
    let depth = i32::try_from(MAX_MENU_DEPTH).unwrap_or(4);
    match call(
        connection,
        item,
        "GetLayout",
        &(0i32, depth, PROPERTIES.to_vec()),
    ) {
        Ok(reply) => layout_of(&reply).map(|root| menu_from_layout(&root)),
        Err(err) => {
            tracing::debug!(%err, "tray: GetLayout failed");
            None
        }
    }
}

/// Sends `Event(id, "clicked", 0, 0)` without waiting for a reply.
fn send_click(connection: &Connection, item: &ItemAddress, entry: i32) -> zbus::Result<()> {
    let Some(path) = item.menu.as_deref() else {
        return Ok(());
    };
    let message = Message::method_call(path, "Event")?
        .destination(item.owner.as_str())?
        .interface(MENU_INTERFACE)?
        .with_flags(Flags::NoReplyExpected)?
        .build(&(entry, "clicked", Value::from(0i32), 0u32))?;
    connection.send(&message)
}

/// The menu client the tray host uses.
#[derive(Debug, Default)]
pub struct DbusMenu {
    /// The menus opened, oldest first; their updates are followed.
    followed: Vec<ItemAddress>,
}

impl DbusMenu {
    fn follow(&mut self, item: &ItemAddress) {
        self.followed.retain(|a| a != item);
        if self.followed.len() >= MAX_FOLLOWED {
            self.followed.remove(0);
        }
        self.followed.push(item.clone());
    }

    fn publish_menu(
        connection: &Connection,
        item: &ItemAddress,
        publish: &mut dyn FnMut(TrayEvent) -> bool,
    ) {
        if let Some(tree) = fetch(connection, item) {
            publish(TrayEvent::Menu(item.id.clone(), tree));
        }
    }
}

impl MenuClient for DbusMenu {
    fn open_menu(
        &mut self,
        connection: &Connection,
        item: &ItemAddress,
        publish: &mut dyn FnMut(TrayEvent) -> bool,
    ) {
        if item.menu.is_none() {
            return;
        }
        about_to_show(connection, item, 0);
        self.follow(item);
        Self::publish_menu(connection, item, publish);
    }

    fn menu_event(&mut self, connection: &Connection, item: &ItemAddress, entry: i32) {
        if let Err(err) = send_click(connection, item, entry) {
            tracing::warn!(%err, item = %item.id.bus_name, "tray: a menu click was not sent");
        }
    }

    fn about_to_show(
        &mut self,
        connection: &Connection,
        item: &ItemAddress,
        entry: i32,
        publish: &mut dyn FnMut(TrayEvent) -> bool,
    ) {
        if item.menu.is_some() && about_to_show(connection, item, entry) {
            Self::publish_menu(connection, item, publish);
        }
    }

    fn menu_changed(
        &mut self,
        connection: &Connection,
        sender: &str,
        path: &str,
        publish: &mut dyn FnMut(TrayEvent) -> bool,
    ) {
        let hits: Vec<ItemAddress> = self
            .followed
            .iter()
            .filter(|a| a.owner == sender && a.menu.as_deref() == Some(path))
            .cloned()
            .collect();
        for item in &hits {
            Self::publish_menu(connection, item, publish);
        }
    }

    fn close_menu(&mut self, item: &ItemAddress) {
        self.followed.retain(|a| a != item);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::AtomicBool,
        mpsc::{channel, Receiver},
        Arc, Mutex,
    };
    use std::time::{Duration, Instant};

    use zbus::zvariant::{OwnedObjectPath, Structure};

    use super::*;
    use crate::{
        providers::tray::{spawn_on, Publish},
        tray::{
            model::{TrayCommand, TrayItem},
            test_bus::PrivateBus,
            watcher::{WATCHER_INTERFACE, WATCHER_NAME, WATCHER_PATH},
            ItemId,
        },
    };

    const WAIT: Duration = Duration::from_secs(5);

    fn owned(value: impl Into<Value<'static>>) -> OwnedValue {
        OwnedValue::try_from(value.into()).expect("an owned value")
    }

    /// A node as a child variant.
    fn node_value(id: i32, props: &[(&str, OwnedValue)], kids: Vec<OwnedValue>) -> OwnedValue {
        let props: HashMap<String, OwnedValue> = props
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.try_clone().expect("cloned")))
            .collect();
        owned(Value::Structure(Structure::from((id, props, kids))))
    }

    fn label(text: &str) -> (&'static str, OwnedValue) {
        ("label", owned(text.to_owned()))
    }

    fn root(kids: Vec<OwnedValue>) -> Layout {
        (0, HashMap::new(), kids)
    }

    #[test]
    fn menu_rows_map_dbusmenu_entries_with_separators_toggles_and_disabled_state() {
        let sub = node_value(
            9,
            &[
                label("_More"),
                ("children-display", owned("submenu".to_owned())),
            ],
            vec![node_value(10, &[label("Inner")], vec![])],
        );
        let layout = root(vec![
            node_value(1, &[label("_Open")], vec![]),
            node_value(2, &[label("Off"), ("enabled", owned(false))], vec![]),
            node_value(3, &[("type", owned("separator".to_owned()))], vec![]),
            node_value(
                4,
                &[
                    label("Check"),
                    ("toggle-type", owned("checkmark".to_owned())),
                    ("toggle-state", owned(1i32)),
                ],
                vec![],
            ),
            node_value(
                5,
                &[
                    label("Radio"),
                    ("toggle-type", owned("radio".to_owned())),
                    ("toggle-state", owned(0i32)),
                ],
                vec![],
            ),
            node_value(6, &[label("Hidden"), ("visible", owned(false))], vec![]),
            node_value(7, &[label(&"w".repeat(500))], vec![]),
            owned("not a node".to_owned()),
            sub,
        ]);
        let tree = menu_from_layout(&layout);
        assert_eq!(tree.id, 0);
        let ids: Vec<i32> = tree.entries.iter().map(|e| e.id).collect();
        assert_eq!(ids, [1, 2, 3, 4, 5, 7, 9]);
        assert_eq!(tree.entries[0].label, "Open");
        assert!(tree.entries[0].enabled && tree.entries[0].toggle.is_none());
        assert!(!tree.entries[1].enabled);
        assert!(tree.entries[2].separator);
        assert_eq!(tree.entries[3].toggle, Some(true));
        assert_eq!(tree.entries[4].toggle, Some(false));
        assert_eq!(tree.entries[5].label.chars().count(), 128);
        let more = &tree.entries[6];
        assert_eq!(more.label, "More");
        let inner = more.submenu.as_ref().expect("a submenu");
        assert_eq!(inner.id, 9);
        assert_eq!(inner.entries[0].label, "Inner");
    }

    #[test]
    fn children_without_children_display_submenu_are_not_followed() {
        let layout = root(vec![node_value(
            1,
            &[label("Plain")],
            vec![node_value(2, &[label("Ignored")], vec![])],
        )]);
        assert_eq!(menu_from_layout(&layout).entries[0].submenu, None);
    }

    fn count(tree: &MenuTree) -> usize {
        tree.entries
            .iter()
            .map(|e| 1 + e.submenu.as_ref().map_or(0, count))
            .sum()
    }

    #[test]
    fn a_deep_or_huge_layout_is_bounded_without_panicking() {
        let submenu = ("children-display", owned("submenu".to_owned()));
        let mut deep = node_value(1, &[label("leaf")], vec![]);
        for id in 2..40 {
            deep = node_value(id, &[label("n"), submenu.try_clone_pair()], vec![deep]);
        }
        let tree = menu_from_layout(&root(vec![deep]));
        let mut depth = 1;
        let mut level = &tree;
        while let Some(sub) = level.entries.first().and_then(|e| e.submenu.as_ref()) {
            depth += 1;
            level = sub;
        }
        assert_eq!(depth, MAX_MENU_DEPTH);

        let wide: Vec<_> = (0..5000)
            .map(|i| node_value(i, &[label("e")], vec![]))
            .collect();
        assert_eq!(
            menu_from_layout(&root(wide)).entries.len(),
            MAX_MENU_ENTRIES
        );

        // Many submenus of many entries: the node budget stops the walk.
        let big: Vec<_> = (0..64)
            .map(|i| {
                let kids = (0..64)
                    .map(|j| {
                        let leaves = (0..64)
                            .map(|k| node_value(k, &[label("l")], vec![]))
                            .collect();
                        node_value(j, &[label("m"), submenu.try_clone_pair()], leaves)
                    })
                    .collect();
                node_value(i, &[label("s"), submenu.try_clone_pair()], kids)
            })
            .collect();
        assert!(count(&menu_from_layout(&root(big))) <= MAX_MENU_TOTAL);
    }

    trait ClonePair {
        fn try_clone_pair(&self) -> (&'static str, OwnedValue);
    }

    impl ClonePair for (&'static str, OwnedValue) {
        fn try_clone_pair(&self) -> (&'static str, OwnedValue) {
            (self.0, self.1.try_clone().expect("cloned"))
        }
    }

    /// A `com.canonical.dbusmenu` served in the test.
    struct FakeMenu {
        layout: Arc<Mutex<Layout>>,
        events: Arc<Mutex<Vec<String>>>,
        needs_update: bool,
    }

    fn clone_layout(layout: &Layout) -> Layout {
        (
            layout.0,
            layout
                .1
                .iter()
                .map(|(k, v)| (k.clone(), v.try_clone().expect("cloned")))
                .collect(),
            layout
                .2
                .iter()
                .map(|v| v.try_clone().expect("cloned"))
                .collect(),
        )
    }

    #[zbus::interface(name = "com.canonical.dbusmenu")]
    impl FakeMenu {
        fn about_to_show(&self, id: i32) -> bool {
            lock(&self.events).push(format!("AboutToShow({id})"));
            self.needs_update
        }

        fn get_layout(&self, parent: i32, depth: i32, names: Vec<String>) -> (u32, Layout) {
            lock(&self.events).push(format!("GetLayout({parent},{depth},{})", names.len()));
            (1, clone_layout(&lock(&self.layout)))
        }

        fn event(&self, id: i32, kind: &str, _data: Value<'_>, timestamp: u32) {
            lock(&self.events).push(format!("Event({id},{kind},{timestamp})"));
        }
    }

    fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        match mutex.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    struct Served {
        connection: Connection,
        layout: Arc<Mutex<Layout>>,
        events: Arc<Mutex<Vec<String>>>,
    }

    fn serve_menu(bus: &PrivateBus, layout: Layout, needs_update: bool) -> Served {
        let layout = Arc::new(Mutex::new(layout));
        let events = Arc::new(Mutex::new(Vec::new()));
        let connection = bus
            .builder()
            .serve_at(
                "/Menu",
                FakeMenu {
                    layout: Arc::clone(&layout),
                    events: Arc::clone(&events),
                    needs_update,
                },
            )
            .expect("served")
            .build()
            .expect("a menu connection");
        Served {
            connection,
            layout,
            events,
        }
    }

    fn address(served: &Served) -> ItemAddress {
        ItemAddress {
            id: ItemId {
                bus_name: "org.fake.Item".into(),
                path: "/StatusNotifierItem".into(),
            },
            owner: served
                .connection
                .unique_name()
                .expect("a unique name")
                .as_str()
                .to_owned(),
            interface: "org.kde.StatusNotifierItem",
            menu: Some("/Menu".to_owned()),
        }
    }

    fn client_connection(bus: &PrivateBus) -> Connection {
        bus.builder()
            .method_timeout(Duration::from_secs(2))
            .build()
            .expect("a client connection")
    }

    fn simple_layout() -> Layout {
        root(vec![
            node_value(1, &[label("_Open")], vec![]),
            node_value(2, &[("type", owned("separator".to_owned()))], vec![]),
            node_value(3, &[label("Quit"), ("enabled", owned(false))], vec![]),
        ])
    }

    #[test]
    fn opening_asks_about_to_show_then_the_layout_and_clicks_reach_the_menu() {
        let Some(bus) = PrivateBus::start("menu-open") else {
            return;
        };
        let served = serve_menu(&bus, simple_layout(), false);
        let client = client_connection(&bus);
        let item = address(&served);
        let mut menu = DbusMenu::default();
        let mut published = Vec::new();
        menu.open_menu(&client, &item, &mut |event| {
            published.push(event);
            true
        });
        let [TrayEvent::Menu(id, tree)] = published.as_slice() else {
            panic!("expected one menu, got {published:?}");
        };
        assert_eq!(*id, item.id);
        assert_eq!(tree.entries.len(), 3);
        assert_eq!(tree.entries[0].label, "Open");
        assert!(tree.entries[1].separator);
        assert!(!tree.entries[2].enabled);
        assert_eq!(
            lock(&served.events).as_slice(),
            ["AboutToShow(0)", "GetLayout(0,4,7)"]
        );

        menu.menu_event(&client, &item, 3);
        let deadline = Instant::now() + WAIT;
        while !lock(&served.events).iter().any(|e| e.starts_with("Event")) {
            assert!(Instant::now() < deadline, "the click never arrived");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            lock(&served.events).last().map(String::as_str),
            Some("Event(3,clicked,0)")
        );
    }

    #[test]
    fn a_submenu_about_to_show_republishes_only_when_the_menu_asks() {
        let Some(bus) = PrivateBus::start("menu-submenu") else {
            return;
        };
        let quiet = serve_menu(&bus, simple_layout(), false);
        let client = client_connection(&bus);
        let mut menu = DbusMenu::default();
        let mut published = 0;
        menu.about_to_show(&client, &address(&quiet), 5, &mut |_| {
            published += 1;
            true
        });
        assert_eq!(published, 0);
        assert_eq!(lock(&quiet.events).as_slice(), ["AboutToShow(5)"]);

        let busy_bus = PrivateBus::start("menu-submenu-2").expect("a second bus");
        let busy = serve_menu(&busy_bus, simple_layout(), true);
        let client = client_connection(&busy_bus);
        menu.about_to_show(&client, &address(&busy), 5, &mut |_| {
            published += 1;
            true
        });
        assert_eq!(published, 1);
    }

    #[test]
    fn a_closed_menu_is_not_followed_any_more() {
        let Some(bus) = PrivateBus::start("menu-close") else {
            return;
        };
        let served = serve_menu(&bus, simple_layout(), false);
        let client = client_connection(&bus);
        let item = address(&served);
        let mut menu = DbusMenu::default();
        let mut published = 0;
        menu.open_menu(&client, &item, &mut |_| {
            published += 1;
            true
        });
        assert_eq!(published, 1);
        // Open, a change is followed...
        menu.menu_changed(&client, &item.owner, "/Menu", &mut |_| {
            published += 1;
            true
        });
        assert_eq!(published, 2);
        // ...closed, it is not.
        menu.close_menu(&item);
        menu.menu_changed(&client, &item.owner, "/Menu", &mut |_| {
            published += 1;
            true
        });
        assert_eq!(published, 2);
        assert_eq!(
            lock(&served.events)
                .iter()
                .filter(|e| e.starts_with("GetLayout"))
                .count(),
            2
        );
    }

    #[test]
    fn hostile_layouts_over_the_wire_are_bounded_or_refused() {
        let Some(bus) = PrivateBus::start("menu-hostile") else {
            return;
        };
        let wide: Vec<_> = (0..3000)
            .map(|i| node_value(i, &[label("entry")], vec![]))
            .collect();
        let served = serve_menu(&bus, root(wide), false);
        let client = client_connection(&bus);
        let item = address(&served);
        let mut menu = DbusMenu::default();
        let mut published = Vec::new();
        menu.open_menu(&client, &item, &mut |event| {
            published.push(event);
            true
        });
        let [TrayEvent::Menu(_, tree)] = published.as_slice() else {
            panic!("expected one menu");
        };
        assert_eq!(tree.entries.len(), MAX_MENU_ENTRIES);

        let fat: Vec<_> = (0..12_000)
            .map(|i| node_value(i, &[label(&"x".repeat(100))], vec![]))
            .collect();
        *lock(&served.layout) = root(fat);
        published.clear();
        menu.open_menu(&client, &item, &mut |event| {
            published.push(event);
            true
        });
        assert!(
            published.is_empty(),
            "a layout past the byte bound is refused"
        );

        let submenu = ("children-display", owned("submenu".to_owned()));
        let nested = |levels: i32| {
            let mut deep = node_value(1, &[label("leaf")], vec![]);
            for id in 2..levels {
                deep = node_value(id, &[label("n"), submenu.try_clone_pair()], vec![deep]);
            }
            root(vec![deep])
        };
        // Past zvariant's own container-depth limit the reply never decodes:
        // no menu, and no panic either.
        *lock(&served.layout) = nested(40);
        menu.open_menu(&client, &item, &mut |event| {
            published.push(event);
            true
        });
        assert!(published.is_empty(), "an over-deep reply yields no menu");

        // Within it, our own depth bound applies.
        *lock(&served.layout) = nested(8);
        menu.open_menu(&client, &item, &mut |event| {
            published.push(event);
            true
        });
        let [TrayEvent::Menu(_, tree)] = published.as_slice() else {
            panic!("expected the nested menu, bounded");
        };
        assert_eq!(count(tree), MAX_MENU_DEPTH);
    }

    struct FakeItem;

    #[zbus::interface(name = "org.kde.StatusNotifierItem")]
    impl FakeItem {
        #[zbus(property)]
        fn id(&self) -> String {
            "fake".to_owned()
        }

        #[zbus(property)]
        fn icon_name(&self) -> String {
            "icon".to_owned()
        }

        #[zbus(property)]
        fn menu(&self) -> OwnedObjectPath {
            OwnedObjectPath::try_from("/Menu").expect("a path")
        }
    }

    fn menu_until(rx: &Receiver<TrayEvent>, accept: impl Fn(&MenuTree) -> bool) -> MenuTree {
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(TrayEvent::Menu(_, tree)) if accept(&tree) => return tree,
                Ok(_) => {}
                Err(_) => panic!("no matching menu within the wait"),
            }
        }
    }

    #[test]
    fn the_host_serves_menus_follows_updates_and_forwards_clicks() {
        let Some(bus) = PrivateBus::start("menu-host") else {
            return;
        };
        let (tx, rx) = channel();
        let publish: Publish = Box::new(move |event| tx.send(event).is_ok());
        let shutdown = Arc::new(AtomicBool::new(false));
        let (provider, worker) = spawn_on(
            Ok(bus.builder()),
            publish,
            crate::providers::tray::production_menu(),
            &shutdown,
        )
        .expect("the tray starts");
        // Both the item and its menu live on one connection.
        let served = serve_menu(&bus, simple_layout(), false);
        served
            .connection
            .object_server()
            .at("/StatusNotifierItem", FakeItem)
            .expect("the item");
        let deadline = Instant::now() + WAIT;
        loop {
            // The host claims the watcher a moment after it starts.
            if served
                .connection
                .call_method(
                    Some(WATCHER_NAME),
                    WATCHER_PATH,
                    Some(WATCHER_INTERFACE),
                    "RegisterStatusNotifierItem",
                    &("/StatusNotifierItem",),
                )
                .is_ok()
            {
                break;
            }
            assert!(Instant::now() < deadline, "the watcher never appeared");
            std::thread::sleep(Duration::from_millis(20));
        }
        let item_id = loop {
            match rx.recv_timeout(WAIT) {
                Ok(TrayEvent::Items(items)) if !items.is_empty() => {
                    break items
                        .first()
                        .map(|i: &TrayItem| i.id.clone())
                        .expect("an item");
                }
                Ok(_) => {}
                Err(_) => panic!("the item never listed"),
            }
        };
        provider
            .command(TrayCommand::OpenMenu(item_id.clone()))
            .expect("queued");
        let first = menu_until(&rx, |_| true);
        assert_eq!(first.entries.len(), 3);

        // The menu changes and says so: the open menu is published again.
        *lock(&served.layout) = root(vec![node_value(1, &[label("Changed")], vec![])]);
        served
            .connection
            .emit_signal(
                None::<&str>,
                "/Menu",
                MENU_INTERFACE,
                "LayoutUpdated",
                &(2u32, 0i32),
            )
            .expect("signalled");
        let changed = menu_until(&rx, |t| t.entries.len() == 1);
        assert_eq!(changed.entries[0].label, "Changed");

        provider
            .command(TrayCommand::MenuEvent(item_id, 1))
            .expect("queued");
        let deadline = Instant::now() + WAIT;
        while !lock(&served.events)
            .iter()
            .any(|e| e == "Event(1,clicked,0)")
        {
            assert!(Instant::now() < deadline, "the click never arrived");
            std::thread::sleep(Duration::from_millis(10));
        }
        shutdown.store(true, std::sync::atomic::Ordering::Release);
        worker.join();
    }
}
