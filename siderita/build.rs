use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

// Los ficheros QML del módulo, en un solo sitio: se registran en el módulo y se
// vigilan para recompilar. Tenerlos en dos listas fue exactamente el fallo que
// hacía que una edición de QML no llegara al binario.
const QML_FILES: &[&str] = &[
    // Lenguaje visual compartido (enlaces a ../celestina-style).
    "qml/GlassSurface.qml",
    "qml/CelestinaButton.qml",
    "qml/CelestinaSurface.qml",
    "qml/CelestinaBackdrop.qml",
    "qml/CelestinaIcon.qml",
    "qml/CelestinaIconButton.qml",
    "qml/CelestinaSectionLabel.qml",
    "qml/CelestinaFocusRing.qml",
    "qml/CelestinaSlider.qml",
    // Shared with Grafita: the quick look reads a file the same way the
    // editor does, so it numbers and scrolls with the same two components.
    "qml/CelestinaScrollBar.qml",
    "qml/CelestinaLineGutter.qml",
    "qml/CelestinaTextField.qml",
    "qml/CelestinaInputShield.qml",
    "qml/CelestinaFolderIcon.qml",
    "qml/CelestinaFileIcon.qml",
    "qml/CelestinaModalLayer.qml",
    "qml/GlassCard.qml",
    // The one elevation shadow every glass surface casts; the menu draws it
    // inline, so the app's module must publish it too.
    "qml/CelestinaShadow.qml",
    "qml/GlassContextMenu.qml",
    "qml/GlassMenuItem.qml",
    // Componentes de presentación de Siderita.
    "qml/components/chrome/GlassPill.qml",
    "qml/components/chrome/HistoryMouseArea.qml",
    "qml/components/chrome/RouteReveal.qml",
    "qml/components/chrome/FloatingButton.qml",
    "qml/components/chrome/HiddenTogglePill.qml",
    "qml/components/chrome/InfoPill.qml",
    "qml/components/chrome/SearchBar.qml",
    "qml/components/chrome/RecentHeader.qml",
    "qml/components/chrome/TrashHeader.qml",
    "qml/components/chrome/TabStrip.qml",
    "qml/components/chrome/DetailsHeader.qml",
    "qml/components/chrome/BottomControls.qml",
    "qml/components/chrome/TopBar.qml",
    "qml/components/sidebar/SidebarChevron.qml",
    "qml/components/sidebar/SidebarSectionHeader.qml",
    "qml/components/sidebar/SidebarPhoneSection.qml",
    "qml/components/sidebar/SidebarFavoriteRow.qml",
    "qml/components/sidebar/SidebarBookmarkRow.qml",
    "qml/components/sidebar/SidebarSavedSections.qml",
    "qml/components/sidebar/SidebarContextMenus.qml",
    "qml/components/sidebar/SidebarInfo.qml",
    "qml/components/entry/EntryGlyph.qml",
    "qml/components/entry/EntryIconRules.qml",
    "qml/components/entry/FavoriteBadge.qml",
    "qml/components/entry/DragScrollEdge.qml",
    "qml/components/entry/FolderRowDelegate.qml",
    "qml/components/entry/FolderCellDelegate.qml",
    "qml/components/folder/FolderListView.qml",
    "qml/components/folder/FolderGridView.qml",
    "qml/components/folder/FolderWheelHandler.qml",
    "qml/components/folder/FolderShortcuts.qml",
    "qml/components/folder/FolderBottomStatus.qml",
    "qml/components/folder/FolderBottomChrome.qml",
    "qml/components/folder/FolderActions.qml",
    "qml/components/folder/OperationsDock.qml",
    "qml/components/folder/OperationRing.qml",
    "qml/components/folder/OperationCallout.qml",
    "qml/components/folder/FolderHeading.qml",
    "qml/components/folder/PhoneMediaButton.qml",
    "qml/components/folder/PhoneMediaUnderBar.qml",
    "qml/components/folder/HeadingState.qml",
    "qml/components/folder/FolderContentChrome.qml",
    "qml/components/folder/FolderContentFrame.qml",
    "qml/components/folder/FolderEmptyState.qml",
    "qml/components/picker/PickerChrome.qml",
    "qml/components/picker/PickerCellDelegate.qml",
    "qml/components/picker/PickerFilterMenu.qml",
    "qml/components/picker/PickerOverwriteDialog.qml",
    "qml/components/picker/PickerSidebar.qml",
    "qml/components/picker/PickerIconRules.qml",
    "qml/components/SizeRow.qml",
    "qml/components/PropRow.qml",
    // Vistas compuestas.
    "qml/views/Sidebar.qml",
    "qml/views/FolderView.qml",
    // Diálogos y overlays de la vista de carpeta.
    "qml/dialogs/NamePromptDialog.qml",
    "qml/dialogs/BatchRenameDialog.qml",
    "qml/dialogs/ConflictDialog.qml",
    "qml/dialogs/OpenWithDialog.qml",
    "qml/dialogs/PropertiesDialog.qml",
    "qml/dialogs/IconPickerDialog.qml",
    "qml/dialogs/CompressDialog.qml",
    "qml/dialogs/PasswordDialog.qml",
    "qml/dialogs/MediaPreview.qml",
    "qml/dialogs/QuickLookView.qml",
    "qml/dialogs/GrafitaEditorDialog.qml",
    "qml/dialogs/PhoneMediaDialog.qml",
    // Menús y popups.
    "qml/menus/FolderSortMenu.qml",
    "qml/menus/PathMenu.qml",
    "qml/menus/FolderMenu.qml",
    "qml/menus/EntryContextMenu.qml",
    "qml/menus/IconAccentMenu.qml",
    "qml/menus/SizePopup.qml",
    // Puntos de entrada.
    "qml/Main.qml",
    "qml/PickerWindow.qml",
];

