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
    "qml/CelestinaTextField.qml",
    "qml/CelestinaInputShield.qml",
    "qml/CelestinaModalLayer.qml",
    "qml/GlassCard.qml",
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
    "qml/components/folder/FolderHeading.qml",
    "qml/components/folder/FolderContentChrome.qml",
    "qml/components/folder/FolderContentFrame.qml",
    "qml/components/folder/FolderEmptyState.qml",
    "qml/components/picker/PickerChrome.qml",
    "qml/components/picker/PickerCellDelegate.qml",
    "qml/components/picker/PickerFilterMenu.qml",
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
        .qml_files(QML_FILES);

    // Los QML también, y explícitamente: en cuanto este script imprime un solo
    // `rerun-if-changed`, cargo deja de vigilar el paquete entero y sólo mira lo
    // que se le nombra. Sin estas líneas una edición de QML compilaba "bien" y
    // no entraba en el binario — un rato perdido persiguiendo un cambio que
    // estaba en el fichero y no en la aplicación.
    for qml in QML_FILES.iter().copied().chain([
        "qml/CelestinaTheme.qml",
        "qml/CelestinaIcons.qml",
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
    println!("cargo::rerun-if-changed=cpp/thumbnailprovider.cpp");
    println!("cargo::rerun-if-changed=cpp/siderita/thumbnailprovider.h");
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
        .files([
            "src/controller.rs",
            "src/dbus.rs",
            "src/editor.rs",
            "src/portal.rs",
        ]);
    // SAFETY: only adds an include directory for our own headers.
    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.include("cpp");
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
