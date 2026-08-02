use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

// Every QML file in one list, so it is both registered in the module and
// watched for rebuilds — the discipline Siderita learned the hard way, where
// two lists meant an edited QML compiled "fine" without reaching the binary.
const QML_FILES: &[&str] = &[
    // The suite's shared visual language, symlinked from ../celestina-style.
    "qml/CelestinaButton.qml",
    "qml/CelestinaSurface.qml",
    "qml/CelestinaBackdrop.qml",
    "qml/CelestinaIcon.qml",
    "qml/CelestinaIconButton.qml",
    "qml/CelestinaSectionLabel.qml",
    "qml/CelestinaFocusRing.qml",
    "qml/CelestinaTextField.qml",
    // La capa modal declara su suelo de entrada con este primitivo, así que
    // el módulo de la app tiene que publicarlo también.
    "qml/CelestinaInputShield.qml",
    "qml/CelestinaModalLayer.qml",
    "qml/GlassSurface.qml",
    "qml/GlassCard.qml",
    // Grafita's own composition: Main owns the window, the components own one
    // region each.
    "qml/components/DocumentView.qml",
    "qml/components/TabStrip.qml",
    "qml/components/FindBar.qml",
    "qml/components/DocumentHeader.qml",
    "qml/components/DocumentFooter.qml",
    "qml/components/UnsavedDialog.qml",
    "qml/Main.qml",
];

fn main() {
    // CelestinaTheme and CelestinaIcons are singletons and live canonically in
    // ../celestina-style; they are symlinked into qml/ so they register under a
    // clean `qml/...` resource path. A direct `../celestina-style/...` source
    // path would embed `..` in the qrc alias and break type resolution at run
    // time.
    let module = QmlModule::new("org.celestina.grafita")
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

    // Naming any rerun-if-changed stops cargo watching the whole package, so
    // every watched file must be listed explicitly.
    for qml in QML_FILES.iter().copied().chain([
        "qml/CelestinaTheme.qml",
        "qml/CelestinaIcons.qml",
        "qml/icons.qrc",
        "qml/fonts.qrc",
    ]) {
        println!("cargo::rerun-if-changed={qml}");
    }
    for source in ["cpp/highlighter.cpp", "cpp/highlighter.h"] {
        println!("cargo::rerun-if-changed={source}");
    }

    let builder = CxxQtBuilder::new_qml_module(module)
        // Shared icon and noise resources used by the glass surfaces.
        .qrc("qml/icons.qrc")
        // Inter Variable, compiled in so the app renders in the suite's
        // typeface (the canonical fonts.qrc is a style symlink).
        .qrc("qml/fonts.qrc")
        // The syntax highlighter: hand-written C++ because colouring a Qt text
        // document without rewriting its text means overriding
        // QSyntaxHighlighter::highlightBlock, which CXX-Qt cannot express. The
        // header is moc'd (Q_OBJECT); the .cpp is compiled.
        .cpp_file("cpp/highlighter.cpp")
        .cpp_file("cpp/highlighter.h")
        .files(["src/activation.rs", "src/session.rs", "src/syntax.rs"]);

    // SAFETY: only adds an include directory for our own headers.
    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.include("cpp");
        })
    };
    builder.build();
}