fn main() {
    // The theme, glass components and fallback icons are the suite's shared
    // visual language and live canonically in ../celestina-style. They are
    // symlinked into qml/ (CelestinaTheme.qml, Glass*.qml, icons.qrc, icons/)
    // so the module registers them under clean `qml/...` resource paths.
    // A direct `../celestina-style/...` source path would embed `..` in the qrc
    // alias (`org/celestina/siderita/../celestina-style/...`) and break QML type
    // resolution at runtime. El resto son de Siderita.
    let module = QmlModule::new("org.celestina.siderita")
        .version(1, 0)
        .qml_file(
            QmlFile::from("qml/CelestinaTheme.qml")
                .version(1, 0)
                .singleton(true),
        )
        .qml_file(
            QmlFile::from("qml/CelestinaIcons.qml")
                .version(1, 0)
                .singleton(true),
        )
        .qml_file(
            QmlFile::from("qml/CelestinaIconShapes.qml")
                .version(1, 0)
                .singleton(true),
        )
        .qml_file(
            QmlFile::from("qml/CelestinaPlaceDefs.qml")
                .version(1, 0)
                .singleton(true),
        )
        .qml_file(
            QmlFile::from("qml/CelestinaFolderTypeIcons.qml")
                .version(1, 0)
                .singleton(true),
        )
        .qml_files(QML_FILES);

    // Los QML también, y explícitamente: en cuanto este script imprime un solo
    // `rerun-if-changed`, cargo deja de vigilar el paquete entero y sólo mira lo
    // que se le nombra. Sin estas líneas una edición de QML compilaba "bien" y
    // no entraba en el binario — un rato perdido persiguiendo un cambio que
    // estaba en el fichero y no en la aplicación.
    for qml in QML_FILES.iter().copied().chain([
        "qml/CelestinaTheme.qml",
        "qml/CelestinaIcons.qml",
        "qml/CelestinaIconShapes.qml",
        "qml/CelestinaPlaceDefs.qml",
        "qml/CelestinaFolderTypeIcons.qml",
        "qml/icons.qrc",
        "qml/fonts.qrc",
    ]) {
        println!("cargo::rerun-if-changed={qml}");
    }

    // The system-clipboard bridge (text/uri-list + x-special/gnome-copied-files)
    // is the one piece of hand-written C++: cxx-qt-lib exposes no QClipboard /
    // QMimeData, so a small shim under cpp/ implements it and the controller
    // bridge declares its free functions. `cpp/` is put on the compiler's include
    // path so both the generated bridge code and clipboard.cpp resolve
    // "siderita/clipboard.h".
    println!("cargo::rerun-if-changed=cpp/clipboard.cpp");
    println!("cargo::rerun-if-changed=cpp/siderita/clipboard.h");
    println!("cargo::rerun-if-changed=cpp/entrymodel.cpp");
    println!("cargo::rerun-if-changed=cpp/siderita/entrymodel.h");
    for source in fluorita_qt::rerun_paths() {
        println!("cargo::rerun-if-changed={source}");
    }
    println!("cargo::rerun-if-changed=cpp/thumbnailprovider.cpp");
    println!("cargo::rerun-if-changed=cpp/siderita/thumbnailprovider.h");
    println!("cargo::rerun-if-changed=cpp/windowparent.cpp");
    println!("cargo::rerun-if-changed=cpp/siderita/windowparent.h");

    // `xdg-foreign` is how a dialog served by *this* process becomes a child of
    // a window owned by another one: the asking application exports its
    // toplevel, the portal hands us the resulting handle, and the picker imports
    // it. The protocol has no C library of its own — every client generates the
    // marshalling from the XML that ships with wayland-protocols, which is what
    // this does, into OUT_DIR. Generated as `.cpp` deliberately: the C the
    // scanner emits is valid C++ and its header already declares everything
    // `extern "C"`, so it joins the existing C++ compilation instead of needing
    // a second toolchain in the build.
    let parenting = wayland_parenting();
    if parenting.is_none() {
        println!(
            "cargo::warning=without wayland-scanner or xdg-foreign-unstable-v2.xml, \
             the picker cannot become a child of its requester"
        );
    }
    let builder = CxxQtBuilder::new_qml_module(module)
        .qrc("qml/icons.qrc")
        // Inter Variable, compiled in so the suite's typeface travels with the
        // binary (the canonical fonts.qrc lives in ../celestina-style, symlinked).
        .qrc("qml/fonts.qrc")
        .cpp_file("cpp/clipboard.cpp")
        // The native list model: the header is moc'd (Q_OBJECT), the .cpp compiled.
        .cpp_file("cpp/entrymodel.cpp")
        .cpp_file("cpp/siderita/entrymodel.h")
        // The freedesktop-thumbnail image provider (no Q_OBJECT of its own — it
        // only emits QQuickImageResponse's inherited signal — so just compiled).
        .cpp_file("cpp/thumbnailprovider.cpp")
        // The transient-parent shim: `Q_OBJECT`, so the header is moc'd too.
        .cpp_file("cpp/windowparent.cpp")
        .cpp_file("cpp/siderita/windowparent.h")
        // The shared media surface, compiled from the crate that owns it.
        .cpp_file(fluorita_qt::VIDEO_ITEM_SOURCE)
        .cpp_file(fluorita_qt::VIDEO_ITEM_HEADER)
        .files([
            "src/controller.rs",
            // Exports one function to the thumbnail provider: the picture a
            // file carries inside itself.
            "src/embedded.rs",
            "src/dbus.rs",
            "src/editor.rs",
            "src/media.rs",
            "src/portal.rs",
            "src/preferences.rs",
            // Binds one C++ helper so a test can pin the thumbnail cache key to
            // its Rust owner; it declares no QObject.
            "src/thumbnails.rs",
        ]);
    // The generated protocol joins the same compilation as the shim that uses
    // it, and only when it could be generated: without it the shim compiles to
    // an honest "no parenting available".
    let builder = match &parenting {
        Some(generated) => builder.cpp_file(&generated.source),
        None => builder,
    };

    // SAFETY: only adds include directories — our own headers, the generated
    // protocol, and Qt's versioned QPA headers.
    let builder = unsafe {
        builder.cc_builder(move |cc| {
            cc.include("cpp");
            cc.include(fluorita_qt::include_dir());
            if let Some(generated) = &parenting {
                cc.include(&generated.include_dir);
                cc.define("SIDERITA_HAS_XDG_FOREIGN", None);
                for include in &generated.qt_private_includes {
                    cc.include(include);
                }
            }
        })
    };

    // Qt QML links Network on macOS even though Siderita itself is offline.
    let builder = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        builder.qt_module("Network")
    } else {
        builder
    };

    builder.build();
}

