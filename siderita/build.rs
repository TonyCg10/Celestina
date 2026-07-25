use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

// Los ficheros QML del módulo, en un solo sitio: se registran en el módulo y se
// vigilan para recompilar. Tenerlos en dos listas fue exactamente el fallo que
// hacía que una edición de QML no llegara al binario.
const QML_FILES: &[&str] = &[
    // Lenguaje visual compartido (enlaces a ../celestina-style).
    "qml/i1/GlassSurface.qml",
    "qml/i1/GlassCard.qml",
    "qml/i1/GlassContextMenu.qml",
    "qml/i1/GlassMenuItem.qml",
    // Piezas de Siderita.
    "qml/i1/GlassPill.qml",
    "qml/i1/SidebarChevron.qml",
    "qml/i1/FavoriteBadge.qml",
    "qml/i1/NavButton.qml",
    "qml/i1/PillButton.qml",
    "qml/i1/InfoPill.qml",
    "qml/i1/RuleField.qml",
    "qml/i1/DragScrollEdge.qml",
    "qml/i1/SizeRow.qml",
    "qml/i1/PropRow.qml",
    // Ventanas.
    "qml/i1/MainI1.qml",
    "qml/i1/PickerWindow.qml",
];

fn main() {
    // The theme, glass components and fallback icons are the suite's shared
    // visual language and live canonically in ../celestina-style. They are
    // symlinked into qml/i1/ (CelestinaTheme.qml, Glass*.qml, icons.qrc, icons/)
    // so the module registers them under clean `qml/i1/...` resource paths.
    // A direct `../celestina-style/...` source path would embed `..` in the qrc
    // alias (`org/celestina/siderita/../celestina-style/...`) and break QML type
    // resolution at runtime. El resto son de Siderita.
    let module = QmlModule::new("org.celestina.siderita")
        .version(1, 0)
        .qml_file(
            QmlFile::from("qml/i1/CelestinaTheme.qml")
                .version(1, 0)
                .singleton(true),
        )
        .qml_files(QML_FILES);

    // Los QML también, y explícitamente: en cuanto este script imprime un solo
    // `rerun-if-changed`, cargo deja de vigilar el paquete entero y sólo mira lo
    // que se le nombra. Sin estas líneas una edición de QML compilaba "bien" y
    // no entraba en el binario — un rato perdido persiguiendo un cambio que
    // estaba en el fichero y no en la aplicación.
    for qml in QML_FILES
        .iter()
        .copied()
        .chain(["qml/i1/CelestinaTheme.qml", "qml/i1/icons.qrc"])
    {
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
    println!("cargo::rerun-if-changed=cpp/icontheme.cpp");
    println!("cargo::rerun-if-changed=cpp/siderita/icontheme.h");
    let builder = CxxQtBuilder::new_qml_module(module)
        .qrc("qml/i1/icons.qrc")
        .cpp_file("cpp/clipboard.cpp")
        // The native list model: the header is moc'd (Q_OBJECT), the .cpp compiled.
        .cpp_file("cpp/entrymodel.cpp")
        .cpp_file("cpp/siderita/entrymodel.h")
        // The freedesktop-thumbnail image provider (no Q_OBJECT of its own — it
        // only emits QQuickImageResponse's inherited signal — so just compiled).
        .cpp_file("cpp/thumbnailprovider.cpp")
        // Pins the freedesktop icon theme before any QML loads (no Q_OBJECT).
        .cpp_file("cpp/icontheme.cpp")
        .files(["src/controller.rs", "src/dbus.rs", "src/portal.rs"]);
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
