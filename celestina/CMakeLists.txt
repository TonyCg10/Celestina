cmake_minimum_required(VERSION 3.20)
project(celestina VERSION 0.1 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 6.5 REQUIRED COMPONENTS Quick DBus)
find_package(LayerShellQt 6.6 REQUIRED)
# KWindowEffects::enableBlurBehind asks the compositor (niri's
# ext-background-effect) to blur the wallpaper behind the panel — the panel's
# glass. KF ≥ 6.19 speaks the new protocol.
find_package(KF6WindowSystem REQUIRED)
qt_standard_project_setup(REQUIRES 6.5)

find_program(CARGO_EXECUTABLE cargo REQUIRED)
set(NIRI_ADAPTER_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/rust-target")
set(NIRI_ADAPTER_BINARY
    "${NIRI_ADAPTER_TARGET_DIR}/release/celestina-niri-adapter"
)

add_custom_target(celestina-niri-adapter ALL
    COMMAND ${CMAKE_COMMAND} -E env
        CARGO_TARGET_DIR=${NIRI_ADAPTER_TARGET_DIR}
        ${CARGO_EXECUTABLE} build
        --release
        --manifest-path ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
        --bin celestina-niri-adapter
    BYPRODUCTS ${NIRI_ADAPTER_BINARY}
    DEPENDS
        Cargo.toml
        src/niri_adapter.rs
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    COMMENT "Building the Rust Niri event-stream adapter"
    VERBATIM
)

if(Qt6_VERSION VERSION_GREATER_EQUAL 6.8)
    qt_policy(SET QTP0004 NEW)
endif()

qt_add_executable(celestina
    src/main.cpp
    src/devicesclient.h
    src/devicesclient.cpp
    src/niriclient.h
    src/niriclient.cpp
)
add_dependencies(celestina celestina-niri-adapter)

# The panel's status icons come from the shared Lucide set. The shell imports
# CelestinaStyle at runtime (source), so its qrc-bundled icons are not reachable
# here — instead the two the panel needs are compiled straight into the shell
# from the canonical files, with an explicit alias so the `..` path does not leak
# into the resource name.
set_source_files_properties(../celestina-style/icons/phone.svg PROPERTIES QT_RESOURCE_ALIAS phone.svg)
set_source_files_properties(../celestina-style/icons/battery-charging.svg PROPERTIES QT_RESOURCE_ALIAS battery-charging.svg)

qt_add_qml_module(celestina
    URI CelestinaDesktop
    VERSION 1.0
    QML_FILES
        qml/Panel.qml
        qml/Clock.qml
        qml/OutputChooser.qml
        qml/WorkspaceStrip.qml
        qml/PhoneStatus.qml
    RESOURCES
        ../celestina-style/icons/phone.svg
        ../celestina-style/icons/battery-charging.svg
)

# The style tree lives beside the shell; the panel imports it from source
# (self-provisioned at runtime — see main.cpp), so main() needs to know where.
target_compile_definitions(celestina PRIVATE
    CELESTINA_STYLE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/../celestina-style"
    CELESTINA_NIRI_ADAPTER="${NIRI_ADAPTER_BINARY}"
)

target_link_libraries(celestina PRIVATE
    Qt6::Quick
    Qt6::DBus
    KF6::WindowSystem
    LayerShellQt::Interface
)
