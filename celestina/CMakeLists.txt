cmake_minimum_required(VERSION 3.20)
project(celestina VERSION 0.1 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 6.5 REQUIRED COMPONENTS Quick DBus)
find_package(LayerShellQt 6.6 REQUIRED)
qt_standard_project_setup(REQUIRES 6.5)

if(Qt6_VERSION VERSION_GREATER_EQUAL 6.8)
    qt_policy(SET QTP0004 NEW)
endif()

qt_add_executable(celestina
    src/main.cpp
    src/devicesclient.h
    src/devicesclient.cpp
)

qt_add_qml_module(celestina
    URI CelestinaDesktop
    VERSION 1.0
    QML_FILES
        qml/Panel.qml
        qml/Clock.qml
        qml/OutputChooser.qml
)

target_link_libraries(celestina PRIVATE
    Qt6::Quick
    Qt6::DBus
    LayerShellQt::Interface
)
