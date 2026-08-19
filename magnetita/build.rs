use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

// The app's QML, in one place so it is both registered and watched — the same
// single-list discipline Siderita learned the hard way. CelestinaButton is the
// suite's shared button (symlinked from celestina-style), so the app never forks
// its own.
const QML_FILES: &[&str] = &[
    "qml/CelestinaButton.qml",
    "qml/CelestinaSurface.qml",
    "qml/CelestinaBackdrop.qml",
    "qml/CelestinaIcon.qml",
    "qml/CelestinaIconButton.qml",
    "qml/CelestinaSectionLabel.qml",
    "qml/CelestinaFocusRing.qml",
    "qml/CelestinaSwitch.qml",
    "qml/CelestinaTextField.qml",
    "qml/GlassSurface.qml",
    "qml/ListSection.qml",
    // App composition: Main owns state/navigation; pages compose reusable pieces.
    "qml/components/AppHeader.qml",
    "qml/components/ConnectedDeviceCard.qml",
    "qml/components/MediaProgress.qml",
    "qml/components/MediaCard.qml",
    "qml/components/DeviceControls.qml",
    "qml/components/QuietIconButton.qml",
    "qml/components/MirrorChoiceRow.qml",
    "qml/components/MirrorSettingsSheet.qml",
    "qml/components/ActivityLog.qml",
    "qml/components/PairedDeviceRow.qml",
    "qml/components/PluginRow.qml",
    "qml/pages/DevicesPage.qml",
    "qml/pages/SettingsPage.qml",
    "qml/Main.qml",
];

fn main() {
    // CelestinaTheme is the suite's shared visual language; it lives canonically
    // in ../celestina-style and is symlinked into qml/ so it registers under a
    // clean `qml/...` resource path (a `../celestina-style/...` source path would
    // embed `..` in the qrc alias and break QML type resolution at runtime).
    let module = QmlModule::new("org.celestina.magnetita")
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
    // every watched QML must be listed explicitly or an edit compiles "fine"
    // without reaching the binary.
    for qml in QML_FILES.iter().copied().chain([
        "qml/CelestinaTheme.qml",
        "qml/CelestinaIcons.qml",
        "qml/fonts.qrc",
        "qml/icons.qrc",
    ]) {
        println!("cargo::rerun-if-changed={qml}");
    }

    CxxQtBuilder::new_qml_module(module)
        // Shared icon/noise resources used by GlassSurface and future icon-only
        // controls; the qrc and directory are canonical style symlinks.
        .qrc("qml/icons.qrc")
        // Inter Variable, compiled in so the app renders in the suite's typeface
        // (the canonical fonts.qrc lives in ../celestina-style, symlinked into qml/).
        .qrc("qml/fonts.qrc")
        .files(["src/controller.rs"])
        .build();
}
