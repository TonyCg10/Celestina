cmake_minimum_required(VERSION 3.20)
project(celestina-style VERSION 1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Qt6 REQUIRED COMPONENTS Quick QuickControls2)
qt_standard_project_setup(REQUIRES 6.5)

# ── QML Module ─────────────────────────────────────────────────────────────────
qt_add_qml_module(celestina-style
    URI CelestinaStyle
    VERSION 1.0
    # This is a "pure QML" plugin — no C++ sources needed
    PLUGIN_TARGET celestina-style-plugin
    NO_PLUGIN_OPTIONAL
    QML_FILES
        CelestinaTheme.qml
        CelestinaGlassPanel.qml
        CelestinaButton.qml
        CelestinaContextMenu.qml
    RESOURCES
        qmldir
)

# ── Install ────────────────────────────────────────────────────────────────────
include(GNUInstallDirs)

# Install the QML module so other projects can find it
set(QML_INSTALL_DIR "${CMAKE_INSTALL_LIBDIR}/qt6/qml/CelestinaStyle")

install(
    FILES
        qmldir
        CelestinaTheme.qml
        CelestinaGlassPanel.qml
        CelestinaButton.qml
        CelestinaContextMenu.qml
    DESTINATION "${QML_INSTALL_DIR}"
)