/// What the transient-parent shim needs to compile: the generated
/// `xdg-foreign-unstable-v2` marshalling and the Qt headers that hand out the
/// window's `wl_surface`.
struct WaylandParenting {
    source: std::path::PathBuf,
    include_dir: std::path::PathBuf,
    /// Qt publishes `QNativeInterface::Private::QWaylandWindow` — the only
    /// supported way to reach a `QWindow`'s `wl_surface` — from a versioned QPA
    /// header rather than from the public include path. The version is asked of
    /// the same Qt the build is using, so this cannot silently pick another one.
    qt_private_includes: Vec<std::path::PathBuf>,
}

fn wayland_parenting() -> Option<WaylandParenting> {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return None;
    }

    let protocol = wayland_protocol_xml()?;
    println!("cargo::rerun-if-changed={}", protocol.display());
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").ok()?);
    let header = out.join("xdg-foreign-unstable-v2-client-protocol.h");
    let code = out.join("xdg-foreign-unstable-v2-protocol.c");
    for (mode, target) in [("client-header", &header), ("private-code", &code)] {
        let status = std::process::Command::new("wayland-scanner")
            .arg(mode)
            .arg(&protocol)
            .arg(target)
            .status()
            .ok()?;
        if !status.success() {
            return None;
        }
    }

    // The scanner emits C, and its interface tables are what the shim links
    // against. Two things have to be said for them to survive a C++ compiler,
    // and this wrapper says both. `extern "C"` gives them C linkage; including
    // the generated header *first* gives them external linkage at all, because
    // a `const` object at namespace scope with no prior `extern` declaration is
    // internal in C++ and external in C. Without the header the interfaces the
    // header does not happen to forward-declare simply vanish from the object
    // file, and only those.
    let source = out.join("xdg-foreign-unstable-v2-protocol-wrapper.cpp");
    std::fs::write(
        &source,
        format!(
            "#include \"{}\"\nextern \"C\" {{\n#include \"{}\"\n}}\n",
            header.display(),
            code.display()
        ),
    )
    .ok()?;

    let qt_private_includes = qt_private_include_dirs()?;
    // The generated marshalling calls into libwayland itself.
    println!("cargo::rustc-link-lib=wayland-client");
    Some(WaylandParenting {
        source,
        include_dir: out,
        qt_private_includes,
    })
}

