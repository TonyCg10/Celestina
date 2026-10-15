// One compact entry point for the system-performance reading.
//
// The provider still owns both percentages and their lifetime. This control
// only decides whether there is a complete reading to open and carries the
// values in its accessible name; PerformanceMenu presents the detail.
pragma ComponentBehavior: Bound

import QtQuick

PanelActionButton {
    id: root

    // The `sysmon` provider's fields, or `undefined` while it carries none.
    // `var` is necessary because QML has no typed map.
    required property var reading

    readonly property bool hasReading: root.reading !== undefined
                                       && root.reading.cpu !== undefined
                                       && root.reading.ram !== undefined

    objectName: "celestina-performance-button"
    visible: root.hasReading
    enabled: root.hasReading
    iconName: "cpu"
    fallbackIcon: "gauge"
    helpText: root.hasReading
              ? qsTr("Rendimiento: procesador %1 %, memoria %2 %")
                    .arg(root.reading.cpu).arg(root.reading.ram)
              : qsTr("Rendimiento sin lectura")
    Accessible.description: qsTr("Abre el menú de rendimiento")
    Accessible.onPressAction: root.requestMenu()
}
