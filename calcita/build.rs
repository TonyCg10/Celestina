use std::path::{Path, PathBuf};
use std::process::Command;

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
    "qml/components/DocumentBar.qml",
    "qml/components/EmptyState.qml",
    "qml/components/LinkPill.qml",
    "qml/components/NoticePill.qml",
    "qml/components/PageView.qml",
    "qml/components/ReadingEffect.qml",
    "qml/components/WindowChrome.qml",
    "qml/DocumentWindow.qml",
    "qml/Main.qml",
    "qml/OutlinePanel.qml",
    "qml/SearchCard.qml",
];

/// The reading mode's fragment shader, compiled to `.qsb` below.
const READING_SHADER: &str = "qml/shaders/reading.frag";

/// Qt's shader baker: `QSB` when set, else the one beside the Qt that
/// `QMAKE` (or `qmake6`, `qmake`) names, in its host binaries or its
/// libexec folder.
fn find_qsb() -> PathBuf {
    if let Some(qsb) = std::env::var_os("QSB") {
        return PathBuf::from(qsb);
    }
    let qmakes: Vec<String> = std::env::var("QMAKE")
        .into_iter()
        .chain(["qmake6".to_owned(), "qmake".to_owned()])
        .collect();
    for qmake in qmakes {
        for query in ["QT_HOST_BINS", "QT_INSTALL_BINS", "QT_HOST_LIBEXECS"] {
            let Ok(output) = Command::new(&qmake).args(["-query", query]).output() else {
                continue;
            };
            let dir = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            let candidate = Path::new(&dir).join("qsb");
            if output.status.success() && !dir.is_empty() && candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from("qsb")
}

/// Compiles the reading shader for every Qt Quick backend (Vulkan, OpenGL
/// and GLES, Metal, Direct3D) and writes the resource file that places it at
/// `qrc:/calcita/shaders/reading.frag.qsb`. Both outputs live in `OUT_DIR`
/// and are only rewritten when they change, so the build stays quiet.
fn bake_shaders() -> PathBuf {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let shaders = out.join("shaders");
    std::fs::create_dir_all(&shaders).expect("the shader folder in OUT_DIR");
    let baked = shaders.join("reading.frag.qsb");
    let fresh = shaders.join("reading.frag.qsb.new");
    let status = Command::new(find_qsb())
        .args([
            "--glsl",
            "100es,120,150",
            "--hlsl",
            "50",
            "--msl",
            "12",
            "-o",
        ])
        .arg(&fresh)
        .arg(READING_SHADER)
        .status()
        .expect("qsb, Qt's shader baker, runs (set QSB to its path)");
    assert!(status.success(), "qsb could not compile {READING_SHADER}");
    // The resource compiler watches the baked file, so it is only replaced
    // when the shader really changed.
    let new_bytes = std::fs::read(&fresh).expect("the baked shader");
    if std::fs::read(&baked).ok().as_deref() != Some(new_bytes.as_slice()) {
        std::fs::rename(&fresh, &baked).expect("the baked shader in place");
    } else {
        let _ = std::fs::remove_file(&fresh);
    }
    let qrc = out.join("shaders.qrc");
    let text = "<RCC>\n  <qresource prefix=\"/calcita/shaders\">\n    \
                <file alias=\"reading.frag.qsb\">shaders/reading.frag.qsb</file>\n  \
                </qresource>\n</RCC>\n";
    if std::fs::read_to_string(&qrc).ok().as_deref() != Some(text) {
        std::fs::write(&qrc, text).expect("the shader resource file in OUT_DIR");
    }
    qrc
}

fn main() {
    // The shared files are symlinked into qml/ so they register under a clean
    // `qml/...` resource path: a `..` in a source path would reach the qrc
    // alias and break type resolution at run time.
    let module = QmlModule::new("org.celestina.calcita")
        .version(1, 0)
        // The page area is QtPdf's `PdfMultiPageView` over a `PdfDocument`;
        // without the dependency qmllint cannot resolve them.
        .depend("QtQuick.Pdf")
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
    println!("cargo::rerun-if-changed={READING_SHADER}");
    println!("cargo::rerun-if-env-changed=QSB");
    let shader_qrc = bake_shaders();
    println!("cargo::rerun-if-changed=cpp/clipboard.cpp");
    println!("cargo::rerun-if-changed=cpp/calcita/clipboard.h");

    let builder = CxxQtBuilder::new_qml_module(module)
        // The shared icons and Inter Variable, compiled in.
        .qrc("qml/icons.qrc")
        .qrc("qml/fonts.qrc")
        // The reading mode's shader, baked from qml/shaders/ in OUT_DIR.
        .qrc(&shader_qrc)
        // «Copiar» puts the selected text on the clipboard: cxx-qt-lib has no
        // QClipboard, so a one-function shim under cpp/ does it on the Qt
        // thread.
        .cpp_file("cpp/clipboard.cpp")
        .files(["src/activation.rs", "src/controller.rs", "src/document.rs"]);
    // SAFETY: only adds the shim's include directory, so the generated bridge
    // and clipboard.cpp both resolve "calcita/clipboard.h".
    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.include("cpp");
        })
    };
    builder.build();
}
