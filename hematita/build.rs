use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

// Every QML file in one list, so it is both registered in the module and
// watched for rebuilds: two lists once let an edited file compile "fine"
// without reaching the binary.
const QML_FILES: &[&str] = &[
    // The suite's shared visual language, symlinked from ../celestina-style.
    "qml/CelestinaButton.qml",
    "qml/CelestinaFocusRing.qml",
    "qml/CelestinaIcon.qml",
    "qml/CelestinaIconButton.qml",
    "qml/CelestinaCapsule.qml",
    "qml/CelestinaSurface.qml",
    "qml/CelestinaSectionLabel.qml",
    "qml/CelestinaTextField.qml",
    "qml/CelestinaRowHighlight.qml",
    "qml/CelestinaScrollBar.qml",
    "qml/CelestinaModalLayer.qml",
    "qml/CelestinaInputShield.qml",
    "qml/CelestinaShadow.qml",
    "qml/ListSection.qml",
    "qml/GlassSurface.qml",
    "qml/GlassCard.qml",
    // Hematita's own composition: Main owns the window, the components own
    // one region each.
    "qml/components/NavItem.qml",
    "qml/components/NavStrip.qml",
    "qml/components/HistoryGraph.qml",
    "qml/components/ResourceRow.qml",
    "qml/components/ResourceDetail.qml",
    "qml/components/CoreGrid.qml",
    "qml/components/PerformancePage.qml",
    "qml/components/ProcessHeader.qml",
    "qml/components/ProcessRow.qml",
    "qml/components/ApplicationRow.qml",
    "qml/components/ProcessTable.qml",
    "qml/components/ConfirmDialog.qml",
    "qml/components/ProcessPage.qml",
    "qml/components/ApplicationsPage.qml",
    "qml/components/SensorRow.qml",
    "qml/components/SensorChipCard.qml",
    "qml/components/SensorsPage.qml",
    "qml/components/ServiceRow.qml",
    "qml/components/ServicesPage.qml",
    "qml/components/PathCrumbs.qml",
    "qml/components/LocationList.qml",
    "qml/components/FolderList.qml",
    "qml/components/StoragePage.qml",
    "qml/Main.qml",
];

fn main() {
    // CelestinaTheme and CelestinaIcons are singletons that live canonically
    // in ../celestina-style; symlinked into qml/ so they register under a
    // clean `qml/...` resource path (a `..` in the source path would embed
    // `..` in the qrc alias and break type resolution at run time).
    let module = QmlModule::new("org.celestina.hematita")
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
    // The bridge files are watched by cxx-qt-build; the plain Rust modules
    // it does not know about are named here.
    println!("cargo::rerun-if-changed=src/browse.rs");
    println!("cargo::rerun-if-changed=src/lists.rs");
    println!("cargo::rerun-if-changed=src/locations.rs");
    println!("cargo::rerun-if-changed=src/privilege.rs");
    println!("cargo::rerun-if-changed=src/publish.rs");
    println!("cargo::rerun-if-changed=src/sampler.rs");

    CxxQtBuilder::new_qml_module(module)
        // Shared icon resources and Inter Variable, compiled in.
        .qrc("qml/icons.qrc")
        .qrc("qml/fonts.qrc")
        .files([
            "src/activation.rs",
            "src/analysis.rs",
            "src/resources.rs",
            "src/processes.rs",
            "src/sensors.rs",
            "src/services.rs",
        ])
        .build();
}
