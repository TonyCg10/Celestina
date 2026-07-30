cmake_minimum_required(VERSION 3.20)
project(celestina-style VERSION 1.0 LANGUAGES CXX)

include(CTest)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Qt6 6.9 REQUIRED COMPONENTS Quick QuickControls2)
qt_standard_project_setup(REQUIRES 6.9)

# Shared token and icon catalogues are singletons.
set_source_files_properties(
    CelestinaTheme.qml
    CelestinaIcons.qml
    PROPERTIES QT_QML_SINGLETON_TYPE TRUE
)

# ── QML Module ─────────────────────────────────────────────────────────────────
# Pure-QML plugin — the singleton tokens and generic glass/controls of the suite.
# OUTPUT_DIRECTORY ends in the module's target path (CelestinaStyle) so the
# generated qmldir/qmltypes land where the URI says they should; without it Qt
# warns that qmllint may not resolve the module (the S1 fix for the parked
# `OUTPUT_DIRECTORY` warning — celestina-style ROADMAP CP0).
qt_add_qml_module(celestina-style
    URI CelestinaStyle
    VERSION 1.0
    PLUGIN_TARGET celestina-style-plugin
    NO_PLUGIN_OPTIONAL
    OUTPUT_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}/CelestinaStyle
    QML_FILES
        CelestinaTheme.qml
        CelestinaIcons.qml
        CelestinaSurface.qml
        CelestinaBackdrop.qml
        CelestinaButton.qml
        CelestinaIconButton.qml
        CelestinaIcon.qml
        CelestinaSectionLabel.qml
        CelestinaFocusRing.qml
        CelestinaTextField.qml
        CelestinaSwitch.qml
        ListSection.qml
        CelestinaModalLayer.qml
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
        icons/plus.svg
        icons/view-grid.svg
        icons/view-list.svg
        icons/view-details.svg
        icons/search.svg
        icons/x.svg
        icons/go-home.svg
        icons/folder.svg
        icons/file.svg
        icons/symlink.svg
        icons/user-trash.svg
        icons/star.svg
        icons/star-outline.svg
        icons/phone.svg
        icons/battery-charging.svg
        icons/settings.svg
        icons/music.svg
        icons/key.svg
        icons/media-skip-back.svg
        icons/media-play.svg
        icons/media-pause.svg
        icons/media-skip-forward.svg
        icons/app-window.svg
        icons/arrow-down.svg
        icons/arrow-right.svg
        icons/binary.svg
        icons/bookmark-plus.svg
        icons/check.svg
        icons/chevron-down.svg
        icons/chevron-right.svg
        icons/circle-alert.svg
        icons/clipboard-paste.svg
        icons/clock-arrow-up.svg
        icons/cloud.svg
        icons/copy.svg
        icons/eraser.svg
        icons/file-archive.svg
        icons/file-braces.svg
        icons/file-code.svg
        icons/file-image.svg
        icons/file-music.svg
        icons/file-plus.svg
        icons/file-text.svg
        icons/file-video-camera.svg
        icons/files.svg
        icons/film.svg
        icons/folder-code.svg
        icons/folder-down.svg
        icons/folder-git-2.svg
        icons/folder-heart.svg
        icons/folder-open.svg
        icons/folder-plus.svg
        icons/folder-sync.svg
        icons/gamepad-2.svg
        icons/hard-drive.svg
        icons/image.svg
        icons/info.svg
        icons/layout-template.svg
        icons/list-x.svg
        icons/mail.svg
        icons/monitor.svg
        icons/paintbrush.svg
        icons/pencil.svg
        icons/printer.svg
        icons/rotate-ccw.svg
        icons/scissors.svg
        icons/share-2.svg
        icons/terminal.svg
        icons/type.svg
        icons/unplug.svg
        icons/glass-noise.png
        fonts/InterVariable.ttf
)

target_link_libraries(celestina-style PRIVATE Qt6::Quick)

if(BUILD_TESTING)
    find_package(Qt6 6.9 REQUIRED COMPONENTS QuickTest)
    qt_add_executable(celestina-style-modal-test
        tests/quicktest_main.cpp
    )
    target_compile_definitions(celestina-style-modal-test PRIVATE
        QUICK_TEST_SOURCE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/tests"
    )
    target_link_libraries(celestina-style-modal-test PRIVATE Qt6::QuickTest)
    add_dependencies(celestina-style-modal-test celestina-style-plugin)
    add_test(NAME celestina-style-modal-focus
             COMMAND celestina-style-modal-test)
    set_tests_properties(celestina-style-modal-focus PROPERTIES
        ENVIRONMENT
            "QML2_IMPORT_PATH=${CMAKE_CURRENT_BINARY_DIR};QT_QPA_PLATFORM=offscreen"
    )
endif()

# NOTE: In-tree consumers (siderita, magnetita) do not use this build — they
# symlink the .qml sources into their own cxx-qt modules, which is the official
# consumption contract (see ROADMAP.md). This CMake module exists for the
# shell's output chooser, qmllint, and the deferred installable release
# (STYLE-D: gated on a consumer outside this tree).
