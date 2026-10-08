use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

// Every QML file in one list, so it is both registered in the module and
// watched for rebuilds: two lists once let an edited file compile "fine"
// without reaching the binary.
const QML_FILES: &[&str] = &[
    // The suite's shared visual language, symlinked from ../celestina-style.
    "qml/CelestinaBackdrop.qml",
    "qml/CelestinaButton.qml",
    "qml/CelestinaCapsule.qml",
    "qml/CelestinaFocusRing.qml",
    "qml/CelestinaIcon.qml",
    "qml/CelestinaIconButton.qml",
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
    // Cuprita's own composition: Main owns the window, the components own
    // one region each and the pages one section each.
    "qml/components/DeviceRow.qml",
    "qml/components/EndpointCard.qml",
    "qml/components/NavItem.qml",
    "qml/components/NavStrip.qml",
    "qml/components/NetworkRow.qml",
    "qml/components/NoticePill.qml",
    "qml/components/SettingRow.qml",
    "qml/components/SectionShortcuts.qml",
    "qml/components/StreamRow.qml",
    "qml/dialogs/WifiPasswordDialog.qml",
    "qml/pages/AudioPage.qml",
    "qml/pages/BluetoothPage.qml",
    "qml/pages/NetworkPage.qml",
    "qml/Main.qml",
];

fn main() {
    // CelestinaTheme and CelestinaIcons are singletons that live canonically
    // in ../celestina-style; symlinked into qml/ so they register under a
    // clean `qml/...` resource path (a `..` in the source path would embed
    // `..` in the qrc alias and break type resolution at run time).
    let module = QmlModule::new("org.celestina.cuprita")
        .version(1, 0)
        // The list models derive from QAbstractListModel, which QtQml.Models
        // describes; without the dependency qmllint cannot resolve them.
        .depend("QtQml.Models")
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
            QmlFile::from("qml/components/Sections.qml")
                .version(1, 0)
                .singleton(true),
        )
        .qml_files(QML_FILES);

    // Naming any rerun-if-changed stops cargo watching the whole package, so
    // every watched file must be listed explicitly.
    for qml in QML_FILES.iter().copied().chain([
        "qml/CelestinaTheme.qml",
        "qml/CelestinaIcons.qml",
        "qml/components/Sections.qml",
        "qml/icons.qrc",
        "qml/fonts.qrc",
    ]) {
        println!("cargo::rerun-if-changed={qml}");
    }

    CxxQtBuilder::new_qml_module(module)
        // Shared icon resources and Inter Variable, compiled in.
        .qrc("qml/icons.qrc")
        .qrc("qml/fonts.qrc")
        .files([
            "src/controller/activation.rs",
            "src/controller/app.rs",
            "src/controller/audio.rs",
            "src/controller/bluetooth.rs",
            "src/controller/device_model.rs",
            "src/controller/endpoint_model.rs",
            "src/controller/network.rs",
            "src/controller/network_model.rs",
            "src/controller/stream_model.rs",
        ])
        .build();
}
