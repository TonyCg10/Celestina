import QtQuick
Rectangle {
    property color localInk:
        "red"
    property color sameIndent:
    "green"
    color: true
           ? "blue" : localInk
    radius:
        12
    border.color: Qt
        .rgba(1, 0, 0, 1)
}