fn wayland_protocol_xml() -> Option<std::path::PathBuf> {
    if std::process::Command::new("wayland-scanner")
        .arg("--version")
        .status()
        .map(|status| !status.success())
        .unwrap_or(true)
    {
        return None;
    }
    let base = std::process::Command::new("pkg-config")
        .args(["--variable=pkgdatadir", "wayland-protocols"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|path| std::path::PathBuf::from(path.trim()))
        .unwrap_or_else(|| std::path::PathBuf::from("/usr/share/wayland-protocols"));
    let xml = base.join("unstable/xdg-foreign/xdg-foreign-unstable-v2.xml");
    xml.exists().then_some(xml)
}

fn qt_private_include_dirs() -> Option<Vec<std::path::PathBuf>> {
    let query = |key: &str| {
        std::process::Command::new("qmake6")
            .args(["-query", key])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|value| value.trim().to_owned())
    };
    let headers = std::path::PathBuf::from(query("QT_INSTALL_HEADERS")?);
    let version = query("QT_VERSION")?;
    let dirs: Vec<std::path::PathBuf> = ["QtGui", "QtCore"]
        .iter()
        .map(|module| headers.join(module).join(&version))
        .collect();
    dirs.iter().all(|dir| dir.exists()).then_some(dirs)
}
