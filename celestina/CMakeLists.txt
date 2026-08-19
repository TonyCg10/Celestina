cmake_minimum_required(VERSION 3.20)
project(celestina VERSION 0.32.0 LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

# QuickControls2: the shared GlassContextMenu is a real QtQuick.Controls Menu,
# so the shell links the module that provides its style plugin.
find_package(Qt6 6.9 REQUIRED COMPONENTS Quick QuickControls2 DBus)
# The lock screen alone needs these: it speaks ext-session-lock-v1 directly,
# because Qt has no public API for a shell role and one integration is chosen
# per process — which is also why the lock is a separate program from a shell
# whose every surface is a layer surface.
find_package(Qt6 6.9 QUIET COMPONENTS WaylandClient WaylandClientPrivate)
find_package(QtWaylandScanner QUIET)
find_package(WaylandScanner QUIET)
find_package(LayerShellQt 6.6 REQUIRED)
# KWindowEffects::enableBlurBehind asks the compositor (niri's
# ext-background-effect) to blur the scene behind the requested finite glass
# region. Whether that scene is the wallpaper-only xray cache or the complete
# composition remains compositor policy. KF >= 6.19 speaks the protocol, so
# that is the declared minimum: an older KF6 builds and silently never reports
# the effect as available.
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

# The one process allowed to say "authenticated". It is its own binary, not a
# slice of the shell and not a thread inside it, because ADR 0004 puts PAM
# behind a process boundary: a module that crashes takes a child with it and
# leaves the locked surface above untouched. It links libpam and nothing else.
find_library(PAM_LIBRARY NAMES pam REQUIRED)
add_executable(celestina-lock-verify src/lockverify/main.cpp)
target_link_libraries(celestina-lock-verify PRIVATE ${PAM_LIBRARY})

# The one process allowed to say "authorized". Its own binary for the same
# reason the verifier is: ADR 0005 puts the helper conversation behind a
# process boundary, so a GLib main loop and polkit's own client library never
# enter the shell's process at all.
#
# It links libpolkit-agent-1 rather than spawning
# `/usr/lib/polkit-1/polkit-agent-helper-1` itself, because which transport
# reaches that helper is polkit's decision and not ours: on this machine the
# binary is not setuid and is reached over a systemd socket instead. Built when
# that library is present and skipped otherwise, so a machine without polkit
# still builds the shell — it simply cannot serve authorization prompts, which
# the agent reports rather than pretends past.
set(CELESTINA_POLKIT_BUILT OFF)
find_package(PkgConfig)
if(PkgConfig_FOUND)
    pkg_check_modules(POLKIT_AGENT IMPORTED_TARGET polkit-agent-1)
endif()
if(TARGET PkgConfig::POLKIT_AGENT)
    set(CELESTINA_POLKIT_BUILT ON)
    add_executable(celestina-polkit-converse src/polkitconverse/main.cpp)
    target_link_libraries(celestina-polkit-converse PRIVATE
        PkgConfig::POLKIT_AGENT)
    message(STATUS "celestina-polkit-converse: polkit-agent-1 FOUND")
else()
    message(STATUS "celestina-polkit-converse: polkit-agent-1 MISSING")
endif()

# The lock screen: its own process, its own shell integration, and the only
# program in this repository that may call `unlock_and_destroy`. Built when the
# private Qt Wayland client API is present and skipped otherwise, so a machine
# without it still builds the shell — it simply cannot lock, which the session
# verb reports rather than pretends past.
set(CELESTINA_LOCK_BUILT OFF)
foreach(_probe Qt6::WaylandClientPrivate Wayland::Scanner Qt6::qtwaylandscanner)
    if(TARGET ${_probe})
        message(STATUS "celestina-lock probe: ${_probe} FOUND")
    else()
        message(STATUS "celestina-lock probe: ${_probe} MISSING")
    endif()
endforeach()
if(TARGET Qt6::WaylandClientPrivate AND TARGET Wayland::Scanner
   AND TARGET Qt6::qtwaylandscanner)
    set(CELESTINA_LOCK_BUILT ON)

    # The facade both halves share: the plugin fills it, the application reads
    # it, and neither the application nor its QML ever sees a Wayland header.
    # One shared object so there is one lock, not one per linked copy.
    qt_add_library(celestina-lock-session SHARED
        src/lock/locksession.h
        src/lock/locksession.cpp
    )
    target_include_directories(celestina-lock-session PUBLIC src/lock)
    target_link_libraries(celestina-lock-session PUBLIC Qt6::Core)

    qt_add_plugin(celestina-lock-shell-integration
        CLASS_NAME SessionLockIntegrationPlugin
        src/lock/plugin.cpp
        src/lock/locksurface.h
        src/lock/locksurface.cpp
    )
    qt6_generate_wayland_protocol_client_sources(celestina-lock-shell-integration
        PRIVATE_CODE
        FILES
            "/usr/share/wayland-protocols/staging/ext-session-lock/ext-session-lock-v1.xml"
    )
    target_include_directories(celestina-lock-shell-integration PRIVATE src)
    target_link_libraries(celestina-lock-shell-integration PRIVATE
        Qt6::Gui
        Qt6::GuiPrivate
        Qt6::WaylandClient
        Qt6::WaylandClientPrivate
        celestina-lock-session
    )
    set_target_properties(celestina-lock-shell-integration PROPERTIES
        LIBRARY_OUTPUT_DIRECTORY
            "${CMAKE_CURRENT_BINARY_DIR}/lock-plugins/wayland-shell-integration"
    )

    qt_add_executable(celestina-lock
        src/lock/main.cpp
        src/lock/lockbackdrop.h
        src/lock/lockbackdrop.cpp
        src/lock/lockuncover.h
        src/lock/lockuncover.cpp
        src/lockauthenticator.h
        src/lockauthenticator.cpp
    )
    # Its own QML module, not the shell's: the shell's is compiled into the
    # shell's own executable and cannot be imported from another process.
    # Both import the same CelestinaStyle, which is a real module on disk.
    qt_add_qml_module(celestina-lock
        URI CelestinaLock
        VERSION 1.0
        IMPORT_PATH "${CELESTINA_STYLE_IMPORT_ROOT}"
        QML_FILES
            # Beside the lock's own sources rather than in the shell's `qml/`.
            # That directory is the shell module's, whole: the architecture
            # scanner requires every file in it to be registered there, and a
            # component belonging to another executable never can be.
            src/lock/LockScreen.qml
        SOURCES
            src/lockauthenticator.h
            src/lockauthenticator.cpp
    )
    target_include_directories(celestina-lock PRIVATE src src/lock)
    target_link_libraries(celestina-lock PRIVATE
        Qt6::Gui
        Qt6::Qml
        Qt6::Quick
        celestina-lock-session
    )
    target_compile_definitions(celestina-lock PRIVATE
        CELESTINA_LOCK_PLUGIN_PATH="${CMAKE_CURRENT_BINARY_DIR}/lock-plugins"
        CELESTINA_STYLE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/../celestina-style"
    )
    add_dependencies(celestina-lock celestina-lock-shell-integration)
endif()

# One cargo invocation builds every helper the shell spawns; they share a
# package, a lockfile and the shared core crate.
# What the helpers are built from, gathered rather than listed.
#
# The hand-written list had drifted to ten of the nineteen files under `src/`,
# and named none of `celestina-shell-core`, which the helpers also compile. It
# costs nothing today because an `ALL` custom target is always considered out of
# date and cargo does its own change tracking — but the list is read as a
# statement about what this target depends on, and it was not one. Globbing at
# configure time keeps it true without asking anyone to remember.
file(GLOB_RECURSE CELESTINA_HELPER_SOURCES CONFIGURE_DEPENDS
    ${CMAKE_CURRENT_SOURCE_DIR}/src/*.rs
    ${CMAKE_CURRENT_SOURCE_DIR}/../celestina-rs/crates/celestina-shell-core/src/*.rs
)
list(APPEND CELESTINA_HELPER_SOURCES
    ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
    ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.lock
)

add_custom_target(celestina-helpers ALL
    COMMAND ${CMAKE_COMMAND} -E env
        CARGO_TARGET_DIR=${NIRI_ADAPTER_TARGET_DIR}
        ${CARGO_EXECUTABLE} build
        --release
        --locked
        --manifest-path ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
        --bins
    BYPRODUCTS ${NIRI_ADAPTER_BINARY} ${PROVIDER_ADAPTER_BINARY}
    DEPENDS ${CELESTINA_HELPER_SOURCES}
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    COMMENT "Building the Rust helpers (Niri event stream, aggregate providers)"
    VERBATIM
)

if(Qt6_VERSION VERSION_GREATER_EQUAL 6.8)
    qt_policy(SET QTP0004 NEW)
endif()

qt_add_executable(celestina
    src/main.cpp
    src/diagnosticjournal.h
    src/diagnosticjournal.cpp
    src/devicesclient.h
    src/devicesclient.cpp
    src/protocoldecoder.h
    src/protocoldecoder.cpp
    src/niriclient.h
    src/niriclient.cpp
    src/panelblurcontroller.h
    src/panelblurcontroller.cpp
    src/denseglass.h
    src/denseglass.cpp
    src/lockcontroller.h
    src/lockcontroller.cpp
    src/panelattachmentlease.h
    src/panelattachmentlease.cpp
    src/panelmanager.h
    src/panelmanager.cpp
    src/shellscale.h
    src/shellscale.cpp
    src/wallpapermanager.h
    src/wallpapermanager.cpp
    src/wallpaperidentity.h
    src/wallpaperidentity.cpp
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
    src/bubbleanchorsource.h
    src/shellservice.h
    src/shellservice.cpp
    src/shellclient.h
    src/shellclient.cpp
    src/providerstates.h
    src/providerstates.cpp
    src/requestledger.h
    src/requestledger.cpp
    src/shellprovidersclient.h
    src/shellprovidersclient.cpp
    src/surfacemanager.h
    src/surfacemanager.cpp
    src/panelmenusurface.h
    src/panelmenusurface.cpp
    src/panelpopupplacement.h
    src/panelmenucontroller.h
    src/panelmenucontroller.cpp
    src/overlaysurface.h
    src/overlaysurface.cpp
    src/quietplacement.h
    src/quietplacement.cpp
    src/overlaycontroller.h
    src/overlaycontroller.cpp
    src/polkitconversation.h
    src/polkitconversation.cpp
    src/polkitagent.h
    src/polkitagent.cpp
    src/polkitpromptcontroller.h
    src/polkitpromptcontroller.cpp
    src/trayitems.h
    src/trayitems.cpp
    src/traywatcher.h
    src/traywatcher.cpp
    src/trayicons.h
    src/trayicons.cpp
    src/trayiconprovider.h
    src/trayiconprovider.cpp
    src/appiconprovider.h
    src/appiconprovider.cpp
    src/traymenu.h
    src/traymenu.cpp
    src/traywatcherservice.h
    src/traywatcherservice.cpp
)
add_dependencies(celestina celestina-helpers)

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
        qml/BubbleAnchorSlot.qml
        qml/BubbleGroup.qml
        qml/BubbleSelector.qml
        qml/Clock.qml
        qml/OutputChooser.qml
        qml/WorkspaceStrip.qml
        qml/WorkspacePill.qml
        qml/WorkspaceGroupCapsule.qml
        qml/WorkspaceMap.qml
        qml/WorkspaceMapBoard.qml
        qml/WorkspaceMapTile.qml
        qml/PhoneStatus.qml
        qml/PanelFlank.qml
        qml/BackdropInk.qml
        qml/BackdropButton.qml
        qml/BackdropIconButton.qml
        qml/BackdropTextField.qml
        qml/CompositorGlassRegion.qml
        qml/PanelPill.qml
        qml/PanelCluster.qml
        qml/PanelPopupPlacement.qml
        qml/PanelMenuButton.qml
        qml/MenuSection.qml
        qml/MenuHeader.qml
        qml/PanelActionButton.qml
        qml/SysMon.qml
        qml/MediaMini.qml
        qml/AudioLevel.qml
        qml/SessionStatus.qml
        qml/BrightnessLevel.qml
        qml/TrayDrawer.qml
        qml/AnchoredCard.qml
        qml/AnchoredMenu.qml
        qml/SoftMenuRow.qml
        qml/SoftMenuField.qml
        qml/SoftOverlayCard.qml
        qml/SoftMenu.qml
        qml/TrayItemsMenu.qml
        qml/TrayMenu.qml
        qml/NetworkMenu.qml
        qml/BluetoothMenu.qml
        qml/PerformanceMenu.qml
        qml/BrightnessMenu.qml
        qml/CalendarMenu.qml
        qml/PhoneMenu.qml
        qml/AudioMenu.qml
        qml/SoftCard.qml
        qml/LevelRow.qml
        qml/DevicePicker.qml
        qml/CaptureButton.qml
        qml/CaptureMenu.qml
        qml/WallpaperMenu.qml
        qml/LauncherOverlay.qml
        qml/ClipboardOverlay.qml
        qml/ClipboardEntryRow.qml
        qml/SessionOsd.qml
        qml/ToastStack.qml
        qml/NotificationIndicator.qml
        qml/NotificationCenter.qml
        qml/ControlCentre.qml
        qml/SessionMenu.qml
        qml/PolkitPrompt.qml
        qml/MonthCalendar.qml
        qml/Wallpaper.qml
    RESOURCES
        qml/EdgeAttachedGeometry.js
        qml/ProviderReading.js
)

# The source tree is the fallback used by the legacy launcher. Production
# launchers override it with the compiled module through CELESTINA_STYLE_PATH.
target_compile_definitions(celestina PRIVATE
    CELESTINA_VERSION="${PROJECT_VERSION}"
    CELESTINA_STYLE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/../celestina-style"
    CELESTINA_NIRI_ADAPTER="${NIRI_ADAPTER_BINARY}"
    CELESTINA_PROVIDER_ADAPTER="${PROVIDER_ADAPTER_BINARY}"
    CELESTINA_LOCK_VERIFY="$<TARGET_FILE:celestina-lock-verify>"
    $<$<BOOL:${CELESTINA_POLKIT_BUILT}>:CELESTINA_POLKIT_CONVERSE="$<TARGET_FILE:celestina-polkit-converse>">
    $<$<BOOL:${CELESTINA_LOCK_BUILT}>:CELESTINA_LOCK_BINARY="${CMAKE_CURRENT_BINARY_DIR}/celestina-lock">
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

    qt_add_executable(celestina-shell-scale-test
        tests/shellscale_test.cpp
        src/shellscale.h
        src/shellscale.cpp
    )
    target_include_directories(celestina-shell-scale-test PRIVATE src)
    target_link_libraries(celestina-shell-scale-test PRIVATE Qt6::Test Qt6::Gui)
    add_test(NAME celestina-shell-scale COMMAND celestina-shell-scale-test)

    qt_add_executable(celestina-lock-controller-test
        tests/lockcontroller_test.cpp
        src/lockcontroller.h
        src/lockcontroller.cpp
    )
    target_include_directories(celestina-lock-controller-test PRIVATE src)
    target_link_libraries(celestina-lock-controller-test PRIVATE
        Qt6::Test Qt6::Gui Qt6::DBus)
    add_test(NAME celestina-lock-controller COMMAND celestina-lock-controller-test)

    # Proves the release cannot be held hostage by the retreat that plays in
    # front of it: every case here stalls, ignores or repeats the animation and
    # still requires exactly one uncovering.
    # Guards the cached-miss path: an application id no theme knows must be
    # searched for once, not on every frame that draws it.
    qt_add_executable(celestina-appicon-provider-test
        tests/appiconprovider_test.cpp
        src/appiconprovider.h
        src/appiconprovider.cpp
        src/trayicons.h
        src/trayicons.cpp
    )
    target_include_directories(celestina-appicon-provider-test PRIVATE src)
    target_link_libraries(celestina-appicon-provider-test PRIVATE
        Qt6::Test Qt6::Gui Qt6::Quick)
    add_test(NAME celestina-appicon-provider COMMAND celestina-appicon-provider-test)

    qt_add_executable(celestina-lock-uncover-test
        tests/lockuncover_test.cpp
        src/lock/lockuncover.h
        src/lock/lockuncover.cpp
    )
    target_include_directories(celestina-lock-uncover-test PRIVATE src)
    target_link_libraries(celestina-lock-uncover-test PRIVATE Qt6::Test Qt6::Core)
    add_test(NAME celestina-lock-uncover COMMAND celestina-lock-uncover-test)

    qt_add_executable(celestina-lock-authenticator-test
        tests/lockauthenticator_test.cpp
        src/lockauthenticator.h
        src/lockauthenticator.cpp
    )
    target_include_directories(celestina-lock-authenticator-test PRIVATE src)
    target_link_libraries(celestina-lock-authenticator-test PRIVATE Qt6::Test Qt6::Gui)
    add_test(NAME celestina-lock-authenticator COMMAND celestina-lock-authenticator-test)

    # Drives the polkit conversation against stand-in children, so every
    # denial branch runs without polkit, without PAM and without a password.
    qt_add_executable(celestina-polkit-conversation-test
        tests/polkitconversation_test.cpp
        src/polkitconversation.h
        src/polkitconversation.cpp
    )
    target_include_directories(celestina-polkit-conversation-test PRIVATE src)
    target_link_libraries(celestina-polkit-conversation-test PRIVATE
        Qt6::Test Qt6::Gui)
    add_test(NAME celestina-polkit-conversation
        COMMAND celestina-polkit-conversation-test)

    # Drives the agent against a stand-in polkitd on the session bus, so
    # registration, re-registration, cancellation and concurrency run without
    # the real authority and without a password.
    qt_add_executable(celestina-polkit-agent-test
        tests/polkitagent_test.cpp
        src/polkitagent.h
        src/polkitagent.cpp
        src/polkitconversation.h
        src/polkitconversation.cpp
    )
    target_include_directories(celestina-polkit-agent-test PRIVATE src)
    target_link_libraries(celestina-polkit-agent-test PRIVATE
        Qt6::Test Qt6::Gui Qt6::DBus)
    add_test(NAME celestina-polkit-agent COMMAND celestina-polkit-agent-test)

    # The prompt's own decision: whether this shell may ask for a password at
    # all, and what it does when it may not.
    qt_add_executable(celestina-polkit-prompt-test
        tests/polkitprompt_test.cpp
        src/polkitpromptcontroller.h
        src/polkitpromptcontroller.cpp
        src/polkitagent.h
        src/polkitagent.cpp
        src/polkitconversation.h
        src/polkitconversation.cpp
        src/surfacemanager.h
        src/surfacemanager.cpp
        src/shellscale.h
        src/shellscale.cpp
        src/panelblurcontroller.h
        src/panelblurcontroller.cpp
        src/denseglass.h
        src/denseglass.cpp
        src/diagnosticjournal.h
        src/diagnosticjournal.cpp
    )
    target_include_directories(celestina-polkit-prompt-test PRIVATE src)
    target_link_libraries(celestina-polkit-prompt-test PRIVATE
        Qt6::Test Qt6::Quick Qt6::DBus KF6::WindowSystem
        LayerShellQt::Interface)
    add_test(NAME celestina-polkit-prompt COMMAND celestina-polkit-prompt-test)

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
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;CELESTINA_SHELL_SCALE=1"
    )

    # The tray host end to end, against a private bus it starts itself. It
    # claims the real watcher name, which is exactly why it may never run on
    # the author's session bus.
    qt_add_executable(celestina-tray-watcher-test
        tests/traywatcher_test.cpp
        src/diagnosticjournal.h
        src/diagnosticjournal.cpp
        src/traywatcher.h
        src/traywatcher.cpp
        src/traywatcherservice.h
        src/traywatcherservice.cpp
        src/trayitems.h
        src/trayitems.cpp
        src/trayicons.h
        src/trayicons.cpp
        src/traymenu.h
        src/traymenu.cpp
    )
    target_include_directories(celestina-tray-watcher-test PRIVATE src)
    target_link_libraries(celestina-tray-watcher-test PRIVATE
        Qt6::Test Qt6::DBus Qt6::Gui
    )
    add_test(NAME celestina-tray-watcher COMMAND celestina-tray-watcher-test)
    set_tests_properties(celestina-tray-watcher PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;CELESTINA_SHELL_SCALE=1"
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
        src/bubbleanchorsource.h
    src/shellservice.h
        src/shellservice.cpp
        src/sessionactions.h
        src/sessionactions.cpp
        src/sessionrequests.h
        src/sessionrequests.cpp
        src/niriclient.h
        src/niriclient.cpp
        src/diagnosticjournal.cpp
        src/protocoldecoder.h
        src/protocoldecoder.cpp
        src/workspacefocusrequests.h
        src/workspacefocusrequests.cpp
        src/overlaycontroller.h
        src/overlaycontroller.cpp
        src/shellscale.h
        src/shellscale.cpp
        src/panelattachmentlease.h
        src/panelattachmentlease.cpp
        src/overlaysurface.h
        src/overlaysurface.cpp
        src/quietplacement.h
        src/quietplacement.cpp
        src/panelblurcontroller.h
        src/panelblurcontroller.cpp
        src/denseglass.h
        src/denseglass.cpp
        src/surfacemanager.h
        src/surfacemanager.cpp
        src/shellprovidersclient.h
        src/shellprovidersclient.cpp
        src/requestledger.h
        src/requestledger.cpp
        src/providerstates.h
        src/providerstates.cpp
        # The session verbs route locking through the controller, so the test
        # that drives those verbs has to link it. Missing since R6-D wired
        # `lock` and `lock-and-suspend`: the shell itself linked, so nothing
        # said this target no longer built.
        src/lockcontroller.h
        src/lockcontroller.cpp
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
        KF6::WindowSystem
    )
    add_test(NAME celestina-shell-service COMMAND celestina-shell-service-test)
    set_tests_properties(celestina-shell-service PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;CELESTINA_SHELL_SCALE=1"
    )

    # The shared layer-surface recipe and the menu surface that shares it; it
    # runs offscreen and proves construction and lifetime, never compositor
    # placement or focus.
    qt_add_executable(celestina-surface-manager-test
        tests/surfacemanager_test.cpp
        src/wallpapermanager.h
        src/wallpapermanager.cpp
        src/wallpaperidentity.h
        src/wallpaperidentity.cpp
        src/shellprovidersclient.h
        src/shellprovidersclient.cpp
        src/requestledger.h
        src/requestledger.cpp
        src/providerstates.h
        src/providerstates.cpp
        src/protocoldecoder.h
        src/protocoldecoder.cpp
        src/surfacemanager.h
        src/surfacemanager.cpp
        src/panelmenusurface.h
        src/panelmenusurface.cpp
        src/panelblurcontroller.h
        src/panelblurcontroller.cpp
        src/denseglass.h
        src/denseglass.cpp
        src/panelmenucontroller.h
        src/panelmenucontroller.cpp
        src/shellscale.h
        src/shellscale.cpp
        src/panelattachmentlease.h
        src/panelattachmentlease.cpp
        src/overlaysurface.h
        src/overlaysurface.cpp
        src/quietplacement.h
        src/quietplacement.cpp
        src/niriclient.h
        src/niriclient.cpp
        src/diagnosticjournal.cpp
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
        KF6::WindowSystem
    )
    add_test(NAME celestina-surface-manager COMMAND celestina-surface-manager-test)
    set_tests_properties(celestina-surface-manager PROPERTIES
        ENVIRONMENT "QT_QPA_PLATFORM=offscreen;CELESTINA_SHELL_SCALE=1"
    )

    # What each overlay component declares, and what a click outside its card
    # does. It loads the QML files directly rather than the compiled module, for
    # the same reason the surface test does: the module is the application's,
    # and this asks about the files themselves.
    qt_add_executable(celestina-overlay-contract-test
        tests/overlaycontract_test.cpp
        src/overlaycontroller.h
        src/overlaycontroller.cpp
        src/shellscale.h
        src/shellscale.cpp
        src/panelattachmentlease.h
        src/panelattachmentlease.cpp
        src/overlaysurface.h
        src/overlaysurface.cpp
        src/quietplacement.h
        src/quietplacement.cpp
        src/panelblurcontroller.h
        src/panelblurcontroller.cpp
        src/denseglass.h
        src/denseglass.cpp
        src/diagnosticjournal.h
        src/diagnosticjournal.cpp
        src/surfacemanager.h
        src/surfacemanager.cpp
    )
    target_include_directories(celestina-overlay-contract-test PRIVATE src)
    target_compile_definitions(celestina-overlay-contract-test PRIVATE
        CELESTINA_STYLE_IMPORT_ROOT="${CELESTINA_STYLE_IMPORT_ROOT}"
        CELESTINA_QML_DIR="${CMAKE_CURRENT_SOURCE_DIR}/qml"
    )
    target_link_libraries(celestina-overlay-contract-test PRIVATE
        Qt6::Test
        Qt6::Quick
        LayerShellQt::Interface
        KF6::WindowSystem
    )
    add_test(NAME celestina-overlay-contract COMMAND celestina-overlay-contract-test)
    set_tests_properties(celestina-overlay-contract PROPERTIES
        ENVIRONMENT
            "QML2_IMPORT_PATH=${CELESTINA_STYLE_IMPORT_ROOT};QT_QPA_PLATFORM=offscreen;CELESTINA_SHELL_SCALE=1"
    )

    # What each connectivity menu is handed, and what a click or Escape does to
    # it. It loads the QML files directly for the same reason the overlay
    # contract test does: the module is the application's, and this asks about
    # the files themselves.
    # The two request contracts, and what happens to a request whose surface is
    # destroyed before it is answered. Driven through the ledger's own sink, so
    # no helper process and no window are involved.
    qt_add_executable(celestina-request-ledger-test
        tests/requestledger_test.cpp
        src/requestledger.h
        src/requestledger.cpp
        src/providerstates.h
        src/providerstates.cpp
    )
    target_include_directories(celestina-request-ledger-test PRIVATE src)
    target_link_libraries(celestina-request-ledger-test PRIVATE Qt6::Test)
    add_test(NAME celestina-request-ledger COMMAND celestina-request-ledger-test)

    qt_add_executable(celestina-indicator-menu-test
        tests/indicatormenu_test.cpp
        src/requestledger.h
        src/requestledger.cpp
        src/panelmenucontroller.h
        src/panelmenucontroller.cpp
        src/quietplacement.h
        src/quietplacement.cpp
        src/shellscale.h
        src/shellscale.cpp
        src/panelattachmentlease.h
        src/panelattachmentlease.cpp
        src/panelmenusurface.h
        src/panelmenusurface.cpp
        src/panelblurcontroller.h
        src/panelblurcontroller.cpp
        src/denseglass.h
        src/denseglass.cpp
        src/surfacemanager.h
        src/surfacemanager.cpp
        src/niriclient.h
        src/niriclient.cpp
        src/diagnosticjournal.cpp
        src/protocoldecoder.h
        src/protocoldecoder.cpp
        src/workspacefocusrequests.h
        src/workspacefocusrequests.cpp
    )
    target_include_directories(celestina-indicator-menu-test PRIVATE src)
    target_compile_definitions(celestina-indicator-menu-test PRIVATE
        CELESTINA_STYLE_IMPORT_ROOT="${CELESTINA_STYLE_IMPORT_ROOT}"
        CELESTINA_QML_DIR="${CMAKE_CURRENT_SOURCE_DIR}/qml"
        CELESTINA_NIRI_ADAPTER="${NIRI_ADAPTER_BINARY}"
    )
    target_link_libraries(celestina-indicator-menu-test PRIVATE
        Qt6::Test
        Qt6::Quick
        LayerShellQt::Interface
        KF6::WindowSystem
    )
    add_test(NAME celestina-indicator-menu COMMAND celestina-indicator-menu-test)
    set_tests_properties(celestina-indicator-menu PROPERTIES
        ENVIRONMENT
            "QML2_IMPORT_PATH=${CELESTINA_STYLE_IMPORT_ROOT};QT_QPA_PLATFORM=offscreen;CELESTINA_SHELL_SCALE=1"
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
            "QML2_IMPORT_PATH=${CELESTINA_STYLE_IMPORT_ROOT};QT_QPA_PLATFORM=offscreen;CELESTINA_SHELL_SCALE=1"
    )
endif()
