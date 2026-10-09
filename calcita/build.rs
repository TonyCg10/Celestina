use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

/// The two shared singletons, registered as such in `main`; watched like the
/// rest.
const SINGLETONS: &[&str] = &["qml/CelestinaTheme.qml", "qml/CelestinaIcons.qml"];

// One list both registers every QML file in the module and watches it for
// rebuilds, so an edited file can never compile without reaching the binary.
const QML_FILES: &[&str] = &[
    // The suite's shared visual language, symlinked from ../celestina-style.
    "qml/CelestinaAppearance.qml",
    "qml/CelestinaBackdrop.qml",
    "qml/CelestinaButton.qml",
    "qml/CelestinaCapsule.qml",
    "qml/CelestinaFocusRing.qml",
    "qml/CelestinaIconButton.qml",
    "qml/CelestinaIcon.qml",
    "qml/CelestinaInputShield.qml",
    "qml/CelestinaModalLayer.qml",
    "qml/CelestinaRowHighlight.qml",
    "qml/CelestinaScrollBar.qml",
    "qml/CelestinaSectionLabel.qml",
    "qml/CelestinaShadow.qml",
    "qml/CelestinaSlider.qml",
    "qml/CelestinaSurface.qml",
    "qml/CelestinaSwitch.qml",
    "qml/CelestinaTextField.qml",
    "qml/CelestinaTreemap.qml",
    "qml/CelestinaUsageList.qml",
    "qml/CelestinaWheelScroll.qml",
    "qml/GlassCard.qml",
    "qml/GlassContextMenu.qml",
    "qml/GlassMenuItem.qml",
    "qml/GlassSurface.qml",
    "qml/ListSection.qml",
    // Calcita's own composition: Main owns the window, each component one
    // region.
    "qml/components/ActivationRoute.qml",
    "qml/components/EmptyState.qml",
    "qml/Main.qml",
];

fn main() {
    // The shared files are symlinked into qml/ so they register under a clean
    // `qml/...` resource path: a `..` in a source path would reach the qrc
    // alias and break type resolution at run time.
    let module = QmlModule::new("org.celestina.calcita")
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

    // Naming any rerun-if-changed stops cargo from watching the whole
    // package, so every input is listed.
    for input in QML_FILES
        .iter()
        .chain(SINGLETONS)
        .copied()
        .chain(["qml/icons.qrc", "qml/fonts.qrc"])
    {
        println!("cargo::rerun-if-changed={input}");
    }

    CxxQtBuilder::new_qml_module(module)
        // The shared icons and Inter Variable, compiled in.
        .qrc("qml/icons.qrc")
        .qrc("qml/fonts.qrc")
        .files(["src/activation.rs", "src/controller.rs"])
        .build();
}
