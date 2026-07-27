cmake_minimum_required(VERSION 3.20)
project(celestina-style VERSION 1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Qt6 6.5 REQUIRED COMPONENTS Quick QuickControls2)
qt_standard_project_setup(REQUIRES 6.5)

# CelestinaTheme is a singleton token store.
set_source_files_properties(CelestinaTheme.qml PROPERTIES QT_QML_SINGLETON_TYPE TRUE)

# ── QML Module ─────────────────────────────────────────────────────────────────
# Pure-QML plugin — the singleton tokens and generic glass/controls of the suite.
qt_add_qml_module(celestina-style
    URI CelestinaStyle
    VERSION 1.0
    PLUGIN_TARGET celestina-style-plugin
    NO_PLUGIN_OPTIONAL
    QML_FILES
        CelestinaTheme.qml
        CelestinaButton.qml
        CelestinaTextField.qml
        GlassSurface.qml
        GlassCard.qml
        GlassContextMenu.qml
        GlassMenuItem.qml
    RESOURCES
        icons/go-previous.svg
        icons/go-next.svg
        icons/go-up.svg
        icons/view-refresh.svg
        icons/view-sort-ascending.svg
        icons/view-sort-descending.svg
        icons/go-home.svg
        icons/folder.svg
        icons/file.svg
        icons/symlink.svg
        icons/user-trash.svg
        icons/media-eject.svg
        icons/star.svg
        icons/star-outline.svg
)

target_link_libraries(celestina-style PRIVATE Qt6::Quick)

# NOTE: In-tree consumers (siderita, magnetita) do not use this build — they
# symlink the .qml sources into their own cxx-qt modules, which is the official
# consumption contract (see ROADMAP.md). This CMake module exists for the
# shell's output chooser, qmllint, and the deferred installable release
# (STYLE-D: gated on a consumer outside this tree).
