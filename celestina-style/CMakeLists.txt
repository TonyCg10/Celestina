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
        GlassSurface.qml
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
)

target_link_libraries(celestina-style PRIVATE Qt6::Quick)

# NOTE: A relocatable install to a clean prefix (module + plugin + type metadata,
# with clean-prefix import fixtures) is CelestinaStyle Checkpoint 0. Within the
# monorepo, Siderita compiles these files into its own binary via cxx-qt-build.
