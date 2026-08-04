use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

// Every QML file in one list, so it is both registered in the module and
// watched for rebuilds. Two lists is how Siderita once shipped an edited QML
// that compiled "fine" without ever reaching the binary.
const QML_FILES: &[&str] = &[
    // The suite's shared visual language, symlinked from ../celestina-style.
    "qml/CelestinaSurface.qml",
    "qml/CelestinaSlider.qml",
    "qml/CelestinaBackdrop.qml",
    "qml/CelestinaInputShield.qml",
    "qml/CelestinaModalLayer.qml",
    "qml/GlassCard.qml",
    "qml/GlassContextMenu.qml",
    "qml/GlassMenuItem.qml",
    "qml/GlassSurface.qml",
    "qml/CelestinaSectionLabel.qml",
    "qml/CelestinaIcon.qml",
    "qml/CelestinaFocusRing.qml",
    "qml/CelestinaButton.qml",
    "qml/CelestinaIconButton.qml",
    // Fluorita's own composition: the window hosts, each component owns one
    // region.
    "qml/components/AmbientLight.qml",
    "qml/components/ContentArrows.qml",
    "qml/components/ContentDock.qml",
    "qml/components/ContentNavigator.qml",
    "qml/components/GalleryGrid.qml",
    "qml/components/ImageView.qml",
    "qml/components/ItemDetailPanel.qml",
    "qml/components/ItemMenu.qml",
    "qml/components/LibrarySidebar.qml",
    "qml/components/LibraryView.qml",
    "qml/components/MusicList.qml",
    "qml/components/PlayerSurface.qml",
    "qml/components/SidebarRow.qml",
    "qml/components/PlayerTransport.qml",
    "qml/components/SeekBar.qml",
    "qml/components/VolumeBar.qml",
    "qml/Main.qml",
];

fn main() {
    // CelestinaTheme and CelestinaIcons are singletons and live canonically in
    // ../celestina-style; they are symlinked into qml/ so they register under a
    // clean `qml/...` resource path. A direct `../celestina-style/...` source
    // path would embed `..` in the qrc alias and break type resolution at run
    // time.
    let module = QmlModule::new("org.celestina.fluorita")
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

    // The Qt Quick video surface is hand-written C++: CXX-Qt 0.9 cannot
    // subclass `QQuickFramebufferObject` nor override its virtual
    // `createRenderer()`, and libmpv's render API must run on Qt's render
    // thread. `cpp/` goes on the include path so both the generated bridge and
    // the shim resolve "fluorita/mpvvideoitem.h".
    println!("cargo::rerun-if-changed=cpp/imageprobe.cpp");
    println!("cargo::rerun-if-changed=cpp/fluorita/imageprobe.h");
    for source in fluorita_qt::rerun_paths() {
        println!("cargo::rerun-if-changed={source}");
    }

    let builder = CxxQtBuilder::new_qml_module(module)
        // The video surface renders into a QOpenGLFramebufferObject, which
        // lives in Qt's OpenGL module rather than QtGui.
        .qt_module("OpenGL")
        // Shared icon and noise resources used by the glass surfaces.
        .qrc("qml/icons.qrc")
        // Inter Variable, compiled in so the app renders in the suite's
        // typeface (the canonical fonts.qrc is a style symlink).
        .qrc("qml/fonts.qrc")
        // The header carries Q_OBJECT, so it is moc'd as well as compiled.
        // No Q_OBJECT: a free function over QImageReader, so it is only
        // compiled, not moc'd.
        .cpp_file("cpp/imageprobe.cpp")
        // The shared render seam, compiled from the crate that owns it.
        .cpp_file(fluorita_qt::VIDEO_ITEM_SOURCE)
        .cpp_file(fluorita_qt::VIDEO_ITEM_HEADER)
        .files(["src/library.rs", "src/player.rs"]);

    // SAFETY: only adds an include directory for our own headers.
    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.include("cpp");
            cc.include(fluorita_qt::include_dir());
        })
    };

    builder.build();
}
