use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

fn main() {
    // The theme, glass components and fallback icons are the suite's shared
    // visual language and live canonically in ../celestina-style. They are
    // symlinked into qml/i1/ (CelestinaTheme.qml, Glass*.qml, icons.qrc, icons/)
    // so the module registers them under clean `qml/i1/...` resource paths.
    // A direct `../celestina-style/...` source path would embed `..` in the qrc
    // alias (`org/celestina/siderita/../celestina-style/...`) and break QML type
    // resolution at runtime. Only MainI1.qml is genuinely Siderita's own file.
    let module = QmlModule::new("org.celestina.siderita")
        .version(1, 0)
        .qml_file(
            QmlFile::from("qml/i1/CelestinaTheme.qml")
                .version(1, 0)
                .singleton(true),
        )
        .qml_files([
            "qml/i1/GlassSurface.qml",
            "qml/i1/GlassContextMenu.qml",
            "qml/i1/GlassMenuItem.qml",
            "qml/i1/MainI1.qml",
        ]);

    let builder = CxxQtBuilder::new_qml_module(module)
        .qrc("qml/i1/icons.qrc")
        .files(["src/controller.rs"]);

    // Qt QML links Network on macOS even though Siderita itself is offline.
    let builder = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        builder.qt_module("Network")
    } else {
        builder
    };

    builder.build();
}
