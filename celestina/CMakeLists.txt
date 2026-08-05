cmake_minimum_required(VERSION 3.20)
project(celestina VERSION 0.5.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

# QuickControls2: the shared GlassContextMenu is a real QtQuick.Controls Menu,
# so the shell links the module that provides its style plugin.
find_package(Qt6 6.9 REQUIRED COMPONENTS Quick QuickControls2 DBus)
find_package(LayerShellQt 6.6 REQUIRED)
# KWindowEffects::enableBlurBehind asks the compositor (niri's
# ext-background-effect) to blur the wallpaper behind the panel — the panel's
# glass. KF ≥ 6.19 speaks the new protocol, so that is the declared minimum:
# an older KF6 builds and silently never reports the effect as available.
find_package(KF6WindowSystem 6.19 REQUIRED)
qt_standard_project_setup(REQUIRES 6.9)

find_program(CARGO_EXECUTABLE cargo REQUIRED)
set(NIRI_ADAPTER_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/rust-target")
set(NIRI_ADAPTER_BINARY
    "${NIRI_ADAPTER_TARGET_DIR}/release/celestina-niri-adapter"
)
set(PROVIDER_ADAPTER_BINARY
    "${NIRI_ADAPTER_TARGET_DIR}/release/celestina-provider-adapter"
)

# One cargo invocation builds every helper the shell spawns; they share a
# package, a lockfile and the shared core crate.
add_custom_target(celestina-helpers ALL
    COMMAND ${CMAKE_COMMAND} -E env
        CARGO_TARGET_DIR=${NIRI_ADAPTER_TARGET_DIR}
        ${CARGO_EXECUTABLE} build
        --release
        --locked
        --manifest-path ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
        --bins
    BYPRODUCTS ${NIRI_ADAPTER_BINARY} ${PROVIDER_ADAPTER_BINARY}
    DEPENDS
        Cargo.toml
        Cargo.lock
        src/niri_adapter.rs
        src/provider_adapter/main.rs
        src/provider_adapter/tools.rs
        src/provider_adapter/sysmon.rs
        src/provider_adapter/media.rs
        src/provider_adapter/audio.rs
        src/provider_adapter/session.rs
        src/provider_adapter/held.rs
        src/provider_adapter/sessionholds.rs
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    COMMENT "Building the Rust helpers (Niri event stream, aggregate providers)"
    VERBATIM
)

if(Qt6_VERSION VERSION_GREATER_EQUAL 6.8)
    qt_policy(SET QTP0004 NEW)
endif()

qt_add_executable(celestina
    src/main.cpp
    src/devicesclient.h
    src/devicesclient.cpp
    src/protocoldecoder.h
    src/protocoldecoder.cpp
    src/niriclient.h
    src/niriclient.cpp
    src/panelblurcontroller.h
    src/panelblurcontroller.cpp
    src/panelmanager.h
    src/panelmanager.cpp
    src/wallpapermanager.h
    src/wallpapermanager.cpp
    src/workspacefocusrequests.h
    src/workspacefocusrequests.cpp
    src/sessionrequests.h
    src/sessionrequests.cpp
    src/osdreadings.h
    src/osdreadings.cpp
    src/osdcontroller.h
    src/osdcontroller.cpp
    src/toastcontroller.h
    src/toastcontroller.cpp
    src/sessionactions.h
    src/sessionactions.cpp
    src/shellcommandline.h
    src/shellcommandline.cpp
    src/shellservice.h
    src/shellservice.cpp
    src/shellclient.h
    src/shellclient.cpp
    src/providerstates.h
    src/providerstates.cpp
    src/shellprovidersclient.h
    src/shellprovidersclient.cpp
    src/surfacemanager.h
    src/surfacemanager.cpp
    src/panelmenusurface.h
    src/panelmenusurface.cpp
    src/panelmenucontroller.h
    src/panelmenucontroller.cpp
    src/overlaysurface.h
    src/overlaysurface.cpp
    src/overlaycontroller.h
    src/overlaycontroller.cpp
    src/trayitems.h
    src/trayitems.cpp
    src/traywatcher.h
    src/traywatcher.cpp
    src/trayicons.h
    src/trayicons.cpp
    src/trayiconprovider.h
    src/trayiconprovider.cpp
    src/traymenu.h
    src/traymenu.cpp
    src/traywatcherservice.h
    src/traywatcherservice.cpp
)
add_dependencies(celestina celestina-helpers)

# The panel's status icons come from the shared Lucide set. The shell imports
# CelestinaStyle at runtime (source), so its qrc-bundled icons are not reachable
# here — instead the two the panel needs are compiled straight into the shell
# from the canonical files, with an explicit alias so the `..` path does not leak
# into the resource name.
set_source_files_properties(../celestina-style/icons/phone.svg PROPERTIES QT_RESOURCE_ALIAS phone.svg)
set_source_files_properties(../celestina-style/icons/battery-charging.svg PROPERTIES QT_RESOURCE_ALIAS battery-charging.svg)
set_source_files_properties(../celestina-style/icons/scissors.svg PROPERTIES QT_RESOURCE_ALIAS scissors.svg)
set_source_files_properties(../celestina-style/icons/chevron-down.svg PROPERTIES QT_RESOURCE_ALIAS chevron-down.svg)
set_source_files_properties(../celestina-style/icons/chevron-right.svg PROPERTIES QT_RESOURCE_ALIAS chevron-right.svg)

# The source directory is named for the repository while its QML URI is
# CelestinaStyle. Give qmllint the same URI-shaped symlink main.cpp provisions
# at runtime, so unresolved imports can never turn the lint target into a pass.
set(CELESTINA_STYLE_IMPORT_ROOT "${CMAKE_CURRENT_BINARY_DIR}/qml-imports")
file(MAKE_DIRECTORY "${CELESTINA_STYLE_IMPORT_ROOT}")
file(REMOVE "${CELESTINA_STYLE_IMPORT_ROOT}/CelestinaStyle")
file(CREATE_LINK
    "${CMAKE_CURRENT_SOURCE_DIR}/../celestina-style"
    "${CELESTINA_STYLE_IMPORT_ROOT}/CelestinaStyle"
    SYMBOLIC
)

qt_add_qml_module(celestina
    URI CelestinaDesktop
    VERSION 1.0
    IMPORT_PATH "${CELESTINA_STYLE_IMPORT_ROOT}"
    QML_FILES
        qml/Panel.qml
        qml/Clock.qml
        qml/OutputChooser.qml
        qml/WorkspaceStrip.qml
        qml/PhoneStatus.qml
        qml/PanelFlank.qml
        qml/SysMon.qml
        qml/MediaMini.qml
        qml/AudioLevel.qml
        qml/SessionStatus.qml
        qml/BrightnessLevel.qml
        qml/TrayDrawer.qml
        qml/TrayMenu.qml
        qml/CaptureButton.qml
        qml/PanelMenu.qml
        qml/LauncherOverlay.qml
        qml/ClipboardOverlay.qml
        qml/SessionOsd.qml
        qml/ToastStack.qml
        qml/NotificationIndicator.qml
        qml/NotificationCenter.qml
        qml/ControlCentre.qml
        qml/SessionMenu.qml
        qml/MonthCalendar.qml
        qml/Wallpaper.qml
    RESOURCES
        ../celestina-style/icons/phone.svg
        ../celestina-style/icons/battery-charging.svg
)

# The source tree is the fallback used by the legacy launcher. Production
# launchers override it with the compiled module through CELESTINA_STYLE_PATH.
target_compile_definitions(celestina PRIVATE
    CELESTINA_STYLE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/../celestina-style"
    CELESTINA_NIRI_ADAPTER="${NIRI_ADAPTER_BINARY}"
    CELESTINA_PROVIDER_ADAPTER="${PROVIDER_ADAPTER_BINARY}"
)

target_link_libraries(celestina PRIVATE
    Qt6::Quick
    Qt6::QuickControls2
    Qt6::DBus
    KF6::WindowSystem
    LayerShellQt::Interface
)

include(CTest)
if(BUILD_TESTING)
    find_package(Qt6 6.9 REQUIRED COMPONENTS Test QuickTest)
    qt_add_executable(celestina-protocol-decoder-test
        tests/protocoldecoder_test.cpp
        src/protocoldecoder.h
        src/protocoldecoder.cpp
    )
    target_include_directories(celestina-protocol-decoder-test PRIVATE src)
    target_link_libraries(celestina-protocol-decoder-test PRIVATE Qt6::Test)
    add_test(NAME celestina-protocol-decoder COMMAND celestina-protocol-decoder-test)

    qt_add_executable(celestina-workspace-focus-test
        tests/workspacefocusrequests_test.cpp
        src/workspacefocusrequests.h
        src/workspacefocusrequests.cpp
    )
    target_include_directories(celestina-workspace-focus-test PRIVATE src)
    target_link_libraries(celestina-workspace-focus-test PRIVATE Qt6::Test)
    add_test(NAME celestina-workspace-focus COMMAND celestina-workspace-focus-test)

    qt_add_executable(celestina-session-requests-test
        tests/sessionrequests_test.cpp
        src/sessionrequests.h
        src/sessionrequests.cpp
    )
    target_include_directories(celestina-session-requests-test PRIVATE src)
    target_link_libraries(celestina-session-requests-test PRIVATE Qt6::Test)
    add_test(NAME celestina-session-requests COMMAND celestina-session-requests-test)

    qt_add_executable(celestina-osd-readings-test
        tests/osdreadings_test.cpp
        src/osdreadings.h
        src/osdreadings.cpp
    )
    target_include_directories(celestina-osd-readings-test PRIVATE src)
    target_link_libraries(celestina-osd-readings-test PRIVATE Qt6::Test)
    add_test(NAME celestina-osd-readings COMMAND celestina-osd-readings-test)

    qt_add_executable(celestina-provider-states-test
        tests/providerstates_test.cpp
        src/providerstates.h
        src/providerstates.cpp
    )
    target_include_directories(celestina-provider-states-test PRIVATE src)
    target_link_libraries(celestina-provider-states-test PRIVATE Qt6::Test)
    add_test(NAME celestina-provider-states COMMAND celestina-provider-states-test)

    qt_add_executable(celestina-tray-items-test
        tests/trayitems_test.cpp
        src/trayitems.h
        src/trayitems.cpp
    )
    target_include_directories(celestina-tray-items-test PRIVATE src)
    target_link_libraries(celestina-tray-items-test PRIVATE Qt6::Test Qt6::DBus)
    add_test(NAME celestina-tray-items COMMAND celestina-tray-items-test)

    qt_add_executable(celestina-tray-icons-test
        tests/trayicons_test.cpp
        src/trayicons.h
        src/trayicons.cpp
    )
    target_include_directories(celestina-tray-icons-test PRIVATE src)
    target_link_libraries(celestina-tray-icons-test PRIVATE Qt6::Test Qt6::Gui)
    add_test(NAME celestina-tray-icons COMMAND celestina-tray-icons-test)

    qt_add_executable(celestina-tray-menu-test
        tests/traymenu_test.cpp
        src/traymenu.h
        src/traymenu.cpp
    )
    target_include_directories(celestina-tray-menu-test PRIVATE src)
    target_link_libraries(celestina-tray-menu-test PRIVATE Qt6::Test)
    add_test(NAME celestina-tray-menu COMMAND celestina-tray-menu-test)

    # Claims a test-only watcher name on a real session bus and exercises the
    # registry other applications depend on; it skips where no bus exists.
    qt_add_executable(celestina-tray-watcher-service-test
        tests/traywatcherservice_test.cpp
        src/traywatcherservice.h
        src/traywatcherservice.cpp
    )
    target_include_directories(celestina-tray-watcher-service-test PRIVATE src)
    target_link_libraries(celestina-tray-watcher-service-test PRIVATE Qt6::Test Qt6::DBus)
    add_test(NAME celestina-tray-watcher-service COMMAND celestina-tray-watcher-service-test)
    set_tests_properties(celestina-tray-watcher-service PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    )

    qt_add_executable(celestina-shell-commandline-test
        tests/shellcommandline_test.cpp
        src/shellcommandline.h
        src/shellcommandline.cpp
    )
    target_include_directories(celestina-shell-commandline-test PRIVATE src)
    target_link_libraries(celestina-shell-commandline-test PRIVATE Qt6::Test)
    add_test(NAME celestina-shell-commandline COMMAND celestina-shell-commandline-test)

    # Exercises the exported interface over a real session bus when one exists;
    # it claims a test-only name and skips where no bus is available.
    qt_add_executable(celestina-shell-service-test
        tests/shellservice_test.cpp
        src/shellservice.h
        src/shellservice.cpp
        src/sessionrequests.h
        src/sessionrequests.cpp
        src/niriclient.h
        src/niriclient.cpp
        src/protocoldecoder.h
        src/protocoldecoder.cpp
        src/workspacefocusrequests.h
        src/workspacefocusrequests.cpp
        src/overlaycontroller.h
        src/overlaycontroller.cpp
        src/overlaysurface.h
        src/overlaysurface.cpp
        src/surfacemanager.h
        src/surfacemanager.cpp
        src/shellprovidersclient.h
        src/shellprovidersclient.cpp
        src/providerstates.h
        src/providerstates.cpp
    )
    target_include_directories(celestina-shell-service-test PRIVATE src)
    target_compile_definitions(celestina-shell-service-test PRIVATE
        CELESTINA_NIRI_ADAPTER="${NIRI_ADAPTER_BINARY}"
        CELESTINA_PROVIDER_ADAPTER="${PROVIDER_ADAPTER_BINARY}"
    )
    target_link_libraries(celestina-shell-service-test PRIVATE
        Qt6::Test
        Qt6::DBus
        Qt6::Quick
        LayerShellQt::Interface
    )
    add_test(NAME celestina-shell-service COMMAND celestina-shell-service-test)
    set_tests_properties(celestina-shell-service PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    )

    # The shared layer-surface recipe and the menu surface that shares it; it
    # runs offscreen and proves construction and lifetime, never compositor
    # placement or focus.
    qt_add_executable(celestina-surface-manager-test
        tests/surfacemanager_test.cpp
        src/wallpapermanager.h
        src/wallpapermanager.cpp
        src/shellprovidersclient.h
        src/shellprovidersclient.cpp
        src/providerstates.h
        src/providerstates.cpp
        src/protocoldecoder.h
        src/protocoldecoder.cpp
        src/surfacemanager.h
        src/surfacemanager.cpp
        src/panelmenusurface.h
        src/panelmenusurface.cpp
        src/panelmenucontroller.h
        src/panelmenucontroller.cpp
        src/overlaysurface.h
        src/overlaysurface.cpp
        src/niriclient.h
        src/niriclient.cpp
        src/protocoldecoder.h
        src/protocoldecoder.cpp
        src/workspacefocusrequests.h
        src/workspacefocusrequests.cpp
    )
    target_include_directories(celestina-surface-manager-test PRIVATE src)
    target_compile_definitions(celestina-surface-manager-test PRIVATE
        CELESTINA_PROVIDER_ADAPTER="${PROVIDER_ADAPTER_BINARY}"
    )
    target_compile_definitions(celestina-surface-manager-test PRIVATE
        CELESTINA_NIRI_ADAPTER="${NIRI_ADAPTER_BINARY}"
        CELESTINA_STYLE_IMPORT_ROOT="${CELESTINA_STYLE_IMPORT_ROOT}"
        CELESTINA_QML_DIR="${CMAKE_CURRENT_SOURCE_DIR}/qml"
    )
    target_link_libraries(celestina-surface-manager-test PRIVATE
        Qt6::Test
        Qt6::Quick
        LayerShellQt::Interface
    )
    add_test(NAME celestina-surface-manager COMMAND celestina-surface-manager-test)
    set_tests_properties(celestina-surface-manager PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen"
    )

    qt_add_executable(celestina-output-chooser-test
        tests/quicktest_main.cpp
    )
    target_compile_definitions(celestina-output-chooser-test PRIVATE
        QUICK_TEST_SOURCE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/tests"
    )
    target_link_libraries(celestina-output-chooser-test PRIVATE Qt6::QuickTest)
    add_test(NAME celestina-output-chooser COMMAND celestina-output-chooser-test)
    set_tests_properties(celestina-output-chooser PROPERTIES
        ENVIRONMENT
            "QML2_IMPORT_PATH=${CELESTINA_STYLE_IMPORT_ROOT};QT_QPA_PLATFORM=offscreen"
    )
endif()
